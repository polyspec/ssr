// Tests of the browser scripts of the shared npm fixture (packages/build/tests/fixtures/browser*.mjs and
// browser-steps.mjs): a hung page step is reported as hung, and a script passes no limit to the browser launch and
// proceeds after a slow launch. The browser driver is a stub (a fake playwright-core in a temporary packages
// directory, and a mocked `chromium`): no package installation or browser runs; the adapter crates' browser cases run
// a real one.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { pathToFileURL } from 'node:url';
import { parseJson } from '../../scripts/json.mts';

const ROOT = path.resolve(import.meta.dirname, '../..');
const FIXTURES = path.join(ROOT, 'packages/build/tests/fixtures');
// The fake browser launch takes this many seconds.
const DELAY = 3;
// Each script runs in its own process group and is killed with its group past this many seconds, below the 30 s
// limit per test of kit's runner so a hung script fails its test with its output rather than the runner's timeout.
const SCRIPT_LIMIT = 20;
// Each script runs with its arguments and fails at its first page step because nothing listens.
const SCRIPTS: Record<string, string[]> = {
  'browser.mjs': [],
  'browser-react-stream.mjs': [],
  'browser-svelte.mjs': ['main'],
  'browser-vanilla.mjs': [],
  'browser-vue.mjs': ['main'],
};

interface Case {
  status: 'PASS' | 'FAIL' | 'TIMEOUT';
  code: number | null;
  stdout: string;
  stderr: string;
}

/** Runs a command in its own process group; past the limit the whole group is killed and the case is a TIMEOUT. */
function runCase(command: string[], cwd: string, env: NodeJS.ProcessEnv): Promise<Case> {
  const [program, ...args] = command;
  assert.ok(program !== undefined, 'a case needs a command');
  return new Promise((resolve, reject) => {
    const child = spawn(program, args, { cwd, env, detached: true, stdio: ['ignore', 'pipe', 'pipe'] });
    let stdout = '';
    let stderr = '';
    child.stdout.setEncoding('utf8').on('data', (chunk: string) => { stdout += chunk; });
    child.stderr.setEncoding('utf8').on('data', (chunk: string) => { stderr += chunk; });
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      if (child.pid !== undefined) process.kill(-child.pid, 'SIGKILL');
    }, SCRIPT_LIMIT * 1000);
    child.on('error', (error) => { clearTimeout(timer); reject(error); });
    child.on('close', (code) => {
      clearTimeout(timer);
      const status = timedOut ? 'TIMEOUT' : code === 0 ? 'PASS' : 'FAIL';
      resolve({ status, code, stdout, stderr });
    });
  });
}

/** A base URL on a local port that nothing listens on. */
async function closedBase(): Promise<string> {
  const server = createServer();
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();
  assert.ok(address !== null && typeof address === 'object');
  await new Promise<void>((resolve, reject) => server.close((error) => { if (error) reject(error); else resolve(); }));
  return `http://127.0.0.1:${address.port}`;
}

function temporary(t: test.TestContext): string {
  const directory = mkdtempSync(path.join(tmpdir(), 'ssr-browser-launch-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  return directory;
}

/** A packages directory whose playwright-core is a stub, for SSR_PACKAGES. */
function stubPackages(directory: string): string {
  const packages = path.join(directory, 'packages');
  const driver = path.join(packages, 'node_modules/playwright-core');
  mkdirSync(driver, { recursive: true });
  writeFileSync(driver + '/index.mjs', 'export const chromium = {};\n');
  return packages;
}

const report = (result: Case, launches: unknown[] = []): string =>
  `${result.status} exit ${result.code}\nlaunches: ${JSON.stringify(launches)}\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`;

const HUNG_SCRIPT = `
import { begin } from "./browser-steps.mjs";
class TimeoutError extends Error {
  constructor(message) { super(message); this.name = "TimeoutError"; }
}
begin("/ssr");
try {
  await Promise.reject(new TimeoutError("page.waitForSelector: Timeout 60000ms exceeded."));
} finally {
  console.log("closed");
}
`;

test('a page step past its limit is reported as hung, with its step and the limit', async (t) => {
  const directory = temporary(t);
  const packages = stubPackages(directory);
  const scripts = path.join(directory, 'fixtures');
  mkdirSync(scripts);
  copyFileSync(path.join(FIXTURES, 'browser-steps.mjs'), path.join(scripts, 'browser-steps.mjs'));
  writeFileSync(path.join(scripts, 'hung.mjs'), HUNG_SCRIPT);
  const result = await runCase([process.execPath, path.join(scripts, 'hung.mjs')], scripts,
    { ...process.env, SSR_PACKAGES: packages });
  assert.deepEqual([result.status, result.code], ['FAIL', 1], report(result));
  assert.equal(result.stdout, 'RUN /ssr\nclosed\n', report(result));
  assert.ok(result.stderr.startsWith('HUNG /ssr: no page event within the 60 s step limit\n'
    + 'TimeoutError: page.waitForSelector: Timeout 60000ms exceeded.'), report(result));
});

interface Launch {
  executablePath: string;
  timeout: number;
}

/**
 * Runs a browser script with a fake `chromium` whose launch takes DELAY seconds, records its options and refuses any
 * launch limit; no browser runs, so the case does not depend on how fast the host pages a browser in. The page steps
 * fail because nothing listens.
 */
async function runScript(t: test.TestContext, name: string, scripts = FIXTURES): Promise<{ result: Case; launches: Launch[] }> {
  const directory = temporary(t);
  const packages = stubPackages(directory);
  const launchesFile = path.join(directory, 'launches.json');
  const steps = pathToFileURL(path.join(scripts, 'browser-steps.mjs')).href;
  const preload = path.join(directory, 'fake-browser.mjs');
  writeFileSync(preload, [
    'import fs from "node:fs";',
    'import { mock } from "node:test";',
    `const real = await import(${JSON.stringify(steps + '?real')});`,
    'const page = {',
    '  on() {},',
    '  async goto(url) { throw new Error(`page.goto: net::ERR_CONNECTION_REFUSED at ${url}`); },',
    '  async close() {},',
    '};',
    'const chromium = {',
    '  async launch(options) {',
    `    fs.appendFileSync(${JSON.stringify(launchesFile)}, JSON.stringify(options) + "\\n");`,
    '    if (options.timeout !== 0) throw new Error(`launch limit ${options.timeout}`);',
    `    await new Promise((resolve) => setTimeout(resolve, ${DELAY * 1000}));`,
    '    return { async newPage() { return page; }, async close() {} };',
    '  },',
    '};',
    `mock.module(${JSON.stringify(steps)}, { exports: { ...real, chromium } });`,
    '',
  ].join('\n'));
  const result = await runCase([
    process.execPath, '--experimental-test-module-mocks', '--no-warnings', '--import', preload,
    path.join(scripts, name), await closedBase(), '/browser/of/the/case', ...(SCRIPTS[name] ?? []),
  ], scripts, { ...process.env, SSR_PACKAGES: packages });
  const launches = existsSync(launchesFile)
    ? readFileSync(launchesFile, 'utf8').split('\n').filter((line) => line !== '').map((line) => parseJson<Launch>(line, 'browser-launch.schema.json', launchesFile))
    : [];
  return { result, launches };
}

async function checkScript(t: test.TestContext, name: string): Promise<void> {
  const { result, launches } = await runScript(t, name);
  const text = report(result, launches);
  assert.equal(result.status, 'FAIL', text);
  assert.deepEqual(launches.map((launch) => [launch.executablePath, launch.timeout]), [['/browser/of/the/case', 0]], text);
  const launched = /^RUN launch\nDONE launch (\d+\.\d+)s\nRUN \//.exec(result.stdout);
  assert.ok(launched?.[1] !== undefined, text);
  assert.ok(Number(launched[1]) >= DELAY, text);
  assert.match(result.stdout, /\nRUN close\nDONE close \d+\.\d+s\n$/, text);
  // The script fails at its first page step, before any page passes.
  assert.ok(!result.stdout.includes('PASS '), text);
  assert.ok(result.stderr.includes('net::ERR_CONNECTION_REFUSED'), text);
}

test('a script that passes a launch limit fails', async (t) => {
  const scripts = temporary(t);
  copyFileSync(path.join(FIXTURES, 'browser-steps.mjs'), path.join(scripts, 'browser-steps.mjs'));
  const original = readFileSync(path.join(FIXTURES, 'browser.mjs'), 'utf8');
  assert.ok(original.includes('timeout: 0'), 'browser.mjs launches with timeout: 0');
  writeFileSync(path.join(scripts, 'browser.mjs'), original.replace('timeout: 0', 'timeout: 300000'));
  const { result, launches } = await runScript(t, 'browser.mjs', scripts);
  const text = report(result, launches);
  assert.equal(result.status, 'FAIL', text);
  assert.deepEqual(launches.map((launch) => launch.timeout), [300000], text);
  assert.ok(result.stderr.includes('launch limit 300000'), text);
});

test('a script without the package installation names the variable', async () => {
  const env = { ...process.env };
  delete env.SSR_PACKAGES;
  const result = await runCase([process.execPath, path.join(FIXTURES, 'browser.mjs'), await closedBase(), '/browser/of/the/case'],
    FIXTURES, env);
  assert.equal(result.status, 'FAIL', report(result));
  assert.match(result.stderr, /SSR_PACKAGES is not set; it names the package installation of scripts\/install-packages\.mts/, report(result));
});

test('the React script proceeds after a slow launch', (t) => checkScript(t, 'browser.mjs'));
test('the React stream script proceeds after a slow launch', (t) => checkScript(t, 'browser-react-stream.mjs'));
test('the Svelte script proceeds after a slow launch', (t) => checkScript(t, 'browser-svelte.mjs'));
test('the vanilla script proceeds after a slow launch', (t) => checkScript(t, 'browser-vanilla.mjs'));
test('the Vue script proceeds after a slow launch', (t) => checkScript(t, 'browser-vue.mjs'));

// Tests of scripts/install-packages.mts, the nextest setup script that installs the packages of the fixtures, and of its check
// that the filter of the setup script names exactly the binaries that read the installation. npm is a stub on PATH.
import assert from 'node:assert/strict';
import { chmodSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, realpathSync, renameSync, rmSync, unlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test, { type TestContext } from 'node:test';
import { browser, digest, filterErrors, FILES, install, main } from '../../scripts/install-packages.mts';

const ROOT = path.resolve(import.meta.dirname, '../..');

const FAKE_NPM = `#!/bin/sh
echo "$*" >> "$NPM_LOG"
mkdir -p node_modules/sample node_modules/playwright-core
echo '{}' > node_modules/sample/package.json
cat > node_modules/playwright-core/cli.js <<'SCRIPT'
const fs = require("node:fs");
if (process.argv.slice(2).join(" ") !== "install chromium") process.exit(3);
fs.mkdirSync(process.env.PLAYWRIGHT_BROWSERS_PATH + "/chromium", { recursive: true });
fs.writeFileSync(process.env.PLAYWRIGHT_BROWSERS_PATH + "/chromium/chrome", "browser");
SCRIPT
cat > node_modules/playwright-core/index.mjs <<'SCRIPT'
export const chromium = { executablePath: () => process.env.PLAYWRIGHT_BROWSERS_PATH + "/chromium/chrome" };
SCRIPT
if [ -n "$NPM_LINK" ]; then ln -s sample node_modules/linked; fi
`;

interface Sandbox { root: string; fixtures: string; packages: string; log: string; env: NodeJS.ProcessEnv }

function sandbox(t: TestContext): Sandbox {
  const root = realpathSync(mkdtempSync(path.join(tmpdir(), 'ssr-install-packages-')));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const fixtures = path.join(root, 'fixtures');
  mkdirSync(fixtures);
  writeFileSync(path.join(fixtures, 'package.json'), '{"name":"sample"}\n');
  writeFileSync(path.join(fixtures, 'package-lock.json'), '{"lockfileVersion":3}\n');
  const bin = path.join(root, 'bin');
  mkdirSync(bin);
  writeFileSync(path.join(bin, 'npm'), FAKE_NPM);
  chmodSync(path.join(bin, 'npm'), 0o755);
  const log = path.join(root, 'npm.log');
  const env: NodeJS.ProcessEnv = { ...process.env, PATH: `${bin}:${process.env.PATH ?? ''}`, NPM_LOG: log };
  delete env.NEXTEST_ENV;
  return { root, fixtures, packages: path.join(root, 'packages'), log, env };
}

test('the lock digest names an installation that is published once', t => {
  const { fixtures, packages, log, env } = sandbox(t);
  const target = install(fixtures, packages, { env });
  assert.equal(target, path.join(packages, digest(fixtures)));
  assert.ok(readFileSync(path.join(target, 'node_modules/sample/package.json'), 'utf8'));
  assert.equal(install(fixtures, packages, { env }), target);
  assert.deepEqual(readFileSync(log, 'utf8').split('\n').filter(Boolean),
    ['ci --install-links --no-bin-links --ignore-scripts --no-audit --no-fund']);
  assert.deepEqual(readdirSync(packages).sort(), [path.basename(target)]);
});

test('a changed lock selects a new installation', t => {
  const { fixtures, packages, env } = sandbox(t);
  const first = install(fixtures, packages, { env });
  writeFileSync(path.join(fixtures, 'package-lock.json'), '{"lockfileVersion":3,"changed":true}\n');
  const second = install(fixtures, packages, { env });
  assert.notEqual(first, second);
  assert.deepEqual(readdirSync(packages).sort(), [path.basename(first), path.basename(second)].sort());
});

test('a symbolic link fails and publishes nothing', t => {
  const { fixtures, packages, env } = sandbox(t);
  assert.throws(() => install(fixtures, packages, { env: { ...env, NPM_LINK: '1' } }),
    /installed package contains a symbolic link: .*linked/);
  assert.deepEqual(readdirSync(packages), []);
});

test('a concurrent publication keeps the first installation', t => {
  const { fixtures, packages, env } = sandbox(t);
  const target = path.join(packages, digest(fixtures));
  // Another run publishes the same digest between this run's install and its rename.
  const publishFirst = (source: string, destination: string): void => {
    mkdirSync(path.join(target, 'node_modules/sample'), { recursive: true });
    for (const name of FILES) writeFileSync(path.join(target, name), readFileSync(path.join(fixtures, name)));
    writeFileSync(path.join(target, 'node_modules/sample/package.json'), '{}\n');
    renameSync(source, destination);
  };
  assert.equal(install(fixtures, packages, { env, rename: publishFirst }), target);
  assert.deepEqual(readdirSync(packages), [path.basename(target)]);
});

test('the setup script names the installation and the sources', t => {
  const { root, fixtures, packages, env } = sandbox(t);
  const destination = path.join(root, 'nextest-env');
  writeFileSync(destination, '');
  assert.equal(main([], fixtures, packages, { ...env, NEXTEST_ENV: destination }), 0);
  const target = path.join(packages, digest(fixtures));
  assert.equal(readFileSync(destination, 'utf8'),
    `SSR_PACKAGES=${target}\nSSR_FIXTURES=${fixtures}\nSSR_BROWSER=${target}/browsers/chromium/chrome\n`);
});

test('the installation holds the pinned browser', t => {
  const { fixtures, packages, env } = sandbox(t);
  const target = install(fixtures, packages, { env });
  assert.equal(browser(target, env), path.join(target, 'browsers/chromium/chrome'));
  unlinkSync(path.join(target, 'browsers/chromium/chrome'));
  assert.throws(() => install(fixtures, packages, { env }), /no chromium executable in .*browsers/);
});

// A Cargo workspace whose folders differ from its package names, with readers and non-readers of the installation.
function checkout(t: TestContext, binaries: string[]): string {
  const root = mkdtempSync(path.join(tmpdir(), 'ssr-install-packages-filter-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const terms = binaries.map(binary => `binary_id(=${binary})`).join(' | ');
  const files: [string, string][] = [
    ['.config/nextest.toml', `[[profile.default.scripts]]\nfilter = 'package(other)'\nsetup = "build-programs"\n\n`
      + `[[profile.default.scripts]]\nsetup = "install-packages"\nfilter = '${terms}'\n`],
    ['Cargo.toml', '[workspace]\nmembers = ["packages/a", "packages/b"]\nresolver = "3"\n'],
    ['packages/a/Cargo.toml', '[package]\nname = "polyspec-ssr-a"\nversion = "0.0.0"\nedition = "2024"\n'],
    ['packages/a/src/lib.rs', '#[cfg(test)]\nmod fixture;\n'],
    ['packages/a/tests/reads.rs', 'mod fixture;\nfn x() {}\n'],
    ['packages/a/tests/plain.rs', 'fn x() {}\n'],
    ['packages/a/tests/fixture/mod.rs', 'fn x() { std::env::var("SSR_FIXTURES"); }\n'],
    ['packages/b/Cargo.toml', '[package]\nname = "polyspec-ssr-b"\nversion = "0.0.0"\nedition = "2024"\n'],
    ['packages/b/src/lib.rs', 'pub fn x() {}\n'],
    ['packages/b/tests/env.rs', 'fn x() { std::env::var("SSR_PACKAGES"); }\n'],
  ];
  for (const [file, text] of files) {
    mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    writeFileSync(path.join(root, file), text);
  }
  return root;
}

test('the filter names exactly the readers', t => {
  const root = checkout(t, ['polyspec-ssr-a', 'polyspec-ssr-a::reads', 'polyspec-ssr-b::env']);
  assert.deepEqual(filterErrors(root), []);
});

test('a reader outside the filter and a stale entry fail', t => {
  const root = checkout(t, ['polyspec-ssr-a', 'polyspec-ssr-a::plain', 'polyspec-ssr-b::env']);
  assert.deepEqual(filterErrors(root), [
    'polyspec-ssr-a::reads reads SSR_PACKAGES or SSR_FIXTURES but is outside the install-packages filter',
    'polyspec-ssr-a::plain is in the install-packages filter but reads no installation']);
});

test('a filter that is not a list of binaries fails', t => {
  const root = checkout(t, ['polyspec-ssr-a']);
  const config = path.join(root, '.config/nextest.toml');
  writeFileSync(config, readFileSync(config, 'utf8').replace('binary_id(=polyspec-ssr-a)', 'all()'));
  assert.ok(filterErrors(root).includes(".config/nextest.toml: install-packages filter term 'all()' is not binary_id(=<id>)"));
});

test('a configuration without the install-packages entry fails', t => {
  const root = checkout(t, ['polyspec-ssr-a']);
  const config = path.join(root, '.config/nextest.toml');
  writeFileSync(config, readFileSync(config, 'utf8').replace('setup = "install-packages"', 'setup = "other"'));
  assert.deepEqual(filterErrors(root), ['.config/nextest.toml: no filter of the setup script install-packages']);
});

test('the repository filter names its readers', () => {
  assert.deepEqual(filterErrors(ROOT), []);
});

// Installs the packages that the tests build with, once per lock, into an immutable directory, and checks which tests read it.
//
// The tests build the applications of packages/build/tests/fixtures with the npm packages that its package.json and
// package-lock.json declare. The packages are installed with npm ci into var/packages/<digest>, where the digest is the SHA-256
// of both files, so a changed lock selects a new directory and an existing one never changes. An installation runs in a
// temporary directory under var/packages and is published by one rename; a run that finds the directory of its digest uses it,
// and the installation of a concurrent run that published first is removed. Nothing is written into the checkout's fixture
// directory.
//
// The installation also holds the chromium build that its playwright-core version pins, installed with
// `playwright-core install chromium` into browsers, so the browser cases run the same browser on every machine. As the nextest
// setup script install-packages (.config/nextest.toml) the tool writes SSR_PACKAGES, the published directory, SSR_FIXTURES, the
// fixture source directory, and SSR_BROWSER, the chromium executable, to the file that NEXTEST_ENV names. Without NEXTEST_ENV it
// prints the three assignments; make check-fixtures runs it so, before the targets that read the installation.
//
// With check-filter the tool checks that the filter of the setup script install-packages names exactly the nextest binaries that
// read the installation: the targets of the workspace (cargo metadata) whose source root includes the fixture module or names
// SSR_PACKAGES or SSR_FIXTURES.
//
//   node scripts/install-packages.mts
//   node scripts/install-packages.mts check-filter
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  appendFileSync, copyFileSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, renameSync, rmSync, statSync,
} from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { parse } from 'smol-toml';
import { checkJson, parseJson } from './json.mts';

const ROOT = path.resolve(import.meta.dirname, '..');
export const FIXTURES = path.join(ROOT, 'packages/build/tests/fixtures');
export const PACKAGES = path.join(ROOT, 'var/packages');
export const FILES = ['package.json', 'package-lock.json'] as const;

// The installation holds the packages and the browser build of their playwright-core; the digest names this content, so an
// installation of another layout is never reused.
const LAYOUT = 'npm ci --no-bin-links; playwright-core install chromium into browsers\0';

/** The SHA-256 of the layout and of both declared files, the name of their installation. */
export function digest(fixtures: string = FIXTURES): string {
  const sha = createHash('sha256').update(LAYOUT);
  for (const name of FILES) {
    const content = readFileSync(path.join(fixtures, name));
    sha.update(`${name}\0${content.length}\0`).update(content);
  }
  return sha.digest('hex');
}

/** An installation holds the declared files, a node_modules without symbolic links and the browser build of its playwright-core. */
export function check(directory: string, fixtures: string = FIXTURES, env: NodeJS.ProcessEnv = process.env): void {
  for (const name of FILES) {
    if (!readFileSync(path.join(directory, name)).equals(readFileSync(path.join(fixtures, name)))) {
      throw new Error(`${path.join(directory, name)} differs from ${path.join(fixtures, name)}`);
    }
  }
  const modules = path.join(directory, 'node_modules');
  if (!existsSync(modules) || !statSync(modules).isDirectory()) throw new Error(`${modules} is missing`);
  for (const entry of readdirSync(modules, { recursive: true, withFileTypes: true })) {
    if (entry.isSymbolicLink()) throw new Error(`installed package contains a symbolic link: ${path.join(entry.parentPath, entry.name)}`);
  }
  browser(directory, env);
}

/** The executable of the chromium build that playwright-core installed in the directory. */
export function browser(directory: string, env: NodeJS.ProcessEnv = process.env): string {
  const script = 'import(process.argv[1]).then((module) => process.stdout.write(module.chromium.executablePath()))';
  const entry = pathToFileURL(path.join(directory, 'node_modules/playwright-core/index.mjs')).href;
  const browsers = path.join(directory, 'browsers');
  const result = spawnSync(process.execPath, ['-e', script, entry], {
    encoding: 'utf8', env: { ...env, PLAYWRIGHT_BROWSERS_PATH: browsers },
  });
  const executable = (result.stdout ?? '').trim();
  if (result.error || result.status !== 0 || !path.isAbsolute(executable) || !existsSync(executable) || !statSync(executable).isFile()) {
    const status = result.error ? result.error.message : String(result.status ?? result.signal);
    throw new Error(`no chromium executable in ${browsers}: exit ${status}, path '${executable}', ${(result.stderr ?? '').trim()}`);
  }
  return executable;
}

function run(command: [string, ...string[]], cwd: string, env: NodeJS.ProcessEnv): void {
  const [program, ...args] = command;
  // The output of the command reaches standard error, so that standard output holds only the assignments.
  const result = spawnSync(program, args, { cwd, env, stdio: ['inherit', 2, 'inherit'] });
  if (result.error) throw new Error(`${command.join(' ')} could not start in ${cwd}: ${result.error.message}`);
  if (result.status !== 0) throw new Error(`${command.join(' ')} exited with ${result.status ?? result.signal} in ${cwd}`);
}

export interface InstallOptions {
  /** The environment of npm and playwright-core. */
  env?: NodeJS.ProcessEnv;
  /** The rename that publishes the installation; a test replaces it to publish concurrently. */
  rename?: (source: string, destination: string) => void;
}

/** The published directory of the current lock, installed when it does not exist. */
export function install(fixtures: string = FIXTURES, packages: string = PACKAGES, options: InstallOptions = {}): string {
  const env = options.env ?? process.env;
  const rename = options.rename ?? renameSync;
  const target = path.join(packages, digest(fixtures));
  if (existsSync(target) && statSync(target).isDirectory()) {
    check(target, fixtures, env);
    process.stderr.write(`PASS packages: ${target} exists\n`);
    return target;
  }
  mkdirSync(packages, { recursive: true });
  const temporary = mkdtempSync(path.join(packages, '.install-'));
  try {
    for (const name of FILES) copyFileSync(path.join(fixtures, name), path.join(temporary, name));
    let command: [string, ...string[]] = ['npm', 'ci', '--no-bin-links', '--ignore-scripts', '--no-audit', '--no-fund'];
    process.stderr.write(`RUN ${command.join(' ')} in ${temporary}\n`);
    const started = performance.now();
    run(command, temporary, env);
    // The browser is the chromium build that this playwright-core version pins, the same on every machine, never a browser of
    // the host that updates itself.
    command = [process.execPath, path.join(temporary, 'node_modules/playwright-core/cli.js'), 'install', 'chromium'];
    process.stderr.write(`RUN ${command.join(' ')}\n`);
    run(command, temporary, { ...env, PLAYWRIGHT_BROWSERS_PATH: path.join(temporary, 'browsers') });
    check(temporary, fixtures, env);
    try {
      rename(temporary, target);
    } catch (error) {
      if (!existsSync(target) || !statSync(target).isDirectory()) throw error;
      process.stderr.write(`PASS packages: a concurrent run published ${target}\n`);
      return target;
    }
    process.stderr.write(`PASS packages: published ${target} ${((performance.now() - started) / 1000).toFixed(1)}s\n`);
    return target;
  } finally {
    if (existsSync(temporary)) rmSync(temporary, { recursive: true, force: true });
  }
}

const READERS = /^mod fixture;$|SSR_PACKAGES|SSR_FIXTURES/m;
const LIBRARY_KINDS = new Set(['lib', 'rlib', 'dylib', 'cdylib', 'staticlib', 'proc-macro']);

/** The output of cargo metadata, the fields of scripts/schema/cargo-metadata.schema.json that this tool reads. */
interface Metadata { packages: { name: string; targets: { name: string; kind: string[]; src_path: string }[] }[] }
/** The parsed .config/nextest.toml, scripts/schema/nextest-config.schema.json. */
interface NextestConfig { profile?: { default?: { scripts?: { setup?: string | string[]; filter?: string }[] } } }

/** The nextest binary id of a Cargo target: a library is its package, an integration test <package>::<target>, and a binary,
 * example or bench <package>::<kind>/<target>; a build script is no test binary. */
export function binaryId(packageName: string, target: { name: string; kind: string[] }): string | undefined {
  if (target.kind.some(kind => LIBRARY_KINDS.has(kind))) return packageName;
  if (target.kind.includes('test')) return `${packageName}::${target.name}`;
  const kind = target.kind.find(kind => kind === 'bin' || kind === 'example' || kind === 'bench');
  return kind === undefined ? undefined : `${packageName}::${kind}/${target.name}`;
}

/** The nextest binary ids whose tests read the installation: the workspace targets whose source root includes the fixture module
 * or names its variables. */
export function readers(root: string = ROOT): Set<string> {
  const result = spawnSync('cargo', ['metadata', '--no-deps', '--offline', '--format-version', '1'], {
    cwd: root, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024,
  });
  if (result.error) throw new Error(`cargo metadata could not start: ${result.error.message}`);
  if (result.status !== 0) throw new Error(`cargo metadata exited with ${result.status ?? result.signal}: ${result.stderr.trim()}`);
  const metadata = parseJson<Metadata>(result.stdout, 'cargo-metadata.schema.json', 'cargo metadata');
  const binaries = new Set<string>();
  for (const pkg of metadata.packages) {
    for (const target of pkg.targets) {
      const id = binaryId(pkg.name, target);
      if (id !== undefined && READERS.test(readFileSync(target.src_path, 'utf8'))) binaries.add(id);
    }
  }
  return binaries;
}

/** The filter of the [[profile.default.scripts]] entry whose setup is install-packages, or an error. */
function installFilter(root: string): string | { error: string } {
  let value: unknown;
  try {
    value = parse(readFileSync(path.join(root, '.config/nextest.toml'), 'utf8'));
  } catch (error) {
    return { error: `.config/nextest.toml: ${error instanceof Error ? error.message : String(error)}` };
  }
  let config: NextestConfig;
  try {
    config = checkJson<NextestConfig>(value, 'nextest-config.schema.json', '.config/nextest.toml');
  } catch (error) {
    return { error: error instanceof Error ? error.message : String(error) };
  }
  const entries = (config.profile?.default?.scripts ?? []).filter(({ setup }) =>
    setup === 'install-packages' || (Array.isArray(setup) && setup.includes('install-packages')));
  if (entries.length > 1) return { error: `.config/nextest.toml: ${entries.length} entries of [[profile.default.scripts]] run the setup script install-packages` };
  const filter = entries[0]?.filter;
  if (typeof filter !== 'string') return { error: '.config/nextest.toml: no filter of the setup script install-packages' };
  return filter;
}

/** The setup script install-packages runs before exactly the binaries that read the installation; a reader outside its filter,
 * or a listed binary that reads nothing, is an error. Its filter is an alternation of binary_id(=<id>), so the selection is
 * visible and checked. */
export function filterErrors(root: string = ROOT): string[] {
  const filter = installFilter(root);
  if (typeof filter !== 'string') return [filter.error];
  const listed = new Set<string>();
  const errors: string[] = [];
  for (const term of filter.split('|').map(term => term.trim())) {
    const found = /^binary_id\(=([a-z0-9_:/-]+)\)$/.exec(term);
    if (found?.[1] === undefined) errors.push(`.config/nextest.toml: install-packages filter term '${term}' is not binary_id(=<id>)`);
    else listed.add(found[1]);
  }
  const needed = readers(root);
  for (const binary of [...needed].filter(binary => !listed.has(binary)).sort()) {
    errors.push(`${binary} reads SSR_PACKAGES or SSR_FIXTURES but is outside the install-packages filter`);
  }
  for (const binary of [...listed].filter(binary => !needed.has(binary)).sort()) {
    errors.push(`${binary} is in the install-packages filter but reads no installation`);
  }
  return errors;
}

function checkFilter(root: string): number {
  let errors: string[];
  try {
    errors = filterErrors(root);
  } catch (error) {
    errors = [error instanceof Error ? error.message : String(error)];
  }
  for (const error of errors) process.stderr.write(`FAIL install-packages filter: ${error}\n`);
  if (errors.length) return 1;
  process.stdout.write('PASS install-packages filter: it names exactly the binaries that read the installation\n');
  return 0;
}

export function main(argv: string[], fixtures: string = FIXTURES, packages: string = PACKAGES, env: NodeJS.ProcessEnv = process.env): number {
  if (argv.length === 1 && argv[0] === 'check-filter') return checkFilter(ROOT);
  if (argv.length) {
    process.stderr.write('usage: node scripts/install-packages.mts [check-filter]\n');
    return 2;
  }
  let target: string;
  let executable: string;
  try {
    target = install(fixtures, packages, { env });
    executable = browser(target, env);
  } catch (error) {
    process.stderr.write(`FAIL packages: ${error instanceof Error ? error.message : String(error)}\n`);
    return 1;
  }
  const assignments = `SSR_PACKAGES=${target}\nSSR_FIXTURES=${fixtures}\nSSR_BROWSER=${executable}\n`;
  const destination = env.NEXTEST_ENV;
  if (destination) appendFileSync(destination, assignments);
  else process.stdout.write(assignments);
  return 0;
}

if (import.meta.main) process.exitCode = main(process.argv.slice(2));

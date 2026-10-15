// Builds the programs that the tests of polyspec-ssr-server run and names them to the tests.
//
// The development and socket tests of polyspec-ssr-server run the examples development_process and socket_process as child
// processes. Cargo builds examples only when no target is selected, so a run of one test target would run a program built from
// older sources. nextest runs this tool as the setup script build-programs (.config/nextest.toml) before any test of
// polyspec-ssr-server: it builds both examples with Cargo, which rebuilds them when their sources or dependencies changed, and
// writes SSR_DEVELOPMENT_PROCESS and SSR_SOCKET_PROCESS, the executables that Cargo reports, to the file that NEXTEST_ENV names.
// The tests read the programs only from these variables. The build is a long operation: it has no time limit, and its progress
// and errors reach standard error as they arrive.
//
//   node scripts/build-programs.mts
import { spawnSync } from 'node:child_process';
import { appendFileSync } from 'node:fs';
import path from 'node:path';
import { parseJson } from './json.mts';

const ROOT = path.resolve(import.meta.dirname, '..');

/** The examples of polyspec-ssr-server and the variable that names the executable of each. */
export const PROGRAMS = { development_process: 'SSR_DEVELOPMENT_PROCESS', socket_process: 'SSR_SOCKET_PROCESS' } as const;
type Program = keyof typeof PROGRAMS;
/** A JSON message of Cargo, the fields of scripts/schema/cargo-message.schema.json. */
type CargoMessage = { reason: string; target?: { name?: string; kind?: string[] }; executable?: string | null };

function isProgram(name: string): name is Program {
  return Object.hasOwn(PROGRAMS, name);
}

/** The Cargo command that builds both examples and reports its artifacts as JSON messages. */
export function command(): [string, ...string[]] {
  return ['cargo', 'build', '--locked', '-p', 'polyspec-ssr-server', '--message-format', 'json-render-diagnostics',
    ...Object.keys(PROGRAMS).flatMap(name => ['--example', name])];
}

/** The executable of each program from the JSON messages of Cargo; a missing one is an error. */
export function executables(messages: string): Record<Program, string> {
  const found = new Map<Program, string>();
  for (const line of messages.split('\n')) {
    if (!line) continue;
    const message = parseJson<CargoMessage>(line, 'cargo-message.schema.json', 'cargo build');
    const name = message.target?.name;
    if (message.reason === 'compiler-artifact' && name !== undefined && isProgram(name)
      && JSON.stringify(message.target?.kind) === '["example"]') {
      if (typeof message.executable !== 'string') throw new Error(`Cargo reported the example ${name} without an executable`);
      found.set(name, message.executable);
    }
  }
  const missing = Object.keys(PROGRAMS).filter(isProgram).filter(name => !found.has(name)).sort();
  if (missing.length) throw new Error(`Cargo reported no executable for ${missing.join(', ')}`);
  const executable = (name: Program): string => {
    const value = found.get(name);
    if (value === undefined) throw new Error(`Cargo reported no executable for ${name}`);
    return value;
  };
  return { development_process: executable('development_process'), socket_process: executable('socket_process') };
}

function main(): number {
  const destination = process.env.NEXTEST_ENV;
  if (!destination) {
    process.stderr.write('FAIL build programs: NEXTEST_ENV is not set; nextest runs this tool as the setup script build-programs\n');
    return 1;
  }
  const [program, ...args] = command();
  process.stderr.write(`RUN ${[program, ...args].join(' ')}\n`);
  const started = performance.now();
  // Cargo's progress and errors reach standard error as they arrive; its JSON messages are read from standard output.
  const result = spawnSync(program, args, { cwd: ROOT, stdio: ['ignore', 'pipe', 'inherit'], encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  const elapsed = ((performance.now() - started) / 1000).toFixed(1);
  if (result.error) {
    process.stderr.write(`FAIL build programs ${elapsed}s: ${program} could not start: ${result.error.message}\n`);
    return 1;
  }
  if (result.status !== 0) {
    process.stderr.write(`FAIL build programs exit ${result.status ?? result.signal} ${elapsed}s\n`);
    return 1;
  }
  let found: Record<Program, string>;
  try {
    found = executables(result.stdout);
  } catch (error) {
    process.stderr.write(`FAIL build programs ${elapsed}s: ${error instanceof Error ? error.message : String(error)}\n`);
    return 1;
  }
  const entries = Object.keys(PROGRAMS).filter(isProgram).map(name => [name, PROGRAMS[name]] as const);
  appendFileSync(destination, entries.map(([name, variable]) => `${variable}=${found[name]}\n`).join(''));
  for (const [name, variable] of entries) process.stderr.write(`PASS ${variable}=${found[name]}\n`);
  process.stderr.write(`PASS build programs ${elapsed}s\n`);
  return 0;
}

if (import.meta.main) process.exitCode = main();

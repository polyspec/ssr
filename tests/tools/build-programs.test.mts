// Tests of scripts/build-programs.mts, the nextest setup script that builds the examples of polyspec-ssr-server.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import test from 'node:test';
import { command, executables } from '../../scripts/build-programs.mts';

const ROOT = path.resolve(import.meta.dirname, '../..');

const artifact = (name: string, kind: string[], executable: string): string =>
  JSON.stringify({ reason: 'compiler-artifact', target: { name, kind }, executable });

test('the command builds both examples of polyspec-ssr-server', () => {
  assert.deepEqual(command(), [
    'cargo', 'build', '--locked', '-p', 'polyspec-ssr-server', '--message-format', 'json-render-diagnostics',
    '--example', 'development_process', '--example', 'socket_process']);
});

test('the executables come from the example artifacts', () => {
  const messages = [
    JSON.stringify({ reason: 'build-finished', success: true }),
    artifact('development_process', ['lib'], '/target/debug/libdevelopment_process.rlib'),
    artifact('development_process', ['example'], '/target/debug/examples/development_process'),
    artifact('socket_process', ['example'], '/target/debug/examples/socket_process'),
  ].join('\n');
  assert.deepEqual(executables(messages), {
    development_process: '/target/debug/examples/development_process',
    socket_process: '/target/debug/examples/socket_process',
  });
});

test('a missing executable is an error', () => {
  assert.throws(() => executables(artifact('development_process', ['example'], '/target/debug/examples/development_process')),
    /Cargo reported no executable for socket_process/);
});

test('the tool runs only as a setup script', () => {
  const env = { ...process.env };
  delete env.NEXTEST_ENV;
  const result = spawnSync(process.execPath, ['scripts/build-programs.mts'], { cwd: ROOT, env, encoding: 'utf8' });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /NEXTEST_ENV is not set/);
});

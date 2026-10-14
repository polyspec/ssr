// Tests of scripts/verify-archive.mts, which verifies the local V8 archive, binding and Cargo features before linking.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { checkDisk, checkEnvironment, checkFeatures, checkFiles, type Metadata } from '../../scripts/verify-archive.mts';

const ROOT = path.resolve(import.meta.dirname, '../..');
const sha256 = (text: string): string => createHash('sha256').update(text).digest('hex');

await test('less than 10 GiB free fails', t => {
  const free = (bavail: bigint) => t.mock.method(fs, 'statfsSync', () => ({ bavail, bsize: 1n }), { times: 1 });
  free(10n * 1024n ** 3n - 1n);
  assert.throws(() => checkDisk('/'), /at least 10 GiB free disk space is required: available_bytes=10737418239/);
  free(10n * 1024n ** 3n);
  checkDisk('/');
});

await test('missing, changed, relative and symbolic inputs fail', t => {
  // The temporary directory is canonical, as the inputs must be (macOS links /var to /private/var).
  const root = fs.realpathSync(fs.mkdtempSync(path.join(tmpdir(), 'ssr-verify-archive-')));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const archive = path.join(root, 'archive.a.gz');
  const binding = path.join(root, 'binding.rs');
  const verify = (archiveFile = archive, bindingFile = binding) => checkFiles(archiveFile, bindingFile, sha256('archive'), sha256('binding'));
  assert.throws(() => verify(), /missing V8 input/);
  fs.writeFileSync(archive, 'archive');
  fs.writeFileSync(binding, 'binding');
  verify();
  fs.writeFileSync(binding, 'changed');
  assert.throws(() => verify(), /SHA-256 mismatch/);
  fs.writeFileSync(binding, 'binding');
  fs.writeFileSync(archive, 'changed');
  assert.throws(() => verify(), /SHA-256 mismatch/);
  assert.throws(() => verify('relative.a'), /absolute canonical regular file/);
  fs.writeFileSync(archive, 'archive');
  const symbolic = path.join(root, 'symbolic.rs');
  fs.symlinkSync(binding, symbolic);
  assert.throws(() => verify(archive, symbolic), /absolute canonical regular file/);
});

await test('the environment rejects source and debug builds, bypasses and another selection', () => {
  for (const env of [
    { V8_FROM_SOURCE: '1' },
    { V8_FORCE_DEBUG: 'true' },
    { DOCS_RS: '1' },
    { DENO_TRYBUILD: '1' },
    { RUSTY_V8_ARCHIVE: 'https://example.invalid/archive.a.gz' },
    { RUSTY_V8_ARCHIVE: '/other/archive.a.gz' },
    { RUSTY_V8_SRC_BINDING_PATH: '/other/binding.rs' },
  ]) {
    assert.throws(() => checkEnvironment('/verified/archive.a.gz', '/verified/binding.rs', env), Error, JSON.stringify(env));
  }
  checkEnvironment('/verified/archive.a.gz', '/verified/binding.rs',
    { RUSTY_V8_ARCHIVE: '/verified/archive.a.gz', RUSTY_V8_SRC_BINDING_PATH: '/verified/binding.rs', V8_FROM_SOURCE: '0' });
});

await test('the default commands require the official local inputs', () => {
  const makefile = fs.readFileSync(path.join(ROOT, 'Makefile'), 'utf8');
  // Every target that links V8 verifies the archive first.
  assert.match(makefile, /\nverify-archive:\n\tnode scripts\/verify-archive\.mts\n/);
  assert.match(makefile, /\ncheck-ownership check-clippy check-examples check-nextest check-bench bench: verify-archive\n/);
  assert.ok(makefile.includes('$(eval nextest-$(package): verify-archive ;'));
  assert.ok(makefile.includes('librusty_v8_simdutf_release_'));
  assert.ok(!makefile.includes('--fetch'));
  assert.ok(!fs.readFileSync(path.join(ROOT, 'Cargo.toml'), 'utf8').includes('"v8_enable_pointer_compression"'));
});

await test('a feature or version mismatch fails', () => {
  const node = { id: 'v8', features: ['default', 'simdutf', 'use_custom_libcxx'] };
  const metadata: Metadata = { packages: [{ name: 'v8', version: '150.4.0', id: 'v8' }], resolve: { nodes: [node] } };
  checkFeatures(metadata);
  node.features = node.features.filter(feature => feature !== 'default');
  checkFeatures(metadata);
  node.features.push('v8_enable_pointer_compression');
  assert.throws(() => checkFeatures(metadata), {
    message: 'V8 Cargo features do not match the simdutf release archive: expected ["simdutf","use_custom_libcxx"], '
      + 'actual [["simdutf","use_custom_libcxx","v8_enable_pointer_compression"]]',
  });
  metadata.packages[0] = { name: 'v8', version: '150.3.0', id: 'v8' };
  assert.throws(() => checkFeatures(metadata), /selected versions: \["150\.3\.0"\]/);
});

await test('the command fails for an undeclared target or an archive without a binding', () => {
  const run = (...args: string[]) => spawnSync(process.execPath, ['scripts/verify-archive.mts', ...args], { cwd: ROOT, encoding: 'utf8' });
  let result = run('riscv64', '--files-only');
  assert.equal(result.status, 1);
  assert.match(result.stderr, /^FAIL V8 archive verification \d+\.\d{3}s: unsupported V8 target: riscv64\n$/);
  result = run('x86_64-unknown-linux-gnu', '--archive', '/verified/archive.a.gz', '--files-only');
  assert.equal(result.status, 1);
  assert.match(result.stderr, /archive and binding must be selected together/);
  result = run('--unknown');
  assert.equal(result.status, 2);
});

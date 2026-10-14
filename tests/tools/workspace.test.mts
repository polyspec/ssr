// Tests of the Cargo workspace and of deny.toml: every folder of packages/ is a member named polyspec-ssr-<folder> of
// edition 2024, and the license exceptions of deny.toml name exact versions. The license case writes a package of each
// version into a temporary directory with the tracked lock of that version (tests/tools/license-fixtures), so it resolves
// nothing with the registry and the tree tracks no Cargo package that no build of this repository builds.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

const ROOT = path.resolve(import.meta.dirname, '../..');
const FIXTURES = path.join(ROOT, 'tests/tools/license-fixtures');
const OFFLINE = { ...process.env, CARGO_NET_OFFLINE: 'true', RUSTUP_AUTO_INSTALL: '0' };

type Metadata = { packages: { name: string; edition: string; manifest_path: string }[]; workspace_members: string[] };

await test('every folder of packages/ is a member named polyspec-ssr-<folder> of edition 2024', () => {
  const result = spawnSync('cargo', ['metadata', '--no-deps', '--offline', '--locked', '--format-version', '1'],
    { cwd: ROOT, env: OFFLINE, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
  assert.equal(result.status, 0, result.stderr);
  const metadata = JSON.parse(result.stdout) as Metadata;
  const members = metadata.packages.map(item => [path.relative(ROOT, path.dirname(item.manifest_path)), item.name, item.edition]);
  const folders = readdirSync(path.join(ROOT, 'packages')).sort();
  assert.deepEqual(members.sort((a, b) => a[0]!.localeCompare(b[0]!)), folders.map(folder => [`packages/${folder}`, `polyspec-ssr-${folder}`, '2024']));
  assert.equal(metadata.workspace_members.length, folders.length);
});

/** The manifest of the license case: a package that depends on xxhash-rust `version` and on dragonbox_ecma 0.1.12, the
 * two crates of the license exceptions of deny.toml. */
const licenseManifest = (version: string): string => `[package]
name = "license-verification"
version = "0.0.1"
edition = "2024"
license = "MIT"

[workspace]

[dependencies]
xxhash-rust = "=${version}"
dragonbox_ecma = "=0.1.12"
`;

await test('the license exceptions are exact versions', t => {
  for (const [version, accepted] of [['0.8.18', true], ['0.8.17', false]] as const) {
    const directory = mkdtempSync(path.join(tmpdir(), 'ssr-workspace-'));
    t.after(() => rmSync(directory, { recursive: true, force: true }));
    writeFileSync(path.join(directory, 'Cargo.toml'), licenseManifest(version));
    copyFileSync(path.join(FIXTURES, `xxhash-${version}.lock`), path.join(directory, 'Cargo.lock'));
    mkdirSync(path.join(directory, 'src'));
    writeFileSync(path.join(directory, 'src/lib.rs'), '');
    const result = spawnSync('cargo', ['deny', '--manifest-path', path.join(directory, 'Cargo.toml'), '--config', path.join(ROOT, 'deny.toml'),
      '--locked', '--offline', 'check', 'licenses', '--hide-inclusion-graph'], { env: OFFLINE, encoding: 'utf8' });
    assert.equal(result.status === 0, accepted, `xxhash-rust ${version}: exit ${result.status}\n${result.stdout}${result.stderr}`);
  }
});

await test('the license fixtures lock the exact versions and are no Cargo packages', () => {
  assert.deepEqual(readdirSync(FIXTURES).sort(), ['xxhash-0.8.17.lock', 'xxhash-0.8.18.lock']);
  for (const version of ['0.8.18', '0.8.17']) {
    assert.ok(readFileSync(path.join(FIXTURES, `xxhash-${version}.lock`), 'utf8').includes(`name = "xxhash-rust"\nversion = "${version}"\n`));
  }
});

// Verifies the exact local V8 archive, binding and Cargo features before linking.
//
// The v8 build script reads the prebuilt archive that RUSTY_V8_ARCHIVE names and the binding that RUSTY_V8_SRC_BINDING_PATH
// names without checking them. Before any target links V8, this tool checks that the archive and binding of the target in
// var/v8 (or the files of --archive and --binding) are absolute canonical regular files with the declared SHA-256, that the
// environment neither builds V8 from source nor selects other inputs, and, unless --files-only is given, that at least 10 GiB
// of disk are free and the offline locked Cargo metadata of --metadata-root (the repository root by default) selects exactly
// v8 150.4.0 with the features of the simdutf release archive. The target defaults to the host.
//
//   node scripts/verify-archive.mts [target] [--archive <path> --binding <path>] [--metadata-root <dir>] [--files-only]
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { parseArgs } from 'node:util';
import { parseJson } from './json.mts';

const ROOT = path.resolve(import.meta.dirname, '..');

/** The SHA-256 of the compressed archive and of the generated binding of each target. */
export const ARCHIVES: Readonly<Record<string, readonly [archive: string, binding: string]>> = {
  'aarch64-apple-darwin': ['5aeffd8d5a0c1b79ac1d70af83d5b19099655fd9c645a794dc43f101f779838c', 'ca5adf0cf89c9a70ad460ae73648b2fe89b74aa113b3cb7f757b6a02b758394f'],
  'x86_64-apple-darwin': ['a750271fec6b211457ed0a5cf7d2eab1924b265621a82da86ab959d6ff0823e4', 'ca5adf0cf89c9a70ad460ae73648b2fe89b74aa113b3cb7f757b6a02b758394f'],
  'aarch64-unknown-linux-gnu': ['539e283815a396a5796f32858b42e517b858ebaaeaaad05d03290ee8c864a527', '7727826ae479bdb645e807239fb12d1f8e2e23de7a6cf16f5ee592690d1d8506'],
  'x86_64-unknown-linux-gnu': ['f48762ca10d1f1fc605a441c5ae430ec8ce1e9e80f14d78fbc42cb878c30b476', '7727826ae479bdb645e807239fb12d1f8e2e23de7a6cf16f5ee592690d1d8506'],
};

/** The declared digests of `target`; an undeclared target is an error. */
export function declared(target: string, archives = ARCHIVES): readonly [string, string] {
  const digests = Object.hasOwn(archives, target) ? archives[target] : undefined;
  if (digests === undefined) throw new Error(`unsupported V8 target: ${target}`);
  return digests;
}

/** The SHA-256 of the file at `file`, read in chunks of 1 MiB. */
export function digest(file: string): string {
  const sha = createHash('sha256');
  const buffer = Buffer.alloc(1024 * 1024);
  const descriptor = fs.openSync(file, 'r');
  try {
    for (let read; (read = fs.readSync(descriptor, buffer)) > 0;) sha.update(buffer.subarray(0, read));
  } finally {
    fs.closeSync(descriptor);
  }
  return sha.digest('hex');
}

/** Fails unless at least 10 GiB are free for an unprivileged user on the file system of `root`. */
export function checkDisk(root: string): void {
  const stats = fs.statfsSync(root, { bigint: true });
  const free = stats.bavail * stats.bsize;
  if (free < 10n * 1024n ** 3n) throw new Error(`at least 10 GiB free disk space is required: available_bytes=${free}`);
}

/** Fails unless the archive and binding are absolute canonical regular files with the declared SHA-256. */
export function checkFiles(archive: string, binding: string, archiveHash: string, bindingHash: string): void {
  for (const [file, expected] of [[archive, archiveHash], [binding, bindingHash]] as const) {
    const canonical = () => new Error(`V8 input must be an absolute canonical regular file: ${file}`);
    if (!path.isAbsolute(file) || path.normalize(file) !== file) throw canonical();
    const stats = fs.lstatSync(file, { throwIfNoEntry: false });
    if (stats === undefined) throw new Error(`missing V8 input: ${file}`);
    if (stats.isSymbolicLink() || fs.realpathSync(file) !== file) throw canonical();
    if (!stats.isFile()) throw new Error(`missing V8 input: ${file}`);
    const actual = digest(file);
    if (actual !== expected) throw new Error(`V8 input SHA-256 mismatch: ${file}: ${actual}`);
  }
}

const TRUE = new Set(['1', 'true', 'yes']);

/** Fails when `env` builds V8 from source or in debug, bypasses the build or selects inputs other than the verified ones. */
export function checkEnvironment(archive: string, binding: string, env: NodeJS.ProcessEnv = process.env): void {
  if (TRUE.has((env.V8_FROM_SOURCE ?? '').toLowerCase())) throw new Error('V8 source builds are not permitted');
  if (TRUE.has((env.V8_FORCE_DEBUG ?? '').toLowerCase())) throw new Error('the verified V8 archive requires the release engine profile');
  for (const name of ['DOCS_RS', 'DENO_TRYBUILD']) {
    if (name in env) throw new Error(`${name} cannot bypass the verified engine build`);
  }
  for (const [name, file] of [['RUSTY_V8_ARCHIVE', archive], ['RUSTY_V8_SRC_BINDING_PATH', binding]] as const) {
    const selected = env[name];
    if (selected !== undefined && selected !== file) throw new Error(`${name} differs from the verified local input: ${selected}`);
  }
}

/** The part of `cargo metadata --format-version 1` that checkFeatures reads. */
export interface Metadata {
  packages: { name: string; version: string; id: string }[];
  resolve: { nodes: { id: string; features: string[] }[] } | null;
}

const FEATURES = ['simdutf', 'use_custom_libcxx'];

/** Fails unless the metadata selects exactly v8 150.4.0 with the features simdutf and use_custom_libcxx (and the marker default). */
export function checkFeatures(metadata: Metadata): void {
  const packages = metadata.packages.filter(item => item.name === 'v8');
  const versions = packages.map(item => item.version);
  if (JSON.stringify(versions) !== '["150.4.0"]') {
    throw new Error(`exactly V8 150.4.0 must be selected; selected versions: ${JSON.stringify(versions)}`);
  }
  const id = packages[0]?.id;
  if (metadata.resolve === null) throw new Error('the Cargo metadata holds no resolve graph');
  const features = metadata.resolve.nodes.filter(node => node.id === id)
    .map(node => [...new Set(node.features)].filter(feature => feature !== 'default').sort());
  if (JSON.stringify(features) !== JSON.stringify([FEATURES])) {
    throw new Error(`V8 Cargo features do not match the simdutf release archive: expected ${JSON.stringify(FEATURES)}, `
      + `actual ${JSON.stringify(features)}`);
  }
}

/** Checks the disk and the V8 features of the offline locked Cargo metadata of the workspace at `root` for `target`. */
export function checkMetadata(root: string, target: string): void {
  if (!path.isAbsolute(root) || !fs.statSync(root, { throwIfNoEntry: false })?.isDirectory() || fs.realpathSync(root) !== root) {
    throw new Error(`metadata root must be an absolute canonical directory: ${root}`);
  }
  checkDisk(root);
  const result = spawnSync('cargo', ['metadata', '--offline', '--locked', '--format-version', '1', '--filter-platform', target],
    { cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  if (result.error) throw new Error(`Cargo metadata could not start: ${result.error.message}`);
  if (result.status !== 0) throw new Error(`Cargo metadata failed: ${result.stderr}`);
  checkFeatures(parseJson<Metadata>(result.stdout, 'cargo-metadata.schema.json', 'cargo metadata'));
}

/** The archive and binding of `target` in var/v8, checked against the environment and the declared digests. */
export function verify(target: string): [archive: string, binding: string] {
  const [archiveHash, bindingHash] = declared(target);
  const archive = path.join(ROOT, 'var/v8', `librusty_v8_simdutf_release_${target}.a.gz`);
  const binding = path.join(ROOT, 'var/v8', `src_binding_simdutf_release_${target}.rs`);
  checkEnvironment(archive, binding);
  checkFiles(archive, binding, archiveHash, bindingHash);
  return [archive, binding];
}

/** The Rust target of the host. */
export function hostTarget(): string {
  const system = ({ darwin: 'apple-darwin', linux: 'unknown-linux-gnu' } as Partial<Record<string, string>>)[process.platform];
  const machine = ({ arm64: 'aarch64', x64: 'x86_64' } as Partial<Record<string, string>>)[process.arch];
  if (system === undefined || machine === undefined) throw new Error(`unsupported V8 host: ${process.arch} ${process.platform}`);
  return `${machine}-${system}`;
}

function main(args: string[]): number {
  const started = performance.now();
  const elapsed = () => `${((performance.now() - started) / 1000).toFixed(3)}s`;
  process.stdout.write('START V8 archive verification\n');
  let parsed;
  try {
    parsed = parseArgs({ args, allowPositionals: true, options: {
      archive: { type: 'string' }, binding: { type: 'string' }, 'metadata-root': { type: 'string', default: ROOT },
      'files-only': { type: 'boolean', default: false },
    } });
    if (parsed.positionals.length > 1) throw new Error(`unexpected arguments: ${parsed.positionals.slice(1).join(' ')}`);
  } catch (error) {
    process.stderr.write(`usage: node scripts/verify-archive.mts [target] [--archive <path> --binding <path>] [--metadata-root <dir>] [--files-only]\n${error instanceof Error ? error.message : String(error)}\n`);
    return 2;
  }
  const { values, positionals } = parsed;
  let target, archive, binding;
  try {
    target = positionals[0] ?? hostTarget();
    const [archiveHash, bindingHash] = declared(target);
    if ((values.archive === undefined) !== (values.binding === undefined)) throw new Error('archive and binding must be selected together');
    if (values.archive === undefined || values.binding === undefined) {
      [archive, binding] = verify(target);
    } else {
      [archive, binding] = [values.archive, values.binding];
      checkEnvironment(archive, binding);
      checkFiles(archive, binding, archiveHash, bindingHash);
    }
    if (!values['files-only']) checkMetadata(values['metadata-root'], target);
  } catch (error) {
    process.stderr.write(`FAIL V8 archive verification ${elapsed()}: ${error instanceof Error ? error.message : String(error)}\n`);
    return 1;
  }
  const [archiveHash, bindingHash] = declared(target);
  process.stdout.write(`PASS V8 archive verification ${elapsed()} target=${target} archive=${archive} archive_sha256=${archiveHash} `
    + `binding=${binding} binding_sha256=${bindingHash} metadata=${!values['files-only']}\n`);
  return 0;
}

if (import.meta.main) process.exitCode = main(process.argv.slice(2));

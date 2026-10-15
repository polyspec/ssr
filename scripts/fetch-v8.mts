// Fetches the official V8 archive and binding of a target and verifies them.
//
// A checkout without the V8 inputs, such as a CI runner, fetches the official release files of rusty_v8 150.4.0 for one
// target into var/v8. Each file is written to a temporary name, verified against the SHA-256 that scripts/verify-archive.mts
// declares and published by one rename; a file of another digest fails and is not kept. An existing verified file is kept.
// No build script downloads V8: the build reads only these verified files.
//
//   node scripts/fetch-v8.mts <target>
import { randomUUID } from 'node:crypto';
import { createWriteStream, existsSync, mkdirSync, renameSync, rmSync, statSync } from 'node:fs';
import path from 'node:path';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import { ARCHIVES, declared, digest } from './verify-archive.mts';

const ROOT = path.resolve(import.meta.dirname, '..');
export const RELEASE = 'https://github.com/denoland/rusty_v8/releases/download/v150.4.0';

/** The archive and binding names of `target` with their declared SHA-256. */
export function files(target: string, archives = ARCHIVES): [name: string, sha256: string][] {
  if (!Object.hasOwn(archives, target)) {
    throw new Error(`unsupported V8 target: ${target}; declared targets: ${JSON.stringify(Object.keys(archives).sort())}`);
  }
  const [archiveHash, bindingHash] = declared(target, archives);
  return [[`librusty_v8_simdutf_release_${target}.a.gz`, archiveHash], [`src_binding_simdutf_release_${target}.rs`, bindingHash]];
}

/** Fetches each file of `target` from `release` into `destination` unless a verified copy exists there. */
export async function fetchInputs(target: string, destination = path.join(ROOT, 'var/v8'), release = RELEASE, archives = ARCHIVES): Promise<void> {
  const inputs = files(target, archives);
  mkdirSync(destination, { recursive: true });
  for (const [name, expected] of inputs) {
    const file = path.join(destination, name);
    if (statSync(file, { throwIfNoEntry: false })?.isFile() && digest(file) === expected) {
      process.stdout.write(`PASS V8 input ${name} exists with SHA-256 ${expected}\n`);
      continue;
    }
    const url = `${release}/${name}`;
    process.stdout.write(`RUN fetch ${url}\n`);
    const started = performance.now();
    const temporary = path.join(destination, `.${name}.${randomUUID()}`);
    try {
      let response;
      try {
        response = await fetch(url);
      } catch (error) {
        if (!(error instanceof Error)) throw error;
        const cause = error.cause;
        throw new Error(`${url}: ${error.message}${cause instanceof Error ? `: ${cause.message}` : ''}`, { cause: error });
      }
      if (!response.ok || response.body === null) throw new Error(`${url}: HTTP ${response.status} ${response.statusText}`);
      await pipeline(Readable.fromWeb(response.body), createWriteStream(temporary, { flags: 'wx' }));
      const actual = digest(temporary);
      if (actual !== expected) throw new Error(`${url}: expected SHA-256 ${expected}, actual ${actual}`);
      renameSync(temporary, file);
    } finally {
      if (existsSync(temporary)) rmSync(temporary);
    }
    process.stdout.write(`PASS fetch ${name} ${((performance.now() - started) / 1000).toFixed(1)}s\n`);
  }
}

async function main(args: string[]): Promise<number> {
  if (args.length !== 1 || args[0] === undefined) {
    process.stderr.write('usage: node scripts/fetch-v8.mts <target>\n');
    return 2;
  }
  try {
    await fetchInputs(args[0]);
  } catch (error) {
    process.stderr.write(`FAIL fetch V8 inputs: ${error instanceof Error ? error.message : String(error)}\n`);
    return 1;
  }
  return 0;
}

if (import.meta.main) process.exitCode = await main(process.argv.slice(2));

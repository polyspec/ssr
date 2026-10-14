// The hexadecimal digest of bytes or of a file. Bytes are a Uint8Array (a Buffer is one), a type of the standard library: a
// consumer lints this copy outside its TypeScript project, where the types of node, NodeJS.ArrayBufferView among them, are
// absent, and an unresolved type in a union failed typescript/no-redundant-type-constituents (K150).
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';

/** The hexadecimal digest of `data` (text or bytes) by `algorithm`. */
export const digest = (algorithm: string, data: string | Uint8Array): string => createHash(algorithm).update(data).digest('hex');

/** The sha256 of `data`. */
export const sha256 = (data: string | Uint8Array): string => digest('sha256', data);

/** The digest of the file `file` by `algorithm`, sha256 when absent. */
export const fileDigest = (file: string, algorithm = 'sha256'): string => digest(algorithm, readFileSync(file));

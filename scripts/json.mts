// The JSON boundary of the tools of ssr: JSON text is parsed and validated against its JSON Schema (draft 2020-12, the
// files of scripts/schema) by ajv in strict mode with every error, and the value is typed by the type guard of the compiled
// schema, never by a type assertion. The shape and the names follow the JSON helper of polyspec/kit (K158), which the
// package does not export.
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { Ajv2020, type ErrorObject, type ValidateFunction } from 'ajv/dist/2020.js';

/** The directory of the schemas of the tools. */
const SCHEMAS = path.join(import.meta.dirname, 'schema');

/** A text that is not JSON. */
export class JsonError extends Error {}

/** A JSON value that its schema refuses: one line per error of ajv, prefixed with the source. */
export class SchemaError extends Error {}

const ajv = new Ajv2020({ strict: true, allErrors: true });
// The schema object of each file: ajv keeps the function that it compiled for a schema object, so a schema compiles once.
const schemas = new Map<string, object>();

// errorLine is one ajv error: the JSON Pointer of the value (`/` for the whole document), the message and its params.
function errorLine(source: string, error: ErrorObject): string {
  return `${source}: ${error.instancePath || '/'} ${error.message ?? 'is invalid'} ${JSON.stringify(error.params)}`;
}

// parse reads JSON text; invalid JSON throws a JsonError that names the source.
function parse(text: string, source: string): unknown {
  try {
    const value: unknown = JSON.parse(text);
    return value;
  } catch (error) {
    throw new JsonError(`${source}: ${error instanceof Error ? error.message : String(error)}`);
  }
}

// validator is the compiled schema file of scripts/schema.
function validator<T>(schema: string): ValidateFunction<T> {
  const file = path.join(SCHEMAS, schema);
  let rules = schemas.get(file);
  if (rules === undefined) {
    const value = parse(readFileSync(file, 'utf8'), file);
    if (value === null || typeof value !== 'object' || Array.isArray(value)) throw new JsonError(`${file}: a JSON Schema is an object`);
    rules = value;
    schemas.set(file, rules);
  }
  return ajv.compile<T>(rules);
}

/** checkJson is a parsed value against the schema file of scripts/schema, typed; it throws a SchemaError naming source. */
export function checkJson<T>(value: unknown, schema: string, source: string): T {
  const check = validator<T>(schema);
  if (check(value)) return value;
  throw new SchemaError((check.errors ?? []).map(error => errorLine(source, error)).join('\n'));
}

/** parseJson is the value of the JSON text against the schema file; it throws a JsonError or a SchemaError naming source. */
export function parseJson<T>(text: string, schema: string, source: string): T {
  return checkJson<T>(parse(text, source), schema, source);
}

/** readJsonFile is the value of the JSON file against the schema file. */
export function readJsonFile<T>(file: string, schema: string): T {
  return parseJson<T>(readFileSync(file, 'utf8'), schema, file);
}

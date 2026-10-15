// Tests of the JSON boundary of the tools (scripts/json.mts): a value that its schema accepts is returned, and a text that
// is not JSON or a value that the schema refuses throws an error that names the source.
import assert from 'node:assert/strict';
import test from 'node:test';
import { JsonError, SchemaError, parseJson } from '../../scripts/json.mts';

test('a value that its schema accepts is returned', () => {
  assert.deepEqual(parseJson('{"executablePath":"/browser","timeout":0}', 'browser-launch.schema.json', 'launch'),
    { executablePath: '/browser', timeout: 0 });
});

test('a text that is not JSON throws a JsonError that names the source', () => {
  assert.throws(() => parseJson('{', 'json-value.schema.json', 'gh api GET /x'),
    (error: unknown) => error instanceof JsonError && error.message.startsWith('gh api GET /x: '));
});

test('a value that the schema refuses throws a SchemaError with the pointer of each error', () => {
  assert.throws(() => parseJson('{"executablePath":1}', 'browser-launch.schema.json', 'launch'),
    (error: unknown) => error instanceof SchemaError
      && error.message.split('\n').sort().join('\n') === [
        'launch: / must have required property \'timeout\' {"missingProperty":"timeout"}',
        'launch: /executablePath must be string {"type":"string"}',
      ].sort().join('\n'));
});

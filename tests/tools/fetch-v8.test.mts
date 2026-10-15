// Tests of scripts/fetch-v8.mts, which fetches the official V8 archive and binding of a target into var/v8 and verifies them.
// A local HTTP server on 127.0.0.1 serves the release in place of GitHub.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, readdirSync, rmSync } from 'node:fs';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fetchInputs, files } from '../../scripts/fetch-v8.mts';

const sha256 = (text: string): string => createHash('sha256').update(text).digest('hex');

test('the files are verified and published, a verified file is kept and another digest fails', async t => {
  const directory = mkdtempSync(path.join(tmpdir(), 'ssr-fetch-v8-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const target = 'x86_64-unknown-linux-gnu';
  const [archive, binding, ...rest] = files(target).map(([name]) => name);
  assert.ok(archive !== undefined && binding !== undefined && rest.length === 0, 'the target has an archive and a binding');
  const names = [archive, binding];
  const release = new Map(names.map(name => [name, `content of ${name}`]));
  const requested: string[] = [];
  const server = createServer((request, response) => {
    const name = decodeURIComponent(request.url ?? '').replace(/^\/release\//, '');
    requested.push(name);
    const content = release.get(name);
    response.writeHead(content === undefined ? 404 : 200).end(content);
  });
  await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(() => server.close());
  const address = server.address();
  assert.ok(address !== null && typeof address !== 'string', 'the server listens on a TCP port');
  const url = `http://127.0.0.1:${address.port}/release`;
  const archives = { [target]: [sha256(`content of ${archive}`), sha256(`content of ${binding}`)] as const };
  const destination = path.join(directory, 'v8');

  await fetchInputs(target, destination, url, archives);
  assert.deepEqual(readdirSync(destination).sort(), [...names].sort());
  assert.deepEqual(requested, names);

  release.set(archive, 'changed');
  rmSync(path.join(destination, archive));
  await assert.rejects(fetchInputs(target, destination, url, archives),
    new RegExp(`expected SHA-256 ${archives[target][0]}, actual ${sha256('changed')}`));
  // The changed file is not kept, and the verified binding was not fetched again.
  assert.deepEqual(readdirSync(destination), [binding]);
  assert.deepEqual(requested, [...names, archive]);

  release.delete(archive);
  await assert.rejects(fetchInputs(target, destination, url, archives), /: HTTP 404 Not Found$/);
  assert.deepEqual(readdirSync(destination), [binding]);
});

test('an undeclared target fails', () => {
  assert.throws(() => files('riscv64'), /unsupported V8 target: riscv64; declared targets: \["aarch64-apple-darwin",/);
});

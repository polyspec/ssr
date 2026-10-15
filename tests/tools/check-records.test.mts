// Tests of scripts/check-records.mts, the checks of the tracked records of ssr.
import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test, { type TestContext } from 'node:test';
import {
  CheckoutError, checkCommitMessages, checkOutsidePaths, checkReactDocument, checkTimedAssertions, checkWords, markdownFiles,
} from '../../scripts/check-records.mts';

const ROOT = path.resolve(import.meta.dirname, '../..');

/** A temporary directory that is removed after the test. */
function temporary(t: TestContext): string {
  const directory = mkdtempSync(path.join(tmpdir(), 'ssr-check-records-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  return directory;
}

/** Writes files under root, creating their directories. */
function write(root: string, files: Record<string, string | Uint8Array>): void {
  for (const [file, content] of Object.entries(files)) {
    mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    writeFileSync(path.join(root, file), content);
  }
}

/** Makes root a Git checkout whose one commit tracks every file not ignored. */
function commitAll(root: string): void {
  for (const args of [['init', '-q'], ['add', '-A'],
    ['-c', 'user.name=test', '-c', 'user.email=test@example.com', '-c', 'commit.gpgsign=false', 'commit', '-q', '-m', 'init']]) {
    execFileSync('git', args, { cwd: root, stdio: 'pipe' });
  }
}

/** Commits the files of the checkout root with the message, also when nothing changed, and returns the short id. */
function commit(root: string, message: string): string {
  const user = ['-c', 'user.name=test', '-c', 'user.email=test@example.com', '-c', 'commit.gpgsign=false'];
  execFileSync('git', [...user, 'commit', '-q', '--allow-empty', '-m', message], { cwd: root, stdio: 'pipe' });
  return execFileSync('git', ['rev-parse', '--short', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim();
}

test('a record word in a commit message before the range is not read', t => {
  // A pushed commit is not rewritten, so only the commits being pushed are read; the whole history named the old commit.
  const root = temporary(t);
  write(root, { 'example.md': 'text\n' });
  commitAll(root);
  commit(root, 'The server answers');
  commit(root, 'A later change');
  assert.deepEqual(checkWords(root), []);
  assert.deepEqual(checkCommitMessages(root, 'HEAD~1..HEAD'), []);
});

test('a record word in a commit message of the range is named', t => {
  const root = temporary(t);
  write(root, { 'example.md': 'text\n' });
  commitAll(root);
  commit(root, 'The server answers');
  const id = commit(root, 'An orphan process');
  commit(root, 'A later change');
  assert.deepEqual(checkCommitMessages(root, 'HEAD~2..HEAD'), [`commit ${id}: prohibited record word orphan`]);
});

test('an empty, unresolvable or malformed range is an error', t => {
  const root = temporary(t);
  write(root, { 'example.md': 'text\n' });
  commitAll(root);
  const fails = (range: string, pattern: RegExp): void => {
    assert.throws(() => checkCommitMessages(root, range), (error: unknown) => error instanceof CheckoutError && pattern.test(error.message));
  };
  fails('HEAD..HEAD', /^the range HEAD\.\.HEAD holds no commit/);
  fails('missing..HEAD', /^the range missing\.\.HEAD does not resolve in /);
  fails('HEAD', /^the range "HEAD" is not <base>\.\.<head>/);
});

test('the record and term checks report prohibited words', t => {
  const root = temporary(t);
  write(root, { 'example.md': 'orphan site\n' });
  commitAll(root);
  assert.deepEqual(checkWords(root), [
    `${path.join(root, 'example.md')}: prohibited record word orphan`,
    `${path.join(root, 'example.md')}: unapproved term site`]);
});

test('a record word in a Rust comment of packages is named, in code it is not', t => {
  const root = temporary(t);
  write(root, { 'packages/core/src/lib.rs': 'fn dead() {}\n// an orphan process\n', 'other/lib.rs': '// orphan\n' });
  commitAll(root);
  assert.deepEqual(checkWords(root), [`${path.join(root, 'packages/core/src/lib.rs')}:2: prohibited record word`]);
});

test('a word only joined to a record word by a hyphen or letter is no record word', t => {
  const root = temporary(t);
  write(root, { 'example.md': 'dead-end, gateway, sites, 사이트맵\n' });
  commitAll(root);
  assert.deepEqual(checkWords(root), []);
});

test('untracked and ignored documents are not read', t => {
  const root = temporary(t);
  write(root, {
    '.gitignore': 'node_modules/\n',
    'packages/sample/node_modules/package/README.md': 'orphan site [missing](absent.md)\n',
    'record.md': 'orphan site [missing](absent.md)\n',
  });
  commitAll(root);
  write(root, { 'notes.md': 'orphan site [missing](absent.md)\n', 'packages/sample/lib.rs': '// orphan\n' });
  assert.equal(checkWords(root).length, 2);
});

test('the copies of kit are no records of this repository', t => {
  // make kit-check of kit compares the copies with the installed package; the record checks read the files of ssr.
  const root = temporary(t);
  write(root, {
    'package.json': '{"devDependencies": {"@polyspec/kit": "1.0.0"}}\n',
    'PRINCIPLES.md': 'An orphan process.\n',
    'record.md': 'An orphan process.\n',
  });
  commitAll(root);
  assert.throws(() => checkWords(root),
    (error: unknown) => error instanceof CheckoutError
      && error.message === 'node_modules/@polyspec/kit/package.json is missing; run make install-packages');
  write(root, { 'node_modules/@polyspec/kit/package.json': '{"kit": {"copies": {"PRINCIPLES.md": "PRINCIPLES.md"}}}\n' });
  assert.deepEqual(checkWords(root), [`${path.join(root, 'record.md')}: prohibited record word orphan`]);
  write(root, { 'package.json': '{"devDependencies": {}}\n' });
  assert.equal(checkWords(root).length, 2);
});

test('a kit package without kit.copies is an error', t => {
  const root = temporary(t);
  write(root, {
    'package.json': '{"devDependencies": {"@polyspec/kit": "1.0.0"}}\n',
    'node_modules/@polyspec/kit/package.json': '{"name": "@polyspec/kit"}\n',
  });
  commitAll(root);
  assert.throws(() => checkWords(root), /declares no object kit\.copies/);
});

test('an absolute path into a home directory fails', t => {
  const root = temporary(t);
  // The path is composed, so this file names no home directory itself.
  const home = '/' + 'Users' + '/someone/';
  write(root, {
    'Cargo.toml': `[dependencies]\nlib = { path = "${home}lib" }\n`,
    'Makefile': 'build:\n\tcargo build --manifest-path $(CURDIR)/Cargo.toml\n',
    'image.png': new Uint8Array([0x89, 0x50, 0xff, 0xfe]),
  });
  commitAll(root);
  assert.deepEqual(checkOutsidePaths(root), [`Cargo.toml:2:17: absolute path into a home directory: ${home}`]);
});

test('the repository names no home directory', () => {
  assert.deepEqual(checkOutsidePaths(ROOT), []);
});

test('an assertion on a measured time is named', t => {
  // The sources are composed, so this file holds no assertion on a measured time itself.
  const word = 'assert';
  const error = 'an assertion judges a measured time; assert the behaviour and print the time';
  const root = temporary(t);
  write(root, {
    'src/worker.rs': 'fn case() {\n    let started = Instant::now();\n    work();\n'
      + `    ${word}!(\n        started.elapsed()\n            < Duration::from_secs(1)\n    );\n`
      + `    ${word}_eq!(work(), Ok(()));\n    println!("{:?}", started.elapsed());\n}\n`,
    'test_tool.py': `started = time.monotonic()\n${word} time.monotonic() - started < 2\n`
      + `self.${word}Less(elapsed, 2.0)\nself.${word}Equal(result, 0)\nprint(time.monotonic() - started)\n`,
    'steps.mjs': `${word}.ok(performance.now() - started < 1000);\n${word}.equal(done, true);\n`,
    'steps.test.mts': `const started = performance.now();\n${word}.equal(run(), 'done (it\\'s finished)');\n`
      + `console.log(performance.now() - started);\n${word}(Date.now() - started < 10);\n`
      + `t.${word}.ok(elapsed < 5);\n`,
  });
  commitAll(root);
  assert.deepEqual(checkTimedAssertions(root), [
    `src/worker.rs:4: ${error}`, `test_tool.py:2: ${error}`, `test_tool.py:3: ${error}`,
    `steps.mjs:1: ${error}`, `steps.test.mts:4: ${error}`, `steps.test.mts:5: ${error}`]);
});

test('no assertion judges a measured time', () => {
  assert.deepEqual(checkTimedAssertions(ROOT), []);
});

test('a checkout that git cannot read fails', t => {
  const root = temporary(t);
  assert.throws(() => markdownFiles(root),
    (error: unknown) => error instanceof CheckoutError
      && /^git ls-files -- \*\.md in .* exited with 128: fatal: not a git repository/.test(error.message));
});

test('the tool takes the range of the commits being pushed and no other argument', () => {
  for (const args of [[], ['records'], ['--range'], ['--range', 'HEAD~1..HEAD', 'records']]) {
    const result = spawnSync(process.execPath, ['scripts/check-records.mts', ...args], { cwd: ROOT, encoding: 'utf8' });
    assert.equal(result.status, 2);
    assert.equal(result.stderr, 'usage: node scripts/check-records.mts --range <base>..<head>\n');
  }
});

const CONTRACT = '`framework_entry` `server_entry` `client_entry` `Pool::new_react` `Pool::render_stream` `ReactAdapter::stream_parts`';
const RUNTIME = '`Pool::new_react` `Pool::render_stream` `setTimeout`';

test('a missing stream API and the old Pool::new are errors of the React document', t => {
  const root = temporary(t);
  write(root, {
    'docs/react.md': '`Pool::new`', 'docs/react.ko.md': CONTRACT, 'docs/runtime.md': RUNTIME, 'docs/runtime.ko.md': RUNTIME,
  });
  const errors = checkReactDocument(root);
  assert.ok(errors.some(error => error.includes('obsolete React Pool::new')));
  assert.ok(errors.some(error => error.includes('omits `Pool::render_stream`')));
});

test('both React documents require the current API', t => {
  const root = temporary(t);
  write(root, {
    'docs/react.md': CONTRACT, 'docs/react.ko.md': CONTRACT, 'docs/runtime.md': RUNTIME, 'docs/runtime.ko.md': RUNTIME,
  });
  assert.deepEqual(checkReactDocument(root), []);
});

test('a missing React document is named', t => {
  const root = temporary(t);
  write(root, { 'docs/react.md': CONTRACT, 'docs/runtime.md': RUNTIME });
  assert.deepEqual(checkReactDocument(root), [
    `${path.join(root, 'docs/react.ko.md')}: React adapter document is missing`,
    `${path.join(root, 'docs/runtime.ko.md')}: React runtime document is missing`]);
});

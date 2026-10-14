// Checks the records of ssr: the tracked files of the checkout name no absolute path into a home directory, the documents and
// the Rust comments use no record word and the documents no unapproved term, the commit messages use no record word, the React
// documents state the current stream contract, and no test judges a measured time.
//
// Only tracked files are read, without the copies of kit (the keys of kit.copies of the installed @polyspec/kit, which make
// kit-check of kit compares with the package), so the result is the same in every checkout of the tree. Files that are neither
// tracked nor ignored are refused by the guard of the full run of kit (make check), and the push gate runs on clean checkouts.
// The document pairs, links and checklists are checked by make documents-check of kit.
//
//   node scripts/check-records.mts
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve(import.meta.dirname, '..');

/** An error of the checkout that stops the checks: Git cannot list its files or the kit package cannot be read. */
export class CheckoutError extends Error {}

export const RECORD_WORDS = ['envelope', 'gate', 'orphan', 'adopt', 'retire', 'dead', 'first-class', 'carries', 'speaks', 'answers'];
export const TERM_WORDS = ['site', 'endpoint', 'skin', 'setting', '사이트', '엔드포인트', '스킨'];

// A word character as Python's re counts it: a Unicode letter or number, or an underscore. JavaScript's \w and \b are ASCII
// only, so the boundaries are written as lookarounds over this class.
const W = String.raw`[\p{L}\p{N}_]`;
const escape = (word: string): string => word.replace(/[.*+?^${}()|[\]\\]/g, String.raw`\$&`);
const wordPattern = (words: string[], flags: string): RegExp =>
  new RegExp(String.raw`(?<!${W}|-)(?:${words.map(escape).join('|')})(?!${W}|-)`, flags);
const RECORD_PATTERN = wordPattern(RECORD_WORDS, 'giu');

/** The record words of a text: the matches of RECORD_WORDS. */
function recordWords(text: string): string[] {
  return [...text.matchAll(RECORD_PATTERN)].map(match => match[0]);
}

/** The lines of a text read by readText, whose line ends are \n. */
function lines(text: string): string[] {
  const parts = text.split('\n');
  if (parts.at(-1) === '') parts.pop();
  return parts;
}

/** The content of a UTF-8 text file with its line ends read as \n; a file that is not UTF-8 throws a TypeError. */
function readText(file: string): string {
  return new TextDecoder('utf-8', { fatal: true }).decode(readFileSync(file)).replace(/\r\n?/g, '\n');
}

function readJson(file: string): unknown {
  return JSON.parse(readFileSync(file, 'utf8')) as unknown;
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

/** The paths of the copies of kit in the checkout root: the keys of kit.copies of the installed package @polyspec/kit when
 * package.json declares it as a devDependency, else none. The copies are records of kit, not of ssr. */
export function kitCopies(root: string): Set<string> {
  const manifest = path.join(root, 'package.json');
  if (!existsSync(manifest) || !statSync(manifest).isFile()) return new Set();
  const declared = readJson(manifest);
  const devDependencies = isRecord(declared) ? declared.devDependencies : undefined;
  if (!isRecord(devDependencies) || !('@polyspec/kit' in devDependencies)) return new Set();
  const packageFile = path.join(root, 'node_modules/@polyspec/kit/package.json');
  if (!existsSync(packageFile) || !statSync(packageFile).isFile()) {
    throw new CheckoutError('node_modules/@polyspec/kit/package.json is missing; run make install-packages');
  }
  const installed = readJson(packageFile);
  const kit = isRecord(installed) ? installed.kit : undefined;
  const copies = isRecord(kit) ? kit.copies : undefined;
  if (!isRecord(copies)) throw new CheckoutError(`${packageFile} declares no object kit.copies`);
  return new Set(Object.keys(copies));
}

/** Compares two relative paths part by part, as Python orders paths. */
function comparePaths(left: string, right: string): number {
  const a = left.split('/');
  const b = right.split('/');
  for (let index = 0; index < Math.min(a.length, b.length); index++) {
    const x = a[index] as string;
    const y = b[index] as string;
    if (x !== y) return x < y ? -1 : 1;
  }
  return a.length - b.length;
}

/** The tracked files of the checkout root that match the Git pathspec pattern, relative to root and without the copies of kit.
 * Files that the tree does not hold, untracked or ignored, are not read. */
export function tracked(root: string, pattern: string): string[] {
  const result = spawnSync('git', ['ls-files', '-z', '--', pattern], { cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  if (result.error) throw new CheckoutError(`git ls-files -- ${pattern} in ${root} could not start: ${result.error.message}`);
  if (result.status !== 0) {
    throw new CheckoutError(`git ls-files -- ${pattern} in ${root} exited with ${result.status ?? result.signal}: ${result.stderr.trim()}`);
  }
  const copies = kitCopies(root);
  return result.stdout.split('\0').filter(file => file && !copies.has(file)).sort(comparePaths);
}

export function markdownFiles(root: string): string[] {
  return tracked(root, '*.md');
}

/** The record words of the documents, the Rust comments of packages/ and the commit messages, and the unapproved terms of the
 * documents. AGENTS.md names the words and is not read; the execution checklists may name the terms. */
export function checkWords(root: string): string[] {
  const errors: string[] = [];
  for (const file of markdownFiles(root)) {
    const name = path.basename(file);
    if (name === 'AGENTS.md' || name === 'AGENTS.ko.md') continue;
    const full = path.join(root, file);
    const content = readText(full);
    for (const word of recordWords(content)) errors.push(`${full}: prohibited record word ${word}`);
    if (name === 'execution-checklist.md' || name === 'execution-checklist.ko.md') continue;
    for (const word of TERM_WORDS) {
      if (wordPattern([word], 'iu').test(content)) errors.push(`${full}: unapproved term ${word}`);
    }
  }
  for (const file of tracked(root, 'packages/*.rs')) {
    const full = path.join(root, file);
    lines(readText(full)).forEach((line, index) => {
      if (line.trimStart().startsWith('//') && recordWords(line).length) errors.push(`${full}:${index + 1}: prohibited record word`);
    });
  }
  if (existsSync(path.join(root, '.git'))) {
    const result = spawnSync('git', ['log', '--format=%B'], { cwd: root, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
    if (result.error) errors.push(`git log could not start: ${result.error.message}`);
    else if (result.status !== 0) errors.push(`git log exited with ${result.status ?? result.signal}: ${result.stderr.trim()}`);
    else if (recordWords(result.stdout).length) errors.push('git log: prohibited record word');
  }
  return errors;
}

const REACT_CONTRACT = ['`framework_entry`', '`server_entry`', '`client_entry`', '`Pool::new_react`', '`Pool::render_stream`',
  '`ReactAdapter::stream_parts`'];
const RUNTIME_CONTRACT = ['`Pool::new_react`', '`Pool::render_stream`', '`setTimeout`'];

const isFile = (file: string): boolean => existsSync(file) && statSync(file).isFile();

/** The React adapter and runtime documents, in both languages, state the stream contract and not the old Pool::new. */
export function checkReactDocument(root: string): string[] {
  const errors: string[] = [];
  for (const name of ['react.md', 'react.ko.md']) {
    const file = path.join(root, 'docs', name);
    if (!isFile(file)) {
      errors.push(`${file}: React adapter document is missing`);
      continue;
    }
    const content = readText(file);
    for (const symbol of REACT_CONTRACT) if (!content.includes(symbol)) errors.push(`${file}: React stream contract omits ${symbol}`);
    if (content.includes('`Pool::new`')) errors.push(`${file}: obsolete React Pool::new contract`);
  }
  for (const name of ['runtime.md', 'runtime.ko.md']) {
    const file = path.join(root, 'docs', name);
    if (!isFile(file)) {
      errors.push(`${file}: React runtime document is missing`);
      continue;
    }
    const content = readText(file);
    for (const symbol of RUNTIME_CONTRACT) if (!content.includes(symbol)) errors.push(`${file}: React runtime contract omits ${symbol}`);
  }
  return errors;
}

// An absolute path into a home directory names a file of one machine, such as another checkout.
const HOME_PATH = /\/(?:Users|home)\/[A-Za-z0-9._-]+\//;

/** Tracked text files name no absolute path into a home directory: such a path exists on one machine only, so the tree would
 * not build or give the same result elsewhere. Files that are not UTF-8 text, and directories (submodules), are not read. */
export function checkOutsidePaths(root: string): string[] {
  const errors: string[] = [];
  for (const file of tracked(root, '*')) {
    let text: string;
    try {
      text = readText(path.join(root, file));
    } catch (error) {
      if (error instanceof TypeError || (error as NodeJS.ErrnoException).code === 'EISDIR') continue;
      throw error;
    }
    lines(text).forEach((line, index) => {
      const match = HOME_PATH.exec(line);
      // The column counts code points, as Python does.
      if (match) {
        errors.push(`${file}:${index + 1}:${Array.from(line.slice(0, match.index)).length + 1}: absolute path into a home directory: ${match[0]}`);
      }
    });
  }
  return errors;
}

// A measured time: an elapsed duration or a clock reading.
const MEASURED_TIME = new RegExp(String.raw`(?<!${W})elapsed(?!${W})|monotonic\(|perf_counter|performance\.now|Date\.now|time\.time\(|Instant::now`, 'u');
// An assertion: a Rust assert macro, a unittest assertion, a call of node:assert or of one of its functions, a call of expect
// of a test framework, or a Python assert statement. The alternatives are written so that this source holds no assertion itself.
const ASSERTION = new RegExp([
  String.raw`(?<!${W})(?:debug_)?(?:assert|assert_eq|assert_ne)!\s*\(`,
  String.raw`(?<!${W})self\.assert${W}*\s*\(`,
  String.raw`(?<!${W})assert\.${W}+\s*\(`,
  String.raw`(?<!${W}|\.)assert\s*\(`,
  String.raw`(?<!${W})expect\s*\(`,
  String.raw`^[ \t]*assert\s`,
].join('|'), 'gmu');
export const SOURCES = ['*.rs', '*.py', '*.js', '*.mjs', '*.ts', '*.mts', '*.tsx'];

/** The text an assertion judges: the balanced arguments of an assertion call, whose string literals (delimited by quotes) take
 * no part in the balance, or the rest of the line of an assert statement. */
export function assertionText(text: string, match: RegExpExecArray, quotes: string): string {
  const start = match.index + match[0].length;
  if (!match[0].endsWith('(')) {
    const end = text.indexOf('\n', start);
    return text.slice(start, end < 0 ? text.length : end);
  }
  let depth = 1;
  let quote: string | undefined;
  for (let index = start; index < text.length; index++) {
    const character = text[index] as string;
    if (quote) {
      if (character === '\\') index++;
      else if (character === quote) quote = undefined;
    } else if (quotes.includes(character)) {
      quote = character;
    } else {
      depth += character === '(' ? 1 : character === ')' ? -1 : 0;
      if (depth === 0) return text.slice(start, index);
    }
  }
  return text.slice(start);
}

/** No test judges a measured time. Performance is measured and printed, and never fails a test: a slower host fails a correct
 * test by the clock and not by its result, so a test asserts the behaviour and prints the time it took. */
export function checkTimedAssertions(root: string): string[] {
  const errors: string[] = [];
  for (const pattern of SOURCES) {
    for (const file of tracked(root, pattern)) {
      const text = readText(path.join(root, file));
      // A Rust apostrophe also starts a lifetime, so only double quotes delimit its strings.
      const quotes = path.extname(file) === '.rs' ? '"' : '"\'`';
      for (const match of text.matchAll(ASSERTION)) {
        if (MEASURED_TIME.test(assertionText(text, match, quotes))) {
          const number = text.slice(0, match.index).split('\n').length;
          errors.push(`${file}:${number}: an assertion judges a measured time; assert the behaviour and print the time`);
        }
      }
    }
  }
  return errors;
}

/** The checks of the tracked files: the paths, the words of the records, the React document and the timed assertions. */
export function checkRecords(root: string): string[] {
  return [...checkOutsidePaths(root), ...checkWords(root), ...checkReactDocument(root), ...checkTimedAssertions(root)];
}

function main(args: string[]): number {
  if (args.length) {
    process.stderr.write('usage: node scripts/check-records.mts\n');
    return 2;
  }
  let errors: string[];
  try {
    errors = checkRecords(ROOT);
  } catch (error) {
    // The checkout cannot be read: Git fails, the kit package is missing, or a tracked file cannot be opened.
    if (error instanceof CheckoutError || typeof (error as NodeJS.ErrnoException).code === 'string') {
      process.stderr.write(`${(error as Error).message}\n`);
      return 1;
    }
    throw error;
  }
  for (const error of errors) process.stderr.write(`${error}\n`);
  if (errors.length) return 1;
  process.stdout.write('records: pass\n');
  return 0;
}

if (import.meta.main) process.exitCode = main(process.argv.slice(2));

// Keeps the GitHub ruleset of the protected branch and the merge settings equal to .github/ruleset.json.
//
//   node scripts/github-ruleset.mts apply   create or update the ruleset of the declared name and change the declared
//                                           repository settings where they differ, then compare again
//   node scripts/github-ruleset.mts check   change nothing; fail when the live ruleset or a setting differs
//
// The declaration names the repository, the repository settings that the pull request flow needs (`settings`, fields of
// PATCH /repos/{owner}/{repo}) and the ruleset as the REST API takes it (`ruleset`). Every change reaches the protected
// branch through a pull request and the merge queue that the ruleset requires; this script publishes nothing.
//
// Requires an authenticated `gh` with administration access to the repository. The script reads nothing of the
// repository but the declaration, so it runs unchanged in every repository that declares its own .github/ruleset.json.
import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve(import.meta.dirname, '..');
const DECLARATION = path.join(ROOT, '.github/ruleset.json');

const USAGE = `usage:
  node scripts/github-ruleset.mts apply   create or update the ruleset of the declared name and change the declared
                                          repository settings where they differ, then compare again
  node scripts/github-ruleset.mts check   change nothing; fail when the live ruleset or a setting differs
`;

export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
type JsonObject = { [key: string]: Json };

/** `gh api` as a function: [status, body]; `body` undefined sends no input. Never fails for an HTTP status. */
export type Api = (method: string, apiPath: string, body?: Json) => [number, Json];

/** A field whose live value differs: [field, actual, wanted]. */
export type Change = [string, Json, Json];

export interface Declaration {
  repository: string;
  settings?: JsonObject;
  ruleset: JsonObject & { name: string };
}

/** A request or the declaration failed; the message names its cause. */
export class Stop extends Error {}

const isObject = (value: Json | undefined): value is JsonObject =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

export function ghApi(method: string, apiPath: string, body?: Json): [number, Json] {
  const args = ['api', '--method', method, '--include', apiPath, ...(body === undefined ? [] : ['--input', '-'])];
  const result = spawnSync('gh', args, { input: body === undefined ? '' : JSON.stringify(body), encoding: 'utf8' });
  if (result.error) throw new Stop(`gh api ${method} ${apiPath} failed: gh could not start: ${result.error.message}`);
  // gh exits non-zero for an HTTP error status; the status line is still on standard output.
  const found = /^HTTP\/[\d.]+ (\d{3})/.exec(result.stdout);
  if (!found?.[1]) {
    throw new Stop(`gh api ${method} ${apiPath} failed: ${result.stderr.trim() || `exit status ${result.status ?? result.signal}`}`);
  }
  const text = result.stdout.split(/\r?\n\r?\n/).slice(1).join('\n\n').trim();
  return [Number(found[1]), text ? JSON.parse(text) as Json : null];
}

function expectOk(api: Api, method: string, apiPath: string, body?: Json): Json {
  const [status, answer] = body === undefined ? api(method, apiPath) : api(method, apiPath, body);
  if (status >= 300) throw new Stop(`${method} ${apiPath} answered ${status}: ${canonical(answer)}`);
  return answer;
}

/** JSON with the keys of every object sorted, so two values compare by content. */
export function canonical(value: Json | undefined): string {
  if (value === undefined || value === null) return 'null';
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
  if (typeof value === 'object') {
    return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${canonical(value[key])}`).join(',')}}`;
  }
  return JSON.stringify(value);
}

const byCanonical = (a: Json, b: Json): number => {
  const [left, right] = [canonical(a), canonical(b)];
  return left < right ? -1 : left > right ? 1 : 0;
};

/** The fields of a ruleset that the declaration sets, in an order-free form: rules sorted, the required checks and
 * the bypass actors sorted. Fields that GitHub adds (id, source, _links, timestamps) are not compared. */
export function normalize(ruleset: Json, keys: string[]): JsonObject {
  const picked: JsonObject = Object.fromEntries(keys.map(key => [key, isObject(ruleset) ? ruleset[key] ?? null : null]));
  const actors = picked.bypass_actors;
  if (Array.isArray(actors)) picked.bypass_actors = [...actors].sort(byCanonical);
  const rules = picked.rules;
  if (Array.isArray(rules)) {
    picked.rules = rules.map(rule => {
      const parameters = isObject(rule) ? rule.parameters : undefined;
      const checks = isObject(parameters) ? parameters.required_status_checks : undefined;
      if (!isObject(rule) || !isObject(parameters) || !Array.isArray(checks)) return rule;
      return { ...rule, parameters: { ...parameters, required_status_checks: [...checks].sort(byCanonical) } };
    }).sort(byCanonical);
  }
  return picked;
}

/** The fields of `declared` whose live value differs, empty when they match. */
export function differences(declared: JsonObject, live: Json, prefix = ''): Change[] {
  const keys = Object.keys(declared);
  const [wanted, actual] = [normalize(declared, keys), normalize(live, keys)];
  return keys.filter(key => canonical(actual[key]) !== canonical(wanted[key]))
    .map(key => [prefix + key, actual[key] ?? null, wanted[key] ?? null]);
}

/** The declaration read from `file`; a missing or malformed field is an error. */
export function readDeclaration(file: string): Declaration {
  const value = JSON.parse(readFileSync(file, 'utf8')) as Json;
  if (!isObject(value)) throw new Stop(`${file} is not a JSON object`);
  const { repository, settings, ruleset } = value;
  if (typeof repository !== 'string') throw new Stop(`${file}: repository is not a string`);
  if (!isObject(ruleset) || typeof ruleset.name !== 'string') throw new Stop(`${file}: ruleset is not an object with a name`);
  if (settings !== undefined && settings !== null && !isObject(settings)) throw new Stop(`${file}: settings is not an object`);
  return { repository, ruleset: { ...ruleset, name: ruleset.name }, ...(isObject(settings) ? { settings } : {}) };
}

function rulesetId(ruleset: Json, repository: string): number {
  if (!isObject(ruleset) || typeof ruleset.id !== 'number') throw new Stop(`${repository} answered a ruleset without an id: ${canonical(ruleset)}`);
  return ruleset.id;
}

/** The live ruleset of the declared name, or null; fails when the name is not unique. */
export function liveRuleset(declaration: Declaration, api: Api): JsonObject | null {
  const repo = `repos/${declaration.repository}`;
  const name = declaration.ruleset.name;
  const listed = expectOk(api, 'GET', `${repo}/rulesets?includes_parents=false&per_page=100`) ?? [];
  if (!Array.isArray(listed)) throw new Stop(`GET ${repo}/rulesets answered no list: ${canonical(listed)}`);
  const named = listed.filter(ruleset => isObject(ruleset) && ruleset.name === name);
  if (named.length > 1) {
    const ids = named.map(ruleset => rulesetId(ruleset, declaration.repository)).join(', ');
    throw new Stop(`${declaration.repository} has ${named.length} rulesets named ${name} (ids ${ids}); delete all but one`);
  }
  const [only] = named;
  if (only === undefined) return null;
  const live = expectOk(api, 'GET', `${repo}/rulesets/${rulesetId(only, declaration.repository)}`);
  if (!isObject(live)) throw new Stop(`GET ${repo}/rulesets/${rulesetId(only, declaration.repository)} answered no object: ${canonical(live)}`);
  return live;
}

/** Compare the declaration with the live repository: [live ruleset, setting changes, ruleset changes]. Reads only. */
export function plan(declaration: Declaration, api: Api): [JsonObject | null, Change[], Change[]] {
  const settings = declaration.settings ?? {};
  const settingChanges = Object.keys(settings).length
    ? differences(settings, expectOk(api, 'GET', `repos/${declaration.repository}`), 'settings.')
    : [];
  const live = liveRuleset(declaration, api);
  if (live === null) return [null, settingChanges, [['ruleset', null, declaration.ruleset.name]]];
  return [live, settingChanges, differences(declaration.ruleset, live)];
}

/** Change the declared settings and create or update the ruleset where they differ, then compare again: the second
 * comparison must be empty. Returns the changes. */
export function apply(declaration: Declaration, api: Api, log: (line: string) => void): Change[] {
  const repo = `repos/${declaration.repository}`;
  const [live, settingChanges, rulesetChanges] = plan(declaration, api);
  for (const [field, actual, wanted] of [...settingChanges, ...rulesetChanges]) {
    log(`[ruleset] ${field}: ${canonical(actual)} -> ${canonical(wanted)}`);
  }
  if (settingChanges.length) expectOk(api, 'PATCH', repo, declaration.settings ?? {});
  if (live === null) expectOk(api, 'POST', `${repo}/rulesets`, declaration.ruleset);
  else if (rulesetChanges.length) expectOk(api, 'PUT', `${repo}/rulesets/${rulesetId(live, declaration.repository)}`, declaration.ruleset);
  const [, settingsLeft, rulesetLeft] = plan(declaration, api);
  if (settingsLeft.length || rulesetLeft.length) {
    const fields = [...settingsLeft, ...rulesetLeft].map(([field]) => field).join(', ');
    throw new Stop(`the repository still differs after applying: ${fields}`);
  }
  return [...settingChanges, ...rulesetChanges];
}

export function main(argv: string[], api: Api = ghApi, file: string = DECLARATION): number {
  const [mode] = argv;
  if (argv.length !== 1 || (mode !== 'apply' && mode !== 'check')) {
    process.stderr.write(USAGE);
    return 2;
  }
  const log = (line: string): void => { process.stdout.write(`${line}\n`); };
  const error = (line: string): void => { process.stderr.write(`${line}\n`); };
  let name: string;
  let changes: Change[];
  try {
    const declaration = readDeclaration(file);
    name = `${declaration.repository} ruleset ${declaration.ruleset.name}`;
    if (mode === 'apply') {
      const applied = apply(declaration, api, log);
      log(`[ruleset] ${name}: ${applied.length} field(s) changed; the repository matches .github/ruleset.json`);
      return 0;
    }
    const [, settingChanges, rulesetChanges] = plan(declaration, api);
    changes = [...settingChanges, ...rulesetChanges];
  } catch (cause) {
    error(`[ruleset] ${(cause as Error).message}`);
    return 1;
  }
  for (const [field, actual, wanted] of changes) {
    error(`[ruleset] differs: ${field}: live ${canonical(actual)}, declared ${canonical(wanted)}`);
  }
  if (changes.length) {
    error(`[ruleset] ${name} differs from .github/ruleset.json; run make github-ruleset`);
    return 1;
  }
  log(`[ruleset] ${name}: the repository matches .github/ruleset.json`);
  return 0;
}

if (import.meta.main) process.exitCode = main(process.argv.slice(2));

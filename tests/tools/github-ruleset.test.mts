// Tests of scripts/github-ruleset.mts, the ruleset of main and the merge settings.
//
// Each case of the comparison copies the script and a declaration into a temporary directory and runs it with a fake
// `gh` first on PATH. The fake keeps its state in a JSON file, answers as the REST API does and logs every call, so a
// case asserts the requests that apply sends and the result of check without reaching GitHub.
import assert from 'node:assert/strict';
import { spawnSync, type SpawnSyncReturns } from 'node:child_process';
import { chmodSync, copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test, { type TestContext } from 'node:test';
import type { Json } from '../../scripts/github-ruleset.mts';

const ROOT = path.resolve(import.meta.dirname, '../..');
const SCRIPT = path.join(ROOT, 'scripts/github-ruleset.mts');
const WORKFLOWS = path.join(ROOT, '.github/workflows');

type JsonObject = { [key: string]: Json };
type Rule = { type: string; parameters?: JsonObject };
type Ruleset = JsonObject & { name: string; rules: Rule[] };
interface DeclarationFile { repository: string; settings: JsonObject; ruleset: Ruleset }
interface State { repository: string; calls: Json[][]; rulesets: JsonObject[]; settings: JsonObject }

const DECLARATION = JSON.parse(readFileSync(path.join(ROOT, '.github/ruleset.json'), 'utf8')) as DeclarationFile;
const REPOSITORY = DECLARATION.repository;
const copy = <T,>(value: T): T => structuredClone(value);

const FAKE_GH = String.raw`
const fs = require('node:fs');
const statePath = process.env.FAKE_STATE;
const state = JSON.parse(fs.readFileSync(statePath, 'utf8'));
const args = process.argv.slice(2);
const method = args[args.indexOf('--method') + 1];
const path = args[args.indexOf('--include') + 1];
const body = args.includes('--input') ? JSON.parse(fs.readFileSync(0, 'utf8') || 'null') : null;
state.calls.push(['gh', method, path, ...(body === null ? [] : [body])]);
const repo = 'repos/' + state.repository;

function answer(status, payload) {
  fs.writeFileSync(statePath, JSON.stringify(state));
  fs.writeSync(1, 'HTTP/2.0 ' + status + ' X\r\nContent-Type: application/json\r\n\r\n'
    + (payload === undefined ? '' : JSON.stringify(payload)));
  process.exit(status >= 400 ? 1 : 0);
}

function stored(ruleset, ident) {
  return { ...ruleset, id: ident, source: state.repository, source_type: 'Repository', node_id: 'RRS_' + ident,
    _links: { self: { href: 'https://api.github.com/' + repo + '/rulesets/' + ident } },
    created_at: '2026-10-06T00:00:00Z', updated_at: '2026-10-06T00:00:00Z', current_user_can_bypass: 'never' };
}

if (path.startsWith(repo + '/rulesets?') && method === 'GET') {
  answer(200, state.rulesets.map(r => ({ id: r.id, name: r.name, target: r.target })));
}
if (path === repo + '/rulesets' && method === 'POST') {
  state.rulesets.push(stored(body, 100 + state.rulesets.length));
  answer(201, state.rulesets[state.rulesets.length - 1]);
}
if (path.startsWith(repo + '/rulesets/')) {
  const ident = Number(path.slice(path.lastIndexOf('/') + 1));
  const index = state.rulesets.findIndex(r => r.id === ident);
  if (index >= 0) {
    if (method === 'PUT') state.rulesets[index] = stored(body, ident);
    answer(200, state.rulesets[index]);
  }
}
if (path === repo && method === 'GET') answer(200, { full_name: state.repository, id: 1, ...state.settings });
if (path === repo && method === 'PATCH') {
  Object.assign(state.settings, body);
  answer(200, { full_name: state.repository, ...state.settings });
}
answer(404, { message: 'Not Found' });
`;

/** The script, a declaration and the fake gh in a temporary directory. */
class Sandbox {
  readonly root: string;
  readonly statePath: string;

  constructor(t: TestContext, state: Partial<State> = {}, declaration: DeclarationFile = DECLARATION) {
    this.root = mkdtempSync(path.join(tmpdir(), 'ssr-github-ruleset-'));
    t.after(() => { rmSync(this.root, { recursive: true, force: true }); });
    for (const directory of ['scripts', '.github', 'bin']) mkdirSync(path.join(this.root, directory));
    copyFileSync(SCRIPT, path.join(this.root, 'scripts/github-ruleset.mts'));
    writeFileSync(path.join(this.root, '.github/ruleset.json'), JSON.stringify(declaration));
    const fake = path.join(this.root, 'bin/gh');
    writeFileSync(fake, `#!${process.execPath}\n${FAKE_GH}`);
    chmodSync(fake, 0o755);
    this.statePath = path.join(this.root, 'state.json');
    const initial: State = { repository: REPOSITORY, calls: [], rulesets: [], settings: { ...declaration.settings }, ...state };
    writeFileSync(this.statePath, JSON.stringify(initial));
  }

  state(): State {
    return JSON.parse(readFileSync(this.statePath, 'utf8')) as State;
  }

  run(mode: string): SpawnSyncReturns<string> {
    const env = { ...process.env, PATH: `${path.join(this.root, 'bin')}${path.delimiter}${process.env.PATH ?? ''}`, FAKE_STATE: this.statePath };
    const result = spawnSync(process.execPath, ['scripts/github-ruleset.mts', mode], { cwd: this.root, env, encoding: 'utf8' });
    if (result.error) throw result.error;
    return result;
  }

  calls(): Json[][] {
    return this.state().calls.filter(call => call[0] === 'gh');
  }

  writes(): Json[][] {
    return this.calls().filter(call => call[1] !== 'GET');
  }
}

function live(ruleset: Ruleset, ident = 7): Ruleset {
  return { ...copy(ruleset), id: ident, source: REPOSITORY, source_type: 'Repository', _links: {},
    created_at: '2026-10-01T00:00:00Z', updated_at: '2026-10-01T00:00:00Z', current_user_can_bypass: 'never' };
}

const count = (text: string, pattern: RegExp): number => [...text.matchAll(pattern)].length;

/** The job IDs of a workflow: the lines indented by two spaces after `jobs:`. */
function jobs(workflow: string): string[] {
  const start = workflow.indexOf('\njobs:\n');
  assert.notEqual(start, -1, 'the workflow has no jobs');
  return [...workflow.slice(start + '\njobs:\n'.length).matchAll(/^ {2}([\w-]+):\s*$/gm)].map(match => match[1] ?? '');
}

test('main takes changes only through pull requests and the merge queue without bypass', () => {
  const ruleset = DECLARATION.ruleset;
  assert.equal(ruleset.enforcement, 'active');
  assert.equal(ruleset.target, 'branch');
  assert.deepEqual(ruleset.bypass_actors, []);
  assert.deepEqual((ruleset.conditions as { ref_name: { include: string[] } }).ref_name.include, ['refs/heads/main']);
  const rules = new Map(ruleset.rules.map(rule => [rule.type, rule.parameters ?? {}]));
  assert.deepEqual([...rules.keys()].sort(), ['deletion', 'merge_queue', 'non_fast_forward', 'pull_request',
    'required_linear_history', 'required_status_checks']);
  const pullRequest = rules.get('pull_request') ?? {};
  assert.equal(pullRequest.required_approving_review_count, 0);
  // The merge queue merges with its own method. gh pr merge --auto asks GitHub for auto-merge with a method of its
  // own choice; a method that this rule does not allow leaves the pull request out of the queue, so the rule allows
  // every method, and the queue and required_linear_history decide.
  assert.deepEqual(pullRequest.allowed_merge_methods, ['merge', 'squash', 'rebase']);
  // GitHub turns this on by default; with no approval required, a one-owner repository asks for none.
  assert.equal(pullRequest.require_extra_approval_for_unattributed_changes, false);
  // Each commit of a pull request lands on main as it is, rebased, so history stays linear.
  assert.equal(rules.get('merge_queue')?.merge_method, 'REBASE');
  assert.equal(rules.get('required_status_checks')?.strict_required_status_checks_policy, false);
  // gh pr merge --auto --rebase needs auto-merge and the rebase method; the merged branch is deleted.
  assert.deepEqual(DECLARATION.settings, { allow_rebase_merge: true, allow_auto_merge: true, delete_branch_on_merge: true });
});

test('the required checks are the push gate and the job ci-passed', () => {
  const checks = DECLARATION.ruleset.rules.find(rule => rule.type === 'required_status_checks')?.parameters?.required_status_checks;
  // The check of a job without a name is the job ID. ci-passed, the last job of ci.yml, passes only when every other
  // job of ci.yml passed, the jobs lint and test on each runner of their matrix included.
  const gate = readFileSync(path.join(WORKFLOWS, 'push-gate.yml'), 'utf8');
  assert.deepEqual(jobs(gate), ['push-gate']);
  assert.doesNotMatch(gate, /^ {4}name:/m);
  const ci = readFileSync(path.join(WORKFLOWS, 'ci.yml'), 'utf8');
  assert.deepEqual(jobs(ci), ['lint', 'test', 'ci-passed']);
  assert.doesNotMatch(ci, /^ {4}name:/m);
  // 15368 is the GitHub Actions app, so a status of the same name from another app does not satisfy the rule.
  assert.deepEqual(checks, ['push-gate', 'ci-passed'].map(job => ({ context: job, integration_id: 15368 })));
});

test('the Makefile runs the script for each target and publishes nothing', () => {
  const makefile = readFileSync(path.join(ROOT, 'Makefile'), 'utf8');
  for (const [target, mode] of [['github-ruleset', 'apply'], ['github-ruleset-check', 'check']] as const) {
    assert.match(makefile, new RegExp(`^${target}:\\n\\t\\$\\(NODE\\) scripts/github-ruleset\\.mts ${mode}$`, 'm'));
  }
  assert.doesNotMatch(makefile, /^push:/m);
  const phony = /^\.PHONY:(.*(?:\\\n.*)*)/m.exec(makefile);
  assert.ok(phony?.[1] !== undefined, 'the Makefile has no .PHONY line');
  assert.ok(!phony[1].split(/\s+/).includes('push'));
});

test('check fails without the ruleset and apply creates it', t => {
  const sandbox = new Sandbox(t);
  const result = sandbox.run('check');
  assert.equal(result.status, 1, result.stdout);
  assert.ok(result.stderr.includes('[ruleset] differs: ruleset: live null, declared "main"'), result.stderr);
  assert.ok(result.stderr.includes('run make github-ruleset'), result.stderr);
  assert.deepEqual(new Set(sandbox.calls().map(call => call[1])), new Set(['GET']), 'check only reads');
  const applied = sandbox.run('apply');
  assert.equal(applied.status, 0, applied.stderr);
  assert.deepEqual(sandbox.calls().filter(call => call[1] === 'POST'),
    [['gh', 'POST', `repos/${REPOSITORY}/rulesets`, DECLARATION.ruleset]]);
  const again = sandbox.run('check');
  assert.equal(again.status, 0, again.stderr);
  assert.ok(again.stdout.includes('the repository matches .github/ruleset.json'), again.stdout);
});

test('a setting that differs fails check and apply patches only the declared settings', t => {
  const sandbox = new Sandbox(t, { rulesets: [live(DECLARATION.ruleset)], settings: { ...DECLARATION.settings, allow_auto_merge: false } });
  const result = sandbox.run('check');
  assert.equal(result.status, 1);
  assert.ok(result.stderr.includes('[ruleset] differs: settings.allow_auto_merge: live false, declared true'), result.stderr);
  assert.equal(count(result.stderr, /\[ruleset\] differs:/g), 1, result.stderr);
  const applied = sandbox.run('apply');
  assert.equal(applied.status, 0, applied.stderr);
  assert.deepEqual(sandbox.writes(), [['gh', 'PATCH', `repos/${REPOSITORY}`, DECLARATION.settings]]);
  assert.equal(sandbox.run('check').status, 0);
});

test('check names a missing merge queue', t => {
  const ruleset = live(DECLARATION.ruleset);
  ruleset.rules = ruleset.rules.filter(rule => rule.type !== 'merge_queue');
  const sandbox = new Sandbox(t, { rulesets: [ruleset] });
  const result = sandbox.run('check');
  assert.equal(result.status, 1);
  assert.ok(result.stderr.includes('[ruleset] differs: rules: live ['), result.stderr);
  assert.ok(result.stderr.slice(result.stderr.indexOf('declared')).includes('"merge_method":"REBASE"'), result.stderr);
});

test('order and fields that GitHub adds are not differences', t => {
  const ruleset = live(DECLARATION.ruleset);
  ruleset.rules = [...ruleset.rules].reverse();
  ruleset.conditions = { ref_name: { exclude: [], include: ['refs/heads/main'] } };
  const sandbox = new Sandbox(t, { rulesets: [ruleset] });
  const result = sandbox.run('check');
  assert.equal(result.status, 0, result.stderr);
});

test('a bypass actor differs and apply replaces the ruleset of the name', t => {
  const ruleset = live(DECLARATION.ruleset);
  ruleset.bypass_actors = [{ actor_id: 5, actor_type: 'RepositoryRole', bypass_mode: 'always' }];
  const other = live({ ...DECLARATION.ruleset, name: 'tags', target: 'tag' }, 8);
  const sandbox = new Sandbox(t, { rulesets: [other, ruleset] });
  const result = sandbox.run('check');
  assert.equal(result.status, 1);
  assert.ok(result.stderr.includes('[ruleset] differs: bypass_actors: live [{"actor_id":5,'), result.stderr);
  assert.ok(result.stderr.includes('declared []'), result.stderr);
  const applied = sandbox.run('apply');
  assert.equal(applied.status, 0, applied.stderr);
  assert.deepEqual(sandbox.writes(), [['gh', 'PUT', `repos/${REPOSITORY}/rulesets/7`, DECLARATION.ruleset]]);
  assert.equal(sandbox.run('check').status, 0);
  assert.equal(sandbox.state().rulesets[0]?.name, 'tags', 'apply changes only the ruleset of the name');
});

test('a missing rule, another check and disabled enforcement each differ', t => {
  const ruleset = live(DECLARATION.ruleset);
  ruleset.enforcement = 'evaluate';
  ruleset.rules = ruleset.rules.filter(rule => rule.type !== 'non_fast_forward');
  const checks = ruleset.rules.find(rule => rule.type === 'required_status_checks');
  assert.ok(checks?.parameters);
  checks.parameters.required_status_checks = [{ context: 'ci', integration_id: 15368 }];
  const sandbox = new Sandbox(t, { rulesets: [ruleset] });
  const result = sandbox.run('check');
  assert.equal(result.status, 1);
  assert.ok(result.stderr.includes('[ruleset] differs: enforcement: live "evaluate", declared "active"'), result.stderr);
  assert.ok(result.stderr.includes('[ruleset] differs: rules: live [{"parameters":'), result.stderr);
  assert.ok(result.stderr.includes('"context":"ci"'), result.stderr);
  assert.equal(count(result.stderr, /\[ruleset\] differs:/g), 2, result.stderr);
});

test('two rulesets of the name fail with both ids', t => {
  const sandbox = new Sandbox(t, { rulesets: [live(DECLARATION.ruleset, 7), live(DECLARATION.ruleset, 9)] });
  for (const mode of ['check', 'apply']) {
    const result = sandbox.run(mode);
    assert.equal(result.status, 1);
    assert.ok(result.stderr.includes(`${REPOSITORY} has 2 rulesets named main (ids 7, 9); delete all but one`), result.stderr);
  }
  assert.deepEqual(new Set(sandbox.calls().map(call => call[1])), new Set(['GET']));
});

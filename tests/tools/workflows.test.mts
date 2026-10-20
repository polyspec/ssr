// Tests of the GitHub workflows of the repository and the make targets they run: every step runs a make target, every
// job that runs checks uploads its report, the actions are pinned, the triggers, the job ci-passed, and the CI targets of
// each architecture. .github/workflows/release.yml and gates.yml are copies of kit, tested in kit. The workflows are read with the
// YAML parser of the ecosystem (yaml).
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { chmodSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { parse } from 'yaml';
import { checkJson } from '../../scripts/json.mts';

const ROOT = path.resolve(import.meta.dirname, '../..');
const WORKFLOWS = ['ci.yml', 'push-gate.yml'];
// The make files of the targets: the Makefile, the bootstrap of kit that it includes and the kit.mk of the installed
// package that the bootstrap includes.
const MAKE_FILES = ['Makefile', 'scripts/kit/kit-bootstrap.mk', 'node_modules/@polyspec/kit/dist/kit.mk'];
// The steps that install the package of kit before any other make target of a job.
const INSTALL_KIT = ['make install-tools TOOLS=npm', 'make install-packages'];
const CI_PASSED_RUN = "make ci-passed RESULTS='${{ toJSON(needs) }}'";
// The jobs that run their checks through make ci-targets of kit and upload its logs and summary.
const REPORTING_JOBS: Record<string, string[]> = { 'ci.yml': ['lint', 'test'], 'push-gate.yml': ['push-gate'] };
// The make target and the variable of the targets of each check job of ci.yml.
const CI_JOBS = { lint: ['ci-lint', 'CI_LINT_TARGETS'], test: ['ci-test', 'CI_TEST_TARGETS'] } as const;
const CI_TARGETS = ['check-tools', 'check-fixture-filter', 'check-fmt', 'check-clippy', 'check-deny', 'check-examples', 'ci-nextest'];

type Step = { uses?: string; run?: string; if?: string; with?: Record<string, unknown>; env?: Record<string, string> };
type Job = { if?: string; needs?: string[]; 'runs-on'?: string; strategy?: { 'fail-fast'?: boolean; matrix?: { runner?: string[] } }; steps: Step[] };
type Workflow = { on: Record<string, unknown>; jobs: Record<string, Job>; concurrency?: Record<string, unknown> };

const read = (file: string): string => readFileSync(path.join(ROOT, file), 'utf8');
const workflow = (name: string): Workflow => {
  const value: unknown = parse(read(`.github/workflows/${name}`));
  return checkJson<Workflow>(value, 'workflow.schema.json', `.github/workflows/${name}`);
};
/** The value that a test expects to be there: a job of a workflow or a group of a match. */
function known<T>(value: T | undefined, what: string): T {
  assert.ok(value !== undefined, `${what} is missing`);
  return value;
}
const runs = (job: Job): string[] => job.steps.flatMap(step => (step.run === undefined ? [] : [step.run]));
const makefile = read('Makefile');

/** The targets of the make files. */
function makeTargets(): Set<string> {
  return new Set(MAKE_FILES.flatMap(file => [...read(file).matchAll(/^([A-Za-z0-9_.-]+):/gm)].map(match => known(match[1], `the target name of ${file}`))));
}

/** The commands of a target of the Makefile, as written. */
function recipe(target: string): string[] {
  const lines = makefile.split('\n');
  const start = lines.indexOf(`${target}:`);
  assert.notEqual(start, -1, `the Makefile has no target ${target}`);
  const commands: string[] = [];
  for (const line of lines.slice(start + 1)) {
    if (!line.startsWith('\t')) break;
    commands.push(line.trim());
  }
  return commands;
}

/** The value of `NAME = value` of the Makefile, split into words. */
function variable(name: string): string[] {
  const found = new RegExp(`^${name} = (.*)$`, 'm').exec(makefile);
  assert.ok(found, `the Makefile has no variable ${name}`);
  return known(found[1], `the value of ${name}`).split(/\s+/);
}

test('every step runs a make target', () => {
  const targets = makeTargets();
  for (const name of WORKFLOWS) {
    const commands = Object.values(workflow(name).jobs).flatMap(runs);
    assert.ok(commands.length, name);
    for (const command of commands) {
      const found = /^make ([a-z0-9-]+)(?: [A-Z_]+='\$\{\{ [^}]+ \}\}'| TOOLS=[a-z]+)*$/.exec(command);
      assert.ok(found, `${name}: ${command} is not a make target`);
      assert.ok(targets.has(known(found[1], command)), `${name}: ${command}`);
    }
  }
});

test('every job uploads its logs and summary also after a failure', () => {
  for (const name of WORKFLOWS) {
    const { jobs } = workflow(name);
    const reporting = REPORTING_JOBS[name] ?? [];
    assert.deepEqual(Object.keys(jobs).filter(job => !reporting.includes(job)), name === 'ci.yml' ? ['ci-passed'] : [], name);
    for (const job of reporting) {
      const body = known(jobs[job], `the job ${job}`);
      const upload = body.steps.find(step => step.uses?.startsWith('actions/upload-artifact@'));
      assert.ok(upload, `${name}: ${job}`);
      assert.equal(upload.if, '${{ !cancelled() }}', `${name}: ${job}`);
      assert.equal(upload.with?.path, 'var/report/ci-targets/', `${name}: ${job}`);
      if (body.strategy?.matrix) assert.equal(body.strategy['fail-fast'], false, `${name}: ${job}`);
    }
  }
});

test('every action is pinned to a commit', () => {
  for (const name of WORKFLOWS) {
    for (const line of read(`.github/workflows/${name}`).split('\n')) {
      if (line.includes('uses: ')) assert.match(line, /uses: [a-z-]+\/[a-z-]+@[0-9a-f]{40} # v\d+\.\d+\.\d+$/, name);
    }
  }
});

test('a new push cancels the previous pull request run', () => {
  assert.deepEqual(workflow('ci.yml').concurrency, {
    group: '${{ github.workflow }}-${{ github.ref }}',
    'cancel-in-progress': "${{ github.event_name == 'pull_request' }}",
  });
  assert.equal(workflow('push-gate.yml').concurrency, undefined);
});

test('each workflow declares exactly its triggers', () => {
  // ci.yml runs the checks on every pull request, merge group and manual run: the merge group runs CI on the commit that
  // main receives. push-gate.yml also runs on every push outside the branches of the merge queue. The only other workflows
  // are the release workflow and the gates workflow of kit, copies that kit-check compares with the package.
  assert.deepEqual(readdirSync(path.join(ROOT, '.github/workflows')).sort(), ['ci.yml', 'gates.yml', 'push-gate.yml', 'release.yml']);
  assert.deepEqual(workflow('ci.yml').on, { pull_request: null, merge_group: null, workflow_dispatch: null });
  assert.deepEqual(workflow('push-gate.yml').on, {
    push: { 'branches-ignore': ['gh-readonly-queue/**'] }, pull_request: null, merge_group: null,
  });
});

test('the push check reads the commit messages of the commits that its event brings', () => {
  // make check-records reads the messages of RANGE only: the commits of a push after the tip it replaced (after the default
  // branch for a new branch), of a pull request and of a merge group after their base. The checkout holds the whole
  // history, so the base resolves.
  const job = known(workflow('push-gate.yml').jobs['push-gate'], 'the job push-gate');
  assert.equal(known(job.steps[0], 'the checkout step').with?.['fetch-depth'], 0);
  const step = known(job.steps.find(each => each.run === 'make ci-push'), 'the step make ci-push');
  assert.deepEqual(step.env, {
    RANGE: "${{ github.event_name == 'pull_request'"
      + " && format('{0}..{1}', github.event.pull_request.base.sha, github.event.pull_request.head.sha)"
      + " || github.event_name == 'merge_group'"
      + " && format('{0}..{1}', github.event.merge_group.base_sha, github.event.merge_group.head_sha)"
      + ' || github.event.created'
      + " && format('origin/{0}..{1}', github.event.repository.default_branch, github.sha)"
      + " || format('{0}..{1}', github.event.before, github.sha) }}",
  });
});

test('the push check runs the record checks and the TypeScript checks', () => {
  assert.deepEqual(variable('CI_PUSH_TARGETS'),
    ['toolchain-check', 'push-gate-commit', 'check-records', 'check-commit-records', 'typecheck', 'lint-ts']);
  assert.deepEqual(recipe('check-records'), ['$(NODE) scripts/check-records.mts']);
  assert.deepEqual(recipe('check-commit-records'), ['$(NODE) scripts/check-records.mts --range $(RANGE)']);
  // The commits being pushed exist only in the push check: make check, on a checkout that may match origin/main, runs the
  // checks of the files and no check of a range.
  assert.ok(!variable('CHECK_TARGETS').includes('check-commit-records'));
  assert.ok(!variable('SSR_TARGETS').includes('check-commit-records'));
  assert.ok(variable('SSR_TARGETS').includes('check-records'));
  // make toolchain-check of ci-push reads the Rust of rust-toolchain.toml and the tools of mise.lock, so the job installs
  // both first: make install-tools of kit installs the Rust toolchain too (kit K151), so no target of ssr runs rustup.
  assert.deepEqual(runs(known(workflow('push-gate.yml').jobs['push-gate'], 'the job push-gate')), [...INSTALL_KIT, 'make install-tools', 'make ci-push']);
  assert.equal(makeTargets().has('install-rust'), false);
  assert.ok(!recipe('ci-setup').some((line) => line.includes('rustup') || line.includes('install-rust')));
});

test('ci-passed is the last job and needs every other job', () => {
  const { jobs } = workflow('ci.yml');
  const names = Object.keys(jobs);
  assert.equal(names.at(-1), 'ci-passed');
  const body = known(jobs['ci-passed'], 'the job ci-passed');
  assert.equal(body.if, '${{ always() }}');
  assert.deepEqual([...(body.needs ?? [])].sort(), names.filter(job => job !== 'ci-passed').sort());
  // The runner of the job push-gate; make ci-passed needs only Node.js and the package of kit.
  assert.equal(body['runs-on'], known(workflow('push-gate.yml').jobs['push-gate'], 'the job push-gate')['runs-on']);
  assert.deepEqual(runs(body), [...INSTALL_KIT, CI_PASSED_RUN]);
  assert.equal(body.steps.at(-1)?.run, CI_PASSED_RUN);
});

test('each architecture runs every CI target in exactly one job', () => {
  const { jobs } = workflow('ci.yml');
  assert.deepEqual(Object.keys(jobs).sort(), [...Object.keys(CI_JOBS), 'ci-passed'].sort());
  const targets: string[] = [];
  for (const [job, [target, name]] of Object.entries(CI_JOBS)) {
    const body = known(jobs[job], `the job ${job}`);
    assert.deepEqual(body.strategy?.matrix?.runner, ['ubuntu-24.04-arm', 'ubuntu-24.04'], job);
    assert.equal(body['runs-on'], '${{ matrix.runner }}', job);
    assert.deepEqual(runs(body), [...INSTALL_KIT, 'make install-tools', 'make ci-setup', `make ${target}`], job);
    const upload = body.steps.find(step => step.uses?.startsWith('actions/upload-artifact@'));
    assert.equal(upload?.with?.name, `ci-${job}-\${{ matrix.runner }}`, job);
    assert.equal(recipe(target).at(-1), `BUILD_JOBS=$$(nproc) $(MAKE) --no-print-directory ci-targets TARGETS="$(${name})"`, job);
    targets.push(...variable(name));
  }
  assert.deepEqual(targets.sort(), [...CI_TARGETS].sort());
  assert.doesNotMatch(makefile, /^ci:/m);
});

test('the CI jobs take cargo-nextest and cargo-deny from mise and run no Python', () => {
  // make install-tools installs the tools of mise.lock, cargo-nextest and cargo-deny among them, and make toolchain-check
  // checks their versions; no step sets up Python and no target of the CI jobs runs it.
  for (const job of Object.keys(CI_JOBS)) {
    assert.ok(!known(workflow('ci.yml').jobs[job], `the job ${job}`).steps.some(step => step.uses?.startsWith('actions/setup-python@')), job);
  }
  for (const target of ['ci-setup', 'ci-lint', 'ci-test']) {
    assert.ok(!recipe(target).some(command => command.includes('python')), target);
  }
  assert.match(read('mise.toml'), /^"github:nextest-rs\/nextest" = \{ version = "0\.9\.146", version_prefix = "cargo-nextest-" \}$/m);
  assert.match(read('mise.toml'), /^cargo-deny = "0\.20\.2"$/m);
});

/** The CARGO_BUILD_JOBS that a run of make ci-nextest gives cargo, with a cargo on PATH that prints it. */
function buildJobs(t: test.TestContext, environment: Record<string, string>): string[] {
  const directory = mkdtempSync(path.join(tmpdir(), 'ssr-workflows-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const cargo = path.join(directory, 'cargo');
  writeFileSync(cargo, '#!/bin/sh\necho "CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS"\n');
  chmodSync(cargo, 0o755);
  const env: NodeJS.ProcessEnv = { ...process.env };
  for (const name of ['MAKEFLAGS', 'MFLAGS', 'MAKELEVEL', 'GNUMAKEFLAGS', 'MAKEFILES', 'BUILD_JOBS', 'CARGO_BUILD_JOBS']) delete env[name];
  env.PATH = `${directory}${path.delimiter}${env.PATH ?? ''}`;
  const ran = spawnSync('make', ['--no-print-directory', 'ci-nextest'], { cwd: ROOT, env: { ...env, ...environment }, encoding: 'utf8' });
  assert.equal(ran.status, 0, ran.stdout + ran.stderr);
  return [...ran.stdout.matchAll(/^CARGO_BUILD_JOBS=(.*)$/gm)].map(match => known(match[1], 'the value of CARGO_BUILD_JOBS'));
}

test('a local build runs one job and BUILD_JOBS sets the count', t => {
  assert.deepEqual(buildJobs(t, {}), ['1']);
  assert.deepEqual(buildJobs(t, { BUILD_JOBS: '4' }), ['4']);
});

test('rustup never installs a toolchain', () => {
  assert.ok(makefile.includes('\nexport RUSTUP_AUTO_INSTALL := 0\n'));
  assert.ok(makefile.includes('V8_TARGET := $(shell RUSTUP_AUTO_INSTALL=0 rustc -vV'));
});

test('the full suite checks no advisory and the review does', () => {
  assert.deepEqual(recipe('check-deny'), ['cargo deny check $(DENY_CHECKS)']);
  assert.deepEqual(variable('DENY_CHECKS'), ['bans', 'licenses', 'sources']);
  assert.deepEqual(recipe('review-advisories'), ['cargo deny check advisories',
    'cargo deny --manifest-path tests/test-ownership/Cargo.toml --config deny.toml --locked check --hide-inclusion-graph advisories']);
  assert.ok(!variable('CHECK_TARGETS').includes('review-advisories'));
});

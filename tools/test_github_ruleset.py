"""The GitHub ruleset of main and the merge settings (tools/github_ruleset.py).

Each case copies the script and a declaration into a temporary directory and runs it with a fake `gh` first on PATH.
The fake keeps its state in a JSON file, answers as the REST API does and logs every call, so a case asserts the
requests that apply sends and the result of check without reaching GitHub.
"""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / 'tools/github_ruleset.py'
DECLARATION = json.loads((ROOT / '.github/ruleset.json').read_text())
WORKFLOWS = ROOT / '.github/workflows'
REPOSITORY = DECLARATION['repository']

FAKE_GH = r'''
import json, os, sys
state_path = os.environ['FAKE_STATE']
state = json.load(open(state_path))
args = sys.argv[1:]
method, path = args[args.index('--method') + 1], args[args.index('--include') + 1]
body = json.loads(sys.stdin.read() or 'null') if '--input' in args else None
state['calls'].append(['gh', method, path] + ([body] if body is not None else []))
repo = 'repos/' + state['repository']

def answer(status, payload=None):
    json.dump(state, open(state_path, 'w'))
    sys.stdout.write(f'HTTP/2.0 {status} X\r\nContent-Type: application/json\r\n\r\n')
    if payload is not None:
        sys.stdout.write(json.dumps(payload))
    sys.exit(1 if status >= 400 else 0)

def stored(ruleset, ident):
    return {**ruleset, 'id': ident, 'source': state['repository'], 'source_type': 'Repository',
            'node_id': f'RRS_{ident}', '_links': {'self': {'href': f'https://api.github.com/{repo}/rulesets/{ident}'}},
            'created_at': '2026-10-06T00:00:00Z', 'updated_at': '2026-10-06T00:00:00Z', 'current_user_can_bypass': 'never'}

if path.startswith(repo + '/rulesets?') and method == 'GET':
    answer(200, [{'id': r['id'], 'name': r['name'], 'target': r['target']} for r in state['rulesets']])
if path == repo + '/rulesets' and method == 'POST':
    ident = 100 + len(state['rulesets'])
    state['rulesets'].append(stored(body, ident))
    answer(201, state['rulesets'][-1])
if path.startswith(repo + '/rulesets/'):
    ident = int(path.rsplit('/', 1)[1])
    index = next(i for i, r in enumerate(state['rulesets']) if r['id'] == ident)
    if method == 'PUT':
        state['rulesets'][index] = stored(body, ident)
    answer(200, state['rulesets'][index])
if path == repo and method == 'GET':
    answer(200, {'full_name': state['repository'], 'id': 1, **state['settings']})
if path == repo and method == 'PATCH':
    state['settings'].update(body)
    answer(200, {'full_name': state['repository'], **state['settings']})
answer(404, {'message': 'Not Found'})
'''

class Sandbox:
    """The script, a declaration and the fake gh in a temporary directory."""

    def __init__(self, test, declaration=DECLARATION, **state):
        self.temporary = tempfile.TemporaryDirectory(prefix='ssr-ruleset-')
        test.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / 'tools').mkdir()
        (self.root / '.github').mkdir()
        (self.root / 'bin').mkdir()
        shutil.copy2(SCRIPT, self.root / 'tools')
        (self.root / '.github/ruleset.json').write_text(json.dumps(declaration))
        fake = self.root / 'bin/gh'
        fake.write_text(f'#!{sys.executable}\n{FAKE_GH}')
        fake.chmod(0o755)
        self.state_path = self.root / 'state.json'
        self.write({'repository': REPOSITORY, 'calls': [], 'rulesets': [], 'settings': dict(declaration['settings']),
                    'other_setting': True, **state})

    def write(self, state):
        self.state_path.write_text(json.dumps(state))

    def state(self):
        return json.loads(self.state_path.read_text())

    def run(self, mode):
        env = {name: value for name, value in os.environ.items() if not name.startswith('GIT_')}
        env.update(PATH=f'{self.root / "bin"}{os.pathsep}{env["PATH"]}', FAKE_STATE=str(self.state_path))
        return subprocess.run([sys.executable, 'tools/github_ruleset.py', mode], cwd=self.root, capture_output=True,
                              text=True, env=env)

    def calls(self, kind=None):
        return [call for call in self.state()['calls'] if kind is None or call[0] == kind]


def live(ruleset, ident=7):
    return {**json.loads(json.dumps(ruleset)), 'id': ident, 'source': REPOSITORY, 'source_type': 'Repository',
            '_links': {}, 'created_at': '2026-10-01T00:00:00Z', 'updated_at': '2026-10-01T00:00:00Z',
            'current_user_can_bypass': 'never'}


class Declaration(unittest.TestCase):
    def test_main_takes_changes_only_through_pull_requests_and_the_merge_queue_without_bypass(self):
        ruleset = DECLARATION['ruleset']
        self.assertEqual(ruleset['enforcement'], 'active')
        self.assertEqual(ruleset['target'], 'branch')
        self.assertEqual(ruleset['bypass_actors'], [])
        self.assertEqual(ruleset['conditions']['ref_name']['include'], ['refs/heads/main'])
        rules = {rule['type']: rule.get('parameters') for rule in ruleset['rules']}
        self.assertEqual(sorted(rules), ['deletion', 'merge_queue', 'non_fast_forward', 'pull_request',
                                         'required_linear_history', 'required_status_checks'])
        self.assertEqual(rules['pull_request']['required_approving_review_count'], 0)
        # The merge queue merges with its own method. gh pr merge --auto asks GitHub for auto-merge with a method of its
        # own choice; a method that this rule does not allow leaves the pull request out of the queue, so the rule allows
        # every method, and the queue and required_linear_history decide.
        self.assertEqual(rules['pull_request']['allowed_merge_methods'], ['merge', 'squash', 'rebase'])
        # GitHub turns this on by default; with no approval required, a one-owner repository asks for none.
        self.assertIs(rules['pull_request']['require_extra_approval_for_unattributed_changes'], False)
        # Each commit of a pull request lands on main as it is, rebased, so history stays linear.
        self.assertEqual(rules['merge_queue']['merge_method'], 'REBASE')
        self.assertIs(rules['required_status_checks']['strict_required_status_checks_policy'], False)
        # gh pr merge --auto --rebase needs auto-merge and the rebase method; the merged branch is deleted.
        self.assertEqual(DECLARATION['settings'], {'allow_rebase_merge': True, 'allow_auto_merge': True,
                                                   'delete_branch_on_merge': True})

    def test_the_required_checks_are_the_push_gate_and_the_job_ci_passed(self):
        checks = next(rule for rule in DECLARATION['ruleset']['rules']
                      if rule['type'] == 'required_status_checks')['parameters']['required_status_checks']
        # The check of a job without a name is the job ID. ci-passed, the last job of ci.yml, passes only when every
        # other job of ci.yml passed (tools/test_ci.py), the jobs lint and test on each runner of their matrix included.
        gate = (WORKFLOWS / 'push-gate.yml').read_text()
        self.assertEqual(re.findall(r'(?m)^  ([\w-]+):\s*$', gate.split('\njobs:\n', 1)[1]), ['push-gate'])
        self.assertNotRegex(gate, r'(?m)^    name:')
        ci = (WORKFLOWS / 'ci.yml').read_text()
        self.assertEqual(re.findall(r'(?m)^  ([\w-]+):\s*$', ci.split('\njobs:\n', 1)[1]), ['lint', 'test', 'ci-passed'])
        self.assertNotRegex(ci, r'(?m)^    name:')
        jobs = ['push-gate', 'ci-passed']
        # 15368 is the GitHub Actions app, so a status of the same name from another app does not satisfy the rule.
        self.assertEqual(checks, [{'context': job, 'integration_id': 15368} for job in jobs])

    def test_the_makefile_runs_the_script_for_each_target_and_publishes_nothing(self):
        makefile = (ROOT / 'Makefile').read_text()
        for target, mode in (('github-ruleset', 'apply'), ('github-ruleset-check', 'check')):
            self.assertRegex(makefile, rf'(?m)^{target}:\n\tpython3 -m tools.github_ruleset {mode}$')
        self.assertNotRegex(makefile, r'(?m)^push:')
        self.assertNotIn('push', re.findall(r'(?m)^\.PHONY:(.*(?:\\\n.*)*)', makefile)[0].split())


class Comparison(unittest.TestCase):
    def test_check_fails_without_the_ruleset_and_apply_creates_it(self):
        sandbox = Sandbox(self)
        result = sandbox.run('check')
        self.assertEqual(result.returncode, 1, result.stdout)
        self.assertIn('[ruleset] differs: ruleset: live null, declared "main"', result.stderr)
        self.assertIn('run make github-ruleset', result.stderr)
        self.assertEqual({call[1] for call in sandbox.calls('gh')}, {'GET'}, 'check only reads')
        applied = sandbox.run('apply')
        self.assertEqual(applied.returncode, 0, applied.stderr)
        posts = [call for call in sandbox.calls('gh') if call[1] == 'POST']
        self.assertEqual(posts, [['gh', 'POST', f'repos/{REPOSITORY}/rulesets', DECLARATION['ruleset']]])
        again = sandbox.run('check')
        self.assertEqual(again.returncode, 0, again.stderr)
        self.assertIn('the repository matches .github/ruleset.json', again.stdout)

    def test_a_setting_that_differs_fails_check_and_apply_patches_only_the_declared_settings(self):
        settings = {**DECLARATION['settings'], 'allow_auto_merge': False}
        sandbox = Sandbox(self, rulesets=[live(DECLARATION['ruleset'])], settings=settings)
        result = sandbox.run('check')
        self.assertEqual(result.returncode, 1)
        self.assertIn('[ruleset] differs: settings.allow_auto_merge: live false, declared true', result.stderr)
        self.assertEqual(len(re.findall(r'\[ruleset\] differs:', result.stderr)), 1, result.stderr)
        applied = sandbox.run('apply')
        self.assertEqual(applied.returncode, 0, applied.stderr)
        writes = [call for call in sandbox.calls('gh') if call[1] != 'GET']
        self.assertEqual(writes, [['gh', 'PATCH', f'repos/{REPOSITORY}', DECLARATION['settings']]])
        self.assertEqual(sandbox.run('check').returncode, 0)

    def test_check_names_a_missing_merge_queue(self):
        ruleset = live(DECLARATION['ruleset'])
        ruleset['rules'] = [rule for rule in ruleset['rules'] if rule['type'] != 'merge_queue']
        sandbox = Sandbox(self, rulesets=[ruleset])
        result = sandbox.run('check')
        self.assertEqual(result.returncode, 1)
        self.assertIn('[ruleset] differs: rules: live [', result.stderr)
        self.assertIn('"merge_method":"REBASE"', result.stderr.split('declared', 1)[1])

    def test_order_and_fields_that_github_adds_are_not_differences(self):
        ruleset = live(DECLARATION['ruleset'])
        ruleset['rules'] = list(reversed(ruleset['rules']))
        ruleset['conditions'] = {'ref_name': {'exclude': [], 'include': ['refs/heads/main']}}
        sandbox = Sandbox(self, rulesets=[ruleset])
        result = sandbox.run('check')
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_a_bypass_actor_differs_and_apply_replaces_the_ruleset_of_the_name(self):
        ruleset = live(DECLARATION['ruleset'])
        ruleset['bypass_actors'] = [{'actor_id': 5, 'actor_type': 'RepositoryRole', 'bypass_mode': 'always'}]
        other = live({**DECLARATION['ruleset'], 'name': 'tags', 'target': 'tag'}, ident=8)
        sandbox = Sandbox(self, rulesets=[other, ruleset])
        result = sandbox.run('check')
        self.assertEqual(result.returncode, 1)
        self.assertIn('[ruleset] differs: bypass_actors: live [{"actor_id":5,', result.stderr)
        self.assertIn('declared []', result.stderr)
        applied = sandbox.run('apply')
        self.assertEqual(applied.returncode, 0, applied.stderr)
        writes = [call for call in sandbox.calls('gh') if call[1] != 'GET']
        self.assertEqual(writes, [['gh', 'PUT', f'repos/{REPOSITORY}/rulesets/7', DECLARATION['ruleset']]])
        self.assertEqual(sandbox.run('check').returncode, 0)
        self.assertEqual(sandbox.state()['rulesets'][0]['name'], 'tags', 'apply changes only the ruleset of the name')

    def test_a_missing_rule_another_check_and_disabled_enforcement_each_differ(self):
        ruleset = live(DECLARATION['ruleset'])
        ruleset['enforcement'] = 'evaluate'
        ruleset['rules'] = [rule for rule in ruleset['rules'] if rule['type'] != 'non_fast_forward']
        checks = next(rule for rule in ruleset['rules'] if rule['type'] == 'required_status_checks')
        checks['parameters']['required_status_checks'] = [{'context': 'ci', 'integration_id': 15368}]
        sandbox = Sandbox(self, rulesets=[ruleset])
        result = sandbox.run('check')
        self.assertEqual(result.returncode, 1)
        self.assertIn('[ruleset] differs: enforcement: live "evaluate", declared "active"', result.stderr)
        self.assertIn('[ruleset] differs: rules: live [{"parameters":', result.stderr)
        self.assertIn('"context":"ci"', result.stderr)
        self.assertEqual(len(re.findall(r'\[ruleset\] differs:', result.stderr)), 2, result.stderr)

    def test_two_rulesets_of_the_name_fail_with_both_ids(self):
        sandbox = Sandbox(self, rulesets=[live(DECLARATION['ruleset'], 7), live(DECLARATION['ruleset'], 9)])
        for mode in ('check', 'apply'):
            result = sandbox.run(mode)
            self.assertEqual(result.returncode, 1)
            self.assertIn(f'{REPOSITORY} has 2 rulesets named main (ids 7, 9); delete all but one', result.stderr)
        self.assertEqual({call[1] for call in sandbox.calls('gh')}, {'GET'})


if __name__ == '__main__':
    unittest.main()

"""Keep the GitHub ruleset of the protected branch and the merge settings equal to .github/ruleset.json.

    python3 -m tools.github_ruleset apply   create or update the ruleset of the declared name and change the declared
                                              repository settings where they differ, then compare again
    python3 -m tools.github_ruleset check   change nothing; fail when the live ruleset or a setting differs

The declaration names the repository, the repository settings that the pull request flow needs (`settings`, fields of
PATCH /repos/{owner}/{repo}) and the ruleset as the REST API takes it (`ruleset`). Every change reaches the protected
branch through a pull request and the merge queue that the ruleset requires; this script publishes nothing.

Requires an authenticated `gh` with administration access to the repository, and only the standard library of Python
3.9. The script reads nothing of the repository but the declaration, so it runs unchanged in every repository that
declares its own .github/ruleset.json.
"""
import json
from pathlib import Path
import re
import subprocess
import sys

DECLARATION = Path(__file__).resolve().parents[1] / '.github/ruleset.json'


class Stop(Exception):
    """A request failed; the message names its cause."""


def gh_api(method, path, body=None):
    """`gh api` as a function: (status, body); never raises for an HTTP status."""
    args = ['gh', 'api', '--method', method, '--include', path] + (['--input', '-'] if body is not None else [])
    result = subprocess.run(args, input='' if body is None else json.dumps(body), capture_output=True, text=True)
    # gh exits non-zero for an HTTP error status; the status line is still on stdout.
    found = re.match(r'HTTP/[\d.]+ (\d{3})', result.stdout)
    if not found:
        raise Stop(f'gh api {method} {path} failed: {result.stderr.strip() or f"exit status {result.returncode}"}')
    text = '\n\n'.join(re.split(r'\r?\n\r?\n', result.stdout)[1:]).strip()
    return int(found.group(1)), (json.loads(text) if text else None)


def expect_ok(api, method, path, body=None):
    status, answer = api(method, path, body)
    if status >= 300:
        raise Stop(f'{method} {path} answered {status}: {canonical(answer)}')
    return answer


def canonical(value):
    """JSON with the keys of every object sorted, so two values compare by content."""
    return json.dumps(value, sort_keys=True, separators=(',', ':'))


def normalize(ruleset, keys):
    """The fields of a ruleset that the declaration sets, in an order-free form: rules sorted, the required checks and
    the bypass actors sorted. Fields that GitHub adds (id, source, _links, timestamps) are not compared."""
    picked = {key: (ruleset or {}).get(key) for key in keys}
    if isinstance(picked.get('bypass_actors'), list):
        picked['bypass_actors'] = sorted(picked['bypass_actors'], key=canonical)
    if isinstance(picked.get('rules'), list):
        rules = []
        for rule in picked['rules']:
            checks = (rule.get('parameters') or {}).get('required_status_checks')
            if isinstance(checks, list):
                rule = {**rule, 'parameters': {**rule['parameters'], 'required_status_checks': sorted(checks, key=canonical)}}
            rules.append(rule)
        picked['rules'] = sorted(rules, key=canonical)
    return picked


def differences(declared, live, prefix=''):
    """The fields of `declared` whose live value differs: [(field, actual, wanted)], empty when they match."""
    keys = list(declared)
    wanted, actual = normalize(declared, keys), normalize(live, keys)
    return [(prefix + key, actual[key], wanted[key]) for key in keys if canonical(actual[key]) != canonical(wanted[key])]


def live_ruleset(declaration, api):
    """The live ruleset of the declared name, or None; fails when the name is not unique."""
    repo = f"repos/{declaration['repository']}"
    name = declaration['ruleset']['name']
    listed = expect_ok(api, 'GET', f'{repo}/rulesets?includes_parents=false&per_page=100') or []
    named = [ruleset for ruleset in listed if ruleset.get('name') == name]
    if len(named) > 1:
        ids = ', '.join(str(ruleset['id']) for ruleset in named)
        raise Stop(f"{declaration['repository']} has {len(named)} rulesets named {name} (ids {ids}); delete all but one")
    return expect_ok(api, 'GET', f"{repo}/rulesets/{named[0]['id']}") if named else None


def plan(declaration, api):
    """Compare the declaration with the live repository: (live ruleset, setting changes, ruleset changes). Reads only."""
    settings = declaration.get('settings') or {}
    setting_changes = (differences(settings, expect_ok(api, 'GET', f"repos/{declaration['repository']}"), 'settings.')
                       if settings else [])
    live = live_ruleset(declaration, api)
    if live is None:
        return None, setting_changes, [('ruleset', None, declaration['ruleset']['name'])]
    return live, setting_changes, differences(declaration['ruleset'], live)


def apply(declaration, api, log):
    """Change the declared settings and create or update the ruleset where they differ, then compare again: the second
    comparison must be empty. Returns the changes."""
    repo = f"repos/{declaration['repository']}"
    live, setting_changes, ruleset_changes = plan(declaration, api)
    for field, actual, wanted in setting_changes + ruleset_changes:
        log(f'[ruleset] {field}: {canonical(actual)} -> {canonical(wanted)}')
    if setting_changes:
        expect_ok(api, 'PATCH', repo, declaration['settings'])
    if live is None:
        expect_ok(api, 'POST', f'{repo}/rulesets', declaration['ruleset'])
    elif ruleset_changes:
        expect_ok(api, 'PUT', f"{repo}/rulesets/{live['id']}", declaration['ruleset'])
    _, settings_left, ruleset_left = plan(declaration, api)
    if settings_left or ruleset_left:
        fields = ', '.join(field for field, _, _ in settings_left + ruleset_left)
        raise Stop(f'the repository still differs after applying: {fields}')
    return setting_changes + ruleset_changes


def main(argv):
    if len(argv) != 1 or argv[0] not in ('apply', 'check'):
        print(__doc__, file=sys.stderr)
        return 2

    def log(line):
        print(line, flush=True)

    def error(line):
        print(line, file=sys.stderr, flush=True)

    try:
        declaration = json.loads(DECLARATION.read_text())
        name = f"{declaration['repository']} ruleset {declaration['ruleset']['name']}"
        if argv[0] == 'apply':
            changes = apply(declaration, gh_api, log)
            log(f'[ruleset] {name}: {len(changes)} field(s) changed; the repository matches .github/ruleset.json')
            return 0
        _, setting_changes, ruleset_changes = plan(declaration, gh_api)
    except (Stop, OSError, ValueError, KeyError) as cause:
        error(f'[ruleset] {cause}')
        return 1
    changes = setting_changes + ruleset_changes
    for field, actual, wanted in changes:
        error(f'[ruleset] differs: {field}: live {canonical(actual)}, declared {canonical(wanted)}')
    if changes:
        error(f'[ruleset] {name} differs from .github/ruleset.json; run make github-ruleset')
        return 1
    log(f'[ruleset] {name}: the repository matches .github/ruleset.json')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))

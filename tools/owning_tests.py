"""Name the owning tests of tracked files.

``tools/test-owners.json`` maps every tracked file to the commands of the tests that own it: each
rule names Git path patterns (``fnmatch``, where ``*`` also matches ``/``) and test commands.
``check`` fails for a tracked file that no rule names, for a pattern that names no tracked file and
for a command that names a test that does not exist: a Python test module, a workspace package or
test target, a make target or a tracked tool. ``select`` prints the owning test commands of the
given paths, or of the files changed against ``HEAD``, staged, unstaged and untracked, once each in
the order of the map; a removed file is not a changed file, and a changed file without an owner
fails. ``tools/check.py`` runs ``check``
before each commit.

    python3 -m tools.owning_tests check
    python3 -m tools.owning_tests select [<path>...]
"""

from fnmatch import fnmatchcase
import json
from pathlib import Path
import re
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
DECLARATION = ROOT / "tools/test-owners.json"


def git(root, *arguments):
    result = subprocess.run(["git", *arguments], cwd=root, capture_output=True, text=True, check=False)
    if result.returncode:
        raise OSError(f"git {' '.join(arguments)} in {root} exited with {result.returncode}: "
                      f"{result.stderr.strip()}")
    return [path for path in result.stdout.split("\0") if path]


def rules(path=DECLARATION):
    document = json.loads(path.read_text())
    owners = document["owners"]
    for rule in owners:
        if set(rule) != {"paths", "tests"} or not rule["paths"] or not rule["tests"]:
            raise ValueError(f"{path}: each rule names nonempty paths and tests: {rule}")
    return owners


def owners_of(path, owners):
    return [rule for rule in owners if any(fnmatchcase(path, pattern) for pattern in rule["paths"])]


def makefile_targets(root):
    return {match.group(1) for match in re.finditer(r"^([A-Za-z0-9_.-]+):", (root / "Makefile").read_text(),
                                                    re.MULTILINE)}


def command_errors(command, root, members, targets, tracked):
    """The reason why ``command`` names no existing test, or None."""
    words = command.split()
    if words[:3] == ["python3", "-m", "unittest"] and len(words) == 4:
        module = words[3]
        if not re.fullmatch(r"tools\.test_[a-z_]+", module) or f"tools/{module[6:]}.py" not in tracked:
            return f"no tracked test module {module}"
        return None
    if words[:3] == ["cargo", "nextest", "run"]:
        rest = words[3:]
        if rest == ["--workspace"]:
            return None
        if len(rest) >= 2 and rest[0] == "-p":
            if rest[1] not in members:
                return f"no workspace package {rest[1]}"
            if rest[2:] == []:
                return None
            if len(rest) == 4 and rest[2] == "--test":
                if f"crates/{rest[1]}/tests/{rest[3]}.rs" not in tracked:
                    return f"no test target {rest[3]} in {rest[1]}"
                return None
        return f"unsupported cargo command {command}"
    if words[:1] == ["make"] and len(words) == 2:
        return None if words[1] in targets else f"no make target {words[1]}"
    if words[:1] == ["python3"]:
        tool = words[2].replace(".", "/") + ".py" if words[1:2] == ["-m"] else words[1]
        return None if tool in tracked else f"no tracked tool {tool}"
    return f"unsupported test command {command}"


def check(root=ROOT, path=DECLARATION):
    owners = rules(path)
    tracked = git(root, "ls-files", "-z")
    tracked_set = set(tracked)
    members = {Path(item).parts[1] for item in tracked if item.startswith("crates/") and item.endswith("/Cargo.toml")}
    targets = makefile_targets(root)
    errors = [f"{file}: no owning tests in tools/test-owners.json" for file in tracked if not owners_of(file, owners)]
    for rule in owners:
        for pattern in rule["paths"]:
            if not any(fnmatchcase(file, pattern) for file in tracked):
                errors.append(f"tools/test-owners.json: {pattern} names no tracked file")
        for command in rule["tests"]:
            error = command_errors(command, root, members, targets, tracked_set)
            if error:
                errors.append(f"tools/test-owners.json: {command}: {error}")
    return list(dict.fromkeys(errors))


def changed(root=ROOT):
    files = git(root, "diff", "-z", "--name-only", "--diff-filter=d", "HEAD")
    files += git(root, "ls-files", "-z", "--others", "--exclude-standard")
    return sorted(set(files))


def select(paths, path=DECLARATION):
    """The owning test commands of ``paths`` once each, and the paths without an owner."""
    owners = rules(path)
    commands, missing = [], []
    for file in paths:
        found = owners_of(file, owners)
        if not found:
            missing.append(file)
        for rule in found:
            commands += [command for command in rule["tests"] if command not in commands]
    return commands, missing


def main(argv):
    try:
        if argv == ["check"]:
            errors = check()
            for error in errors:
                print(f"FAIL owning tests: {error}", file=sys.stderr)
            if not errors:
                print("PASS owning tests: every tracked file has owning tests", flush=True)
            return 1 if errors else 0
        if argv[:1] == ["select"]:
            paths = argv[1:] or changed()
            commands, missing = select(paths)
            for file in missing:
                print(f"FAIL owning tests: {file} has no owning tests in tools/test-owners.json", file=sys.stderr)
            for command in commands:
                print(command)
            return 1 if missing else 0
    except (OSError, ValueError, KeyError, json.JSONDecodeError) as error:
        print(f"FAIL owning tests: {error}", file=sys.stderr)
        return 1
    print("usage: python3 -m tools.owning_tests check | select [<path>...]", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

"""Refuse a push while an item of ``docs/checklist.md`` is in progress.

A push happens only when no checklist item is in progress (AGENTS.md). The items are read with
``active_items`` of ``tools/full_run.py``, the parser of the full suite guard.

    python3 -m tools.push_gate hook          the pre-push hook ``.githooks/pre-push``
    python3 -m tools.push_gate commit <rev>  the job ``push-gate`` of ``.github/workflows/push-gate.yml``
    python3 -m tools.push_gate hooks-check   ``make hooks``, ``tools/check.py`` and ``tools/full_run.py``

``hook`` reads the lines ``<local ref> <local sha> <remote ref> <remote sha>`` that Git writes to
the standard input of the pre-push hook. It reads the checklist of each pushed commit (a line with
a zero local sha deletes a remote ref and pushes no commit) and the checklist of the working tree,
and refuses the push with exit status 1, naming each item in progress with the ref, the commit, the
ID and the title. A pushed commit without the checklist, a Git error or a parser error refuses the
push with its cause.

``commit`` reads the checklist of one commit and also requires that the commit tracks the hook
with mode 100755. It prints each line of a refusal as a GitHub error annotation and appends the
refusal to the file that ``GITHUB_STEP_SUMMARY`` names, when that variable is set.

``hooks-check`` fails unless ``core.hooksPath`` is ``.githooks`` and the hook in the working tree
is executable. Every make invocation sets ``core.hooksPath`` (Makefile); ``make hooks`` sets and
checks it.
"""

import os
from pathlib import Path
import subprocess
import sys

from tools import full_run


CHECKLIST = "docs/checklist.md"
HOOK = ".githooks/pre-push"
HOOKS_PATH = ".githooks"
RULE = ("A push happens only when no checklist item is in progress (AGENTS.md); the full suite runs "
        "once, when every active item is complete.")
ACTION = ("Complete each item ([o] with its changelog entry, committed), or mark it [!] when it must "
          "be bypassed; then push again.")


class Refused(Exception):
    """A push is refused for a cause other than an item in progress."""


def git(root, *arguments):
    result = subprocess.run(["git", *arguments], cwd=root, capture_output=True, text=True, check=False)
    if result.returncode:
        raise Refused(f"git {' '.join(arguments)} exited with {result.returncode}: {result.stderr.strip()}")
    return result.stdout


def committed_items(root, name, sha):
    """The items in progress of the checklist of commit ``sha``, which ``name`` names."""
    listed = git(root, "ls-tree", "--name-only", sha, "--", CHECKLIST).strip()
    if listed != CHECKLIST:
        raise Refused(f"{name} {sha[:12]} has no {CHECKLIST}")
    return full_run.active_items(git(root, "show", f"{sha}:{CHECKLIST}"))


def working_items(root):
    path = Path(root) / CHECKLIST
    if not path.is_file():
        raise Refused(f"the working tree has no {CHECKLIST}")
    return full_run.active_items(path.read_text(encoding="utf-8"))


def refusal(entries):
    """The lines of a refusal for ``entries``, pairs of a place and the items in progress there."""
    lines = [f"push refused: checklist items are in progress ({CHECKLIST})"]
    for place, items in entries:
        lines += [f"  {place}: {item} {title}" for item, title in items]
    return lines + [RULE, ACTION]


def hook(root, lines):
    """The refusal lines for the pre-push input ``lines``; empty when the push may proceed."""
    entries = []
    for line in lines:
        if not line.strip():
            continue
        fields = line.split()
        if len(fields) != 4:
            raise Refused(f"the pre-push input line {line!r} does not have four fields")
        sha, remote_ref = fields[1], fields[2]
        if set(sha) == {"0"}:
            continue
        items = committed_items(root, remote_ref, sha)
        if items:
            entries.append((f"{remote_ref} {sha[:12]}", items))
    items = working_items(root)
    if items:
        entries.append(("working tree", items))
    return refusal(entries) if entries else []


def commit(root, rev):
    """The refusal lines for commit ``rev``; empty when it may be pushed."""
    sha = git(root, "rev-parse", "--verify", f"{rev}^{{commit}}").strip()
    tracked = git(root, "ls-tree", sha, "--", HOOK).strip()
    if not tracked:
        raise Refused(f"{rev} does not track {HOOK}")
    mode = tracked.split()[0]
    if mode != "100755":
        raise Refused(f"{rev} tracks {HOOK} with mode {mode}, not 100755")
    items = committed_items(root, rev, sha)
    return refusal([(f"{rev} {sha[:12]}", items)]) if items else []


def hooks_check(root):
    """The reasons why the pre-push hook of ``root`` does not run; empty when it runs."""
    errors = []
    result = subprocess.run(["git", "config", "core.hooksPath"], cwd=root, capture_output=True,
                            text=True, check=False)
    configured = result.stdout.strip()
    if configured != HOOKS_PATH:
        errors.append(f"core.hooksPath is not {HOOKS_PATH} (it is {configured or 'not set'}); run make hooks")
    path = Path(root) / HOOK
    if not path.is_file():
        errors.append(f"{HOOK} does not exist; restore it from Git and run make hooks")
    elif not os.access(path, os.X_OK):
        errors.append(f"{HOOK} is not executable; restore its mode 100755 from Git and run make hooks")
    return errors


def annotate(lines):
    """Print ``lines`` as GitHub error annotations and append them to the job summary."""
    for line in lines:
        text = line.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
        print(f"::error::{text}", flush=True)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as file:
            file.write("```\n" + "\n".join(lines) + "\n```\n")


def main(argv, root=None):
    root = Path(root or os.getcwd())
    try:
        if argv == ["hook"]:
            lines = hook(root, sys.stdin.read().splitlines())
            for line in lines:
                print(line, file=sys.stderr)
            return 1 if lines else 0
        if len(argv) == 2 and argv[0] == "commit":
            lines = commit(root, argv[1])
            if lines:
                annotate(lines)
                return 1
            print(f"push-gate: no checklist item is in progress in {argv[1]}, which tracks {HOOK} "
                  "with mode 100755")
            return 0
        if argv == ["hooks-check"]:
            errors = hooks_check(root)
            for error in errors:
                print(f"hooks-check: {error}", file=sys.stderr)
            if not errors:
                print(f"hooks-check: core.hooksPath is {HOOKS_PATH} and {HOOK} is executable")
            return 1 if errors else 0
    except Refused as refused:
        lines = [f"push refused: {refused}", RULE]
        if argv[:1] == ["commit"]:
            annotate(lines)
        else:
            for line in lines:
                print(line, file=sys.stderr)
        return 1
    print("usage: python3 -m tools.push_gate hook | commit <rev> | hooks-check", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

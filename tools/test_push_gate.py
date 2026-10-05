import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
from tempfile import TemporaryDirectory
from unittest import TestCase


ROOT = Path(__file__).resolve().parents[1]
CHECKLIST = """# Checklist

## Items

- [o] S-1 Finish the first item. Evidence: done.
- [~] S-2 Run the second item. Priority: next.
- [o] S-2-1 Close a part of the second item.
- [~] S-2-2 Keep the nested part active. Cause: open.
- [ ] S-3 Wait for the third item.
"""
DONE = CHECKLIST.replace("[~]", "[o]")
FILES = ("Makefile", ".githooks/pre-push", "tools/__init__.py", "tools/full_run.py", "tools/push_gate.py",
         "tools/tool_versions.py", "tools/tool-versions.json")


def run(root, *arguments, env=None, stdin=None):
    return subprocess.run(list(arguments), cwd=root, capture_output=True, text=True, check=False,
                          env=env, input=stdin)


def git(root, *arguments):
    result = run(root, "git", *arguments)
    if result.returncode:
        raise AssertionError(f"git {' '.join(arguments)}: {result.stderr}")
    return result.stdout.strip()


class Checkout:
    """A Git checkout in a temporary directory with the hook, the Makefile and the tools of this
    repository, a checklist and a bare repository as its remote `origin`."""

    def __init__(self, directory, checklist):
        directory = Path(directory).resolve()
        self.root = directory / "checkout"
        self.remote = directory / "remote.git"
        for name in FILES:
            (self.root / name).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / name, self.root / name)
        (self.root / "docs").mkdir()
        (self.root / "docs/checklist.md").write_text(checklist)
        git(self.root, "init", "-q", "-b", "main")
        git(self.root, "config", "user.name", "test")
        git(self.root, "config", "user.email", "test@example.com")
        self.commit("init")
        git(directory, "init", "-q", "--bare", str(self.remote))
        git(self.root, "remote", "add", "origin", str(self.remote))

    def commit(self, message):
        git(self.root, "add", "-A")
        git(self.root, "commit", "-q", "-m", message)
        return git(self.root, "rev-parse", "HEAD")

    def install(self):
        git(self.root, "config", "core.hooksPath", ".githooks")

    def push(self):
        return run(self.root, "git", "push", "origin", "HEAD:refs/heads/main")

    def remote_main(self):
        result = run(self.remote, "git", "rev-parse", "--verify", "-q", "refs/heads/main")
        return result.stdout.strip() or None

    def gate(self, *arguments, env=None):
        return run(self.root, sys.executable, "-m", "tools.push_gate", *arguments, env=env)


class HookTest(TestCase):
    def test_push_without_an_item_in_progress_updates_the_remote(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            checkout.install()
            result = checkout.push()
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(checkout.remote_main(), git(checkout.root, "rev-parse", "HEAD"))

    def test_push_of_a_commit_with_an_item_in_progress_is_refused(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            checkout.install()
            self.assertEqual(checkout.push().returncode, 0)
            clean = checkout.remote_main()
            (checkout.root / "docs/checklist.md").write_text(CHECKLIST)
            active = checkout.commit("start S-2")
            result = checkout.push()
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("push refused: checklist items are in progress (docs/checklist.md)", result.stderr)
            self.assertIn(f"refs/heads/main {active[:12]}: S-2 Run the second item.", result.stderr)
            self.assertIn(f"refs/heads/main {active[:12]}: S-2-2 Keep the nested part active.", result.stderr)
            self.assertIn("working tree: S-2 Run the second item.", result.stderr)
            self.assertIn("Complete each item", result.stderr)
            self.assertNotIn("no-verify", result.stderr)
            self.assertEqual(checkout.remote_main(), clean)

    def test_item_in_progress_in_the_working_tree_refuses_a_clean_commit(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            checkout.install()
            (checkout.root / "docs/checklist.md").write_text(CHECKLIST)
            result = checkout.push()
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("working tree: S-2 Run the second item.", result.stderr)
            self.assertNotIn("refs/heads/main", result.stderr)
            self.assertIsNone(checkout.remote_main())

    def test_commit_without_a_checklist_is_refused(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            checkout.install()
            (checkout.root / "docs/checklist.md").unlink()
            removed = checkout.commit("remove the checklist")
            (checkout.root / "docs/checklist.md").write_text(DONE)
            result = checkout.push()
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(f"push refused: refs/heads/main {removed[:12]} has no docs/checklist.md", result.stderr)
            self.assertIsNone(checkout.remote_main())

    def test_a_python_of_another_version_refuses_the_push(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            declaration = checkout.root / "tools/tool-versions.json"
            declaration.write_text(declaration.read_text().replace('"version": "Python ', '"version": "Python 0.'))
            checkout.commit("declare another Python")
            checkout.install()
            result = checkout.push()
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("push refused: the push check runs with another Python than tools/tool-versions.json "
                          "declares: python3: expected 'Python 0.", result.stderr)
            self.assertIsNone(checkout.remote_main())

    def test_deleting_a_remote_branch_reads_only_the_working_tree(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            checkout.install()
            self.assertEqual(run(checkout.root, "git", "push", "origin", "HEAD:refs/heads/topic").returncode, 0)
            (checkout.root / "docs/checklist.md").write_text(CHECKLIST)
            refused = run(checkout.root, "git", "push", "origin", ":refs/heads/topic")
            self.assertNotEqual(refused.returncode, 0)
            self.assertIn("working tree: S-2 Run the second item.", refused.stderr)
            (checkout.root / "docs/checklist.md").write_text(DONE)
            result = run(checkout.root, "git", "push", "origin", ":refs/heads/topic")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(run(checkout.remote, "git", "branch", "--list", "topic").stdout, "")


class HooksCheckTest(TestCase):
    def test_hooks_check_fails_until_make_hooks_sets_the_path(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            result = checkout.gate("hooks-check")
            self.assertEqual(result.returncode, 1)
            self.assertIn("core.hooksPath is not .githooks", result.stderr)
            self.assertIn("make hooks", result.stderr)
            installed = run(checkout.root, "make", "--no-print-directory", "hooks")
            self.assertEqual(installed.returncode, 0, installed.stdout + installed.stderr)
            self.assertEqual(git(checkout.root, "config", "core.hooksPath"), ".githooks")
            self.assertEqual(checkout.gate("hooks-check").returncode, 0)

    def test_every_make_invocation_sets_the_hooks_path(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            git(checkout.root, "config", "core.hooksPath", "elsewhere")
            result = run(checkout.root, "make", "-n", "--no-print-directory", "verify-archive")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(git(checkout.root, "config", "core.hooksPath"), ".githooks")

    def test_hook_that_is_not_executable_fails_the_check(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            checkout.install()
            hook = checkout.root / ".githooks/pre-push"
            hook.chmod(stat.S_IRUSR | stat.S_IWUSR)
            result = checkout.gate("hooks-check")
            self.assertEqual(result.returncode, 1)
            self.assertIn(".githooks/pre-push is not executable", result.stderr)


    def test_record_check_fails_while_the_hooks_path_is_not_set(self):
        from tools import check
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            self.assertEqual(check.check_hooks(checkout.root), [
                "hooks-check: core.hooksPath is not .githooks (it is not set); run make hooks"])
            checkout.install()
            self.assertEqual(check.check_hooks(checkout.root), [])


class CommitTest(TestCase):
    def test_commit_with_an_item_in_progress_fails_with_annotations(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, CHECKLIST)
            summary = Path(directory) / "summary.md"
            result = checkout.gate("commit", "HEAD", env={**os.environ, "GITHUB_STEP_SUMMARY": str(summary)})
            self.assertEqual(result.returncode, 1)
            self.assertIn("::error::push refused: checklist items are in progress (docs/checklist.md)", result.stdout)
            self.assertIn("::error::  HEAD ", result.stdout)
            self.assertIn(": S-2 Run the second item.", result.stdout)
            self.assertIn("S-2-2 Keep the nested part active.", summary.read_text())

    def test_clean_commit_passes(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            result = checkout.gate("commit", "HEAD")
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertNotIn("::error::", result.stdout)

    def test_commit_without_an_executable_hook_fails(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            git(checkout.root, "update-index", "--chmod=-x", ".githooks/pre-push")
            git(checkout.root, "commit", "-q", "-m", "hook mode 100644")
            result = checkout.gate("commit", "HEAD")
            self.assertEqual(result.returncode, 1)
            self.assertIn("::error::push refused: HEAD tracks .githooks/pre-push with mode 100644, not 100755",
                          result.stdout)
            git(checkout.root, "rm", "-q", "-f", ".githooks/pre-push")
            git(checkout.root, "commit", "-q", "-m", "no hook")
            result = checkout.gate("commit", "HEAD")
            self.assertEqual(result.returncode, 1)
            self.assertIn("::error::push refused: HEAD does not track .githooks/pre-push", result.stdout)

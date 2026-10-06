import json
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory
from unittest import TestCase

from tools import owning_tests


def commit_all(root):
    for arguments in (["init", "-q"], ["add", "-A"],
                      ["-c", "user.name=test", "-c", "user.email=test@example.com", "commit", "-q", "-m", "init"]):
        subprocess.run(["git", *arguments], cwd=root, check=True, capture_output=True, text=True)


class OwningTestsTest(TestCase):
    def checkout(self, directory, owners):
        root = Path(directory)
        (root / "tools").mkdir()
        (root / "crates/polyspec-ssr-core/tests").mkdir(parents=True)
        (root / "crates/polyspec-ssr-core/Cargo.toml").write_text("")
        (root / "crates/polyspec-ssr-core/tests/page.rs").write_text("")
        (root / "tools/run.py").write_text("")
        (root / "tools/test_run.py").write_text("")
        (root / "Makefile").write_text("check-deny:\n\tcargo deny check\n")
        declaration = root / "tools/test-owners.json"
        declaration.write_text(json.dumps({"owners": owners}))
        commit_all(root)
        return root, declaration

    def test_a_tracked_file_without_owning_tests_fails(self):
        owners = [{"paths": ["tools/*", "Makefile"], "tests": ["python3 -m unittest tools.test_run"]}]
        with TemporaryDirectory() as directory:
            root, declaration = self.checkout(directory, owners)
            self.assertEqual(owning_tests.check(root, declaration), [
                "crates/polyspec-ssr-core/Cargo.toml: no owning tests in tools/test-owners.json",
                "crates/polyspec-ssr-core/tests/page.rs: no owning tests in tools/test-owners.json"])

    def test_commands_must_name_existing_tests_and_patterns_tracked_files(self):
        owners = [{"paths": ["*", "absent/*"], "tests": [
            "python3 -m unittest tools.test_run", "python3 -m unittest tools.test_absent",
            "cargo nextest run -p polyspec-ssr-core --test page", "cargo nextest run -p polyspec-ssr-core --test absent",
            "cargo nextest run -p ssr-absent", "make check-deny", "make absent", "python3 tools/run.py",
            "sh -c true"]}]
        with TemporaryDirectory() as directory:
            root, declaration = self.checkout(directory, owners)
            self.assertEqual(owning_tests.check(root, declaration), [
                "tools/test-owners.json: absent/* names no tracked file",
                "tools/test-owners.json: python3 -m unittest tools.test_absent: no tracked test module tools.test_absent",
                "tools/test-owners.json: cargo nextest run -p polyspec-ssr-core --test absent: no test target absent in polyspec-ssr-core",
                "tools/test-owners.json: cargo nextest run -p ssr-absent: no workspace package ssr-absent",
                "tools/test-owners.json: make absent: no make target absent",
                "tools/test-owners.json: sh -c true: unsupported test command sh -c true"])

    def test_select_names_each_owning_command_once(self):
        with TemporaryDirectory() as directory:
            path = Path(directory) / "owners.json"
            path.write_text(json.dumps({"owners": [
                {"paths": ["tools/run.py"], "tests": ["a", "b"]},
                {"paths": ["tools/*"], "tests": ["b", "c"]}]}))
            self.assertEqual(owning_tests.select(["tools/run.py", "tools/other.py", "docs/x.md"], path),
                             (["a", "b", "c"], ["docs/x.md"]))

    def test_a_removed_file_is_not_a_changed_file(self):
        owners = [{"paths": ["*"], "tests": ["python3 -m unittest tools.test_run"]}]
        with TemporaryDirectory() as directory:
            root, _ = self.checkout(directory, owners)
            subprocess.run(["git", "rm", "-q", "tools/run.py"], cwd=root, check=True)
            (root / "crates/polyspec-ssr-core/tests/page.rs").unlink()
            (root / "tools/test_run.py").write_text("changed\n")
            self.assertEqual(owning_tests.changed(root), ["tools/test_run.py"])

    def test_every_tracked_file_of_the_repository_has_owning_tests(self):
        self.assertEqual(owning_tests.check(), [])

import json
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory
from unittest import TestCase
import os
import signal
import time


def alive(pid):
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    return True

from tools import full_run


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
# Each stub target appends its name to the log outside the checkout; `b` passes only while the
# untracked file `pass-b` exists, so a test changes its result without changing the tree.
MAKEFILE = """setup:
\techo setup >> {log}
broken:
\techo broken >> {log}; false
a:
\techo a >> {log}
b:
\techo b >> {log}; test -f pass-b
c:
\techo c >> {log}
"""


def git(root, *arguments):
    subprocess.run(["git", *arguments], cwd=root, check=True, capture_output=True, text=True)


class Checkout:
    """A Git checkout in a temporary directory with a checklist and a Makefile of stub targets."""

    def __init__(self, directory, checklist):
        self.root = Path(directory).resolve() / "checkout"
        self.log = Path(directory).resolve() / "ran.log"
        (self.root / "docs").mkdir(parents=True)
        (self.root / "docs/checklist.md").write_text(checklist)
        (self.root / "Makefile").write_text(MAKEFILE.format(log=self.log))
        (self.root / ".gitignore").write_text("/var/\npass-b\n")
        (self.root / ".githooks").mkdir()
        (self.root / ".githooks/pre-push").write_text((ROOT / ".githooks/pre-push").read_text())
        (self.root / ".githooks/pre-push").chmod(0o755)
        git(self.root, "init", "-q")
        git(self.root, "config", "core.hooksPath", ".githooks")
        git(self.root, "add", ".")
        git(self.root, "-c", "user.name=test", "-c", "user.email=test@example.com", "commit", "-q", "-m", "init")

    def run(self, mode):
        arguments = [sys.executable, "-m", "tools.full_run", mode, "--root", str(self.root), "--setup", "setup"]
        if mode == "check":
            arguments += ["--targets", "a", "b", "c"]
        return subprocess.run(arguments, cwd=ROOT, capture_output=True, text=True, check=False)

    def ran(self):
        names = self.log.read_text().split() if self.log.exists() else []
        self.log.unlink(missing_ok=True)
        return names

    def record(self):
        return json.loads((self.root / "var/full-run.json").read_text())


class DecisionTest(TestCase):
    def test_active_items_include_sub_items(self):
        self.assertEqual(full_run.active_items(CHECKLIST), [
            ("S-2", "Run the second item."), ("S-2-2", "Keep the nested part active.")])

    def test_check_with_an_active_item_is_refused(self):
        decision = full_run.decide("check", full_run.active_items(CHECKLIST), [], [], [], [], "t1", None, ["a"])
        self.assertFalse(decision.allowed)
        self.assertIn("S-2 Run the second item.", "\n".join(decision.reasons))
        self.assertIn("S-2-2 Keep the nested part active.", "\n".join(decision.reasons))

    def test_check_without_the_pre_push_hook_is_refused(self):
        hooks = ["core.hooksPath is not .githooks; run make hooks"]
        for mode in ("check", "rerun-failed"):
            decision = full_run.decide(mode, [], [], [], hooks, [], "t1", None, ["a"])
            self.assertFalse(decision.allowed)
            self.assertIn("the pre-push hook is not installed:", decision.reasons)
            self.assertIn("  core.hooksPath is not .githooks; run make hooks", decision.reasons)

    def test_check_of_a_committed_tree_without_a_record_is_allowed(self):
        decision = full_run.decide("check", [], [], [], [], [], "t1", None, ["a", "b"])
        self.assertTrue(decision.allowed)
        self.assertEqual(decision.targets, ["a", "b"])

    def test_a_tool_of_another_version_refuses_both_entries(self):
        tools = ["node: expected 'v26.8.1' from node --version, actual 'v26.9.0'"]
        for mode in ("check", "rerun-failed"):
            decision = full_run.decide(mode, [], [], [], [], tools, "t1", None, ["a"])
            self.assertFalse(decision.allowed)
            self.assertIn("tools differ from tools/tool-versions.json:", decision.reasons)
            self.assertIn(f"  {tools[0]}", decision.reasons)

    def test_rerun_of_another_tree_is_refused(self):
        record = {"tree": "t0", "targets": [{"name": "a", "status": "failed"}]}
        decision = full_run.decide("rerun-failed", [], [], [], [], [], "t1", record, None)
        self.assertFalse(decision.allowed)
        self.assertIn("tree t0", decision.reasons[0])


class EntryTest(TestCase):
    def test_make_check_starts_with_the_guard(self):
        lines = (ROOT / "Makefile").read_text().splitlines()
        self.assertEqual(lines[lines.index("check:") + 1].strip(),
                         "$(FULL_RUN) check --setup $(CHECK_SETUP) --targets $(CHECK_TARGETS) --needs $(CHECK_NEEDS)")
        self.assertEqual(lines[lines.index("rerun-failed:") + 1].strip(),
                         "$(FULL_RUN) rerun-failed --setup $(CHECK_SETUP) --needs $(CHECK_NEEDS)")
        self.assertIn("FULL_RUN = python3 -m tools.holder_lock run check -- python3 -m tools.full_run", lines)
        self.assertIn(".DEFAULT_GOAL := check", lines)


class GuardTest(TestCase):
    def test_active_item_refuses_before_any_step(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, CHECKLIST)
            result = checkout.run("check")
            self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
            self.assertIn("full-run: refused make check", result.stdout)
            self.assertIn("S-2 Run the second item.", result.stdout)
            self.assertIn("S-2-2 Keep the nested part active.", result.stdout)
            self.assertEqual(checkout.ran(), [])
            self.assertFalse((checkout.root / "var/full-run.json").exists())

    def test_hooks_path_that_is_not_set_refuses(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            git(checkout.root, "config", "--unset", "core.hooksPath")
            result = checkout.run("check")
            self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
            self.assertIn("the pre-push hook is not installed:", result.stdout)
            self.assertIn("make hooks", result.stdout)
            self.assertEqual(checkout.ran(), [])

    def test_uncommitted_tracked_change_refuses(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            (checkout.root / "Makefile").write_text((checkout.root / "Makefile").read_text() + "\n")
            result = checkout.run("check")
            self.assertEqual(result.returncode, 2)
            self.assertIn("uncommitted changes of tracked files", result.stdout)
            self.assertIn("Makefile", result.stdout)
            self.assertEqual(checkout.ran(), [])

    def test_untracked_file_refuses(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            (checkout.root / "docs/module.rs").write_text("pub fn added() {}\n")
            result = checkout.run("check")
            self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
            self.assertIn("full-run: refused make check", result.stdout)
            self.assertIn("untracked files, which the run would read but the tree does not hold:", result.stdout)
            self.assertIn("  docs/module.rs", result.stdout)
            self.assertEqual(checkout.ran(), [])

    def test_a_failed_setup_step_skips_only_the_targets_that_read_it(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            (checkout.root / "pass-b").write_text("")
            result = subprocess.run(
                [sys.executable, "-m", "tools.full_run", "check", "--root", str(checkout.root),
                 "--setup", "broken", "setup", "--targets", "a", "b", "c", "--needs", "a=broken", "c=setup"],
                cwd=ROOT, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
            self.assertEqual(checkout.ran(), ["broken", "setup", "b", "c"])
            self.assertIn("SKIP a: it reads the output of the failed setup step broken", result.stdout)
            record = checkout.record()
            self.assertEqual([(step["name"], step["status"]) for step in record["targets"]],
                             [("a", "skipped"), ("b", "passed"), ("c", "passed")])
            self.assertEqual(record["targets"][0]["reason"], "setup step broken failed")
            self.assertEqual(record["failed"], ["a"])

    def test_needs_name_known_setup_steps(self):
        for needs in (["a=other"], ["a="], ["=setup"], ["a=setup", "a=setup"]):
            self.assertIsNone(full_run.parse("check", ["--setup", "setup", "--targets", "a", "--needs", *needs]),
                              needs)
        self.assertEqual(full_run.parse("rerun-failed", ["--setup", "setup", "--needs", "a=setup"])[1:],
                         (["setup"], [], {"a": ["setup"]}))

    def test_second_full_run_of_the_same_tree_is_refused(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            first = checkout.run("check")
            self.assertEqual(first.returncode, 1, first.stdout + first.stderr)
            self.assertEqual(checkout.ran(), ["setup", "a", "b", "c"])
            record = checkout.record()
            self.assertEqual(record["result"], "failed")
            self.assertEqual(record["failed"], ["b"])
            self.assertEqual(record["tree"], subprocess.run(
                ["git", "rev-parse", "HEAD^{tree}"], cwd=checkout.root, capture_output=True,
                text=True, check=True).stdout.strip())
            second = checkout.run("check")
            self.assertEqual(second.returncode, 2)
            self.assertIn(f"the full run of tree {record['tree']} started {record['started']}", second.stdout)
            self.assertEqual(checkout.ran(), [])

    def test_rerun_without_a_record_is_refused(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            result = checkout.run("rerun-failed")
            self.assertEqual(result.returncode, 2)
            self.assertIn("no full run is recorded", result.stdout)
            self.assertEqual(checkout.ran(), [])

    def test_rerun_runs_only_the_recorded_failed_targets(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            checkout.run("check")
            checkout.ran()
            (checkout.root / "pass-b").write_text("")
            result = checkout.run("rerun-failed")
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual(checkout.ran(), ["setup", "b"])
            record = checkout.record()
            self.assertEqual(record["result"], "passed")
            self.assertEqual(record["failed"], [])
            self.assertEqual([rerun["targets"] for rerun in record["reruns"]], [["b"]])
            again = checkout.run("rerun-failed")
            self.assertEqual(again.returncode, 2)
            self.assertIn("nothing to rerun", again.stdout)

    def test_a_signal_reaches_every_process_of_the_step(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            started = Path(directory) / "started"
            makefile = checkout.root / "Makefile"
            # The step runs a tool that starts a command in a session of its own, as the ownership and
            # feature checks start cargo through tools/cargo_case.py.
            finished = Path(directory) / "finished"
            program = ("from tools.cargo_case import run; "
                       f"run(['sh', '-c', 'echo $$$$ > {started}; sleep 20; echo done > {finished}'], cwd='.')")
            makefile.write_text(makefile.read_text().replace(
                "\techo a >> {log}".format(log=checkout.log),
                f"\tPYTHONPATH={ROOT} {sys.executable} -c \"{program}\""))
            git(checkout.root, "-c", "user.name=test", "-c", "user.email=test@example.com",
                "commit", "-q", "-am", "hold")
            arguments = [sys.executable, "-m", "tools.full_run", "check", "--root", str(checkout.root),
                         "--setup", "setup", "--targets", "a", "b", "c"]
            run = subprocess.Popen(arguments, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            while not started.exists() or not started.read_text().strip():
                if run.poll() is not None:
                    self.fail(f"the run ended before its step started: {run.communicate()}")
                time.sleep(0.05)
            sleeper = int(started.read_text())
            run.send_signal(signal.SIGINT)
            stdout, stderr = run.communicate()
            self.assertEqual(run.returncode, 128 + signal.SIGINT, stdout + stderr)
            deadline = time.monotonic() + 5
            while alive(sleeper) and time.monotonic() < deadline:
                time.sleep(0.05)
            self.assertFalse(alive(sleeper), f"process {sleeper} of the interrupted step still runs")
            # The command was stopped by the signal; it did not run to its end.
            self.assertFalse(finished.exists(), "the command of the interrupted step ran to its end")
            self.assertEqual([(target["name"], target["status"]) for target in checkout.record()["targets"]],
                             [("a", "interrupted"), ("b", "not run"), ("c", "not run")])

    def test_killed_run_stays_incomplete(self):
        with TemporaryDirectory() as directory:
            checkout = Checkout(directory, DONE)
            makefile = checkout.root / "Makefile"
            makefile.write_text(makefile.read_text().replace("\techo a >> ", "\tkill -KILL $$(ps -o ppid= -p $$PPID); echo a >> "))
            git(checkout.root, "-c", "user.name=test", "-c", "user.email=test@example.com",
                "commit", "-q", "-am", "kill")
            result = checkout.run("check")
            self.assertEqual(result.returncode, -9)
            record = checkout.record()
            self.assertEqual(record["result"], "incomplete")
            self.assertEqual([(target["name"], target["status"]) for target in record["targets"]],
                             [("a", "running"), ("b", "not run"), ("c", "not run")])

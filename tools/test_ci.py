import hashlib
import os
from pathlib import Path
import re
import subprocess
import sys
from tempfile import TemporaryDirectory
from unittest import TestCase
from unittest.mock import patch

from tools import ci_run, fetch_v8, verify_archive


ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = sorted((ROOT / ".github/workflows").glob("*.yml"))


def makefile_targets():
    return set(re.findall(r"^([A-Za-z0-9_.-]+):", (ROOT / "Makefile").read_text(), re.MULTILINE))


class WorkflowRuleTest(TestCase):
    def test_every_step_runs_a_make_target(self):
        targets = makefile_targets()
        for workflow in WORKFLOWS:
            commands = re.findall(r"^\s+run: (.+)$", workflow.read_text(), re.MULTILINE)
            self.assertTrue(commands, workflow)
            for command in commands:
                found = re.fullmatch(r"make ([a-z0-9-]+)", command)
                self.assertIsNotNone(found, f"{workflow.name}: {command} is not a make target")
                self.assertIn(found.group(1), targets, f"{workflow.name}: {command}")

    def test_every_job_uploads_its_logs_and_summary_also_after_a_failure(self):
        for workflow in WORKFLOWS:
            text = workflow.read_text()
            jobs = re.split(r"^  ([a-z-]+):\n", text.split("\njobs:\n", 1)[1], flags=re.MULTILINE)[1:]
            for name, body in zip(jobs[::2], jobs[1::2]):
                self.assertRegex(body, r"if: \$\{\{ !cancelled\(\) \}\}\n\s+uses: actions/upload-artifact@",
                                 f"{workflow.name}: {name}")
                self.assertIn("path: var/ci/", body, f"{workflow.name}: {name}")
                if "matrix:" in body:
                    self.assertIn("fail-fast: false", body, f"{workflow.name}: {name}")

    def test_every_action_is_pinned_to_a_commit(self):
        for workflow in WORKFLOWS:
            for line in workflow.read_text().splitlines():
                if "uses: " in line:
                    self.assertRegex(line, r"uses: [a-z-]+/[a-z-]+@[0-9a-f]{40} # v\d+$", workflow.name)

    def test_a_new_push_cancels_the_previous_pull_request_run(self):
        text = (ROOT / ".github/workflows/ci.yml").read_text()
        self.assertIn("\nconcurrency:\n  group: ${{ github.workflow }}-${{ github.ref }}\n"
                      "  cancel-in-progress: ${{ github.event_name == 'pull_request' }}\n", text)
        self.assertNotIn("concurrency", (ROOT / ".github/workflows/push-gate.yml").read_text())

    def test_the_workflows_run_on_pull_requests_and_merge_groups(self):
        for name in ("ci.yml", "push-gate.yml"):
            trigger = re.search(r"(?ms)^on:\n(.*?)^\S", (ROOT / ".github/workflows" / name).read_text()).group(1)
            events = re.findall(r"(?m)^  ([\w-]+):", trigger)
            self.assertIn("pull_request", events, name)
            self.assertIn("merge_group", events, name)
            if name == "ci.yml":
                self.assertNotIn("push", events, "the merge group runs CI on the commit that main receives")
            else:
                self.assertIn("    branches-ignore: ['gh-readonly-queue/**']\n", trigger)

    def test_the_push_check_runs_the_record_checks_and_ci_runs_every_check(self):
        makefile = (ROOT / "Makefile").read_text()
        push = re.search(r"^CI_PUSH_TARGETS = (.*)$", makefile, re.MULTILINE).group(1).split()
        ci = re.search(r"^CI_TARGETS = (.*)$", makefile, re.MULTILINE).group(1).split()
        self.assertIn("push-records", push)
        self.assertIn("ci-records", ci)
        self.assertRegex(makefile, r"(?m)^push-records:\n\tpython3 tools/check.py records$")
        self.assertRegex(makefile, r"(?m)^ci-records:\n\tpython3 tools/check.py ci$")
        self.assertIn("run: make ci-push", (ROOT / ".github/workflows/push-gate.yml").read_text())

    def test_the_record_mode_fails_a_tree_whose_records_fail(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "docs").mkdir()
            marker = "[" + "~" + "]"
            for name in ("checklist.md", "checklist.ko.md"):
                (root / "docs" / name).write_text(f"- [o] S-1 Item, once marked `{marker}`.\n")
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            subprocess.run(["git", "add", "-A"], cwd=root, check=True)
            from tools import check
            errors = check.check_records(root)
        detail = "outside an item state; a checklist marker appears only as the state of an item"
        for name in ("checklist.md", "checklist.ko.md"):
            self.assertIn(f"docs/{name}:1:30: state marker {marker} {detail}", errors)
        result = subprocess.run([sys.executable, "tools/check.py", "records"], cwd=ROOT, capture_output=True, text=True)
        self.assertEqual((result.returncode, result.stdout), (0, "records: pass\n"), result.stderr)
        usage = subprocess.run([sys.executable, "tools/check.py", "other"], cwd=ROOT, capture_output=True, text=True)
        self.assertEqual(usage.returncode, 2)

    def test_the_linux_jobs_build_both_architectures(self):
        text = (ROOT / ".github/workflows/ci.yml").read_text()
        self.assertIn("runner: [ubuntu-24.04-arm, ubuntu-24.04]", text)
        self.assertIn("run: make ci-setup", text)
        self.assertIn("run: make ci", text)


STUB = """good:
\techo good output
bad:
\t@echo compiling; echo 'error[E0425]: cannot find value'; false
late:
\techo late output
"""


class CiRunTest(TestCase):
    def test_every_target_runs_and_each_failure_is_summarized(self):
        with TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Makefile").write_text(STUB)
            summary_file = root / "step-summary.md"
            with patch.dict(os.environ, {"GITHUB_STEP_SUMMARY": str(summary_file)}):
                status = ci_run.run(["good", "bad", "late"], root)
            self.assertEqual(status, 1)
            self.assertEqual(sorted(path.name for path in (root / "var/ci").iterdir()),
                             ["bad.log", "good.log", "late.log", "summary.md"])
            self.assertIn("late output", (root / "var/ci/late.log").read_text())
            summary = (root / "var/ci/summary.md").read_text()
            self.assertIn("| `good` | PASS | 0 |", summary)
            self.assertIn("| `bad` | FAIL | 2 |", summary)
            self.assertIn("| `late` | PASS | 0 |", summary)
            self.assertIn("## `bad`\n\n```\nerror[E0425]: cannot find value\n", summary)
            self.assertEqual(summary_file.read_text(), summary)

    def test_a_failure_without_an_error_line_shows_the_last_lines(self):
        log = "\n".join(f"line {number}" for number in range(40))
        self.assertEqual(ci_run.first_lines(log), [f"line {number}" for number in range(20, 40)])


class FetchV8Test(TestCase):
    def test_the_files_are_verified_and_published_and_another_digest_fails(self):
        with TemporaryDirectory() as directory:
            release = Path(directory) / "release"
            release.mkdir()
            target = "x86_64-unknown-linux-gnu"
            names = [name for name, _ in fetch_v8.files(target)]
            for name in names:
                (release / name).write_bytes(f"content of {name}".encode())
            digests = tuple(hashlib.sha256(f"content of {name}".encode()).hexdigest() for name in names)
            destination = Path(directory) / "v8"
            with patch.dict(verify_archive.ARCHIVES, {target: digests}):
                fetch_v8.fetch(target, destination, release.as_uri())
                self.assertEqual(sorted(path.name for path in destination.iterdir()), sorted(names))
                (release / names[0]).write_bytes(b"changed")
                (destination / names[0]).unlink()
                with self.assertRaisesRegex(ValueError, f"expected SHA-256 {digests[0]}, actual "):
                    fetch_v8.fetch(target, destination, release.as_uri())
            self.assertEqual(sorted(path.name for path in destination.iterdir()), [names[1]])

    def test_an_undeclared_target_fails(self):
        with self.assertRaisesRegex(ValueError, "unsupported V8 target: riscv64"):
            fetch_v8.files("riscv64")

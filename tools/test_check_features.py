import json
import re
import subprocess
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import TestCase

from tools import check_features


def manifest(features):
    return {"features": features}


def report(name, event="ok"):
    """A libtest-json report line of nextest for one test."""
    return json.dumps({"type": "test", "event": event, "name": name}, separators=(",", ":")) + "\n"


def feature(name, case):
    return {
        "name": name,
        "supported": True,
        "test": {"package": "ssr-adapter-react", "binary": "react", "case": case},
    }


class FeatureCheckTest(TestCase):
    def test_cargo_compilation_has_no_total_timeout(self):
        def runner(command, **kwargs):
            self.assertNotIn("timeout", kwargs)
            return subprocess.CompletedProcess(command, 0, report("ssr-adapter-react::react$case"), "")
        self.run_manifest(manifest([feature("react-ssr", "case")]), runner)

    def test_make_check_runs_feature_cases(self):
        makefile = (check_features.ROOT / "Makefile").read_text()
        targets = re.search(r"^CHECK_TARGETS = (.*)$", makefile, re.MULTILINE).group(1).split()
        self.assertIn("check-features", targets)
        self.assertIn("\ncheck-features:\n\tpython3 tools/check_features.py\n", makefile)

    def run_manifest(self, data, runner):
        with TemporaryDirectory() as temporary:
            path = Path(temporary) / "features.json"
            path.write_text(json.dumps(data))
            return check_features.check(path, runner)

    def test_runs_every_true_case_with_exact_matching_and_nextest_deadlines(self):
        commands = []

        def runner(command, *, cwd):
            commands.append((command, cwd))
            return subprocess.CompletedProcess(command, 0, report(f"ssr-adapter-react::react${command[-1]}"), "")

        self.run_manifest(manifest([feature("react-ssr", "first"), feature("react-csr", "second")]), runner)
        self.assertEqual(len(commands), 2)
        for (command, cwd), case in zip(commands, ("first", "second")):
            self.assertEqual(
                command,
                ["cargo", "nextest", "run", "--locked", "--no-tests", "fail", "--color", "never",
                 "--message-format", "libtest-json", "--message-format-version", "0.1", "-p",
                 "ssr-adapter-react", "--test", "react", "--", "--exact", case],
            )
            self.assertEqual(cwd, check_features.ROOT)
        config = (check_features.ROOT / ".config/nextest.toml").read_text()
        self.assertIn('slow-timeout = { period = "30s", terminate-after = 1 }', config)

    def test_rejects_missing_and_extra_evidence_before_execution(self):
        cases = [
            manifest([]),
            manifest([{"name": "react-ssr", "supported": True}]),
            manifest([{"name": "react-ssr", "supported": True,
                       "test": {"package": "ssr-adapter-react", "binary": "react", "case": ""}}]),
            manifest([feature("react-ssr", "same"), feature("react-ssr", "same")]),
            manifest([dict(feature("react-ssr", "case"), ignored="output")]),
        ]
        for data in cases:
            with self.subTest(data=data), self.assertRaises(ValueError):
                self.run_manifest(data, lambda *_args, **_kwargs: self.fail("runner called"))

    def test_rejects_duplicate_json_members(self):
        with TemporaryDirectory() as temporary:
            path = Path(temporary) / "features.json"
            path.write_text('{"features":[],"features":[]}')
            with self.assertRaises(ValueError):
                check_features.check(path, lambda *_args, **_kwargs: self.fail("runner called"))

    def test_command_failure_fails_the_check(self):
        def failed(command, *, cwd):
            return subprocess.CompletedProcess(command, 3, "", "failed")

        with self.assertRaises(RuntimeError):
            self.run_manifest(manifest([feature("react-ssr", "case")]), failed)

    def test_the_feature_check_handles_no_time_limit(self):
        self.assertNotIn("TimeoutExpired", Path(check_features.__file__).read_text())

    def test_failed_case_does_not_drop_the_following_case(self):
        executed = []

        def runner(command, *, cwd):
            executed.append(command[-1])
            return subprocess.CompletedProcess(command, 1 if len(executed) == 1 else 0, "", "")

        with self.assertRaises(RuntimeError):
            self.run_manifest(manifest([feature("react-ssr", "first"), feature("react-csr", "second")]), runner)
        self.assertEqual(executed, ["first", "second"])

    def test_zero_exit_without_the_declared_case_pass_is_rejected(self):
        for output in ("0 passed, 1 skipped", "PASS [ 0.001s] ssr-adapter-react::react case",
                       report("ssr-adapter-react::react$another"),
                       report("ssr-adapter-react::react$case", "ignored")):
            def runner(command, *, cwd):
                return subprocess.CompletedProcess(command, 0, output, "")
            with self.subTest(output=output), self.assertRaises(RuntimeError):
                self.run_manifest(manifest([feature("react-ssr", "case")]), runner)

import json
import subprocess
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import TestCase

from tools import check_features


def manifest(features):
    return {"features": features}


def feature(name, case):
    return {
        "name": name,
        "supported": True,
        "test": {"package": "ssr-adapter-react", "binary": "react", "case": case},
    }


class FeatureCheckTest(TestCase):
    def test_make_check_runs_feature_cases(self):
        makefile = (check_features.ROOT / "Makefile").read_text()
        recipe = makefile.split("\ncheck:\n", 1)[1].split("\nbench:\n", 1)[0]
        self.assertIn("\tpython3 tools/check_features.py\n", recipe)

    def run_manifest(self, data, runner):
        with TemporaryDirectory() as temporary:
            path = Path(temporary) / "features.json"
            path.write_text(json.dumps(data))
            return check_features.check(path, runner)

    def test_runs_every_true_case_with_exact_matching_and_timeouts(self):
        commands = []

        def runner(command, *, timeout):
            commands.append((command, timeout))
            return subprocess.CompletedProcess(command, 0, "PASS", "")

        self.run_manifest(manifest([feature("react-ssr", "first"), feature("react-csr", "second")]), runner)
        self.assertEqual(len(commands), 2)
        for (command, timeout), case in zip(commands, ("first", "second")):
            self.assertEqual(
                command,
                ["cargo", "nextest", "run", "--locked", "--no-tests", "fail", "-p",
                 "ssr-adapter-react", "--test", "react", "--", "--exact", case],
            )
            self.assertGreater(timeout, 0)

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

    def test_command_failure_and_timeout_fail_the_check(self):
        def failed(command, *, timeout):
            return subprocess.CompletedProcess(command, 3, "", "failed")

        def timed_out(command, *, timeout):
            raise subprocess.TimeoutExpired(command, timeout)

        for runner in (failed, timed_out):
            with self.subTest(runner=runner), self.assertRaises(RuntimeError):
                self.run_manifest(manifest([feature("react-ssr", "case")]), runner)

    def test_failed_case_does_not_drop_the_following_case(self):
        executed = []

        def runner(command, *, timeout):
            executed.append(command[-1])
            return subprocess.CompletedProcess(command, 1 if len(executed) == 1 else 0, "", "")

        with self.assertRaises(RuntimeError):
            self.run_manifest(manifest([feature("react-ssr", "first"), feature("react-csr", "second")]), runner)
        self.assertEqual(executed, ["first", "second"])

import json
import os
import subprocess
import sys
from unittest import TestCase

from tools import build_programs


def artifact(name, kind, executable):
    return json.dumps({"reason": "compiler-artifact", "target": {"name": name, "kind": kind},
                       "executable": executable})


class BuildProgramsTest(TestCase):
    def test_the_command_builds_both_examples_of_polyspec_ssr_server(self):
        self.assertEqual(build_programs.command(), [
            "cargo", "build", "--locked", "-p", "polyspec-ssr-server", "--message-format", "json-render-diagnostics",
            "--example", "development_process", "--example", "socket_process"])

    def test_executables_come_from_the_example_artifacts(self):
        messages = "\n".join([
            json.dumps({"reason": "build-finished", "success": True}),
            artifact("development_process", ["lib"], "/target/debug/libdevelopment_process.rlib"),
            artifact("development_process", ["example"], "/target/debug/examples/development_process"),
            artifact("socket_process", ["example"], "/target/debug/examples/socket_process"),
        ])
        self.assertEqual(build_programs.executables(messages), {
            "development_process": "/target/debug/examples/development_process",
            "socket_process": "/target/debug/examples/socket_process"})

    def test_a_missing_executable_is_an_error(self):
        messages = artifact("development_process", ["example"], "/target/debug/examples/development_process")
        with self.assertRaisesRegex(ValueError, "Cargo reported no executable for socket_process"):
            build_programs.executables(messages)

    def test_the_tool_runs_only_as_a_setup_script(self):
        environment = {key: value for key, value in os.environ.items() if key != "NEXTEST_ENV"}
        result = subprocess.run([sys.executable, "-m", "tools.build_programs"], cwd=build_programs.ROOT,
                                env=environment, capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 1)
        self.assertIn("NEXTEST_ENV is not set", result.stderr)

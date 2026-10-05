import json
import pathlib
import platform
import re
import socket
import tempfile
import unittest

import os

from tools import install_packages, run_tests


ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tools/build-probe/tests/fixtures"
BROWSER = {
    "Darwin": "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "Linux": "/usr/bin/google-chrome",
}[platform.system()]
# The browser starts this many seconds late, longer than the injected launch limit below.
DELAY = 3
# Each script runs with its arguments and fails at its first page step because nothing listens.
SCRIPTS = {
    "browser.mjs": [],
    "browser-react-stream.mjs": [],
    "browser-svelte.mjs": ["main"],
    "browser-vanilla.mjs": [],
    "browser-vue.mjs": ["main"],
}


def closed_base():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return f"http://127.0.0.1:{listener.getsockname()[1]}"


class BrowserLaunchTests(unittest.TestCase):
    """A browser launch has no time limit; the browser scripts proceed after a slow launch."""

    @classmethod
    def setUpClass(cls):
        cls.environment = {**os.environ, "SSR_PACKAGES": str(install_packages.install())}

    def run_script(self, name):
        self.assertTrue(pathlib.Path(BROWSER).is_file(), f"browser test environment missing: {BROWSER}")
        with tempfile.TemporaryDirectory() as directory:
            directory = pathlib.Path(directory)
            slow = directory / "slow-browser"
            slow.write_text(f'#!/bin/sh\nsleep {DELAY}\nexec {json.dumps(BROWSER)} "$@"\n', encoding="utf-8")
            slow.chmod(0o755)
            # LAUNCH_TIMEOUT is the launch limit the scripts had; a launch has no limit.
            steps = (FIXTURES / "browser-steps.mjs").as_uri()
            preload = directory / "launch-limit.mjs"
            preload.write_text(
                'import { mock } from "node:test";\n'
                f"const real = await import({json.dumps(steps + '?real')});\n"
                f"mock.module({json.dumps(steps)}, {{ exports: {{ ...real, LAUNCH_TIMEOUT: 1000 }} }});\n",
                encoding="utf-8",
            )
            # The script runs in its own process group, which is killed with the browser that it
            # started when the script ends or exceeds the limit.
            status, code, stdout, stderr = run_tests.run_case(
                ["node", "--experimental-test-module-mocks", "--no-warnings", "--import", str(preload),
                 str(FIXTURES / name), closed_base(), str(slow), *SCRIPTS[name]], 30, cwd=FIXTURES,
                env=self.environment)
        report = f"{status} exit {code}\nstdout:\n{stdout}\nstderr:\n{stderr}"
        self.assertEqual(status, "FAIL", report)
        launched = re.match(r"RUN launch\nDONE launch (\d+\.\d+)s\nRUN /", stdout)
        self.assertIsNotNone(launched, report)
        self.assertGreaterEqual(float(launched.group(1)), DELAY)
        self.assertRegex(stdout, r"\nRUN close\nDONE close \d+\.\d+s\n\Z", report)
        # The script fails at its first page step, before any page passes; the browser's error
        # text differs between browser versions, so the exit status and the step lines decide.
        self.assertNotIn("PASS ", stdout, report)

    def test_a_script_without_the_package_installation_names_the_variable(self):
        environment = {key: value for key, value in self.environment.items() if key != "SSR_PACKAGES"}
        status, code, stdout, stderr = run_tests.run_case(
            ["node", str(FIXTURES / "browser.mjs"), closed_base(), BROWSER], 30, cwd=FIXTURES, env=environment)
        self.assertEqual(status, "FAIL", stdout + stderr)
        self.assertIn("SSR_PACKAGES is not set; it names the package installation of tools/install_packages.py",
                      stderr)

    def test_react_script_proceeds_after_a_slow_launch(self):
        self.run_script("browser.mjs")

    def test_react_stream_script_proceeds_after_a_slow_launch(self):
        self.run_script("browser-react-stream.mjs")

    def test_svelte_script_proceeds_after_a_slow_launch(self):
        self.run_script("browser-svelte.mjs")

    def test_vanilla_script_proceeds_after_a_slow_launch(self):
        self.run_script("browser-vanilla.mjs")

    def test_vue_script_proceeds_after_a_slow_launch(self):
        self.run_script("browser-vue.mjs")


if __name__ == "__main__":
    unittest.main()

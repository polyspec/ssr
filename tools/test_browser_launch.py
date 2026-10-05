import json
import pathlib
import re
import socket
import tempfile
import unittest

import os

from tools import install_packages, run_tests


ROOT = pathlib.Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tools/build-probe/tests/fixtures"
# The fake browser launch takes this many seconds.
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
    """A script passes no limit to the browser launch and proceeds after a slow launch. The browser
    is a fake: a real browser's launch time grows with the page-ins of its executable under memory
    pressure, which the host does not bound; the adapter crates' browser cases run a real one."""

    @classmethod
    def setUpClass(cls):
        cls.environment = {**os.environ, "SSR_PACKAGES": str(install_packages.install())}

    def run_script(self, name, scripts=FIXTURES):
        """Run a browser script with a fake ``chromium`` whose launch takes DELAY seconds, records
        its options and refuses any launch limit; no browser runs, so the case does not depend on
        how fast the host pages a browser in. The page steps fail because nothing listens."""
        with tempfile.TemporaryDirectory() as directory:
            directory = pathlib.Path(directory)
            launches = directory / "launches.json"
            steps = (scripts / "browser-steps.mjs").as_uri()
            preload = directory / "fake-browser.mjs"
            preload.write_text(
                'import fs from "node:fs";\n'
                'import { mock } from "node:test";\n'
                f"const real = await import({json.dumps(steps + '?real')});\n"
                "const page = {\n"
                "  on() {},\n"
                "  async goto(url) { throw new Error(`page.goto: net::ERR_CONNECTION_REFUSED at ${url}`); },\n"
                "  async close() {},\n"
                "};\n"
                "const chromium = {\n"
                "  async launch(options) {\n"
                f"    fs.appendFileSync({json.dumps(str(launches))}, JSON.stringify(options) + '\\n');\n"
                "    if (options.timeout !== 0) throw new Error(`launch limit ${options.timeout}`);\n"
                f"    await new Promise((resolve) => setTimeout(resolve, {DELAY * 1000}));\n"
                "    return { async newPage() { return page; }, async close() {} };\n"
                "  },\n"
                "};\n"
                f"mock.module({json.dumps(steps)}, {{ exports: {{ ...real, chromium }} }});\n",
                encoding="utf-8",
            )
            status, code, stdout, stderr = run_tests.run_case(
                ["node", "--experimental-test-module-mocks", "--no-warnings", "--import", str(preload),
                 str(scripts / name), closed_base(), "/browser/of/the/case", *SCRIPTS[name]], 30, cwd=scripts,
                env=self.environment)
            recorded = [json.loads(line) for line in launches.read_text().splitlines()] if launches.exists() else []
        report = f"{status} exit {code}\nlaunches: {recorded}\nstdout:\n{stdout}\nstderr:\n{stderr}"
        return status, stdout, stderr, recorded, report

    def check_script(self, name):
        status, stdout, stderr, recorded, report = self.run_script(name)
        self.assertEqual(status, "FAIL", report)
        self.assertEqual([(launch["executablePath"], launch["timeout"]) for launch in recorded],
                         [("/browser/of/the/case", 0)], report)
        launched = re.match(r"RUN launch\nDONE launch (\d+\.\d+)s\nRUN /", stdout)
        self.assertIsNotNone(launched, report)
        self.assertGreaterEqual(float(launched.group(1)), DELAY)
        self.assertRegex(stdout, r"\nRUN close\nDONE close \d+\.\d+s\n\Z", report)
        # The script fails at its first page step, before any page passes.
        self.assertNotIn("PASS ", stdout, report)
        self.assertIn("net::ERR_CONNECTION_REFUSED", stderr, report)

    def test_a_script_that_passes_a_launch_limit_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            scripts = pathlib.Path(directory)
            for name in ("browser.mjs", "browser-steps.mjs"):
                (scripts / name).write_text((FIXTURES / name).read_text(encoding="utf-8"), encoding="utf-8")
            script = scripts / "browser.mjs"
            script.write_text(script.read_text(encoding="utf-8").replace("timeout: 0", "timeout: 300000"),
                              encoding="utf-8")
            status, stdout, stderr, recorded, report = self.run_script("browser.mjs", scripts)
        self.assertEqual(status, "FAIL", report)
        self.assertEqual([launch["timeout"] for launch in recorded], [300000], report)
        self.assertIn("launch limit 300000", stderr, report)

    def test_a_script_without_the_package_installation_names_the_variable(self):
        environment = {key: value for key, value in self.environment.items() if key != "SSR_PACKAGES"}
        status, code, stdout, stderr = run_tests.run_case(
            ["node", str(FIXTURES / "browser.mjs"), closed_base(), "/browser/of/the/case"], 30, cwd=FIXTURES, env=environment)
        self.assertEqual(status, "FAIL", stdout + stderr)
        self.assertIn("SSR_PACKAGES is not set; it names the package installation of tools/install_packages.py",
                      stderr)

    def test_react_script_proceeds_after_a_slow_launch(self):
        self.check_script("browser.mjs")

    def test_react_stream_script_proceeds_after_a_slow_launch(self):
        self.check_script("browser-react-stream.mjs")

    def test_svelte_script_proceeds_after_a_slow_launch(self):
        self.check_script("browser-svelte.mjs")

    def test_vanilla_script_proceeds_after_a_slow_launch(self):
        self.check_script("browser-vanilla.mjs")

    def test_vue_script_proceeds_after_a_slow_launch(self):
        self.check_script("browser-vue.mjs")


if __name__ == "__main__":
    unittest.main()

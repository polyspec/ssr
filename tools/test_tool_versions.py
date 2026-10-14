import hashlib
import json
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
from unittest import TestCase

from tools import tool_versions


ROOT = Path(__file__).resolve().parents[1]


def recipe(target):
    lines = (ROOT / "Makefile").read_text().splitlines()
    start = lines.index(f"{target}:") + 1
    commands = []
    for line in lines[start:]:
        if not line.startswith("\t"):
            break
        commands.append(line.strip())
    return commands


class ToolVersionsTest(TestCase):
    def tools(self, report):
        return {"tool": {"name": "tool", "command": [sys.executable, "-c", f"print({report!r})"],
                         "version": "tool 1.2.3"}}

    def test_the_declared_report_passes_and_another_fails_with_both(self):
        self.assertEqual(tool_versions.check(tools=self.tools("tool 1.2.3\nmore")), [])
        self.assertEqual(tool_versions.check(tools=self.tools("tool 1.2.4")), [
            f"tool: expected 'tool 1.2.3' from {sys.executable} -c print('tool 1.2.4'), actual 'tool 1.2.4'"])

    def test_a_minor_version_accepts_its_patch_releases_and_records_the_running_one(self):
        def tools(report):
            return {"python3": {"name": "python3", "command": [sys.executable, "-c", f"print({report!r})"],
                                "minor": "Python 3.9"}}
        evidence = []
        self.assertEqual(tool_versions.check(tools=tools("Python 3.9.25"), evidence=evidence), [])
        self.assertEqual(evidence, ["python3: Python 3.9.25 (declared minor version Python 3.9)"])
        for report in ("Python 3.10.1", "Python 3.9", "Python 3.91.0", "Python 3.9.6rc1"):
            [error] = tool_versions.check(tools=tools(report))
            self.assertIn(f"expected a patch release of 'Python 3.9'", error)
            self.assertIn(f"actual {report!r}", error)

    def test_python_is_pinned_to_its_minor_version_here_and_in_the_workflow(self):
        # The jobs lint and test of ci.yml run the tool version check of this module (make ci-lint, make ci-test).
        self.assertEqual(tool_versions.declared()["python3"]["minor"], "Python 3.9")
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        self.assertEqual(workflow.count('python-version: "3.9"'), 2)
        for target in ("ci-lint", "ci-test"):
            self.assertIn("python3 -m tools.tool_versions check python3 cargo-nextest cargo-deny", recipe(target))

    def test_a_missing_tool_fails_with_its_cause(self):
        tools = {"absent": {"name": "absent", "command": ["/nonexistent/tool", "--version"], "version": "1"}}
        [error] = tool_versions.check(tools=tools)
        self.assertRegex(error, r"^absent: expected '1' from /nonexistent/tool --version, but "
                                r"/nonexistent/tool --version cannot run: ")

    def test_a_program_without_a_version_is_named_by_its_digest(self):
        program = Path(sys.executable).resolve()
        digest = hashlib.sha256(program.read_bytes()).hexdigest()
        tools = {"interpreter": {"name": "interpreter", "program": str(program), "sha256": digest}}
        self.assertEqual(tool_versions.check(tools=tools), [])
        tools["interpreter"]["sha256"] = "0" * 64
        self.assertEqual(tool_versions.check(tools=tools), [
            f"interpreter: expected '{'0' * 64}' from SHA-256 of {program}, actual '{digest}'"])

    def test_an_undeclared_tool_is_an_error(self):
        self.assertEqual(tool_versions.check(["absent"], tools=self.tools("tool 1.2.3")),
                         ["undeclared tool: absent"])

    def test_the_declaration_names_each_tool_once(self):
        with TemporaryDirectory() as directory:
            path = Path(directory) / "tool-versions.json"
            tool = {"name": "node", "command": ["node", "--version"], "version": "v1"}
            path.write_text(json.dumps({"tools": [tool, tool]}))
            with self.assertRaisesRegex(ValueError, "node is declared twice"):
                tool_versions.declared(path)
            path.write_text(json.dumps({"tools": [{"name": "node", "version": "v1"}]}))
            with self.assertRaisesRegex(ValueError, "each tool names name, command and version"):
                tool_versions.declared(path)

    def test_the_repository_declares_every_tool_it_runs(self):
        # Node.js, npm and Rust are declared in .node-version, packageManager of package.json and rust-toolchain.toml,
        # which make toolchain-check of kit checks.
        self.assertEqual(set(tool_versions.declared()), {"python3", "make", "cargo-nextest", "cargo-deny"})

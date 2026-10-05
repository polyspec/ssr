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
        self.assertEqual(set(tool_versions.declared()), {
            "python3", "make", "rustc", "cargo-nextest", "cargo-deny", "node", "npm", "zig", "chrome",
            "container", "containerctl"})


class EntryTest(TestCase):
    def test_every_make_entry_outside_the_guard_checks_the_tools_first(self):
        for target in ("bench", "verify-build", "verify-engine-linux-arm64", "review-advisories"):
            self.assertEqual(recipe(target)[0], "python3 -m tools.tool_versions check", target)

    def test_rustup_never_installs_a_toolchain(self):
        makefile = (ROOT / "Makefile").read_text()
        self.assertIn("\nexport RUSTUP_AUTO_INSTALL := 0\n", makefile)
        self.assertIn("V8_TARGET := $(shell RUSTUP_AUTO_INSTALL=0 rustc -vV", makefile)

    def test_the_full_suite_checks_no_advisory_and_the_review_does(self):
        self.assertEqual(recipe("check-deny"), ["cargo deny check $(DENY_CHECKS)"])
        self.assertIn("DENY_CHECKS = bans licenses sources", (ROOT / "Makefile").read_text().splitlines())
        for command in recipe("verify-engine-deps") + recipe("verify-build")[-1:]:
            self.assertTrue(command.endswith("$(DENY_CHECKS)"), command)
        self.assertEqual([command for command in recipe("review-advisories") if "cargo deny" in command], [
            "cargo deny check advisories",
            "cargo deny --manifest-path verification/engine/Cargo.toml --config deny.toml check "
            "--hide-inclusion-graph advisories",
            "cargo deny --manifest-path tools/build-probe/Cargo.toml --config deny.toml --locked check "
            "--hide-inclusion-graph advisories"])
        self.assertNotIn("review-advisories", next(line for line in (ROOT / "Makefile").read_text().splitlines()
                                                   if line.startswith("CHECK_TARGETS = ")))

import hashlib
import json
from pathlib import Path
import re
import tarfile
from tempfile import TemporaryDirectory
from unittest import TestCase

from tools import fetch_tools, tool_versions


ROOT = Path(__file__).resolve().parents[1]
# The make target of each check job of ci.yml.
CI_JOBS = ("ci-lint", "ci-test")


def recipe(target):
    """The commands of a target of the Makefile, as written."""
    lines = (ROOT / "Makefile").read_text().splitlines()
    start = lines.index(f"{target}:") + 1
    commands = []
    for line in lines[start:]:
        if not line.startswith("\t"):
            break
        commands.append(line.strip())
    return commands


class FetchToolsTest(TestCase):
    """make ci-setup installs the release binaries of the test tools, each archive verified by its SHA-256."""

    def declaration(self, directory, content, member="tool-1.0/tool"):
        archive = Path(directory) / "tool.tar.gz"
        source = Path(directory) / "tool"
        source.write_bytes(content)
        with tarfile.open(archive, "w:gz") as bundle:
            bundle.add(source, arcname=member)
        path = Path(directory) / "tool-versions.json"
        path.write_text(json.dumps({
            "tools": [{"name": "tool", "command": ["tool", "--version"], "version": "tool 1.0"}],
            "releases": [{"name": "tool", "target": "x86_64-unknown-linux-gnu", "url": archive.as_uri(),
                          "sha256": hashlib.sha256(archive.read_bytes()).hexdigest(), "executable": "tool-1.0/tool"}]}))
        return path

    def test_the_executable_is_verified_and_installed_and_another_digest_installs_nothing(self):
        with TemporaryDirectory() as directory:
            declaration = self.declaration(directory, b"#!/bin/sh\necho tool 1.0\n")
            destination = Path(directory) / "bin"
            fetch_tools.fetch("x86_64-unknown-linux-gnu", destination, declaration)
            installed = destination / "tool"
            self.assertEqual(installed.read_bytes(), b"#!/bin/sh\necho tool 1.0\n")
            self.assertEqual(installed.stat().st_mode & 0o777, 0o755)
            self.assertEqual([path.name for path in destination.iterdir()], ["tool"])
            installed.unlink()
            expected = json.loads(declaration.read_text())["releases"][0]["sha256"]
            self.declaration(directory, b"changed")
            document = json.loads(declaration.read_text())
            document["releases"][0]["sha256"] = expected
            declaration.write_text(json.dumps(document))
            with self.assertRaisesRegex(ValueError, f"expected SHA-256 {expected}, actual "):
                fetch_tools.fetch("x86_64-unknown-linux-gnu", destination, declaration)
            self.assertEqual(list(destination.iterdir()), [])

    def test_a_missing_executable_an_undeclared_target_or_tool_and_a_missing_key_fail(self):
        with TemporaryDirectory() as directory:
            declaration = self.declaration(directory, b"tool", member="other/tool")
            destination = Path(directory) / "bin"
            with self.assertRaisesRegex(ValueError, "the archive has no tool-1.0/tool"):
                fetch_tools.fetch("x86_64-unknown-linux-gnu", destination, declaration)
            self.assertEqual(list(destination.iterdir()), [])
            with self.assertRaisesRegex(ValueError, r"no tool release for the target riscv64; declared targets: "
                                                    r"\['x86_64-unknown-linux-gnu'\]"):
                fetch_tools.releases("riscv64", declaration)
            document = json.loads(declaration.read_text())
            document["releases"][0]["name"] = "other"
            declaration.write_text(json.dumps(document))
            with self.assertRaisesRegex(ValueError, "the release of other names an undeclared tool"):
                fetch_tools.releases("x86_64-unknown-linux-gnu", declaration)
            del document["releases"][0]["sha256"]
            declaration.write_text(json.dumps(document))
            with self.assertRaisesRegex(ValueError, "each release names executable, name, sha256, target, url"):
                fetch_tools.releases("x86_64-unknown-linux-gnu", declaration)

    def test_each_runner_installs_the_declared_versions_of_the_test_tools_from_their_releases(self):
        tools = tool_versions.declared()
        for target in ("aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"):
            releases = {release["name"]: release for release in fetch_tools.releases(target)}
            self.assertEqual(sorted(releases), ["cargo-deny", "cargo-nextest"], target)
            for name, release in releases.items():
                version = tools[name]["version"].split()[1]
                self.assertRegex(release["url"], rf"^https://github\.com/[\w-]+/[\w-]+/releases/download/\S*{re.escape(version)}"
                                                 rf"/{name}-{re.escape(version)}-{target.split('-')[0]}-\S+\.tar\.gz$")
                self.assertRegex(release["sha256"], r"^[0-9a-f]{64}$")
        setup = recipe("ci-setup")
        self.assertFalse([command for command in setup if "cargo install" in command], setup)
        self.assertIn("python3 -m tools.fetch_tools \"$$(RUSTUP_AUTO_INSTALL=0 rustc -vV | sed -n 's/^host: //p')\"", setup)
        for target in CI_JOBS:
            self.assertEqual(recipe(target)[:2], ["$(MAKE) --no-print-directory toolchain-check",
                                                  "python3 -m tools.tool_versions check python3 cargo-nextest cargo-deny"])

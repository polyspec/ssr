"""Check the tools of this repository against their tracked versions.

``tools/tool-versions.json`` names every tool that a check, a build or a test runs, the command
that reports its version and the exact first line of that report; a tool without a version
command is named by the SHA-256 of its executable. Python is pinned to a minor version
(``minor``): the tools use only its standard library, whose behavior is stable within a minor
version, and one patch release cannot be installed identically on macOS and on the CI runners;
the check prints the running patch release as evidence and fails for another minor version. Every entry checks the declaration before any
step: ``make check`` and ``make rerun-failed`` through the guard of ``tools/full_run.py``,
``tools/check.py`` before a commit, the other make targets as their first command, and the push
check for the tools it runs. A missing tool or another version fails with the expected and the
actual report, so the same tree runs with the same tools on every machine.

    python3 -m tools.tool_versions check [<tool>...]
    python3 -m tools.tool_versions report
"""

import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
DECLARATION = ROOT / "tools/tool-versions.json"


def declared(path=DECLARATION):
    """The declared tools by name; each has ``command`` and ``version`` or ``program`` and
    ``sha256``."""
    document = json.loads(path.read_text())
    tools = {}
    for tool in document["tools"]:
        if set(tool) not in ({"name", "command", "version"}, {"name", "command", "minor"},
                             {"name", "program", "sha256"}):
            raise ValueError(f"{path}: each tool names name, command and version, name, command and minor, "
                             f"or name, program and sha256: {tool}")
        if tool["name"] in tools:
            raise ValueError(f"{path}: {tool['name']} is declared twice")
        tools[tool["name"]] = tool
    return tools


def actual(tool):
    """The report of the installed tool: the first line of its version command, or the SHA-256
    of its executable; a missing tool or a failed command is an error with its cause."""
    if "program" in tool:
        program = shutil.which(tool["program"])
        if program is None:
            raise OSError(f"{tool['program']} is not on PATH")
        return hashlib.sha256(Path(program).read_bytes()).hexdigest()
    try:
        result = subprocess.run(tool["command"], cwd=ROOT, capture_output=True, text=True, check=False,
                                env={**os.environ, "RUSTUP_AUTO_INSTALL": "0"})
    except OSError as error:
        raise OSError(f"{' '.join(tool['command'])} cannot run: {error}") from error
    if result.returncode:
        raise OSError(f"{' '.join(tool['command'])} exited with {result.returncode}: "
                      f"{(result.stdout + result.stderr).strip()}")
    return (result.stdout or result.stderr).splitlines()[0].strip()


def matches(tool, found):
    """Whether the report ``found`` is the declared one; a ``minor`` declaration accepts any patch
    release of that minor version."""
    if "minor" in tool:
        return re.fullmatch(re.escape(tool["minor"]) + r"\.\d+", found) is not None
    return found == tool.get("version", tool.get("sha256"))


def check(names=None, tools=None, evidence=None):
    """The errors of the selected declared tools; every tool when ``names`` is None. The report of
    each tool pinned to a minor version is appended to ``evidence``."""
    tools = declared() if tools is None else tools
    unknown = sorted(set(names or []) - set(tools))
    if unknown:
        return [f"undeclared tool: {name}" for name in unknown]
    errors = []
    for name in (names or tools):
        tool = tools[name]
        expected = tool.get("version", tool.get("minor", tool.get("sha256")))
        source = " ".join(tool["command"]) if "command" in tool else f"SHA-256 of {tool['program']}"
        try:
            found = actual(tool)
        except OSError as error:
            errors.append(f"{name}: expected {expected!r} from {source}, but {error}")
            continue
        if not matches(tool, found):
            kind = "a patch release of " if "minor" in tool else ""
            errors.append(f"{name}: expected {kind}{expected!r} from {source}, actual {found!r}")
        elif "minor" in tool and evidence is not None:
            evidence.append(f"{name}: {found} (declared minor version {expected})")
    return errors


def main(argv):
    if argv[:1] == ["check"]:
        try:
            evidence = []
            errors = check(argv[1:] or None, evidence=evidence)
        except (OSError, ValueError, KeyError, json.JSONDecodeError) as error:
            errors = [f"{DECLARATION}: {error}"]
        for error in errors:
            print(f"FAIL tool version: {error}", file=sys.stderr)
        for line in evidence:
            print(f"PASS tool version: {line}", flush=True)
        if not errors:
            print(f"PASS tool versions: {', '.join(argv[1:]) or 'every declared tool'}", flush=True)
        return 1 if errors else 0
    if argv == ["report"]:
        for name, tool in declared().items():
            try:
                print(f"{name}: {actual(tool)}")
            except OSError as error:
                print(f"{name}: {error}")
        return 0
    print("usage: python3 -m tools.tool_versions check [<tool>...] | report", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

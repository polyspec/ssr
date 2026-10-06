"""Build the programs that the ``polyspec-ssr-server`` tests run and name them to the tests.

The development and socket tests of ``polyspec-ssr-server`` run the ``development_process`` and
``socket_process`` examples as child processes. Cargo builds examples only when no target is
selected, so a run of one test target would run a program built from older sources. nextest runs
this tool as the setup script ``build-programs`` (``.config/nextest.toml``) before any test of
``polyspec-ssr-server``: it builds both examples with Cargo, which rebuilds them when their sources or
dependencies changed, and writes ``SSR_DEVELOPMENT_PROCESS`` and ``SSR_SOCKET_PROCESS``, the
executables that Cargo reports, to the file that ``NEXTEST_ENV`` names. The tests read the
programs only from these variables. The build is a long operation: it has no time limit, and its
progress and errors reach standard error as they arrive.

    python3 -m tools.build_programs
"""

import json
import os
from pathlib import Path
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[1]
PROGRAMS = {"development_process": "SSR_DEVELOPMENT_PROCESS", "socket_process": "SSR_SOCKET_PROCESS"}


def command():
    arguments = ["cargo", "build", "--locked", "-p", "polyspec-ssr-server",
                 "--message-format", "json-render-diagnostics"]
    for name in PROGRAMS:
        arguments += ["--example", name]
    return arguments


def executables(messages):
    """The executable of each program from Cargo's JSON messages; a missing one is an error."""
    found = {}
    for line in messages.splitlines():
        message = json.loads(line)
        target = message.get("target", {})
        if (message.get("reason") == "compiler-artifact" and target.get("name") in PROGRAMS
                and target.get("kind") == ["example"]):
            found[target["name"]] = message["executable"]
    missing = sorted(set(PROGRAMS) - set(found))
    if missing:
        raise ValueError(f"Cargo reported no executable for {', '.join(missing)}")
    return found


def main():
    destination = os.environ.get("NEXTEST_ENV")
    if not destination:
        print("FAIL build programs: NEXTEST_ENV is not set; nextest runs this tool as the setup "
              "script build-programs", file=sys.stderr)
        return 1
    arguments = command()
    print(f"RUN {' '.join(arguments)}", file=sys.stderr, flush=True)
    started = time.monotonic()
    result = subprocess.run(arguments, cwd=ROOT, stdout=subprocess.PIPE, text=True, check=False)
    elapsed = time.monotonic() - started
    if result.returncode:
        print(f"FAIL build programs exit {result.returncode} {elapsed:.1f}s", file=sys.stderr)
        return 1
    try:
        found = executables(result.stdout)
    except (ValueError, KeyError, json.JSONDecodeError) as error:
        print(f"FAIL build programs {elapsed:.1f}s: {error}", file=sys.stderr)
        return 1
    with open(destination, "a", encoding="utf-8") as environment:
        for name, variable in PROGRAMS.items():
            environment.write(f"{variable}={found[name]}\n")
    for name, variable in PROGRAMS.items():
        print(f"PASS {variable}={found[name]}", file=sys.stderr)
    print(f"PASS build programs {elapsed:.1f}s", file=sys.stderr, flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())

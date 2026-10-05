"""Forward Cargo progress while nextest enforces individual test deadlines.

A run reads test results from the machine-readable report of nextest, ``--message-format
libtest-json`` at the fixed structure ``--message-format-version 0.1`` of the pinned nextest
0.9.146, never from its human output, whose text differs between versions.
"""

import json
import os
import signal
import subprocess

# The report options and the variable that enables this experimental format of nextest.
REPORT = ["--message-format", "libtest-json", "--message-format-version", "0.1"]
ENVIRONMENT = {"NEXTEST_EXPERIMENTAL_LIBTEST_JSON": "1"}


def results(output):
    """The last event of each test in the report: ``ok``, ``failed``, ``ignored`` or ``timeout``.
    A test is named ``<package>::<binary>$<test>``; lines that are not report events are skipped."""
    events = {}
    for line in output.splitlines():
        if not line.startswith('{"type":'):
            continue
        event = json.loads(line)
        if event.get("type") == "test" and event.get("event") != "started":
            events[event["name"]] = event["event"]
    return events


def run(command, *, cwd):
    print(f"COMMAND {' '.join(command)}", flush=True)
    process = subprocess.Popen(
        command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        text=True, start_new_session=True, env={**os.environ, **ENVIRONMENT},
    )
    lines = []
    try:
        for line in process.stdout:
            print(line, end="", flush=True)
            lines.append(line)
        code = process.wait()
    except BaseException:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            # The process group has already exited; wait still reaps the child.
            pass
        process.wait()
        raise
    finally:
        process.stdout.close()
    return subprocess.CompletedProcess(command, code, "".join(lines), "")

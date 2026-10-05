"""Run every tool test case in its own process with its own time limit.

Each case runs in a new process group. A case that exceeds its limit is reported with the output
it wrote, and its whole process group is killed and waited for, so no process that the case
started keeps running.
"""

import importlib
import os
import pathlib
import pkgutil
import signal
import subprocess
import sys
import time
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))


def test_ids():
    suite = unittest.TestSuite()
    for module in pkgutil.iter_modules([str(pathlib.Path(__file__).parent)]):
        if module.name.startswith("test_"):
            suite.addTests(unittest.defaultTestLoader.loadTestsFromModule(importlib.import_module(f"tools.{module.name}")))

    def walk(tests):
        for test in tests:
            if isinstance(test, unittest.TestSuite):
                yield from walk(test)
            else:
                yield test.id()

    return list(walk(suite))


def run_case(command, timeout, cwd=None, env=None):
    """Run one command in its own process group; return ``PASS``, ``FAIL`` or ``TIMEOUT``, its
    exit status, its standard output and its standard error. The group is killed when the
    command ends or exceeds ``timeout`` seconds."""
    process = subprocess.Popen(command, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               text=True, start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        stdout, stderr = process.communicate()
        return "TIMEOUT", process.returncode, stdout, stderr
    try:
        # Processes that the command left running are stopped with it. A group whose members have
        # all ended is gone (ProcessLookupError) or, on macOS, holds only exited members (EPERM).
        os.killpg(process.pid, signal.SIGKILL)
    except (ProcessLookupError, PermissionError):
        pass
    if process.returncode or "skipped=" in stderr:
        return "FAIL", process.returncode, stdout, stderr
    return "PASS", process.returncode, stdout, stderr


def main():
    ids = test_ids()
    if not ids:
        print("FAIL no tool tests")
        return 1
    failures = 0
    for test_id in ids:
        print(f"RUN {test_id}", flush=True)
        start = time.monotonic()
        status, _, stdout, stderr = run_case([sys.executable, "-m", "unittest", test_id], 30)
        elapsed = time.monotonic() - start
        if status == "TIMEOUT":
            print(f"TIMEOUT {test_id} after the limit of 30 s, {elapsed:.3f}s; its output:", flush=True)
        else:
            print(f"{status} {test_id} {elapsed:.3f}s", flush=True)
        if status != "PASS":
            print(stdout + stderr, file=sys.stderr, flush=True)
            failures += 1
    print(f"SUMMARY {len(ids) - failures} passed, {failures} failed")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())

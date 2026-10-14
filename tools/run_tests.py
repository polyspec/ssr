"""Run the tool test cases, each in its own process with its own time limit.

    python3 tools/run_tests.py [tools/test_<name>.py...]

Without arguments every test module of ``tools`` runs; with arguments only the cases of the named
test files, which make owner-check passes for the changed test files (config/owner-checks.json).

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


def test_ids(files=None):
    """The ids of the cases of the test files ``files``, paths ``tools/test_<name>.py``, or of every
    test module of ``tools`` when ``files`` is None; a name that is no tool test file is an error."""
    directory = pathlib.Path(__file__).parent
    modules = sorted(module.name for module in pkgutil.iter_modules([str(directory)]) if module.name.startswith("test_"))
    if files is not None:
        selected = []
        for name in files:
            path = pathlib.PurePosixPath(name)
            if path.parent != pathlib.PurePosixPath("tools") or path.suffix != ".py" or path.stem not in modules:
                raise ValueError(f"{name}: not a tool test file; name a file tools/test_<name>.py")
            selected.append(path.stem)
        modules = selected
    suite = unittest.TestSuite()
    for module in modules:
        suite.addTests(unittest.defaultTestLoader.loadTestsFromModule(importlib.import_module(f"tools.{module}")))

    def walk(tests):
        for test in tests:
            if isinstance(test, unittest.TestSuite):
                yield from walk(test)
            else:
                yield test.id()

    return list(walk(suite))


def run_case(command, timeout, cwd=None, env=None):
    """Run one command in its own process group; return ``PASS``, ``FAIL`` or ``HUNG``, its
    exit status, its standard output and its standard error. The group is killed when the
    command ends or runs past ``timeout`` seconds; the limit only ends a hung case, far above the
    time a case takes, so a case past it is reported as hung."""
    process = subprocess.Popen(command, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               text=True, start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        stdout, stderr = process.communicate()
        return "HUNG", process.returncode, stdout, stderr
    try:
        # Processes that the command left running are stopped with it. A group whose members have
        # all ended is gone (ProcessLookupError) or, on macOS, holds only exited members (EPERM).
        os.killpg(process.pid, signal.SIGKILL)
    except (ProcessLookupError, PermissionError):
        pass
    if process.returncode or "skipped=" in stderr:
        return "FAIL", process.returncode, stdout, stderr
    return "PASS", process.returncode, stdout, stderr


def main(argv=()):
    try:
        ids = test_ids(list(argv) if argv else None)
    except ValueError as error:
        print(f"FAIL {error}")
        return 2
    if not ids:
        print("FAIL no tool tests")
        return 1
    failures = 0
    for test_id in ids:
        print(f"RUN {test_id}", flush=True)
        start = time.monotonic()
        status, _, stdout, stderr = run_case([sys.executable, "-m", "unittest", test_id], 30)
        elapsed = time.monotonic() - start
        if status == "HUNG":
            print(f"HUNG {test_id}: ended by its case limit of 30 s after {elapsed:.3f}s; its output:", flush=True)
        else:
            print(f"{status} {test_id} {elapsed:.3f}s", flush=True)
        if status != "PASS":
            print(stdout + stderr, file=sys.stderr, flush=True)
            failures += 1
    print(f"SUMMARY {len(ids) - failures} passed, {failures} failed")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

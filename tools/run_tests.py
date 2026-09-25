import importlib
import pathlib
import pkgutil
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


def main():
    ids = test_ids()
    if not ids:
        print("FAIL no tool tests")
        return 1
    failures = 0
    for test_id in ids:
        print(f"RUN {test_id}", flush=True)
        start = time.monotonic()
        try:
            result = subprocess.run(
                [sys.executable, "-m", "unittest", test_id],
                capture_output=True, text=True, timeout=30, check=False,
            )
        except subprocess.TimeoutExpired:
            print(f"TIMEOUT {test_id} {time.monotonic() - start:.3f}s")
            failures += 1
            continue
        elapsed = time.monotonic() - start
        if result.returncode or "skipped=" in result.stderr:
            print(f"FAIL {test_id} {elapsed:.3f}s")
            print(result.stdout + result.stderr, file=sys.stderr)
            failures += 1
        else:
            print(f"PASS {test_id} {elapsed:.3f}s")
    print(f"SUMMARY {len(ids) - failures} passed, {failures} failed")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())

import os
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
import time
from unittest import TestCase

from tools import run_tests


CHILD = """
import subprocess, sys, time
grandchild = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"])
open(sys.argv[1], "w").write(str(grandchild.pid))
print("started the grandchild", flush=True)
print("the case waits", file=sys.stderr, flush=True)
time.sleep(60)
"""


def running(pid):
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    return True


class RunCaseTest(TestCase):
    def test_a_hung_case_is_killed_with_its_process_group_and_keeps_the_output(self):
        with TemporaryDirectory() as directory:
            record = Path(directory) / "pid"
            status, _, stdout, stderr = run_tests.run_case([sys.executable, "-c", CHILD, str(record)], 2)
            self.assertEqual(status, "HUNG")
            self.assertEqual(stdout, "started the grandchild\n")
            self.assertEqual(stderr, "the case waits\n")
            pid = int(record.read_text())
            deadline = time.monotonic() + 5
            while running(pid) and time.monotonic() < deadline:
                time.sleep(0.05)
            self.assertFalse(running(pid), f"grandchild {pid} still runs after the hung case was killed")

    def test_exit_status_and_skip_decide_the_result(self):
        self.assertEqual(run_tests.run_case([sys.executable, "-c", "print('ok')"], 10), ("PASS", 0, "ok\n", ""))
        self.assertEqual(run_tests.run_case([sys.executable, "-c", "raise SystemExit(3)"], 10)[0], "FAIL")
        skipped = "import sys; print('OK (skipped=1)', file=sys.stderr)"
        self.assertEqual(run_tests.run_case([sys.executable, "-c", skipped], 10)[0], "FAIL")


class SelectionTest(TestCase):
    def test_the_named_test_files_select_their_cases_and_no_name_selects_every_module(self):
        selected = run_tests.test_ids(["tools/test_bench.py", "tools/test_cargo_case.py"])
        self.assertTrue(selected)
        self.assertEqual({test_id.split(".")[1] for test_id in selected}, {"test_bench", "test_cargo_case"})
        every = run_tests.test_ids()
        self.assertTrue(set(selected) < set(every))
        self.assertIn("test_run_tests", {test_id.split(".")[1] for test_id in every})

    def test_a_name_that_is_no_tool_test_file_fails(self):
        for name in ("tools/check.py", "tools/test_absent.py", "test_bench"):
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, f"{name}: not a tool test file"):
                run_tests.test_ids([name])

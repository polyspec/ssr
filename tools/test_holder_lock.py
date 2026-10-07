import json
import os
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory
from unittest import TestCase

from tools import holder_lock


HOLD = ("import sys; from pathlib import Path; from tools import holder_lock; "
        "lock = holder_lock.acquire(Path(sys.argv[1]), 'holder case'); print('held', flush=True); "
        "sys.stdin.read(); lock.release()")


class HolderLockTest(TestCase):
    def setUp(self):
        self.temporary = TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.path = Path(self.temporary.name).resolve() / "resource.lock"

    def holder(self):
        """Start a process that holds the lock until its standard input closes."""
        child = subprocess.Popen([sys.executable, "-c", HOLD, str(self.path)], cwd=holder_lock.ROOT,
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        self.assertEqual(child.stdout.readline(), "held\n")
        return child

    def stopped_pid(self):
        child = subprocess.Popen([sys.executable, "-c", ""])
        child.wait()
        self.assertIsNone(holder_lock.process_start(child.pid))
        return child.pid

    def test_record_names_checkout_pid_and_process_start(self):
        lock = holder_lock.acquire(self.path, "record case")
        record = holder_lock.read_record(self.path)
        self.assertEqual(record["checkout"], str(holder_lock.ROOT))
        self.assertEqual(record["pid"], os.getpid())
        self.assertEqual(record["processStart"], holder_lock.process_start(os.getpid()))
        self.assertEqual(record["command"], "record case")
        lock.release()
        self.assertEqual(list(self.path.parent.iterdir()), [])

    def test_second_run_is_refused_with_the_holder(self):
        child = self.holder()
        try:
            with self.assertRaises(holder_lock.HolderLockRefused) as refused:
                holder_lock.acquire(self.path)
            message = str(refused.exception)
            self.assertIn(f"is held by pid {child.pid} (process started {holder_lock.process_start(child.pid)})", message)
            self.assertIn(f"of checkout {holder_lock.ROOT}", message)
            self.assertIn("the holder releases it", message)
        finally:
            child.communicate("")
        self.assertEqual(child.returncode, 0)
        self.assertFalse(self.path.exists())

    def test_only_one_of_concurrent_runs_takes_the_lock(self):
        program = ("import sys; from pathlib import Path; from tools import holder_lock\n"
                   "try:\n    lock = holder_lock.acquire(Path(sys.argv[1]))\n"
                   "except holder_lock.HolderLockRefused:\n    print('refused', flush=True)\n"
                   "else:\n    print('held', flush=True); sys.stdin.read(); lock.release()\n")
        runs = [subprocess.Popen([sys.executable, "-c", program, str(self.path)], cwd=holder_lock.ROOT,
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True) for _ in range(8)]
        results = [run.stdout.readline().strip() for run in runs]
        for run in runs:
            run.communicate("")
        self.assertEqual(sorted(results), ["held"] + ["refused"] * 7)

    def test_lock_of_a_stopped_holder_is_reported_and_kept_until_removed(self):
        pid = self.stopped_pid()
        record = {"checkout": "/elsewhere/ssr", "pid": pid, "processStart": "Mon Oct  5 10:00:00 2026",
                  "acquired": "2026-10-05T01:00:00.000Z", "command": "make check",
                  "token": "stopped"}
        self.path.write_text(json.dumps(record) + "\n")
        with self.assertRaisesRegex(holder_lock.HolderLockRefused,
                                    rf"pid {pid} \(process started Mon Oct  5 10:00:00 2026\) of checkout "
                                    r"/elsewhere/ssr.*which is not running; remove the lock with: "
                                    r"python3 -m tools.holder_lock remove-stopped "):
            holder_lock.acquire(self.path)
        self.assertEqual(holder_lock.read_record(self.path), record)
        removal = subprocess.run([sys.executable, "-m", "tools.holder_lock", "remove-stopped", str(self.path)],
                                 cwd=holder_lock.ROOT, capture_output=True, text=True, check=False)
        self.assertEqual(removal.returncode, 0, removal.stderr)
        self.assertIn(f"removed {self.path} of pid {pid} ", removal.stdout)
        self.assertFalse(self.path.exists())

    def test_reused_pid_with_another_start_time_is_a_stopped_holder(self):
        self.path.write_text(json.dumps({
            "checkout": str(holder_lock.ROOT), "pid": os.getpid(), "processStart": "Thu Jan  1 00:00:00 1970",
            "acquired": "1970-01-01T00:00:00.000Z", "command": "earlier run", "token": "reused"}) + "\n")
        with self.assertRaisesRegex(holder_lock.HolderLockRefused, "which is not running"):
            holder_lock.acquire(self.path)
        self.assertEqual(holder_lock.remove_stopped(self.path)["token"], "reused")

    def test_removing_the_lock_of_a_running_holder_is_refused(self):
        child = self.holder()
        try:
            with self.assertRaisesRegex(holder_lock.HolderLockRefused, f"is held by pid {child.pid} "):
                holder_lock.remove_stopped(self.path)
            self.assertEqual(holder_lock.read_record(self.path)["pid"], child.pid)
        finally:
            child.communicate("")

    def test_only_the_holder_releases_its_lock(self):
        lock = holder_lock.acquire(self.path)
        replaced = {**holder_lock.read_record(self.path), "token": "another"}
        self.path.write_text(json.dumps(replaced) + "\n")
        with self.assertRaisesRegex(ValueError, "was replaced while this run held it"):
            lock.release()
        self.assertEqual(holder_lock.read_record(self.path)["token"], "another")

    def test_lock_file_without_a_record_is_an_error(self):
        self.path.write_text("")
        with self.assertRaisesRegex(ValueError, "holds no lock record"):
            holder_lock.acquire(self.path)
        self.assertEqual(self.path.read_text(), "")

    def test_start_time_on_linux_comes_from_proc(self):
        stat = "4242 (node (a) b) S 1 4242 4242 0 -1 4194560 100 0 0 0 1 2 0 0 20 0 11 0 987654 1000 50"
        self.assertEqual(holder_lock.proc_start(stat, "cpu  1 2 3\nbtime 1790000000\nprocesses 9\n"),
                         "987654 clock ticks after the boot at 2026-09-21T14:13:20.000Z")
        with self.assertRaisesRegex(ValueError, "no start time"):
            holder_lock.proc_start("4242 (node) S 1", "btime 1790000000\n")

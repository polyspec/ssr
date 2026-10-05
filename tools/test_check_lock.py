from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory
from unittest import TestCase

from tools import holder_lock


ROOT = Path(__file__).resolve().parents[1]


def recipe(target):
    lines = (ROOT / "Makefile").read_text().splitlines()
    start = lines.index(f"{target}:")
    body = []
    for line in lines[start + 1:]:
        if not line.startswith("\t"):
            break
        body.append(line.strip())
    return body


class CheckLockTest(TestCase):
    def test_check_runs_its_steps_under_the_checkout_lock(self):
        self.assertIn("FULL_RUN = python3 -m tools.holder_lock run check -- python3 -m tools.full_run",
                      (ROOT / "Makefile").read_text().splitlines())
        self.assertEqual(recipe("check"), ["$(FULL_RUN) check --setup $(CHECK_SETUP) --targets $(CHECK_TARGETS) --needs $(CHECK_NEEDS)"])
        self.assertEqual(recipe("rerun-failed"), ["$(FULL_RUN) rerun-failed --setup $(CHECK_SETUP) --needs $(CHECK_NEEDS)"])
        self.assertEqual(recipe("check-fixtures"), ["npm ci --prefix tools/build-probe/tests/fixtures --install-links --ignore-scripts --no-audit --no-fund"])
        self.assertEqual(recipe("check-nextest"), ["cargo nextest run --workspace --locked --no-tests fail"])

    def test_run_holds_the_lock_while_the_command_runs(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            marker = root / "ran"
            program = (f"from pathlib import Path; p = Path({str(root)!r}) / 'var/locks/check.lock'; "
                       f"assert p.exists(); Path({str(marker)!r}).write_text('ran')")
            result = subprocess.run(
                [sys.executable, "-m", "tools.holder_lock", "run", "check", "--root", str(root), "--",
                 sys.executable, "-c", program], cwd=ROOT, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue(marker.exists())
            self.assertFalse((root / "var/locks/check.lock").exists())
            failing = subprocess.run(
                [sys.executable, "-m", "tools.holder_lock", "run", "check", "--root", str(root), "--",
                 sys.executable, "-c", "raise SystemExit(4)"], cwd=ROOT, capture_output=True, text=True, check=False)
            self.assertEqual(failing.returncode, 4)
            self.assertFalse((root / "var/locks/check.lock").exists())

    def test_run_is_refused_while_another_run_holds_the_lock(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            marker = root / "ran"
            lock = holder_lock.acquire(holder_lock.lock_file("check", root), "make check")
            try:
                result = subprocess.run(
                    [sys.executable, "-m", "tools.holder_lock", "run", "check", "--root", str(root), "--",
                     sys.executable, "-c", f"open({str(marker)!r}, 'w')"],
                    cwd=ROOT, capture_output=True, text=True, check=False)
            finally:
                lock.release()
            self.assertEqual(result.returncode, 1)
            self.assertIn("is held by pid ", result.stderr)
            self.assertFalse(marker.exists())

from pathlib import Path
import sys
from tempfile import TemporaryDirectory
from unittest import TestCase

from tools import holder_lock, prepare_engine_compose, verify_engine_linux


ROOT = Path(__file__).resolve().parents[1]


class EngineStackTest(TestCase):
    def test_each_checkout_has_its_own_project_container_and_image(self):
        first, second = (prepare_engine_compose.names(Path(path)) for path in ("/work/ssr", "/work/ssr-copy"))
        self.assertEqual(first, prepare_engine_compose.names(Path("/work/ssr")))
        for key in ("project", "container", "image"):
            self.assertNotEqual(first[key], second[key], key)
        self.assertEqual(first["container"], f"{first['project']}-engine")
        content = prepare_engine_compose.render(ROOT)
        own = prepare_engine_compose.names(ROOT)
        self.assertIn(f"name: {own['project']}\n", content)
        self.assertIn(f"image: {own['image']}\n", content)

    def test_no_engine_command_names_a_fixed_stack(self):
        sources = [ROOT / "Makefile", ROOT / "verification/engine/linux/compose.yaml",
                   *(ROOT / "tools").glob("*.py")]
        for source in sources:
            if source.name.startswith("test_"):
                continue
            text = source.read_text()
            for fixed in ("ssr-engine-verify:0.0.1",):
                self.assertNotIn(fixed, text, source)

    def test_verification_refuses_while_another_run_holds_the_checkout_stack(self):
        with TemporaryDirectory() as temporary:
            lock_path = Path(temporary).resolve() / "engine-verification.lock"
            marker = Path(temporary) / "ran"
            lock = holder_lock.acquire(lock_path, "make verify-engine-linux-arm64")
            try:
                status = verify_engine_linux.run_locked(
                    [[sys.executable, "-c", f"open({str(marker)!r}, 'w')"]], lock_path)
            finally:
                lock.release()
            self.assertEqual(status, 1)
            self.assertFalse(marker.exists())

    def test_verification_runs_its_steps_in_order_and_releases_the_lock(self):
        with TemporaryDirectory() as temporary:
            lock_path = Path(temporary).resolve() / "engine-verification.lock"
            log = Path(temporary) / "log"
            step = lambda text, code=0: [sys.executable, "-c",
                                         f"open({str(log)!r}, 'a').write({text!r}); raise SystemExit({code})"]
            self.assertEqual(verify_engine_linux.run_locked([step("a"), step("b", 3), step("c")], lock_path), 3)
            self.assertEqual(log.read_text(), "ab")
            self.assertFalse(lock_path.exists())
            self.assertEqual(verify_engine_linux.run_locked([step("d")], lock_path), 0)
            self.assertEqual(log.read_text(), "abd")

    def test_a_started_stack_is_stopped_after_a_failing_step(self):
        with TemporaryDirectory() as temporary:
            lock_path = Path(temporary).resolve() / "engine-verification.lock"
            log = Path(temporary) / "log"
            step = lambda text, code=0: [sys.executable, "-c",
                                         f"open({str(log)!r}, 'a').write({text!r}); raise SystemExit({code})"]
            commands = [step("a"), step("b"), step("c", 3), step("d")]
            self.assertEqual(verify_engine_linux.run_locked(commands, lock_path, step("z"), commands[1]), 3)
            self.assertEqual(log.read_text(), "abcz")
            log.unlink()
            commands = [step("a", 4), step("b")]
            self.assertEqual(verify_engine_linux.run_locked(commands, lock_path, step("z"), commands[1]), 4)
            self.assertEqual(log.read_text(), "a")
            self.assertFalse(lock_path.exists())

    def test_verification_stops_the_stack_it_started(self):
        steps = verify_engine_linux.verify_steps(ROOT)
        self.assertEqual(steps[3], ["containerctl", "-f", str(prepare_engine_compose.OUTPUT), "up"])
        self.assertEqual(verify_engine_linux.stop(),
                         ["containerctl", "-f", str(prepare_engine_compose.OUTPUT), "down"])
        self.assertNotIn(verify_engine_linux.stop(), steps)

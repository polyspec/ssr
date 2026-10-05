from pathlib import Path
import re
import sys
from tempfile import TemporaryDirectory
from unittest import TestCase

from tools import check_linux, prepare_engine_compose, verify_engine_linux


ROOT = Path(__file__).resolve().parents[1]


class CheckLinuxTest(TestCase):
    def test_the_full_suite_runs_the_linux_target(self):
        makefile = (ROOT / "Makefile").read_text()
        targets = re.search(r"^CHECK_TARGETS = (.*)$", makefile, re.MULTILINE).group(1).split()
        self.assertIn("check-linux", targets)
        self.assertIn("\ncheck-linux:\n\tpython3 -m tools.check_linux\n", makefile)

    def test_the_steps_build_lint_and_test_ssr_server_in_the_checkout_stack(self):
        stack = prepare_engine_compose.names(ROOT)
        steps = [" ".join(map(str, step)) for step in check_linux.steps(ROOT)]
        def index(fragment):
            matches = [position for position, step in enumerate(steps) if fragment in step]
            self.assertEqual(len(matches), 1, f"{fragment}: {steps}")
            return matches[0]
        order = [
            index("tools/verify_archive.py aarch64-unknown-linux-gnu"),
            index("tools.prepare_engine_compose"),
            index(f"container build --platform linux/arm64 -f verification/engine/linux/Dockerfile -t {stack['image']}"),
            index("var/engine-compose.yaml up"),
            index("tools.verify_engine_status"),
            index("verify_engine_mounts.py"),
            index("cargo fetch --locked"),
            index("cargo clippy --locked -p ssr-server --all-targets -- -D warnings"),
            index("cargo build --locked -p ssr-server --example development_process --example socket_process"),
            index("cargo nextest run --locked -p ssr-server --lib --test development --test process --no-tests fail"),
        ]
        self.assertEqual(order, sorted(order))
        for step in steps:
            if "container exec" in step:
                self.assertIn(stack["container"], step)
        self.assertIn("var/engine-compose.yaml up", steps[3])
        self.assertTrue(" ".join(check_linux.stop()).endswith("var/engine-compose.yaml down"))

    def test_a_started_stack_is_stopped_after_a_failing_step(self):
        with TemporaryDirectory() as temporary:
            lock_path = Path(temporary).resolve() / "engine-verification.lock"
            log = Path(temporary) / "log"
            step = lambda text, code=0: [sys.executable, "-c",
                                         f"open({str(log)!r}, 'a').write({text!r}); raise SystemExit({code})"]
            commands = [step("a"), step("up"), step("b", 3), step("c")]
            self.assertEqual(verify_engine_linux.run_locked(commands, lock_path, step("down"), commands[1]), 3)
            self.assertEqual(log.read_text(), "aupbdown")
            self.assertFalse(lock_path.exists())
            log.unlink()
            commands = [step("a", 4), step("up")]
            self.assertEqual(verify_engine_linux.run_locked(commands, lock_path, step("down"), commands[1]), 4)
            self.assertEqual(log.read_text(), "a")
            log.unlink()
            commands = [step("up"), step("b")]
            self.assertEqual(verify_engine_linux.run_locked(commands, lock_path, step("down", 5), commands[0]), 5)
            self.assertEqual(log.read_text(), "upbdown")

    def test_checkouts_are_mounted_read_only_and_only_cache_and_output_are_writable(self):
        content = prepare_engine_compose.render(ROOT)
        volumes = re.findall(r"^      - (.+)$", content, re.MULTILINE)
        writable = [volume for volume in volumes if not volume.endswith(":ro")]
        self.assertEqual(writable, [f"{ROOT / 'var/engine-cargo'}:/cargo", f"{ROOT / 'var/engine-target'}:/target"])
        self.assertEqual(len(volumes), 6, volumes)

    def test_the_host_engine_inputs_are_exported_to_host_targets_only(self):
        # check-linux verifies the Linux V8 inputs, which a host archive in RUSTY_V8_ARCHIVE refuses.
        makefile = (ROOT / "Makefile").read_text()
        exports = [line for line in makefile.splitlines() if ": export " in line]
        self.assertTrue(exports)
        for line in exports:
            targets = line.split(":")[0]
            self.assertNotIn("$(CHECK_TARGETS)", targets, line)
            self.assertNotIn("check-linux", targets, line)
        self.assertIn("CHECK_HOST_TARGETS = $(filter-out check-linux,$(CHECK_TARGETS))", makefile.splitlines())

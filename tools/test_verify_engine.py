from contextlib import redirect_stderr
import io
import os
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
from unittest import TestCase
from unittest.mock import patch
import subprocess

from tools import verify_archive, verify_engine

TARGET = "aarch64-apple-darwin"


class EngineArchiveTest(TestCase):
    def test_missing_inputs_fail_before_build(self):
        with TemporaryDirectory() as temporary:
            with patch.object(verify_engine, "ROOT", Path(temporary).resolve()):
                with patch.object(verify_engine.subprocess, "run") as build:
                    self.assertEqual(verify_engine.verify(TARGET), 1)
                    build.assert_not_called()

    def test_offline_build_selects_verified_archive_and_binding(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            archive, binding = root / "archive.a.gz", root / "binding.rs"
            with patch.dict(os.environ, {}, clear=True), patch.object(verify_engine, "ROOT", root):
                with patch.object(verify_archive, "verify", return_value=(archive, binding)):
                    with patch.object(verify_archive, "check_metadata") as metadata:
                        with patch.object(verify_engine.subprocess, "run", return_value=SimpleNamespace(returncode=0)) as run:
                            self.assertEqual(verify_engine.verify(TARGET), 0)
                        metadata.assert_called_once_with(root / "verification/engine", TARGET)
            build = run.call_args_list[0]
            self.assertEqual(build.kwargs["env"]["CARGO_NET_OFFLINE"], "true")
            self.assertEqual(build.kwargs["env"]["RUSTY_V8_ARCHIVE"], str(archive))
            self.assertEqual(build.kwargs["env"]["RUSTY_V8_SRC_BINDING_PATH"], str(binding))
            self.assertNotIn("timeout", build.kwargs)
            self.assertIn("--verbose", build.args[0])

    def test_a_hung_engine_execution_is_named_with_its_limit(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            archive, binding = root / "archive.a.gz", root / "binding.rs"
            hung = subprocess.TimeoutExpired(["polyspec-ssr-engine-verification"], 30)
            error = io.StringIO()
            with patch.dict(os.environ, {"CARGO_TARGET_DIR": str(root / "target")}, clear=True), \
                    patch.object(verify_engine, "ROOT", root), \
                    patch.object(verify_engine.platform, "system", return_value="Darwin"), \
                    patch.object(verify_engine.platform, "machine", return_value="arm64"), \
                    patch.object(verify_archive, "verify", return_value=(archive, binding)), \
                    patch.object(verify_archive, "check_metadata"), \
                    patch.object(verify_engine.subprocess, "run",
                                 side_effect=[SimpleNamespace(returncode=0), hung]) as run, \
                    redirect_stderr(error):
                self.assertEqual(verify_engine.verify(TARGET), 1)
            executable = root / "target" / TARGET / "debug/polyspec-ssr-engine-verification"
            self.assertEqual(run.call_args.args[0], [str(executable)])
            self.assertIn(f"HUNG engine execution {TARGET}: {executable} did not exit within its 30 s limit",
                          error.getvalue())

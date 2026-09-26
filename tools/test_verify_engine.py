import hashlib
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
from unittest import TestCase
from unittest.mock import patch
from urllib.error import URLError

from tools import verify_engine


TARGET = "aarch64-apple-darwin"


class EngineArchiveTest(TestCase):
    def test_missing_and_changed_archives_fail(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(verify_engine, "ROOT", root):
                self.assertEqual(verify_engine.verify(TARGET, False, True, False), 1)
                archive = root / "var/v8" / f"librusty_v8_simdutf_release_{TARGET}.a.gz"
                archive.parent.mkdir(parents=True)
                archive.write_bytes(b"changed")
                self.assertEqual(verify_engine.verify(TARGET, False, True, False), 1)

    def test_failed_download_is_an_error(self):
        with TemporaryDirectory() as temporary:
            with patch.object(verify_engine, "ROOT", Path(temporary)):
                with patch.object(verify_engine.urllib.request, "urlopen", side_effect=URLError("unavailable")):
                    self.assertEqual(verify_engine.verify(TARGET, True, True, False), 1)

    def test_offline_build_uses_verified_archive_without_network(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / "var/v8" / f"librusty_v8_simdutf_release_{TARGET}.a.gz"
            archive.parent.mkdir(parents=True)
            archive.write_bytes(b"verified archive")
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            with patch.object(verify_engine, "ROOT", root):
                with patch.dict(verify_engine.ARCHIVES, {TARGET: digest}):
                    with patch.object(verify_engine.subprocess, "run", return_value=SimpleNamespace(returncode=0)) as build:
                        self.assertEqual(verify_engine.verify(TARGET, False, False, True), 0)
            self.assertEqual(build.call_args.kwargs["env"]["CARGO_NET_OFFLINE"], "true")
            self.assertEqual(build.call_args.kwargs["env"]["RUSTY_V8_ARCHIVE"], str(archive))

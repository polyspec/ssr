import hashlib
import os
from pathlib import Path
import sys
from tempfile import TemporaryDirectory
from unittest import TestCase
from unittest.mock import patch

from tools import verify_group_archive


def files(root):
    source = root / "target/debug/gn_out/obj/librusty_v8.a"
    archive = root / "var/v8/librusty_v8_source_aarch64-apple-darwin.a"
    binding_source = root / "target/debug/gn_out/src_binding.rs"
    binding = root / "var/v8/src_binding_source_aarch64-apple-darwin.rs"
    archive_hash = hashlib.sha256(b"verified archive").hexdigest()
    binding_hash = hashlib.sha256(b"verified binding").hexdigest()
    return archive, source, archive_hash, binding, binding_source, binding_hash


class GroupArchiveTest(TestCase):
    def test_routine_targets_verify_archive_without_source_build(self):
        makefile = (Path(__file__).resolve().parents[1] / "Makefile").read_text()
        self.assertIn("check bench: verify-group-archive", makefile)
        self.assertIn("RUSTY_V8_ARCHIVE", makefile)
        self.assertIn("RUSTY_V8_SRC_BINDING_PATH", makefile)
        self.assertNotIn("check bench: export V8_FROM_SOURCE", makefile)

    def test_missing_and_changed_archive_or_binding_fail(self):
        with TemporaryDirectory() as temporary:
            paths = files(Path(temporary))
            archive, _, _, binding, _, _ = paths
            self.assertEqual(verify_group_archive.verify(*paths, False), 1)
            archive.parent.mkdir(parents=True)
            archive.write_bytes(b"verified archive")
            self.assertEqual(verify_group_archive.verify(*paths, False), 1)
            binding.write_bytes(b"verified binding")
            self.assertEqual(verify_group_archive.verify(*paths, False), 0)
            archive.write_bytes(b"changed")
            self.assertEqual(verify_group_archive.verify(*paths, False), 1)
            archive.write_bytes(b"verified archive")
            binding.write_bytes(b"changed")
            self.assertEqual(verify_group_archive.verify(*paths, False), 1)

    def test_install_verifies_both_sources_and_copies_to_separate_paths(self):
        with TemporaryDirectory() as temporary:
            paths = files(Path(temporary))
            archive, source, _, binding, binding_source, _ = paths
            source.parent.mkdir(parents=True)
            source.write_bytes(b"verified archive")
            binding_source.write_bytes(b"verified binding")
            self.assertEqual(verify_group_archive.verify(*paths, True), 0)
            self.assertEqual(archive.read_bytes(), source.read_bytes())
            self.assertEqual(binding.read_bytes(), binding_source.read_bytes())
            source.write_bytes(b"changed")
            self.assertEqual(verify_group_archive.verify(*paths, True), 1)
            self.assertEqual(archive.read_bytes(), b"verified archive")
            binding_source.write_bytes(b"changed")
            source.write_bytes(b"verified archive")
            self.assertEqual(verify_group_archive.verify(*paths, True), 1)
            self.assertEqual(binding.read_bytes(), b"verified binding")

    def test_nonabsolute_and_identical_paths_fail_before_copy(self):
        with TemporaryDirectory() as temporary:
            paths = files(Path(temporary))
            archive, source, archive_hash, binding, binding_source, binding_hash = paths
            source.parent.mkdir(parents=True)
            source.write_bytes(b"verified archive")
            binding_source.write_bytes(b"verified binding")
            self.assertEqual(verify_group_archive.verify(source, source, archive_hash, binding, binding_source, binding_hash, True), 1)
            self.assertEqual(source.read_bytes(), b"verified archive")
            self.assertEqual(verify_group_archive.verify(archive, source, archive_hash, binding_source, binding_source, binding_hash, True), 1)
            self.assertEqual(binding_source.read_bytes(), b"verified binding")
            self.assertEqual(verify_group_archive.verify(Path("relative.a"), source, archive_hash, binding, binding_source, binding_hash, True), 1)

    def test_routine_verification_rejects_source_build_or_mismatched_selection(self):
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            paths = files(root)
            archive, source, _, binding, _, _ = paths
            archive.parent.mkdir(parents=True)
            archive.write_bytes(b"verified archive")
            binding.write_bytes(b"verified binding")
            with patch.dict(os.environ, {"V8_FROM_SOURCE": "1"}):
                self.assertEqual(verify_group_archive.verify(*paths, False), 1)
            with patch.dict(os.environ, {"RUSTY_V8_ARCHIVE": str(source), "CARGO_TARGET_DIR": str(root)}):
                with patch.object(verify_group_archive, "ROOT", root):
                    with patch.object(sys, "argv", ["verify_group_archive.py", "verify"]):
                        with patch.object(verify_group_archive.platform, "system", return_value="Darwin"):
                            with patch.object(verify_group_archive.platform, "machine", return_value="arm64"):
                                self.assertEqual(verify_group_archive.main(), 1)
            with patch.dict(os.environ, {"RUSTY_V8_SRC_BINDING_PATH": str(source), "CARGO_TARGET_DIR": str(root)}):
                with patch.object(verify_group_archive, "ROOT", root):
                    with patch.object(sys, "argv", ["verify_group_archive.py", "verify"]):
                        with patch.object(verify_group_archive.platform, "system", return_value="Darwin"):
                            with patch.object(verify_group_archive.platform, "machine", return_value="arm64"):
                                self.assertEqual(verify_group_archive.main(), 1)

    def test_unverified_host_fails(self):
        with patch.object(sys, "argv", ["verify_group_archive.py", "verify"]):
            with patch.object(verify_group_archive.platform, "system", return_value="Linux"):
                with patch.object(verify_group_archive.platform, "machine", return_value="x86_64"):
                    self.assertEqual(verify_group_archive.main(), 1)

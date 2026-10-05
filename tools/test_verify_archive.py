import hashlib
import json
import os
import subprocess
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
from unittest import TestCase
from unittest.mock import patch

from tools import verify_archive


class ArchiveTest(TestCase):
    def test_build_verification_does_not_require_engine_inputs(self):
        root = Path(__file__).resolve().parents[1]
        metadata = subprocess.run(["cargo", "metadata", "--manifest-path", "tools/build-probe/Cargo.toml",
                                   "--no-deps", "--offline", "--format-version", "1"], cwd=root,
                                  capture_output=True, text=True, timeout=10, check=False)
        self.assertEqual(metadata.returncode, 0, metadata.stderr)
        packages = json.loads(metadata.stdout)["packages"]
        dependencies = {item["name"] for package in packages for item in package["dependencies"]}
        self.assertFalse({"deno_core", "v8"} & dependencies)
        result = subprocess.run(["make", "--dry-run", "verify-build"], cwd=root,
                                capture_output=True, text=True, timeout=10, check=False)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn("tools/verify_archive.py", result.stdout)

    def test_less_than_ten_gib_free_fails(self):
        with patch.object(verify_archive.shutil, "disk_usage", return_value=SimpleNamespace(free=10 * 1024**3 - 1)):
            with self.assertRaises(ValueError):
                verify_archive.check_disk(Path("/"))
        with patch.object(verify_archive.shutil, "disk_usage", return_value=SimpleNamespace(free=10 * 1024**3)):
            verify_archive.check_disk(Path("/"))

    def test_missing_changed_relative_and_symbolic_inputs_fail(self):
        with TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            archive = root / "archive.a.gz"
            binding = root / "binding.rs"
            archive_hash = hashlib.sha256(b"archive").hexdigest()
            binding_hash = hashlib.sha256(b"binding").hexdigest()
            def verify():
                return verify_archive.check_files(archive, binding, archive_hash, binding_hash)
            with self.assertRaisesRegex(ValueError, "missing V8 input"):
                verify()
            archive.write_bytes(b"archive")
            binding.write_bytes(b"binding")
            verify()
            binding.write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
                verify()
            binding.write_bytes(b"binding")
            archive.write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
                verify()
            with self.assertRaisesRegex(ValueError, "absolute canonical regular file"):
                verify_archive.check_files(Path("relative.a"), binding, archive_hash, binding_hash)
            archive.write_bytes(b"archive")
            symbolic = root / "symbolic.rs"
            symbolic.symlink_to(binding)
            with self.assertRaisesRegex(ValueError, "absolute canonical regular file"):
                verify_archive.check_files(archive, symbolic, archive_hash, binding_hash)

    def test_environment_rejects_download_source_build_and_changed_selection(self):
        archive = Path("/verified/archive.a.gz")
        binding = Path("/verified/binding.rs")
        for values in (
            {"V8_FROM_SOURCE": "1"},
            {"V8_FORCE_DEBUG": "true"},
            {"DOCS_RS": "1"},
            {"DENO_TRYBUILD": "1"},
            {"RUSTY_V8_ARCHIVE": "https://example.invalid/archive.a.gz"},
            {"RUSTY_V8_ARCHIVE": "/other/archive.a.gz"},
            {"RUSTY_V8_SRC_BINDING_PATH": "/other/binding.rs"},
        ):
            with self.subTest(values=values), patch.dict(os.environ, values, clear=True):
                with self.assertRaises(ValueError):
                    verify_archive.check_environment(archive, binding)

    def test_default_commands_require_official_local_inputs(self):
        root = Path(__file__).resolve().parents[1]
        makefile = (root / "Makefile").read_text()
        self.assertIn("check-steps bench: verify-archive", makefile)
        self.assertIn("librusty_v8_simdutf_release_", makefile)
        self.assertNotIn("--fetch", makefile)
        manifest = (root / "Cargo.toml").read_text()
        self.assertNotIn('"v8_enable_pointer_compression"', manifest)

    def test_feature_or_version_mismatch_fails(self):
        metadata = {"packages": [{"name": "v8", "version": "150.4.0", "id": "v8"}],
                    "resolve": {"nodes": [{"id": "v8", "features": ["default", "simdutf", "use_custom_libcxx"]}]}}
        verify_archive.check_features(metadata)
        metadata["resolve"]["nodes"][0]["features"].remove("default")
        verify_archive.check_features(metadata)
        metadata["resolve"]["nodes"][0]["features"].append("v8_enable_pointer_compression")
        with self.assertRaises(ValueError):
            verify_archive.check_features(metadata)
        metadata["packages"][0]["version"] = "150.3.0"
        with self.assertRaises(ValueError):
            verify_archive.check_features(metadata)

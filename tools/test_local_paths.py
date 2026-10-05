from pathlib import Path
import os
import tempfile
import unittest
from unittest import mock

from tools import local_paths, prepare_engine_compose, verify_engine_mounts


class LocalPathsTest(unittest.TestCase):
    def test_absolute_path_dependencies_name_declared_checkouts(self):
        declared = local_paths.declared()
        for manifest in local_paths.MANIFESTS:
            for name, path in local_paths.path_dependencies(manifest).items():
                self.assertTrue((path / "Cargo.toml").is_file(), f"{manifest}: {name} path {path} is absent")
                self.assertEqual(path, declared.get(name), f"{manifest}: {name} differs from the root manifest")

    def test_engine_files_name_no_host_path(self):
        for relative in ("verification/engine/linux/compose.yaml", "tools/verify_engine_mounts.py",
                         "tools/prepare_engine_compose.py"):
            self.assertNotIn("/Users/", (local_paths.ROOT / relative).read_text(), relative)

    def test_engine_compose_mounts_declared_checkouts(self):
        content = prepare_engine_compose.render(prepare_engine_compose.ROOT)
        declared = local_paths.declared()
        for name in ("v8", "ordered-json"):
            root = local_paths.checkout(declared[name])
            self.assertIn(f"- {root}:{root}:ro", content)

    def test_mount_check_requires_absolute_environment_paths(self):
        with mock.patch.dict(os.environ, {"SSR_V8_DIR": "/v8", "SSR_ORDERED_JSON_DIR": "json"}):
            with self.assertRaisesRegex(ValueError, "SSR_ORDERED_JSON_DIR"):
                verify_engine_mounts.expected()
        with mock.patch.dict(os.environ, {"SSR_V8_DIR": "/v8", "SSR_ORDERED_JSON_DIR": "/json"}):
            self.assertEqual(verify_engine_mounts.expected()["/json"], "ro")

    def test_unsupported_path_declaration_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            manifest = Path(temporary) / "Cargo.toml"
            manifest.write_text('[dependencies.v8]\npath = "/opt/v8"\n')
            with self.assertRaisesRegex(ValueError, "unsupported path declaration"):
                local_paths.path_dependencies(manifest)

    def test_missing_root_declaration_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "Cargo.toml").write_text('[patch.crates-io]\nv8 = { path = "/opt/v8" }\n')
            with self.assertRaisesRegex(ValueError, "lightningcss, ordered-json"):
                local_paths.declared(root)


if __name__ == "__main__":
    unittest.main()

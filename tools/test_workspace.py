import pathlib
import tempfile
import unittest

from tools import check


ROOT = pathlib.Path(__file__).resolve().parents[1]
CRATES = (
    "ssr-core",
    "ssr-build",
    "ssr-runtime",
    "ssr-adapter-react",
    "ssr-adapter-vue",
    "ssr-adapter-svelte",
    "ssr-adapter-vanilla",
    "ssr-server",
)


class WorkspaceTest(unittest.TestCase):
    def test_workspace_members_and_edition(self):
        manifest = (ROOT / "Cargo.toml").read_text()
        for crate in CRATES:
            self.assertTrue((ROOT / "crates" / crate / "Cargo.toml").is_file())
            self.assertIn(f'"crates/{crate}"', manifest)
            self.assertIn('edition = "2024"', (ROOT / "crates" / crate / "Cargo.toml").read_text())
        self.assertEqual(manifest.count('"crates/ssr-'), len(CRATES))

    def test_toolchain_and_targets(self):
        self.assertIn('channel = "1.98.1"', (ROOT / "rust-toolchain.toml").read_text())
        makefile = (ROOT / "Makefile").read_text()
        self.assertIn("check:", makefile)
        self.assertIn("bench:", makefile)

    def test_document_checks_report_missing_pair_and_link(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / "docs").mkdir()
            (root / "docs/example.md").write_text("[missing](absent.md)\n")
            errors = check.check_pairs_and_links(root)
            self.assertEqual(len(errors), 2)

    def test_document_checks_compare_temporary_bypass_state(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / "docs").mkdir()
            (root / "docs/checklist.md").write_text("- [!] S-0-2 cause; retry condition\n")
            (root / "docs/checklist.ko.md").write_text("- [ ] S-0-2 원인; 재시도 조건\n")
            self.assertIn(
                "docs/checklist.md: item IDs or states differ from Korean document",
                check.check_pairs_and_links(root),
            )

    def test_record_and_term_checks_report_prohibited_words(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / "example.md").write_text("ported site\n")
            errors = check.check_words(root)
            self.assertEqual(len(errors), 2)

    def test_nested_installed_package_documents_are_excluded(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            package = root / "crates" / "sample" / "node_modules" / "package"
            package.mkdir(parents=True)
            (package / "README.md").write_text("ported site [missing](absent.md)\n")
            (root / "record.md").write_text("ported site [missing](absent.md)\n")
            self.assertEqual(len(check.check_pairs_and_links(root)), 2)
            self.assertEqual(len(check.check_words(root)), 2)


if __name__ == "__main__":
    unittest.main()

import os
import pathlib
import subprocess
import tempfile
import unittest

from tools import check


ROOT = pathlib.Path(__file__).resolve().parents[1]


def commit_all(root):
    """Make ``root`` a Git checkout whose one commit tracks every file not ignored."""
    for arguments in (["init", "-q"], ["add", "-A"],
                      ["-c", "user.name=test", "-c", "user.email=test@example.com", "commit", "-q", "-m", "init"]):
        subprocess.run(["git", *arguments], cwd=root, capture_output=True, text=True, check=True)
CRATES = (
    "polyspec-ssr-core",
    "polyspec-ssr-build",
    "polyspec-ssr-runtime",
    "polyspec-ssr-adapter-react",
    "polyspec-ssr-adapter-vue",
    "polyspec-ssr-adapter-svelte",
    "polyspec-ssr-adapter-vanilla",
    "polyspec-ssr-nonce",
    "polyspec-ssr-server",
)


class WorkspaceTest(unittest.TestCase):
    def test_workspace_members_and_edition(self):
        manifest = (ROOT / "Cargo.toml").read_text()
        for crate in CRATES:
            self.assertTrue((ROOT / "crates" / crate / "Cargo.toml").is_file())
            self.assertIn(f'"crates/{crate}"', manifest)
            self.assertIn('edition = "2024"', (ROOT / "crates" / crate / "Cargo.toml").read_text())
        self.assertEqual(manifest.count('"crates/polyspec-ssr-'), len(CRATES))

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
            commit_all(root)
            errors = check.check_pairs_and_links(root)
            self.assertEqual(len(errors), 2)

    def test_document_checks_compare_temporary_bypass_state(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / "docs").mkdir()
            (root / "docs/checklist.md").write_text("- [!] S-0-2 cause; retry condition\n")
            (root / "docs/checklist.ko.md").write_text("- [ ] S-0-2 원인; 재시도 조건\n")
            commit_all(root)
            self.assertIn(
                "docs/checklist.md: item IDs or states differ from Korean document",
                check.check_pairs_and_links(root),
            )

    def test_document_checks_compare_nested_subitem_state(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / "docs").mkdir()
            (root / "docs/checklist.md").write_text("- [o] S-0-2-1 Define the threshold.\n")
            (root / "docs/checklist.ko.md").write_text("- [ ] S-0-2-1 기준 정의.\n")
            commit_all(root)
            self.assertIn(
                "docs/checklist.md: item IDs or states differ from Korean document",
                check.check_pairs_and_links(root),
            )

    def test_record_and_term_checks_report_prohibited_words(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / "example.md").write_text("orphan site\n")
            commit_all(root)
            errors = check.check_words(root)
            self.assertEqual(len(errors), 2)

    def test_untracked_and_ignored_documents_are_not_read(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            package = root / "crates" / "sample" / "node_modules" / "package"
            package.mkdir(parents=True)
            (root / ".gitignore").write_text("node_modules/\n")
            (package / "README.md").write_text("orphan site [missing](absent.md)\n")
            (root / "record.md").write_text("orphan site [missing](absent.md)\n")
            commit_all(root)
            (root / "notes.md").write_text("orphan site [missing](absent.md)\n")
            (root / "crates" / "sample" / "lib.rs").write_text("// orphan\n")
            self.assertEqual(len(check.check_pairs_and_links(root)), 2)
            self.assertEqual(len(check.check_words(root)), 2)

    def test_an_absolute_path_into_a_home_directory_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            home = "/" + "Users" + "/someone/"
            (root / "Cargo.toml").write_text(f'[dependencies]\nlib = {{ path = "{home}lib" }}\n')
            (root / "Makefile").write_text("build:\n\tcargo build --manifest-path $(CURDIR)/Cargo.toml\n")
            (root / "image.png").write_bytes(bytes([0x89, 0x50, 0xff, 0xfe]))
            commit_all(root)
            self.assertEqual(check.check_outside_paths(root), [
                f"Cargo.toml:2:17: absolute path into a home directory: {home}"])

    def test_the_repository_names_no_home_directory(self):
        self.assertEqual(check.check_outside_paths(ROOT), [])

    def test_an_assertion_on_a_measured_time_is_named(self):
        # The sources are composed, so this file holds no assertion on a measured time itself.
        word = "assert"
        error = "an assertion judges a measured time; assert the behaviour and print the time"
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / "src").mkdir()
            (root / "src/worker.rs").write_text(
                "fn case() {\n    let started = Instant::now();\n    work();\n"
                f"    {word}!(\n        started.elapsed()\n            < Duration::from_secs(1)\n    );\n"
                f"    {word}_eq!(work(), Ok(()));\n    println!(\"{{:?}}\", started.elapsed());\n}}\n")
            (root / "test_tool.py").write_text(
                f"started = time.monotonic()\n{word} time.monotonic() - started < 2\n"
                f"self.{word}Less(elapsed, 2.0)\nself.{word}Equal(result, 0)\nprint(time.monotonic() - started)\n")
            (root / "steps.mjs").write_text(
                f"{word}.ok(performance.now() - started < 1000);\n{word}.equal(done, true);\n")
            commit_all(root)
            self.assertEqual(check.check_timed_assertions(root), [
                f"src/worker.rs:4: {error}", f"test_tool.py:2: {error}", f"test_tool.py:3: {error}",
                f"steps.mjs:1: {error}"])

    def test_no_assertion_judges_a_measured_time(self):
        self.assertEqual(check.check_timed_assertions(ROOT), [])

    def test_python_bytecode_is_ignored(self):
        # A Python without a cache prefix writes __pycache__ next to the sources, as on a CI runner.
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / ".gitignore").write_bytes((ROOT / ".gitignore").read_bytes())
            (root / "tools").mkdir()
            (root / "tools/check.py").write_text("")
            commit_all(root)
            (root / "tools/__pycache__").mkdir()
            (root / "tools/__pycache__/check.cpython-39.pyc").write_bytes(b"bytecode")
            self.assertEqual(check.check_untracked(root), [])

    def test_untracked_file_is_named(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / ".gitignore").write_text("/var/\n")
            commit_all(root)
            (root / "var").mkdir()
            (root / "var/record.json").write_text("{}\n")
            (root / "docs").mkdir()
            (root / "docs/new.md").write_text("new\n")
            self.assertEqual(check.check_untracked(root), [
                "docs/new.md: untracked file; add it to Git or ignore it, the checks read tracked files only"])

    def test_a_checkout_that_git_cannot_read_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(OSError, r"git ls-files -- \*\.md in .* exited with 128: fatal: not a git repository"):
                check.markdown_files(pathlib.Path(temporary))

    def test_license_exceptions_are_exact_versions(self):
        # Each tracked package has a tracked lock, so the case resolves nothing with the registry.
        for version, accepted in (("0.8.18", True), ("0.8.17", False)):
            manifest = ROOT / f"tools/license-fixtures/xxhash-{version}/Cargo.toml"
            result = subprocess.run([
                "cargo", "deny", "--manifest-path", str(manifest),
                "--config", str(ROOT / "deny.toml"), "--locked", "--offline", "check", "licenses",
                "--hide-inclusion-graph",
            ], capture_output=True, text=True, check=False,
                env={**os.environ, "CARGO_NET_OFFLINE": "true", "RUSTUP_AUTO_INSTALL": "0"})
            self.assertEqual(result.returncode == 0, accepted,
                             f"xxhash-rust {version}: exit {result.returncode}\n{result.stdout}{result.stderr}")

    def test_license_fixtures_lock_the_exact_versions(self):
        for version in ("0.8.18", "0.8.17"):
            lock = (ROOT / f"tools/license-fixtures/xxhash-{version}/Cargo.lock").read_text()
            self.assertIn(f'name = "xxhash-rust"\nversion = "{version}"\n', lock)


if __name__ == "__main__":
    unittest.main()

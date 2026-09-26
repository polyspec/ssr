import copy
from pathlib import Path
import subprocess
import tempfile
import unittest

from tools import test_ownership


class TestOwnershipTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "Cargo.toml").write_text(
            '[workspace]\nmembers = ["crates/ssr-core", "crates/ssr-runtime"]\n'
        )
        for crate in ("ssr-core", "ssr-runtime"):
            folder = self.root / "crates" / crate
            (folder / "tests").mkdir(parents=True)
            (folder / "src").mkdir()
            (folder / "src/lib.rs").write_text("")
            manifest = f'[package]\nname = "{crate}"\nversion = "0.0.1"\nedition = "2024"\n'
            if crate == "ssr-runtime":
                manifest += '[dependencies]\nssr-core = { path = "../ssr-core" }\n'
            (folder / "Cargo.toml").write_text(manifest)
        (self.root / "crates/ssr-core/tests/page.rs").write_text(
            '#[test]\nfn base() { assert_eq!(1, 1); }\n'
        )
        (self.root / "crates/ssr-runtime/tests/use_page.rs").write_text(
            '#[test]\nfn uses_page() { assert_eq!(1, 1); }\n'
        )
        self.declaration = {"behaviors": [{
            "id": "page",
            "owner": {"crate": "ssr-core", "target": "page", "test": "base"},
            "consumers": [{"crate": "ssr-runtime", "target": "use_page", "test": "uses_page"}],
        }]}

    def test_valid_declaration_names_executable_cases(self):
        cases = test_ownership.validate(self.root, self.declaration)
        self.assertEqual([(case.crate, case.test) for case in cases],
                         [("ssr-core", "base"), ("ssr-runtime", "uses_page")])

    def test_missing_consumer_is_rejected(self):
        declaration = copy.deepcopy(self.declaration)
        declaration["behaviors"][0]["consumers"] = []
        with self.assertRaisesRegex(test_ownership.OwnershipError, "consumer"):
            test_ownership.validate(self.root, declaration)

    def test_misplaced_test_is_rejected(self):
        declaration = copy.deepcopy(self.declaration)
        declaration["behaviors"][0]["owner"]["crate"] = "ssr-runtime"
        with self.assertRaises(test_ownership.OwnershipError):
            test_ownership.validate(self.root, declaration)

    def test_missing_test_is_rejected(self):
        declaration = copy.deepcopy(self.declaration)
        declaration["behaviors"][0]["owner"]["test"] = "absent"
        with self.assertRaisesRegex(test_ownership.OwnershipError, "absent"):
            test_ownership.validate(self.root, declaration)

    def test_file_name_without_test_is_rejected(self):
        declaration = copy.deepcopy(self.declaration)
        del declaration["behaviors"][0]["owner"]["test"]
        with self.assertRaisesRegex(test_ownership.OwnershipError, "test"):
            test_ownership.validate(self.root, declaration)

    def test_empty_and_comment_only_tests_are_rejected(self):
        target = self.root / "crates/ssr-core/tests/page.rs"
        for body in ("", "// no assertion\n", "/* no assertion */"):
            target.write_text(f'#[test]\nfn base() {{ {body} }}\n')
            with self.assertRaisesRegex(test_ownership.OwnershipError, "empty"):
                test_ownership.validate(self.root, self.declaration)

    def test_failed_test_is_rejected(self):
        cases = test_ownership.validate(self.root, self.declaration)
        failure = subprocess.CompletedProcess([], 100, "", "failed")
        with self.assertRaisesRegex(test_ownership.OwnershipError, "failed"):
            test_ownership.run(self.root, cases, command=lambda *args, **kwargs: failure)

    def test_nextest_pass_on_stderr_is_accepted(self):
        cases = test_ownership.validate(self.root, self.declaration)
        def passed(args, **kwargs):
            crate = args[args.index("-p") + 1]
            target = args[args.index("--test") + 1]
            name = args[-1]
            return subprocess.CompletedProcess(
                args, 0, "", f"PASS [ 0.001s] (1/1) {crate}::{target} {name}\n"
            )
        test_ownership.run(self.root, cases, command=passed)

    def test_timed_out_test_is_rejected(self):
        cases = test_ownership.validate(self.root, self.declaration)
        def timeout(*args, **kwargs):
            raise subprocess.TimeoutExpired("cargo nextest", 180)
        with self.assertRaisesRegex(test_ownership.OwnershipError, "timeout"):
            test_ownership.run(self.root, cases, command=timeout)

    def test_ignored_test_is_rejected(self):
        cases = test_ownership.validate(self.root, self.declaration)
        ignored = subprocess.CompletedProcess([], 0, "Summary: 0 passed, 1 skipped", "")
        with self.assertRaisesRegex(test_ownership.OwnershipError, "did not pass"):
            test_ownership.run(self.root, cases, command=lambda *args, **kwargs: ignored)


if __name__ == "__main__":
    unittest.main()

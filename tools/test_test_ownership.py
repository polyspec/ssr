from contextlib import redirect_stdout
import copy
import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

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
            (folder / "src/lib.rs").write_text(
                "pub struct Page;\n" if crate == "ssr-core" else "use ssr_core::Page;\n"
            )
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
            "exports": ["Page"],
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

    def test_different_behavior_does_not_require_unrelated_consumer_test(self):
        manifest = self.root / "Cargo.toml"
        manifest.write_text(manifest.read_text().replace(
            '"crates/ssr-runtime"]', '"crates/ssr-runtime", "crates/ssr-build"]'
        ))
        folder = self.root / "crates/ssr-build"
        (folder / "src").mkdir(parents=True)
        (folder / "tests").mkdir()
        (folder / "Cargo.toml").write_text(
            '[package]\nname="ssr-build"\nversion="0.0.1"\nedition="2024"\n'
            '[dependencies]\nssr-core={path="../ssr-core"}\n'
        )
        (folder / "src/lib.rs").write_text("use ssr_core::process;\n")
        (folder / "tests/process.rs").write_text(
            '#[test]\nfn uses_process() { assert_eq!(1, 1); }\n'
        )
        (self.root / "crates/ssr-core/src/lib.rs").write_text("pub struct Page;\npub mod process {}\n")
        (self.root / "crates/ssr-runtime/src/lib.rs").write_text("use ssr_core::Page;\n")
        (self.root / "crates/ssr-core/tests/process.rs").write_text(
            '#[test]\nfn process_base() { assert_eq!(1, 1); }\n'
        )
        declaration = copy.deepcopy(self.declaration)
        declaration["behaviors"].append({
            "id": "process",
            "owner": {"crate": "ssr-core", "target": "process", "test": "process_base"},
            "exports": ["process"],
            "consumers": [{"crate": "ssr-build", "target": "process", "test": "uses_process"}],
        })
        cases = test_ownership.validate(self.root, declaration)
        self.assertEqual(len(cases), 4)

    def test_declared_consumer_without_source_use_is_rejected(self):
        (self.root / "crates/ssr-runtime/src/lib.rs").write_text("")
        with self.assertRaisesRegex(test_ownership.OwnershipError, "source consumers"):
            test_ownership.validate(self.root, self.declaration)

    def test_missing_source_consumer_is_rejected(self):
        declaration = copy.deepcopy(self.declaration)
        declaration["behaviors"][0]["consumers"] = []
        with self.assertRaisesRegex(test_ownership.OwnershipError, "source consumers"):
            test_ownership.validate(self.root, declaration)

    def test_all_public_root_exports_must_be_mapped_once(self):
        (self.root / "crates/ssr-core/src/lib.rs").write_text("pub struct Page;\npub const OTHER: u8 = 1;\n")
        with self.assertRaisesRegex(test_ownership.OwnershipError, "differ from root exports"):
            test_ownership.validate(self.root, self.declaration)

    def test_grouped_and_renamed_imports_identify_behavior_consumer(self):
        manifest = self.root / "crates/ssr-runtime/Cargo.toml"
        manifest.write_text(manifest.read_text().replace(
            'ssr-core = { path = "../ssr-core" }', 'core_alias = { package = "ssr-core", path = "../ssr-core" }'
        ))
        (self.root / "crates/ssr-runtime/src/lib.rs").write_text(
            "use core_alias::{Page as AppPage};\nfn consume(_: AppPage) {}\n"
        )
        self.assertEqual(len(test_ownership.validate(self.root, self.declaration)), 2)

    def test_qualified_reference_identifies_consumer_but_literal_does_not(self):
        (self.root / "crates/ssr-runtime/src/lib.rs").write_text(
            'const PATH: &str = "ssr_core::Other";\nfn consume() { let _ = ssr_core::Page; }\n'
        )
        self.assertEqual(len(test_ownership.validate(self.root, self.declaration)), 2)

    def test_precise_capture_is_not_an_import_statement(self):
        (self.root / "crates/ssr-runtime/src/lib.rs").write_text(
            "fn consume() -> impl Iterator<Item = ssr_core::Page> + use<> { std::iter::empty() }\n"
        )
        self.assertEqual(len(test_ownership.validate(self.root, self.declaration)), 2)

    def test_escaped_and_raw_strings_do_not_create_consumers(self):
        (self.root / "crates/ssr-runtime/src/lib.rs").write_text(
            'const ESCAPED: &str = "quote: \\\" use ssr_core::Page";\n'
            'const RAW: &str = r##"use ssr_core::Page; ssr_core::Page"##;\n'
            'const BYTE_RAW: &[u8] = br#"ssr_core::Page"#;\n'
        )
        with self.assertRaisesRegex(test_ownership.OwnershipError, "source consumers"):
            test_ownership.validate(self.root, self.declaration)

    def test_nested_grouped_self_import_resolves_to_root_module(self):
        paths = list(test_ownership.import_paths([
            "ssr_core", "::", "process", "::", "{", "self", ",", "build", "}"
        ]))
        self.assertEqual(paths, [("ssr_core", "process"), ("ssr_core", "process", "build")])

    def test_public_use_alias_is_the_exported_root_name(self):
        (self.root / "crates/ssr-core/src/lib.rs").write_text(
            "mod internal { pub struct Original; }\npub use internal::Original as Page;\n"
        )
        self.assertEqual(test_ownership.public_root_exports(self.root / "crates/ssr-core"), {"Page"})

    def test_comment_reference_does_not_create_consumer(self):
        (self.root / "crates/ssr-runtime/src/lib.rs").write_text("// use ssr_core::Page;\n")
        with self.assertRaisesRegex(test_ownership.OwnershipError, "source consumers"):
            test_ownership.validate(self.root, self.declaration)

    def test_owner_wildcard_import_is_an_error(self):
        (self.root / "crates/ssr-runtime/src/lib.rs").write_text("use ssr_core::*;\n")
        with self.assertRaisesRegex(test_ownership.OwnershipError, "wildcard"):
            test_ownership.validate(self.root, self.declaration)

    def test_owner_crate_root_alias_is_an_error(self):
        (self.root / "crates/ssr-runtime/src/lib.rs").write_text("use ssr_core as core_api;\n")
        with self.assertRaisesRegex(test_ownership.OwnershipError, "alias"):
            test_ownership.validate(self.root, self.declaration)

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

    def test_cargo_compilation_has_no_total_timeout(self):
        cases = test_ownership.validate(self.root, self.declaration)
        def command(args, **kwargs):
            self.assertNotIn("timeout", kwargs)
            self.assertNotIn("capture_output", kwargs)
            output = "".join(f"PASS [ 0.001s] {case.crate}::{case.target} {case.test}\n" for case in cases)
            return subprocess.CompletedProcess(args, 0, output, "")
        test_ownership.run(self.root, cases, command=command)

    def test_nextest_pass_on_stderr_is_accepted(self):
        cases = test_ownership.validate(self.root, self.declaration)
        def passed(args, **kwargs):
            output = "".join(
                f"PASS [ 0.001s] (1/2) {case.crate}::{case.target} {case.test}\n" for case in cases
            )
            return subprocess.CompletedProcess(args, 0, "", output)
        test_ownership.run(self.root, cases, command=passed)

    def test_timed_out_test_is_rejected(self):
        cases = test_ownership.validate(self.root, self.declaration)
        def timeout(*args, **kwargs):
            raise subprocess.TimeoutExpired("cargo nextest", 180)
        with self.assertRaisesRegex(test_ownership.OwnershipError, "timeout"):
            test_ownership.run(self.root, cases, command=timeout)

    def test_cases_run_once_on_one_workspace_build(self):
        cases = test_ownership.validate(self.root, self.declaration)
        calls = []
        def command(args, **kwargs):
            calls.append(args)
            output = "".join(
                f"PASS [ 0.001s] ({index}/2) {case.crate}::{case.target} {case.test}\n"
                for index, case in enumerate(cases, 1)
            )
            return subprocess.CompletedProcess(args, 0, "" if "--no-run" in args else output, "")
        test_ownership.run(self.root, cases, command=command)
        self.assertEqual(len(calls), 2)
        build, tests = calls
        self.assertEqual(build[:3], ["cargo", "nextest", "run"])
        self.assertIn("--workspace", build)
        self.assertIn("--no-run", build)
        self.assertIn("--workspace", tests)
        self.assertNotIn("--no-run", tests)
        self.assertNotIn("-p", tests)

    def test_ignored_test_is_rejected(self):
        cases = test_ownership.validate(self.root, self.declaration)
        ignored = subprocess.CompletedProcess([], 0, "Summary: 0 passed, 1 skipped", "")
        with self.assertRaisesRegex(test_ownership.OwnershipError, "did not pass"):
            test_ownership.run(self.root, cases, command=lambda *args, **kwargs: ignored)


    def slow_cargo(self, body):
        """Put a cargo first on PATH that is silent for 2 s and then runs body."""
        folder = self.root / "bin"
        folder.mkdir()
        cargo = folder / "cargo"
        cargo.write_text(f"#!/bin/sh\nsleep 2\n{body}\n")
        cargo.chmod(0o755)
        return patch.dict(os.environ, {"PATH": f"{folder}{os.pathsep}{os.environ['PATH']}"})

    def test_cargo_metadata_runs_past_any_limit_to_its_exit(self):
        def limited(run):
            def call(*args, **kwargs):
                if "timeout" in kwargs:
                    kwargs["timeout"] = 1
                return run(*args, **kwargs)
            return call
        output = io.StringIO()
        with self.slow_cargo(f'exec {json.dumps(shutil.which("cargo"))} "$@"'), redirect_stdout(output), \
                patch("tools.test_ownership.subprocess.run", limited(subprocess.run)):
            cases = test_ownership.validate(self.root, self.declaration)
        self.assertEqual(len(cases), 2)
        self.assertRegex(output.getvalue(),
                         r"\ARUN cargo metadata --no-deps --offline --format-version 1\n"
                         r"PASS cargo metadata (\d+\.\d)s\n")
        elapsed = float(re.search(r"PASS cargo metadata (\d+\.\d)s", output.getvalue()).group(1))
        self.assertGreaterEqual(elapsed, 2.0)

    def test_cargo_metadata_failure_names_its_exit_code(self):
        output = io.StringIO()
        with self.slow_cargo("exit 101"), redirect_stdout(output):
            with self.assertRaisesRegex(test_ownership.OwnershipError, "workspace metadata failed with exit 101"):
                test_ownership.validate(self.root, self.declaration)
        self.assertRegex(output.getvalue(), r"FAIL cargo metadata exit 101 \d+\.\ds\n\Z")

if __name__ == "__main__":
    unittest.main()

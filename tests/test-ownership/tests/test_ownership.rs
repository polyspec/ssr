//! The test ownership check on temporary fake workspaces of two packages, `packages/core` (`polyspec-ssr-core`, the
//! owner of `Page`) and `packages/runtime` (`polyspec-ssr-runtime`, its consumer); fake `cargo` programs on `PATH`
//! stand in for Cargo where a case needs a particular Cargo behavior.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

use polyspec_ssr_test_ownership::{
    Cargo, Case, Exit, Outcome, OwnershipError, cargo_case, import_paths, parse_declaration,
    public_root_exports, run, validate,
};
use serde_json::{Value, json};
use tempfile::TempDir;

struct Fixture {
    _directory: TempDir,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::Builder::new()
            .prefix("ssr-test-ownership-")
            .tempdir()
            .expect("temporary folder");
        let fixture = Self {
            root: directory.path().to_path_buf(),
            _directory: directory,
        };
        fixture.write(
            "Cargo.toml",
            "[workspace]\nresolver = \"3\"\nmembers = [\"packages/core\", \"packages/runtime\"]\n",
        );
        fixture.write("packages/core/Cargo.toml", &manifest("core", ""));
        fixture.write("packages/core/src/lib.rs", "pub struct Page;\n");
        fixture.write(
            "packages/core/tests/page.rs",
            "#[test]\nfn base() { assert_eq!(1, 1); }\n",
        );
        fixture.write(
            "packages/runtime/Cargo.toml",
            &manifest(
                "runtime",
                "[dependencies]\npolyspec-ssr-core = { path = \"../core\" }\n",
            ),
        );
        fixture.write(
            "packages/runtime/src/lib.rs",
            "use polyspec_ssr_core::Page;\n",
        );
        fixture.write(
            "packages/runtime/tests/use_page.rs",
            "#[test]\nfn uses_page() { assert_eq!(1, 1); }\n",
        );
        fixture
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().expect("parent folder")).expect("folder");
        fs::write(path, text).expect("file");
    }

    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.root.join(relative)).expect("file")
    }

    fn add_member(&self, name: &str) {
        let text = self.read("Cargo.toml").replace(
            "\"packages/runtime\"]",
            &format!("\"packages/runtime\", \"packages/{name}\"]"),
        );
        self.write("Cargo.toml", &text);
    }

    fn validate(&self, declaration: &Value) -> Result<Vec<Case>, OwnershipError> {
        self.validate_with(declaration, &Cargo::default(), &mut Vec::new())
    }

    fn validate_with(
        &self,
        declaration: &Value,
        cargo: &Cargo,
        out: &mut Vec<u8>,
    ) -> Result<Vec<Case>, OwnershipError> {
        let declaration = parse_declaration(&declaration.to_string())?;
        validate(&self.root, &declaration, cargo, out)
    }

    /// Put a `cargo` first on `PATH` that runs the shell script body.
    fn fake_cargo(&self, body: &str) -> Cargo {
        let folder = self.root.join("bin");
        fs::create_dir_all(&folder).expect("folder");
        let cargo = folder.join("cargo");
        fs::write(&cargo, format!("#!/bin/sh\n{body}\n")).expect("script");
        fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).expect("permissions");
        let path = env::var("PATH").expect("PATH");
        Cargo::with_path(format!("{}:{path}", folder.display()).into())
    }
}

fn manifest(name: &str, rest: &str) -> String {
    format!(
        "[package]\nname = \"polyspec-ssr-{name}\"\nversion = \"0.0.1\"\nedition = \"2024\"\n{rest}"
    )
}

fn declaration() -> Value {
    json!({"behaviors": [{
        "id": "page",
        "owner": {"crate": "polyspec-ssr-core", "target": "page", "test": "base"},
        "exports": ["Page"],
        "consumers": [{"crate": "polyspec-ssr-runtime", "target": "use_page", "test": "uses_page"}],
    }]})
}

fn error<T: std::fmt::Debug>(result: Result<T, OwnershipError>) -> String {
    result.expect_err("an ownership error").0
}

/// A libtest-json report line of nextest for one declared case, in nextest's field order (`type` first).
fn report(case: &Case, event: &str) -> String {
    let name = format!("{}::{}${}", case.krate, case.target, case.test);
    match event {
        "ok" | "failed" => format!(
            "{{\"type\":\"test\",\"event\":\"{event}\",\"name\":\"{name}\",\"exec_time\":0.001}}\n"
        ),
        _ => format!("{{\"type\":\"test\",\"event\":\"{event}\",\"name\":\"{name}\"}}\n"),
    }
}

fn reports(cases: &[Case], event: &str) -> String {
    cases.iter().map(|case| report(case, event)).collect()
}

fn outcome(code: i32, output: String) -> Result<Outcome, OwnershipError> {
    Ok(Outcome {
        exit: Exit::Code(code),
        output,
    })
}

#[test]
fn a_valid_declaration_names_executable_cases() {
    let fixture = Fixture::new();
    let cases = fixture.validate(&declaration()).expect("valid");
    let named: Vec<(&str, &str)> = cases
        .iter()
        .map(|case| (case.krate.as_str(), case.test.as_str()))
        .collect();
    assert_eq!(
        named,
        [
            ("polyspec-ssr-core", "base"),
            ("polyspec-ssr-runtime", "uses_page")
        ]
    );
}

#[test]
fn every_declaration_error_is_reported() {
    let fixture = Fixture::new();
    let mut value = declaration();
    value["behaviors"][0]["consumers"][0]["test"] = json!("absent");
    value["behaviors"][0]["exports"] = json!(["Page", "Missing"]);
    let message = error(fixture.validate(&value));
    assert!(
        message.contains("missing or repeated test polyspec-ssr-runtime::use_page absent"),
        "{message}"
    );
    assert!(
        message.contains("owner does not export polyspec-ssr-core::Missing"),
        "{message}"
    );
}

#[test]
fn a_dependency_without_a_declared_behavior_is_rejected() {
    let fixture = Fixture::new();
    fixture.add_member("server");
    fixture.write(
        "packages/server/Cargo.toml",
        &manifest(
            "server",
            "[dependencies]\npolyspec-ssr-runtime = { path = \"../runtime\" }\n",
        ),
    );
    fixture.write("packages/server/src/lib.rs", "");
    let message = error(fixture.validate(&declaration()));
    assert!(
        message.contains("polyspec-ssr-runtime is a dependency of ['polyspec-ssr-server'] but owns no declared behavior"),
        "{message}"
    );
}

#[test]
fn a_missing_consumer_is_rejected() {
    let fixture = Fixture::new();
    let mut value = declaration();
    value["behaviors"][0]["consumers"] = json!([]);
    let message = error(fixture.validate(&value));
    assert!(message.contains("consumer"), "{message}");
}

#[test]
fn a_different_behavior_does_not_require_an_unrelated_consumer_test() {
    let fixture = Fixture::new();
    fixture.add_member("build");
    fixture.write(
        "packages/build/Cargo.toml",
        &manifest(
            "build",
            "[dependencies]\npolyspec-ssr-core = { path = \"../core\" }\n",
        ),
    );
    fixture.write(
        "packages/build/src/lib.rs",
        "use polyspec_ssr_core::process;\n",
    );
    fixture.write(
        "packages/build/tests/process.rs",
        "#[test]\nfn uses_process() { assert_eq!(1, 1); }\n",
    );
    fixture.write(
        "packages/core/src/lib.rs",
        "pub struct Page;\npub mod process {}\n",
    );
    fixture.write(
        "packages/core/tests/process.rs",
        "#[test]\nfn process_base() { assert_eq!(1, 1); }\n",
    );
    let mut value = declaration();
    value["behaviors"].as_array_mut().expect("behaviors").push(json!({
        "id": "process",
        "owner": {"crate": "polyspec-ssr-core", "target": "process", "test": "process_base"},
        "exports": ["process"],
        "consumers": [{"crate": "polyspec-ssr-build", "target": "process", "test": "uses_process"}],
    }));
    assert_eq!(fixture.validate(&value).expect("valid").len(), 4);
}

#[test]
fn a_declared_consumer_without_source_use_is_rejected() {
    let fixture = Fixture::new();
    fixture.write("packages/runtime/src/lib.rs", "");
    let message = error(fixture.validate(&declaration()));
    assert!(message.contains("source consumers"), "{message}");
}

#[test]
fn a_missing_source_consumer_is_rejected() {
    let fixture = Fixture::new();
    let mut value = declaration();
    value["behaviors"][0]["consumers"] = json!([]);
    let message = error(fixture.validate(&value));
    assert!(message.contains("source consumers"), "{message}");
}

#[test]
fn all_public_root_exports_must_be_mapped_once() {
    let fixture = Fixture::new();
    fixture.write(
        "packages/core/src/lib.rs",
        "pub struct Page;\npub const OTHER: u8 = 1;\n",
    );
    let message = error(fixture.validate(&declaration()));
    assert!(message.contains("differ from root exports"), "{message}");
}

#[test]
fn grouped_and_renamed_imports_identify_the_behavior_consumer() {
    let fixture = Fixture::new();
    let text = fixture.read("packages/runtime/Cargo.toml").replace(
        "polyspec-ssr-core = { path = \"../core\" }",
        "core_alias = { package = \"polyspec-ssr-core\", path = \"../core\" }",
    );
    fixture.write("packages/runtime/Cargo.toml", &text);
    fixture.write(
        "packages/runtime/src/lib.rs",
        "use core_alias::{Page as AppPage};\nfn consume(_: AppPage) {}\n",
    );
    assert_eq!(fixture.validate(&declaration()).expect("valid").len(), 2);
}

#[test]
fn a_qualified_reference_identifies_a_consumer_but_a_literal_does_not() {
    let fixture = Fixture::new();
    fixture.write(
        "packages/runtime/src/lib.rs",
        "const PATH: &str = \"polyspec_ssr_core::Other\";\nfn consume() { let _ = polyspec_ssr_core::Page; }\n",
    );
    assert_eq!(fixture.validate(&declaration()).expect("valid").len(), 2);
}

#[test]
fn a_reference_inside_a_macro_invocation_identifies_a_consumer() {
    let fixture = Fixture::new();
    fixture.write(
        "packages/runtime/src/lib.rs",
        "fn consume(value: Option<u8>) -> bool { matches!(value, Some(_) if polyspec_ssr_core::Page == polyspec_ssr_core::Page) }\n",
    );
    assert_eq!(fixture.validate(&declaration()).expect("valid").len(), 2);
}

#[test]
fn a_precise_capture_is_not_an_import_statement() {
    let fixture = Fixture::new();
    fixture.write(
        "packages/runtime/src/lib.rs",
        "fn consume() -> impl Iterator<Item = polyspec_ssr_core::Page> + use<> { std::iter::empty() }\n",
    );
    assert_eq!(fixture.validate(&declaration()).expect("valid").len(), 2);
}

#[test]
fn escaped_and_raw_strings_do_not_create_consumers() {
    let fixture = Fixture::new();
    fixture.write(
        "packages/runtime/src/lib.rs",
        concat!(
            "const ESCAPED: &str = \"quote: \\\" use polyspec_ssr_core::Page\";\n",
            "const RAW: &str = r##\"use polyspec_ssr_core::Page; polyspec_ssr_core::Page\"##;\n",
            "const BYTE_RAW: &[u8] = br#\"polyspec_ssr_core::Page\"#;\n",
        ),
    );
    let message = error(fixture.validate(&declaration()));
    assert!(message.contains("source consumers"), "{message}");
}

#[test]
fn a_nested_grouped_self_import_resolves_to_the_root_module() {
    let tree: syn::UseTree =
        syn::parse_str("polyspec_ssr_core::process::{self, build}").expect("use tree");
    assert_eq!(
        import_paths(&tree),
        [
            vec!["polyspec_ssr_core", "process"],
            vec!["polyspec_ssr_core", "process", "build"]
        ]
    );
}

#[test]
fn a_public_use_alias_is_the_exported_root_name() {
    let fixture = Fixture::new();
    fixture.write(
        "packages/core/src/lib.rs",
        "mod internal { pub struct Original; }\npub use internal::Original as Page;\n",
    );
    let exports =
        public_root_exports(&fixture.root.join("packages/core/src/lib.rs")).expect("exports");
    assert_eq!(exports.into_iter().collect::<Vec<_>>(), ["Page"]);
}

#[test]
fn qualified_root_items_export_their_own_names() {
    let fixture = Fixture::new();
    fixture.write(
        "packages/core/src/lib.rs",
        "pub const fn page() {}\npub static mut COUNT: u8 = 0;\npub unsafe extern \"C\" fn call() {}\npub(crate) struct Hidden;\n",
    );
    let exports =
        public_root_exports(&fixture.root.join("packages/core/src/lib.rs")).expect("exports");
    assert_eq!(
        exports.into_iter().collect::<Vec<_>>(),
        ["COUNT", "call", "page"]
    );
}

#[test]
fn a_comment_reference_does_not_create_a_consumer() {
    let fixture = Fixture::new();
    fixture.write(
        "packages/runtime/src/lib.rs",
        "// use polyspec_ssr_core::Page;\n/* polyspec_ssr_core::Page */\n",
    );
    let message = error(fixture.validate(&declaration()));
    assert!(message.contains("source consumers"), "{message}");
}

#[test]
fn a_use_in_a_module_file_of_a_test_target_identifies_a_consumer() {
    let fixture = Fixture::new();
    fixture.write("packages/runtime/src/lib.rs", "");
    fixture.write(
        "packages/runtime/tests/use_page.rs",
        "mod support;\n#[test]\nfn uses_page() { support::page(); }\n",
    );
    fixture.write(
        "packages/runtime/tests/support/mod.rs",
        "pub fn page() { let _ = polyspec_ssr_core::Page; }\n",
    );
    assert_eq!(fixture.validate(&declaration()).expect("valid").len(), 2);
}

#[test]
fn a_symbolic_link_in_a_source_folder_is_rejected() {
    let fixture = Fixture::new();
    fixture.write("packages/runtime/src/lib.rs", "mod linked;\n");
    fixture.write(
        "elsewhere.rs",
        "pub fn page() { let _ = polyspec_ssr_core::Page; }\n",
    );
    std::os::unix::fs::symlink(
        fixture.root.join("elsewhere.rs"),
        fixture.root.join("packages/runtime/src/linked.rs"),
    )
    .expect("link");
    let message = error(fixture.validate(&declaration()));
    assert!(
        message.contains("symbolic link in Rust source"),
        "{message}"
    );
}

#[test]
fn an_owner_wildcard_import_is_an_error() {
    let fixture = Fixture::new();
    fixture.write("packages/runtime/src/lib.rs", "use polyspec_ssr_core::*;\n");
    let message = error(fixture.validate(&declaration()));
    assert!(message.contains("wildcard"), "{message}");
}

#[test]
fn an_owner_crate_root_alias_is_an_error() {
    let fixture = Fixture::new();
    fixture.write(
        "packages/runtime/src/lib.rs",
        "use polyspec_ssr_core as core_api;\n",
    );
    let message = error(fixture.validate(&declaration()));
    assert!(message.contains("alias"), "{message}");
}

#[test]
fn a_misplaced_test_is_rejected() {
    let fixture = Fixture::new();
    let mut value = declaration();
    value["behaviors"][0]["owner"]["crate"] = json!("polyspec-ssr-runtime");
    let message = error(fixture.validate(&value));
    assert!(
        message.contains("missing test target in polyspec-ssr-runtime: page"),
        "{message}"
    );
}

#[test]
fn a_missing_test_is_rejected() {
    let fixture = Fixture::new();
    let mut value = declaration();
    value["behaviors"][0]["owner"]["test"] = json!("absent");
    let message = error(fixture.validate(&value));
    assert!(message.contains("absent"), "{message}");
}

#[test]
fn a_file_name_without_a_test_is_rejected() {
    let mut value = declaration();
    value["behaviors"][0]["owner"]
        .as_object_mut()
        .expect("owner")
        .remove("test");
    let message = error(parse_declaration(&value.to_string()));
    assert!(message.contains("missing field `test`"), "{message}");
}

#[test]
fn unknown_fields_are_rejected_in_every_object() {
    for pointer in [
        "",
        "/behaviors/0",
        "/behaviors/0/owner",
        "/behaviors/0/consumers/0",
    ] {
        let mut value = declaration();
        value
            .pointer_mut(pointer)
            .and_then(Value::as_object_mut)
            .expect("object")
            .insert("extra".into(), json!(1));
        let message = error(parse_declaration(&value.to_string()));
        assert!(
            message.contains("unknown field `extra`"),
            "{pointer}: {message}"
        );
    }
}

#[test]
fn repeated_keys_are_rejected_in_every_object() {
    let case = r#"{"crate": "polyspec-ssr-core", "target": "page", "test": "base"}"#;
    let repeated_case =
        r#"{"crate": "polyspec-ssr-core", "target": "page", "test": "base", "test": "other"}"#;
    let behavior = |owner: &str, consumer: &str, repeated: &str| {
        format!(
            r#"{{"id": "page", "owner": {owner}, "exports": ["Page"], "consumers": [{consumer}]{repeated}}}"#
        )
    };
    parse_declaration(&format!(
        r#"{{"behaviors": [{}]}}"#,
        behavior(case, case, "")
    ))
    .expect("valid without repeats");
    let texts = [
        (
            format!(
                r#"{{"behaviors": [{}], "behaviors": []}}"#,
                behavior(case, case, "")
            ),
            "behaviors",
        ),
        (
            format!(
                r#"{{"behaviors": [{}]}}"#,
                behavior(case, case, r#", "id": "other""#)
            ),
            "id",
        ),
        (
            format!(
                r#"{{"behaviors": [{}]}}"#,
                behavior(repeated_case, case, "")
            ),
            "test",
        ),
        (
            format!(
                r#"{{"behaviors": [{}]}}"#,
                behavior(case, repeated_case, "")
            ),
            "test",
        ),
    ];
    for (text, field) in texts {
        let message = error(parse_declaration(&text));
        assert!(
            message.contains(&format!("duplicate field `{field}`")),
            "{text}: {message}"
        );
    }
}

#[test]
fn empty_and_comment_only_tests_are_rejected() {
    let fixture = Fixture::new();
    for body in ["", "// no assertion\n", "/* no assertion */"] {
        fixture.write(
            "packages/core/tests/page.rs",
            &format!("#[test]\nfn base() {{ {body} }}\n"),
        );
        let message = error(fixture.validate(&declaration()));
        assert!(message.contains("empty"), "{message}");
    }
}

#[test]
fn a_failed_test_is_rejected() {
    let fixture = Fixture::new();
    let cases = fixture.validate(&declaration()).expect("valid");
    let message = error(run(
        &cases,
        &mut |_, _| outcome(100, "failed".into()),
        &mut Vec::new(),
    ));
    assert!(message.contains("failed"), "{message}");
}

#[test]
fn a_nextest_report_on_standard_error_is_accepted_and_forwarded() {
    let fixture = Fixture::new();
    let cases = fixture.validate(&declaration()).expect("valid");
    let lines = reports(&cases, "ok");
    let cargo = fixture.fake_cargo(&format!(
        "case \" $* \" in *\" --no-run \"*) echo compiled; exit 0;; esac\nprintf '%s' '{lines}' >&2"
    ));
    let mut out = Vec::new();
    run(
        &cases,
        &mut |args, out| cargo_case::run(&cargo, &fixture.root, args, out),
        &mut out,
    )
    .expect("passed");
    let out = String::from_utf8(out).expect("text");
    assert!(
        out.contains(
            "COMMAND cargo nextest run --workspace --locked --no-run --color never\ncompiled\n"
        ),
        "{out}"
    );
    assert!(out.contains(&lines), "{out}");
    assert!(
        out.contains("PASS page: polyspec-ssr-runtime::use_page uses_page\n"),
        "{out}"
    );
}

#[test]
fn the_human_output_of_nextest_is_not_read() {
    let fixture = Fixture::new();
    let cases = fixture.validate(&declaration()).expect("valid");
    let human: String = cases
        .iter()
        .map(|case| {
            format!(
                "PASS [ 0.001s] {}::{} {}\n",
                case.krate, case.target, case.test
            )
        })
        .collect();
    let message = error(run(
        &cases,
        &mut |_, _| outcome(0, human.clone()),
        &mut Vec::new(),
    ));
    assert!(message.contains("did not pass"), "{message}");
}

#[test]
fn a_hung_case_is_named() {
    let fixture = Fixture::new();
    let cases = fixture.validate(&declaration()).expect("valid");
    // nextest reports a case that it ended at its time limit as a failure with this reason; libtest as `timeout`.
    let ended = |case: &Case| {
        let name = format!("{}::{}${}", case.krate, case.target, case.test);
        format!(
            "{{\"type\":\"test\",\"event\":\"failed\",\"name\":\"{name}\",\"exec_time\":1.0,\"reason\":\"time limit exceeded\"}}\n"
        )
    };
    for output in [
        reports(&cases, "timeout"),
        cases.iter().map(ended).collect(),
    ] {
        let mut out = Vec::new();
        let message = error(run(
            &cases,
            &mut |args, _| {
                outcome(
                    if args.contains(&"--no-run") { 0 } else { 100 },
                    output.clone(),
                )
            },
            &mut out,
        ));
        assert!(message.contains("failed"), "{message}");
        let out = String::from_utf8(out).expect("text");
        assert!(
            out.contains("FAIL page: polyspec-ssr-core::page base (hung)\n"),
            "{out}"
        );
    }
}

#[test]
fn cases_run_once_on_one_workspace_build() {
    let fixture = Fixture::new();
    let cases = fixture.validate(&declaration()).expect("valid");
    let mut calls: Vec<Vec<String>> = Vec::new();
    run(
        &cases,
        &mut |args, _| {
            calls.push(args.iter().map(ToString::to_string).collect());
            outcome(
                0,
                if args.contains(&"--no-run") {
                    String::new()
                } else {
                    reports(&cases, "ok")
                },
            )
        },
        &mut Vec::new(),
    )
    .expect("passed");
    let [build, tests] = calls.as_slice() else {
        panic!("two Cargo runs: {calls:?}")
    };
    assert_eq!(build[..2], ["nextest", "run"]);
    assert!(
        build.contains(&"--workspace".into()) && build.contains(&"--no-run".into()),
        "{build:?}"
    );
    assert!(tests.contains(&"--workspace".into()), "{tests:?}");
    assert!(
        !tests.contains(&"--no-run".into()) && !tests.contains(&"-p".into()),
        "{tests:?}"
    );
}

#[test]
fn an_ignored_test_is_rejected() {
    let fixture = Fixture::new();
    let cases = fixture.validate(&declaration()).expect("valid");
    for output in [
        "Summary: 0 passed, 1 skipped".to_owned(),
        reports(&cases, "ignored"),
    ] {
        let message = error(run(
            &cases,
            &mut |_, _| outcome(0, output.clone()),
            &mut Vec::new(),
        ));
        assert!(message.contains("did not pass"), "{message}");
    }
}

#[test]
fn cargo_metadata_is_a_reported_step_judged_by_its_exit() {
    let fixture = Fixture::new();
    let real = env::var("CARGO").expect("the cargo that runs the tests");
    let cargo = fixture.fake_cargo(&format!("sleep 1\nexec '{real}' \"$@\""));
    let mut out = Vec::new();
    let cases = fixture
        .validate_with(&declaration(), &cargo, &mut out)
        .expect("valid");
    assert_eq!(cases.len(), 2);
    let out = String::from_utf8(out).expect("text");
    let mut lines = out.lines();
    assert_eq!(
        lines.next(),
        Some("RUN cargo metadata --no-deps --offline --format-version 1")
    );
    let pass = lines.next().expect("a result line");
    assert!(
        pass.starts_with("PASS cargo metadata ") && reports_seconds(pass),
        "{out}"
    );
}

#[test]
fn a_cargo_metadata_failure_names_its_exit_code() {
    let fixture = Fixture::new();
    let cargo = fixture.fake_cargo("exit 101");
    let mut out = Vec::new();
    let message = error(fixture.validate_with(&declaration(), &cargo, &mut out));
    assert!(
        message.contains("workspace metadata failed with exit 101"),
        "{message}"
    );
    let out = String::from_utf8(out).expect("text");
    let last = out.lines().last().expect("a result line");
    assert!(
        last.starts_with("FAIL cargo metadata exit 101 ") && reports_seconds(last),
        "{out}"
    );
}

/// Whether a step line ends with its duration in the format `<n>.<n>s`; the value is printed, never judged.
fn reports_seconds(line: &str) -> bool {
    let word = line.rsplit(' ').next().unwrap_or_default();
    let Some((whole, fraction)) = word
        .strip_suffix('s')
        .and_then(|value| value.split_once('.'))
    else {
        return false;
    };
    !whole.is_empty()
        && whole.chars().all(|c| c.is_ascii_digit())
        && fraction.len() == 1
        && fraction.chars().all(|c| c.is_ascii_digit())
}

/// A sink that fails once the child's first line arrives, so the run ends while the child still runs.
struct FailingAfterStart;

impl Write for FailingAfterStart {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.starts_with(b"started") {
            return Err(io::Error::other("closed"));
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_run_that_ends_early_terminates_the_cargo_process_group() {
    let fixture = Fixture::new();
    let marker = fixture.root.join("grandchild");
    let cargo = fixture.fake_cargo(&format!(
        "sleep 30 &\necho $! > '{}'\necho started\nwait",
        marker.display()
    ));
    let message = error(cargo_case::run(
        &cargo,
        &fixture.root,
        &["nextest"],
        &mut FailingAfterStart,
    ));
    assert!(message.contains("closed"), "{message}");
    let grandchild = fs::read_to_string(&marker).expect("the grandchild's process id");
    let state = Command::new("ps")
        .args(["-o", "stat=", "-p", grandchild.trim()])
        .output()
        .expect("ps");
    let state = String::from_utf8_lossy(&state.stdout);
    assert!(
        state.trim().is_empty() || state.trim().starts_with('Z'),
        "the grandchild still runs: {state}"
    );
}

#[test]
fn a_report_line_that_cannot_be_read_fails() {
    let message = error(cargo_case::results(
        "{\"type\":\"test\",\"event\":\"ok\"}\n",
    ));
    assert!(
        message.contains("unreadable nextest report line"),
        "{message}"
    );
}

#[test]
fn exported_macros_are_public_root_names_and_other_macros_are_not() {
    let fixture = Fixture::new();
    fixture.write(
        "packages/core/src/lib.rs",
        "#[macro_export]\nmacro_rules! shared { () => {}; }\nmacro_rules! private { () => {}; }\npub struct Page;\n",
    );
    let exports =
        public_root_exports(&fixture.root.join("packages/core/src/lib.rs")).expect("exports");
    assert_eq!(exports.into_iter().collect::<Vec<_>>(), ["Page", "shared"]);
}

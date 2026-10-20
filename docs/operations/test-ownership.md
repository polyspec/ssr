<!-- doc-id: test-ownership -->
[Korean](test-ownership.ko.md)

# Test ownership

`config/test-ownership.json` declares the behavior of every crate that another workspace crate
depends on: page, build, process, runtime, nonce, the code that the adapters share, and the React, Svelte and vanilla adapters; a
dependency without a declared behavior fails. Each behavior maps
public names at its owner's crate root to one integration test in that crate and one actual use
test in every consuming crate. All public root names of a declared owner belong to exactly one
behavior. A declaration gives the crate, integration test target and exact test function. A file
name alone does not identify an executable case.

The checker is the Rust package `tests/test-ownership`, which is not a member of the workspace; `make
check-ownership` runs it (`cargo run --locked --manifest-path tests/test-ownership/Cargo.toml -- <root>`), and `make
tool-test-test-ownership` runs its tests, formatting and Clippy. It reads the Rust sources with syn, the Rust parser,
and derives consumers from Rust imports and qualified paths in source,
integration tests, examples and benchmarks, including development dependencies. It takes the
packages, their source paths, dependency names and renamed dependencies from `cargo metadata`. A dependency on one behavior
does not require a test of another behavior from the same crate. Comments and string literals
do not count as imports. Grouped imports and renamed symbols are supported; wildcard imports
and crate aliases for declared owners fail because they do not identify a public root name.
Missing or repeated export declarations, unrecognized owner exports, missing consumers and
declared consumers without actual source use fail. The checker verifies that each named function
is a nonempty test in its own crate. It then builds every workspace test once with
`cargo nextest run --workspace --no-run` and executes all named tests in one `cargo nextest` run
on those builds. A separate run for each package would resolve features for that package alone
and compile the shared crates again for each feature set; the workspace build is also the build
that the later workspace test suite uses. Missing, misplaced, empty, failed, ignored and timed-out cases fail. The checker
reports each case and its result; `make check` runs it after package installation and before
the workspace test suite. This check establishes test execution and location. The named tests
assert the behavior's input and output in their own crates.

The checker reads workspace metadata with `cargo metadata`, reports it as a step with its elapsed
time, and judges it by its exit code without a time limit; Cargo progress and errors reach standard
error as they arrive. Cargo output is forwarded as it arrives. Compilation has no total duration
limit; nextest enforces the configured deadline for each test. An interrupted runner terminates its
Cargo process group and waits for the child. A zero exit code without the exact declared case's pass
result is a failure. The result of each case is read from the machine-readable report of nextest
(`--message-format libtest-json` at `--message-format-version 0.1`), never from its human output,
whose text differs between nextest versions.

`config/owner-checks.json` maps every tracked file to the make targets of its owning checks: path globs and
targets, such as `tool-test-<name>` for the tool test file `tests/tools/<name>.test.mts`, `nextest-<package>` for the
tests of `packages/<package>`, and `check-tools`, which runs the changed tool test files that it receives in
`TESTS`. `make owner-validate` of kit fails for a tracked file without an owner and for a target that does not exist.
`make owner-check` runs the owners of the files changed against `HEAD` (or of `PATHS`), once each; these are the Red
and Green tests to run before a commit.

[Korean](test-ownership.ko.md)

# Test ownership

`tools/test-ownership.json` declares shared page, build and process behavior. Each behavior maps
public names at its owner's crate root to one integration test in that crate and one actual use
test in every consuming crate. All public root names of a declared owner belong to exactly one
behavior. A declaration gives the crate, integration test target and exact test function. A file
name alone does not identify an executable case.

`tools/test_ownership.py` derives consumers from Rust imports and qualified paths in source,
integration tests, examples and benchmarks, including development dependencies. It matches
dependency names and renamed dependencies from workspace metadata. A dependency on one behavior
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

Cargo output is forwarded as it arrives. Compilation has no total duration limit; nextest enforces
the configured deadline for each test. An interrupted runner terminates its Cargo process group and
waits for the child. A zero exit code without the exact declared case's pass result is a failure.

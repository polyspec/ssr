[Korean](workspace.ko.md)

# Workspace

The Rust 1.98.1 workspace contains the eight crates named in AGENTS.md. All crates use edition
2024 and version 0.0.1. The workspace lockfile records dependency resolution. Each direct
dependency must use an exact version after its official release is checked.

Run `make check` to check the document pairs and links, records, terminology, Python tool tests,
Rust formatting, Clippy warnings, dependency advisories, licenses and sources, and Rust unit tests.
Rust tests run with cargo-nextest 0.9.146. The nextest configuration terminates each test after
30 seconds and prints its start and result with elapsed time. A missing test tool fails the command.
`make check` installs the build-probe fixture that its tests read, so
`python3 -m tools.holder_lock run check` holds the checkout lock `var/locks/check.lock` for all of its
steps (`make check-steps`): a second `make check` of the same checkout is refused with the holder's
checkout, pid and process start time. A test run outside `make check` does not take this lock.

Run `make bench` to execute Cargo benchmarks. The maintained measurements and limits are specified
in S-11.

Rust 1.98.1 is the pinned [Rust release](https://github.com/rust-lang/rust/releases/tag/1.98.1).
cargo-nextest 0.9.146 is the pinned [test runner release](https://github.com/nextest-rs/nextest/releases/tag/cargo-nextest-0.9.146).
The dependency check allows MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0,
Zlib and MPL-2.0. Other licenses fail the check.
The [xxhash-rust 0.8.18](https://crates.io/crates/xxhash-rust/0.8.18) metadata declares BSL-1.0.
The [dragonbox_ecma 0.1.12](https://crates.io/crates/dragonbox_ecma/0.1.12) metadata declares
Apache-2.0 with the LLVM exception or BSL-1.0. The dependency check allows BSL-1.0 only for
xxhash-rust 0.8.18 and Apache-2.0 with the LLVM exception only for dragonbox_ecma 0.1.12.

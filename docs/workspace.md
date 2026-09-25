[Korean](workspace.ko.md)

# Workspace

The Rust 1.98.1 workspace contains the eight crates named in AGENTS.md. All crates use edition
2024 and version 0.0.1. The workspace lockfile records dependency resolution. Each direct
dependency must use an exact version after its official release is checked.

Run `make check` to check the document pairs and links, records, terminology, Python tool tests,
Rust formatting, Clippy warnings, dependency advisories, licenses and sources, and Rust unit tests.
Rust tests run with cargo-nextest 0.9.146. The nextest configuration terminates each test after
30 seconds and prints its start and result with elapsed time. A missing test tool fails the command.

Run `make bench` to execute Cargo benchmarks. The maintained measurements and limits are specified
in S-11.

Rust 1.98.1 is the pinned [Rust release](https://github.com/rust-lang/rust/releases/tag/1.98.1).
cargo-nextest 0.9.146 is the pinned [test runner release](https://github.com/nextest-rs/nextest/releases/tag/cargo-nextest-0.9.146).

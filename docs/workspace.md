[Korean](workspace.ko.md)

# Workspace

The Rust 1.98.1 workspace contains the eight crates named in AGENTS.md. All crates use edition
2024 and version 0.0.1. The workspace lockfile records dependency resolution. Each direct
dependency must use an exact version after its official release is checked.

Run `make check` to check the document pairs and links, records, terminology, Python tool tests,
Rust formatting, Clippy warnings, dependency advisories, licenses and sources, and Rust unit tests.
Rust tests run with cargo-nextest 0.9.146. The nextest configuration terminates each test after
30 seconds and prints its start and result with elapsed time. A missing test tool fails the command.
`make check` runs the full suite through `tools/full_run.py`: the setup steps `CHECK_SETUP`
(the archive check and the fixture installs), then each target of `CHECK_TARGETS` as its own make
target. Before any step it refuses the run, with the reasons and a nonzero exit status, while an
item of `docs/checklist.md`, sub-items included, is `[~]` (each is named with its ID and title),
while tracked files have uncommitted changes, while the pre-push hook is not installed (see below),
and when `var/full-run.json` records a full run of
the same tree (`git rev-parse HEAD^{tree}`), which it names. The record holds the tree, the commit,
the result, the targets that did not pass and the times of each step; it is written before the
first step and after each step starts and ends, so a killed run stays recorded as `incomplete`.
A failed target does not stop the run. `make rerun-failed` reruns the setup steps and only the
targets of that record that did not pass, and is refused without a record of the current tree or
when every target passed. A fresh checkout has no record. Both entries install
the build-probe fixture that the tests read, so `python3 -m tools.holder_lock run check` holds the
checkout lock `var/locks/check.lock` for all of their steps: a second run of the same checkout is
refused with the holder's checkout, pid and process start time. A make without a target runs
`make check`.

A push happens only when no item of `docs/checklist.md`, sub-items included, is `[~]`. The tracked
pre-push hook `.githooks/pre-push` runs `python3 -m tools.push_gate hook`: it reads the checklist of
every pushed commit and of the working tree with the parser of `tools/full_run.py` and refuses the
push with exit status 1, naming each item in progress with the remote ref, the commit, the ID and
the title. A pushed commit without `docs/checklist.md` and a Git error also refuse the push. A push
that deletes a remote branch pushes no commit, so only the working tree is read. Every make
invocation sets `core.hooksPath` to `.githooks` when it differs, so the hook runs in every checkout
where make has run; `make hooks` sets it and runs `make hooks-check`, which fails unless
`core.hooksPath` is `.githooks` and the hook is executable. `tools/check.py` and the guard of
`make check` run the same check. The workflow `.github/workflows/push-gate.yml` runs
`python3 -m tools.push_gate commit HEAD` in the job `push-gate` on the pushed commit of every push
and on the head commit of every pull request: it fails with each item in progress as an error
annotation and in the job summary, and when the commit does not track the hook with mode 100755.

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

[Korean](workspace.ko.md)

# Workspace

The Rust 1.98.1 workspace contains the eight crates named in AGENTS.md. All crates use edition
2024 and version 0.0.1. The workspace lockfile records dependency resolution. Each direct
dependency must use an exact version after its official release is checked.

Run `make check` to check the document pairs and links, records, terminology, Python tool tests,
Rust formatting, Clippy warnings, dependency bans, licenses and sources, and Rust unit tests.
Security advisories come from a database that changes over time, so `make check` does not read
them; `make review-advisories` reviews them for the workspace, the engine verification and the
build verification. `tools/tool-versions.json` names every tool that the checks, builds and tests
run with the exact first line of its version report, or the SHA-256 of a tool without one;
`tools/tool_versions.py` compares the installed tools before any step of `make check` and
`make rerun-failed`, in `tools/check.py`, as the first command of the other make targets and,
for Python, in the pre-push hook, and fails naming the expected and the actual report. Every make
invocation sets `RUSTUP_AUTO_INSTALL=0`, so a missing toolchain fails instead of being installed.
Rust tests run with cargo-nextest 0.9.146. The nextest configuration terminates each test after
30 seconds and prints its start and result with elapsed time. A missing test tool fails the command.
The Python tool tests run through `tools/run_tests.py`, each in its own process group with a limit
of 30 seconds; a case that exceeds it is reported with its output, and its whole process group is
killed, so no process that the case started keeps running. Rust tests remove their temporary
directories through a guard, also when an assertion fails. Before any test of `ssr-server`, the
nextest setup script `build-programs` (`tools/build_programs.py`, an experimental feature of the
pinned nextest) builds the `development_process` and `socket_process` examples that the
development and socket tests run and names them in `SSR_DEVELOPMENT_PROCESS` and
`SSR_SOCKET_PROCESS`; the tests run the programs only from these variables, so a run of one test
target never runs a program built from older sources. The build verification workspace
`tools/build-probe` has its own nextest configuration.
`make check` runs the full suite through `tools/full_run.py`: the setup steps `CHECK_SETUP`
(the archive check and the fixture installs), then each target of `CHECK_TARGETS` as its own make
target. Before any step it refuses the run, with the reasons and a nonzero exit status, while an
item of `docs/checklist.md`, sub-items included, is `[~]` (each is named with its ID and title),
while tracked files have uncommitted changes, while files that are neither tracked nor ignored exist
(each is named; a step would read such a file although the tree does not hold it), while the
pre-push hook is not installed (see below),
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
the tip commit of each pushed ref and of the working tree with the parser of `tools/full_run.py` and refuses the
push with exit status 1, naming each item in progress with the remote ref, the commit, the ID and
the title. Earlier commits of a push are not read, because an item is recorded in progress in
one commit and completed in a later one. A pushed tip commit without `docs/checklist.md` and a Git error also refuse the push. A push
that deletes a remote branch pushes no commit, so only the working tree is read. Every make
invocation sets `core.hooksPath` to `.githooks` when it differs, so the hook runs in every checkout
where make has run; `make hooks` sets it and runs `make hooks-check`, which fails unless
`core.hooksPath` is `.githooks` and the hook is executable. `tools/check.py` and the guard of
`make check` run the same check. The workflow `.github/workflows/push-gate.yml` runs
`python3 -m tools.push_gate commit HEAD` in the job `push-gate` on the pushed tip commit of every push
and on the head commit of every pull request, not on a merge commit: it fails with each item in progress as an error
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

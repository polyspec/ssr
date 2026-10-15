<!-- doc-id: workspace -->
[Korean](workspace.ko.md)

# Workspace

The Rust 1.99.0 workspace contains the nine packages named in AGENTS.md, each in `packages/<name>`
(`members = ["packages/*"]` of `Cargo.toml`), named `polyspec-ssr-<name>`. All packages use edition
2024 and version 0.0.2. The workspace lockfile records dependency resolution. Each direct
dependency must use an exact version after its official release is checked. `[workspace.lints]` of `Cargo.toml` denies
`unsafe_code` and `clippy::allow_attributes_without_reason`, and every package takes these lints with `[lints]
workspace = true`.

Run `make check` to check the document pairs and links, records, terminology, the types and the lint of the TypeScript
of `scripts/` and `tests/` (`make typecheck` with `tsconfig.json`, `make lint-ts` of kit), the tool tests, the test
ownership, Rust formatting, Clippy warnings, dependency bans, licenses and sources, and the Rust tests.
Security advisories come from a database that changes over time, so `make check` does not read
them; `make review-advisories` (`cargo deny check advisories`) reviews them for the workspace.
`mise.toml` declares cargo-nextest 0.9.146 and cargo-deny 0.20.2, `mise.lock` pins their release binaries for every
platform, `make install-tools` installs them with mise and `make toolchain-check` compares their version reports with the
declaration. Node.js, npm and Rust are declared in `mise.toml`, `packageManager` of
`package.json` and `rust-toolchain.toml`, at the pins of kit, and `make toolchain-check` compares them. Every make
invocation sets `RUSTUP_AUTO_INSTALL=0`, so a missing toolchain fails instead of being installed.
Rust tests run with cargo-nextest 0.9.146. The nextest configuration `.config/nextest.toml` terminates each test after
30 seconds and prints its start and result with elapsed time. A missing test tool fails the command.
The tool tests `tests/tools/<name>.test.mts` test the TypeScript tools of `scripts/` and the configuration of the
repository (the workflows, the Makefile, `Cargo.toml` and `deny.toml`); `make check-tools` runs them through the test
runner of kit (`node $(KIT)/run-tests.$(KIT_EXT) node`), every file or the files of `TESTS`, and `make
tool-test-<name>` runs one file. The runner prints each test as it starts and ends with its elapsed time and gives
every test its own timeout. Rust tests remove their temporary
directories through a guard, also when an assertion fails. Before any test of `polyspec-ssr-server`, the
nextest setup script `build-programs` (`scripts/build-programs.mts`, an experimental feature of the
pinned nextest) builds the `development_process` and `socket_process` examples that the
development and socket tests run and names them in `SSR_DEVELOPMENT_PROCESS` and
`SSR_SOCKET_PROCESS`; the tests run the programs only from these variables, so a run of one test
target never runs a program built from older sources.
`make check` runs the full suite through the guard of kit (`full-run` of the package `@polyspec/kit`): each target
of `CHECK_TARGETS` (the kit checks `kit-check`, `toolchain-check`, `layout-check`, `documents-check`,
`owner-validate` and `hooks-check`, then the targets
of this repository) runs as `make -k <target>` to its end. Before any target it refuses the run, with the reasons and a
nonzero exit status, while an item of `docs/plans/execution-checklist.md`, sub-items included, is `[~]`, while tracked
files have uncommitted changes, while files that are neither tracked nor ignored exist, while the pre-push hook is not
installed (see below), while another run of the checkout holds the lock `var/full-run.lock`, and when
`var/full-run.json` records a full run of the same tree (`git rev-parse HEAD^{tree}`). The record holds the tree, the
result and each target with its status and times, and is written before and after each target, so a stopped run stays
recorded as `incomplete`. A failed target does not stop the run. A target that reads the V8 inputs or the fixture
packages names its setup step, `verify-archive` (`scripts/verify-archive.mts`) or `check-fixtures`
(`scripts/install-packages.mts`), as a prerequisite, so a failed setup step fails only the targets that read it. `make
rerun-failed` reruns only the targets of that record that did not pass, and is refused without a record of the current
tree or when every target passed. A fresh checkout has no record. A make without a target runs
`make check`. The tests never write into the checkout's fixture directory: before the tests
that read them, the nextest setup script `install-packages` (`scripts/install-packages.mts`) installs
the packages of `packages/build/tests/fixtures` once per lock into the immutable
`var/packages/<SHA-256 of package.json and package-lock.json>`, published by one rename, and names
it in `SSR_PACKAGES` and the sources in `SSR_FIXTURES`. Each test copies the sources into a new
temporary root of its own (`packages/build/tests/fixture/mod.rs`), writes its generated entries
there, builds with the installation as `BuildConfig::dependencies` and removes the root when it
ends; the browser scripts import `playwright-core` from `SSR_PACKAGES`, and the browser cases run the
chromium build that this `playwright-core` pins, installed into the same directory and named in
`SSR_BROWSER`, never a browser of the host. The filter of the script
names exactly the test binaries that read the installation, which `make check-fixture-filter` checks
(`scripts/install-packages.mts check-filter`): it reads the filter from `.config/nextest.toml` with smol-toml and the
test binaries of the workspace from `cargo metadata`.

`make check-ownership` checks the test ownership of `config/test-ownership.json` ([test ownership](test-ownership.md))
with the package `tests/test-ownership`, which is not a member of the workspace (`cargo run --locked --manifest-path
tests/test-ownership/Cargo.toml -- <root>`); `make tool-test-test-ownership` runs its tests, formatting and Clippy.
`config/layout.yml` declares the folders of the repository for `make layout-check` of kit: `.config/` and `deny.toml`
at the root, where cargo-nextest and cargo-deny read them by their conventions, the packages of `packages/`,
`tests/tools` and `tests/test-ownership`, which is written in Rust because it reads the Rust sources with syn.

A push happens only when no item of `docs/plans/execution-checklist.md`, sub-items included, is `[~]`. The tracked
pre-push hook `.githooks/pre-push`, which `make hooks` of kit writes, runs `make push-gate-hook`: the push check of kit
reads the checklist of the tip commit of each pushed ref and of the working tree with `config/checklist.json` and
refuses the push, naming each item in progress, and it checks the messages of the pushed commits against
`config/commits.json`. Every make invocation sets `core.hooksPath` to `.githooks` when it differs, so the hook runs in
every checkout where make has run; `make hooks-check` fails unless `core.hooksPath` is `.githooks` and the hook is the
one of kit and executable, and the guard of `make check` runs the same check. The workflow
`.github/workflows/push-gate.yml` installs the tools of `mise.lock` and the Rust toolchain of `rust-toolchain.toml` (`make install-tools`,
kit K151), which `make toolchain-check` reads, and runs `make ci-push` in the job `push-gate` on the pushed tip commit of every push and
on the head commit of every pull request, not on a merge commit: the targets `CI_PUSH_TARGETS` through `make
ci-targets` of kit, which are `toolchain-check`, `push-gate-commit` (the push check
of kit on `HEAD`, which fails with each item in progress as an error annotation and in the job summary, and when the
commit does not track the hook with mode 100755), `check-records` (`node scripts/check-records.mts --range <base>..<head>`, the
checks of the tracked files: the record words, the React document, the paths into a home directory and the timed assertions,
and the record words of the messages of the commits of `RANGE`, which the job sets to the commits that its event brings: of a
push after the tip it replaced, or after the default branch for a new branch, and of a pull request or merge group after its
base; the checkout fetches the whole history so the range resolves, `make check-records` reads `origin/main..HEAD` by default,
a pushed commit is not read again, and an empty or unresolvable range fails),
`typecheck` and `lint-ts`. The job runs on `ubuntu-24.04-arm` with the Node.js of `mise.toml`, the actions pinned
to commits with their versions in comments, and needs no Python or Rust. The ruleset requires this job on `main`, so a commit whose
records fail never reaches `main`. `make documents-check` of kit checks the document pairs, revisions and links and the checklists.

`.github/workflows/ci.yml` runs two jobs on each of `ubuntu-24.04-arm` and `ubuntu-24.04`. Each job first installs
the npm of `packageManager` and the packages of `package-lock.json` with `@polyspec/kit` (`make install-tools
TOOLS=npm`, `make install-packages`) and the tools of `mise.lock` (`make install-tools`). The job `lint` runs
`make ci-setup` and `make ci-lint`, the targets `CI_LINT_TARGETS` (`check-fixture-filter`, `check-fmt`,
`check-clippy` and `check-deny`), and the job `test` runs `make ci-setup` and `make ci-test`, the targets `CI_TEST_TARGETS`
(`check-examples` and `ci-nextest`), so every CI target runs in exactly one job of each architecture and the two jobs
run at the same time; both run their targets through `make ci-targets` of kit, past failures, and each job uploads
`var/report/ci-targets`, the log of each target and the summary, as `ci-<job>-<runner>`. `make install-tools`
installs the Rust toolchain of `rust-toolchain.toml` (kit K151) and `make ci-setup` the official V8 inputs of the host
target (`scripts/fetch-v8.mts`). `make ci-lint` and `make ci-test` first run `make toolchain-check`, which checks
cargo-nextest and cargo-deny too. Cargo builds
the check targets with `BUILD_JOBS` jobs: 1 by default, so a local build leaves the other processors of the host to
other work, and `make ci-lint` and `make ci-test` set `BUILD_JOBS` to the processor count of the runner (`nproc`).
These jobs are the Linux verification of the repository on AArch64 and x86_64: `check-clippy` lints every target of the
workspace, `check-examples` builds the examples and `ci-nextest` runs every test of the workspace. No make target runs a
container tool of a development machine.

Every change reaches `main` through a pull request and the merge queue; no command of this repository pushes `main`.
Publish a branch with the standard commands of GitHub, or with the GitHub UI:

```sh
git push origin HEAD:refs/heads/<branch>
gh pr create --base main --head <branch> --fill
gh pr merge <branch> --auto --rebase
```

The GitHub ruleset `main` of `.github/ruleset.json` applies to `refs/heads/main` with enforcement `active` and no bypass
actor, so it binds administrators too. Its rules: `pull_request`, a change arrives through a pull request with no
approval required, and every merge method is allowed, because `gh pr merge --auto` asks for auto-merge with a method of
its own choice; `merge_queue`, the queue merges with the method `REBASE` and the grouping strategy `ALLGREEN`, builds
and merges at most 5 entries at once without waiting for more, and `check_response_timeout_minutes` is 360, the maximum
of GitHub; `required_linear_history`, `non_fast_forward` and `deletion`, no merge commit, no force-push and no deletion
of `main`; `required_status_checks`, exactly the checks `push-gate` and `ci-passed` of the GitHub Actions app
(integration 15368), the job of `.github/workflows/push-gate.yml` and the last job of `.github/workflows/ci.yml`. The
job `ci-passed` needs every other job of `ci.yml`, runs after each of them also when one failed, was skipped or was
cancelled (`if: ${{ always() }}`), and runs `make ci-passed RESULTS='${{ toJSON(needs) }}'` of kit, which prints the result of every needed job and
fails unless each one is `success`. A job added to `ci.yml` is
listed in `needs`, so the required check covers it; `tests/tools/workflows.test.mts` fails when `ci-passed` is not the last job,
lacks `if: ${{ always() }}`, does not need every other job, runs on another runner than `push-gate` or runs another
step. A direct `git push origin <commit>:main` is refused with `GH013: Repository
rule violations found`. `gh pr merge --auto` adds the pull request to the merge queue once its required checks pass on
the pull request; the queue rebases it onto `main` as a merge group on the branch
`gh-readonly-queue/main/pr-<number>-<sha>`, both workflows run on that commit (`merge_group`), and the queue moves `main`
to exactly that commit when the checks pass; a failed check removes the pull request from the queue, and `main` does not
move. `ci.yml` runs on pull requests, merge groups and manual runs (`workflow_dispatch`), and cancels a run only for a new push to a pull request;
`push-gate.yml` also runs on every push except the branches of the queue. The branch of a merged pull request is deleted
(`delete_branch_on_merge`). The rebase gives the merged commits new hashes; `git pull --rebase` drops the local commits
that the queue merged. `make github-ruleset` changes the declared repository settings (`allow_rebase_merge`,
`allow_auto_merge`, `delete_branch_on_merge`) and creates or updates the ruleset of the declared name where they differ,
and compares again; `make github-ruleset-check` changes nothing and fails, naming each field with its live and its
declared value, when a declared repository field differs or the live ruleset is missing or differs. Both need an authenticated `gh` with
administration access to the repository. `tests/tools/github-ruleset.test.mts` runs `scripts/github-ruleset.mts`
against a fake `gh`.

A release is a tag `vX.Y.Z` of a commit of `main` ([release procedure](../AGENTS.md)); it releases the packages of
`packages/` at version X.Y.Z. A crate is not released as an archive; it is consumed by git tag, because `cargo package`
rewrites git dependencies, such as `polyspec-ordered-json`, into crates.io requirements that do not resolve. The push of
the tag runs `.github/workflows/release.yml`, the release workflow of kit, which `make kit-sync` copies and `make
kit-check` compares. Its steps install the package of kit and run, in this order and stopping at the first failure,
`make release-tools`, `make release-verify` (the tagged commit is an ancestor of `origin/main`, and the latest check
runs `push-gate` and `ci-passed` of that commit concluded `success`), `make release-versions` (X.Y.Z in every manifest
of `config/release.json` and the section `## X.Y.Z` in `docs/changelog.md` and `docs/changelog.ko.md`), `make
release-assets` (no archive, since `config/release.json` declares no package), `make release-publish` (the GitHub
Release as a draft with the section as notes), `make release-proof` (each crate is resolved from the tag as a git
dependency of a new crate and `cargo check` passes, `proof.gitTag` of `config/release.json`) and `make
release-undraft`. `config/release.json` declares the workspace manifest by its version, each crate manifest as
released by git tag, and every other tracked manifest as not released, with the reason; `make release-coverage` fails
for a manifest that it does not classify.

`make bench` runs the render benchmark: `cargo run --locked -p polyspec-ssr-runtime --example bench --features bench`
builds the example `bench` and runs it, and `make check-bench` runs its tests. The maintained measurements and limits
are specified in S-11.

Rust 1.99.0 is the pinned [Rust release](https://github.com/rust-lang/rust/releases/tag/1.99.0), and Node.js 26.12.0 is the pinned Node.js release of `mise.toml`, which `make install-tools TOOLS=npm` installs into var/tools. Every Cargo package declares `rust-version = "1.99.0"`.
cargo-nextest 0.9.146 is the pinned [test runner release](https://github.com/nextest-rs/nextest/releases/tag/cargo-nextest-0.9.146).
The dependency check allows MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0,
Zlib and MPL-2.0. Other licenses fail the check.
The [xxhash-rust 0.8.18](https://crates.io/crates/xxhash-rust/0.8.18) metadata declares BSL-1.0.
The [dragonbox_ecma 0.1.12](https://crates.io/crates/dragonbox_ecma/0.1.12) metadata declares
Apache-2.0 with the LLVM exception or BSL-1.0. The dependency check allows BSL-1.0 only for
xxhash-rust 0.8.18 and Apache-2.0 with the LLVM exception only for dragonbox_ecma 0.1.12.

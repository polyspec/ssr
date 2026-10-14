# The shared tools of the package @polyspec/kit. scripts/kit/kit-bootstrap.mk, a copy that make kit-sync writes, includes
# the kit.mk of the installed package: the targets of the toolchains, the Git hooks, the push check, the guard of the full
# run, the documents, the owner checks, the CI report and the release, which read config/*.json. Before the package is
# installed only `make install-tools TOOLS=npm` and `make install-packages` run (AGENTS.md).
include scripts/kit/kit-bootstrap.mk

# rustup never installs a missing toolchain or component: rust-toolchain.toml names the toolchain,
# and a missing one fails the command instead of being downloaded.
export RUSTUP_AUTO_INSTALL := 0
V8_TARGET := $(shell RUSTUP_AUTO_INSTALL=0 rustc -vV | sed -n 's/^host: //p')
V8_ARCHIVE := $(CURDIR)/var/v8/librusty_v8_simdutf_release_$(V8_TARGET).a.gz
V8_BINDING := $(CURDIR)/var/v8/src_binding_simdutf_release_$(V8_TARGET).rs

# make check runs the full suite through the guard of kit (full-run of the installed package): it refuses the run before any
# target while a checklist item is [~], while tracked files have uncommitted changes, while files that are neither tracked
# nor ignored exist, while the pre-push hook is not installed, while another run of the checkout holds the lock, and when
# var/full-run.json records a full run of the same tree; each target runs as `make -k <target>` to its end. make
# rerun-failed (kit.mk) runs only the targets of that record that did not pass. A target that reads the fixture packages
# of packages/build/tests/fixtures or the V8 inputs names their setup step as a prerequisite, so a failed setup step fails
# only the targets that read it.
# The tool tests tests/tools/<name>.test.mts and the packages of packages/, which name the owner targets tool-test-<name>
# and nextest-<package> below.
TOOL_TESTS := $(patsubst tests/tools/%.test.mts,%,$(wildcard tests/tools/*.test.mts))
PACKAGES := $(patsubst packages/%/Cargo.toml,%,$(wildcard packages/*/Cargo.toml))
SSR_TARGETS = typecheck lint-ts check-tools tool-test-test-ownership check-ownership check-records check-fixture-filter check-fmt check-clippy check-deny check-examples check-nextest check-bench
CHECK_TARGETS = kit-check toolchain-check layout-check documents-check owner-validate hooks-check dependency-policy-check $(SSR_TARGETS)
# The number of Cargo build jobs of the check targets: 1 by default, so a local build leaves the
# other processors of the host to the other work; the CI targets set it to the processor count of
# the runner (nproc).
BUILD_JOBS ?= 1

# A make without a target runs the full suite, so no command runs its steps without the guard.
.DEFAULT_GOAL := check
.PHONY: check review-advisories ci-setup ci-lint ci-test ci-nextest ci-push github-ruleset github-ruleset-check check-fixtures $(SSR_TARGETS) bench verify-archive

check:
	node $(KIT)/full-run.$(KIT_EXT) run $(CHECK_TARGETS)

check-ownership check-nextest: check-fixtures
check-ownership check-clippy check-examples check-nextest check-bench bench: verify-archive

check-fixtures $(SSR_TARGETS) $(addprefix tool-test-,$(TOOL_TESTS)) $(addprefix nextest-,$(PACKAGES)) ci-nextest bench verify-archive: export RUSTY_V8_ARCHIVE := $(V8_ARCHIVE)
check-fixtures $(SSR_TARGETS) $(addprefix tool-test-,$(TOOL_TESTS)) $(addprefix nextest-,$(PACKAGES)) ci-nextest bench verify-archive: export RUSTY_V8_SRC_BINDING_PATH := $(V8_BINDING)
check-fixtures $(SSR_TARGETS) $(addprefix tool-test-,$(TOOL_TESTS)) $(addprefix nextest-,$(PACKAGES)) ci-nextest bench: export CARGO_INCREMENTAL := 0
check-fixtures $(SSR_TARGETS) $(addprefix tool-test-,$(TOOL_TESTS)) $(addprefix nextest-,$(PACKAGES)) ci-nextest bench: export CARGO_BUILD_JOBS := $(BUILD_JOBS)
check-fixtures $(SSR_TARGETS) $(addprefix tool-test-,$(TOOL_TESTS)) $(addprefix nextest-,$(PACKAGES)) ci-nextest bench: export CARGO_NET_OFFLINE := true

verify-archive:
	node scripts/verify-archive.mts

check-fixtures:
	node scripts/install-packages.mts

# The TypeScript of scripts/ and tests/ (tsconfig.json): its types, and the lint of kit (lint-ts of kit.mk).
typecheck:
	node_modules/.bin/tsc -p tsconfig.json

# check-tools runs the tool tests of tests/tools with the test runner of kit: every test file, or the files of TESTS,
# which make owner-check passes for the changed test files (config/owner-checks.json). Each test file has the target
# tool-test-<name>, and each package the target nextest-<package>, which the owner rules of config/owner-checks.json name
# for the files they own.
check-tools:
	node $(KIT)/run-tests.$(KIT_EXT) node -- $(or $(TESTS),tests/tools)

$(foreach test,$(TOOL_TESTS),$(eval tool-test-$(test): ; node $$(KIT)/run-tests.$$(KIT_EXT) node -- tests/tools/$(test).test.mts))
$(foreach package,$(PACKAGES),$(eval nextest-$(package): verify-archive ; cargo nextest run -p polyspec-ssr-$(package) --locked --no-tests fail))
.PHONY: $(addprefix tool-test-,$(TOOL_TESTS)) $(addprefix nextest-,$(PACKAGES))

# The test ownership of config/test-ownership.json (docs/test-ownership.md): the standalone package tests/test-ownership,
# built into the target directory of the workspace.
check-ownership:
	CARGO_TARGET_DIR=$(CURDIR)/target cargo run --locked --manifest-path tests/test-ownership/Cargo.toml -- $(CURDIR)

# The tests, the formatting, Clippy and the dependency check of the package tests/test-ownership, which the workspace
# targets do not reach.
tool-test-test-ownership:
	CARGO_TARGET_DIR=$(CURDIR)/target cargo test --locked --manifest-path tests/test-ownership/Cargo.toml
	cargo fmt --manifest-path tests/test-ownership/Cargo.toml -- --check
	CARGO_TARGET_DIR=$(CURDIR)/target cargo clippy --locked --manifest-path tests/test-ownership/Cargo.toml --all-targets -- -D warnings
	cargo deny --manifest-path tests/test-ownership/Cargo.toml --config deny.toml --locked check --hide-inclusion-graph $(DENY_CHECKS)

# The checks of the tracked files (scripts/check-records.mts); the job push-gate runs them, so a commit whose records fail
# never reaches main.
check-records:
	node scripts/check-records.mts

# The filter of the setup script install-packages of .config/nextest.toml names exactly the test binaries that read the
# fixture packages (scripts/install-packages.mts check-filter).
check-fixture-filter:
	node scripts/install-packages.mts check-filter

check-fmt:
	cargo fmt --all -- --check

check-clippy:
	cargo clippy --workspace --all-targets --features polyspec-ssr-runtime/bench --locked -- -D warnings

# The checks of make check judge the tree only: bans, licenses and sources. Security advisories
# come from a database that changes over time, so make review-advisories reviews them separately.
DENY_CHECKS = bans licenses sources

check-deny:
	cargo deny check $(DENY_CHECKS)

review-advisories:
	cargo deny check advisories
	cargo deny --manifest-path tests/test-ownership/Cargo.toml --config deny.toml --locked check --hide-inclusion-graph advisories

check-examples:
	cargo build -p polyspec-ssr-server --example development_process --example socket_process --locked

check-nextest:
	cargo nextest run --workspace --locked --no-tests fail

check-bench:
	cargo nextest run -p polyspec-ssr-runtime --example bench --features bench --locked --no-tests fail

# The render benchmark: cargo run builds the example bench and runs the program it built.
bench:
	cargo run --locked -p polyspec-ssr-runtime --example bench --features bench

# GitHub CI (.github/workflows/ci.yml and push-gate.yml) runs only make targets. A job installs the npm of packageManager
# (make install-tools TOOLS=npm), the packages of package-lock.json with @polyspec/kit (make install-packages) and the
# tools of mise.lock, cargo-nextest and cargo-deny among them (make install-tools). ci-setup then prepares a fresh Linux
# runner: the Rust toolchain of rust-toolchain.toml and the official V8 inputs of the host target (scripts/fetch-v8.mts),
# verified by their SHA-256, and the locked dependencies. On each architecture ci-lint and ci-test, two jobs of ci.yml,
# check the versions of the tools (make toolchain-check) and run the lint and the test
# targets through make ci-targets of kit, which runs every target past failures and writes var/report/ci-targets with a
# log of each target and summary.md, the job summary; Cargo builds with one job per processor of the runner (BUILD_JOBS).
# The job push-gate runs the checks that need no Rust: the toolchain, the checklist, the records and the TypeScript.
CI_LINT_TARGETS = check-fixture-filter check-fmt check-clippy check-deny
CI_TEST_TARGETS = check-examples ci-nextest
CI_PUSH_TARGETS = toolchain-check push-gate-commit check-records typecheck lint-ts

ci-setup:
	rustup toolchain install --no-self-update
	node scripts/fetch-v8.mts "$$(RUSTUP_AUTO_INSTALL=0 rustc -vV | sed -n 's/^host: //p')"
	cargo fetch --locked

ci-lint:
	$(MAKE) --no-print-directory toolchain-check
	BUILD_JOBS=$$(nproc) $(MAKE) --no-print-directory ci-targets TARGETS="$(CI_LINT_TARGETS)"

ci-test:
	$(MAKE) --no-print-directory toolchain-check
	BUILD_JOBS=$$(nproc) $(MAKE) --no-print-directory ci-targets TARGETS="$(CI_TEST_TARGETS)"

ci-nextest:
	cargo nextest run --workspace --locked --no-tests fail

ci-push:
	$(MAKE) --no-print-directory ci-targets TARGETS="$(CI_PUSH_TARGETS)"

# The GitHub ruleset of main and the merge settings, declared in .github/ruleset.json (scripts/github-ruleset.mts,
# docs/workspace.md): every change reaches main through a pull request and the merge queue, which requires the checks
# push-gate and ci-passed. github-ruleset creates or updates the ruleset of the declared name and the
# settings; github-ruleset-check fails when the live ruleset or a setting differs from the declaration.
github-ruleset:
	node scripts/github-ruleset.mts apply

github-ruleset-check:
	node scripts/github-ruleset.mts check

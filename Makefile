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
# rerun-failed (kit.mk) runs only the targets of that record that did not pass. A target that reads the build-probe
# fixture packages or the V8 inputs names their setup step as a prerequisite, so a failed setup step fails only the
# targets that read it.
# The tool test modules of tools/test_*.py (tools/test_ownership.py is the ownership check, no test module) and the
# packages of packages/, which name the owner targets tool-test-<name> and
# nextest-<package> below.
TOOL_TESTS := $(filter-out ownership,$(patsubst tools/test_%.py,%,$(wildcard tools/test_*.py)))
CRATES := $(patsubst packages/%/Cargo.toml,%,$(wildcard packages/*/Cargo.toml))
SSR_TARGETS = check-tools check-ownership check-records check-fmt check-clippy check-deny check-engine-deps check-examples check-nextest check-features check-bench
CHECK_TARGETS = kit-check toolchain-check documents-check owner-validate hooks-check $(SSR_TARGETS)
# The number of Cargo build jobs of the check targets: 1 by default, so a local build leaves the
# other processors of the host to the other work; the CI targets set it to the processor count of
# the runner (nproc).
BUILD_JOBS ?= 1

# A make without a target runs the full suite, so no command runs its steps without the guard.
.DEFAULT_GOAL := check
.PHONY: check review-advisories ci-setup ci-lint ci-test ci-nextest ci-push ci-python push-records ci-records github-ruleset github-ruleset-check tool-versions-check check-fixtures $(SSR_TARGETS) bench verify-archive verify-engine-deps verify-build

check:
	node $(KIT)/full-run.$(KIT_EXT) run $(CHECK_TARGETS)

check-tools check-ownership check-nextest check-features: check-fixtures
check-ownership check-clippy check-examples check-nextest check-features check-bench bench: verify-archive

check-fixtures $(SSR_TARGETS) $(addprefix tool-test-,$(TOOL_TESTS)) $(addprefix nextest-,$(CRATES)) ci-nextest bench verify-archive: export RUSTY_V8_ARCHIVE := $(V8_ARCHIVE)
check-fixtures $(SSR_TARGETS) $(addprefix tool-test-,$(TOOL_TESTS)) $(addprefix nextest-,$(CRATES)) ci-nextest bench verify-archive: export RUSTY_V8_SRC_BINDING_PATH := $(V8_BINDING)
check-fixtures $(SSR_TARGETS) $(addprefix tool-test-,$(TOOL_TESTS)) $(addprefix nextest-,$(CRATES)) ci-nextest bench verify-build: export CARGO_INCREMENTAL := 0
check-fixtures $(SSR_TARGETS) $(addprefix tool-test-,$(TOOL_TESTS)) $(addprefix nextest-,$(CRATES)) ci-nextest bench verify-build: export CARGO_BUILD_JOBS := $(BUILD_JOBS)
check-fixtures $(SSR_TARGETS) $(addprefix tool-test-,$(TOOL_TESTS)) $(addprefix nextest-,$(CRATES)) ci-nextest bench verify-build: export CARGO_NET_OFFLINE := true

verify-archive:
	python3 tools/verify_archive.py

check-fixtures:
	python3 -m tools.install_packages

# check-tools runs the tool tests of tools/test_*.py: every module, or the files of TESTS, which make owner-check passes
# for the changed test files (config/owner-checks.json). Each module has the target tool-test-<name>, and each crate the
# target nextest-<crate>, which the owner rules of config/owner-checks.json name for the files they own.
check-tools:
	python3 tools/run_tests.py $(TESTS)

$(foreach test,$(TOOL_TESTS),$(eval tool-test-$(test): ; python3 tools/run_tests.py tools/test_$(test).py))
$(foreach crate,$(CRATES),$(eval nextest-$(crate): verify-archive ; cargo nextest run -p polyspec-ssr-$(crate) --locked --no-tests fail))
.PHONY: $(addprefix tool-test-,$(TOOL_TESTS)) $(addprefix nextest-,$(CRATES))

# tool-versions-check compares every tool of tools/tool-versions.json with its declaration.
tool-versions-check:
	python3 -m tools.tool_versions check

check-ownership:
	python3 tools/test_ownership.py

check-records:
	python3 tools/check.py

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
	python3 -m tools.tool_versions check
	cargo deny check advisories
	cargo deny --manifest-path verification/engine/Cargo.toml --config deny.toml check --hide-inclusion-graph advisories
	cargo deny --manifest-path tools/build-probe/Cargo.toml --config deny.toml --locked check --hide-inclusion-graph advisories

check-engine-deps: verify-engine-deps

check-examples:
	cargo build -p polyspec-ssr-server --example development_process --example socket_process --locked

check-nextest:
	cargo nextest run --workspace --locked --no-tests fail

check-features:
	python3 tools/check_features.py

check-bench:
	cargo nextest run -p polyspec-ssr-runtime --example bench --features bench --locked --no-tests fail

bench:
	python3 -m tools.tool_versions check
	python3 tools/bench.py

verify-engine-deps:
	cargo deny --manifest-path verification/engine/Cargo.toml --config deny.toml check --hide-inclusion-graph $(DENY_CHECKS)

# Every check of the build verification runs, also after a failed one; the recipe names each
# failed check and fails after the last one.
verify-build:
	python3 -m tools.tool_versions check
	status=0; \
	assignments=$$(python3 -m tools.install_packages) && eval "$$assignments" && export SSR_PACKAGES SSR_FIXTURES SSR_BROWSER || { status=1; echo 'FAIL verify-build: install packages'; }; \
	cargo fmt --manifest-path tools/build-probe/Cargo.toml -- --check || { status=1; echo 'FAIL verify-build: cargo fmt'; }; \
	CARGO_TARGET_DIR=$(CURDIR)/target cargo clippy --manifest-path tools/build-probe/Cargo.toml --all-targets --locked -- -D warnings || { status=1; echo 'FAIL verify-build: cargo clippy'; }; \
	CARGO_TARGET_DIR=$(CURDIR)/target cargo nextest run --manifest-path tools/build-probe/Cargo.toml --test build_verification --locked --no-tests fail || { status=1; echo 'FAIL verify-build: cargo nextest run'; }; \
	cargo deny --manifest-path tools/build-probe/Cargo.toml --config deny.toml --locked check --hide-inclusion-graph $(DENY_CHECKS) || { status=1; echo 'FAIL verify-build: cargo deny'; }; \
	exit $$status

# GitHub CI (.github/workflows/ci.yml and push-gate.yml) runs only make targets. A job installs the npm of packageManager
# (make install-tools TOOLS=npm), the packages of package-lock.json with @polyspec/kit (make install-packages) and the
# tools of config/toolchain.json (make install-tools). ci-setup then prepares a fresh Linux runner: the Rust toolchain of
# rust-toolchain.toml, the release binaries of the test tools and the official V8 inputs of the host target, each verified
# by its SHA-256 (tools/fetch_tools.py, tools/fetch_v8.py), and the locked dependencies. On each architecture ci-lint and
# ci-test, two jobs of ci.yml, check the versions of the tools and run the lint and the test targets through make
# ci-targets of kit, which runs every target past failures and writes var/report/ci-targets with a log of each target and
# summary.md, the job summary; Cargo builds with one job per processor of the runner (BUILD_JOBS).
CI_LINT_TARGETS = ci-records check-fmt check-clippy check-deny
CI_TEST_TARGETS = check-examples ci-nextest
CI_PUSH_TARGETS = toolchain-check ci-python push-gate-commit push-records

ci-setup:
	rustup toolchain install --no-self-update
	python3 -m tools.fetch_tools "$$(RUSTUP_AUTO_INSTALL=0 rustc -vV | sed -n 's/^host: //p')"
	python3 -m tools.fetch_v8 "$$(RUSTUP_AUTO_INSTALL=0 rustc -vV | sed -n 's/^host: //p')"
	cargo fetch --locked

ci-lint:
	$(MAKE) --no-print-directory toolchain-check
	python3 -m tools.tool_versions check python3 cargo-nextest cargo-deny
	BUILD_JOBS=$$(nproc) $(MAKE) --no-print-directory ci-targets TARGETS="$(CI_LINT_TARGETS)"

ci-test:
	$(MAKE) --no-print-directory toolchain-check
	python3 -m tools.tool_versions check python3 cargo-nextest cargo-deny
	BUILD_JOBS=$$(nproc) $(MAKE) --no-print-directory ci-targets TARGETS="$(CI_TEST_TARGETS)"

ci-nextest:
	cargo nextest run --workspace --locked --no-tests fail

ci-python:
	python3 -m tools.tool_versions check python3

ci-push:
	$(MAKE) --no-print-directory ci-targets TARGETS="$(CI_PUSH_TARGETS)"

# The record checks of tools/check.py, which read only tracked files, in the job push-gate: the ruleset requires that
# job on main, so a commit whose records fail never reaches main. ci-records runs every check of tools/check.py in CI.
push-records:
	python3 tools/check.py records

ci-records:
	python3 tools/check.py

# The GitHub ruleset of main and the merge settings, declared in .github/ruleset.json (tools/github_ruleset.py,
# docs/workspace.md): every change reaches main through a pull request and the merge queue, which requires the checks
# push-gate and ci-passed. github-ruleset creates or updates the ruleset of the declared name and the
# settings; github-ruleset-check fails when the live ruleset or a setting differs from the declaration.
github-ruleset:
	python3 -m tools.github_ruleset apply

github-ruleset-check:
	python3 -m tools.github_ruleset check

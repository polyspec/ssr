# rustup never installs a missing toolchain or component: rust-toolchain.toml names the toolchain,
# and a missing one fails the command instead of being downloaded.
export RUSTUP_AUTO_INSTALL := 0
V8_TARGET := $(shell RUSTUP_AUTO_INSTALL=0 rustc -vV | sed -n 's/^host: //p')
V8_ARCHIVE := $(CURDIR)/var/v8/librusty_v8_simdutf_release_$(V8_TARGET).a.gz
V8_BINDING := $(CURDIR)/var/v8/src_binding_simdutf_release_$(V8_TARGET).rs

# Every make invocation sets core.hooksPath to .githooks when it differs, so the tracked pre-push hook
# .githooks/pre-push runs in every checkout where make runs: it refuses a push while a checklist item
# is in progress (tools/push_gate.py). make hooks sets the path and checks it; tools/check.py and the
# guard of make check fail while it is not set.
HOOKS_PATH := $(shell test "$$(git config core.hooksPath)" = .githooks || git config core.hooksPath .githooks; git config core.hooksPath)

# make check runs the full suite: the setup steps, then every target, each as its own make target
# through tools/full_run.py. The guard refuses the run before any step while a checklist item is in
# progress, while tracked files have uncommitted changes, while untracked files that are not ignored
# exist, and when var/full-run.json records a full run of the same tree; make rerun-failed runs
# only the targets of that record that did not pass.
# The setup steps install the build-probe fixture packages that the targets read, so they run
# before the targets of both entries. Both entries run under the checkout lock check
# (tools/holder_lock.py): a second run of the checkout is refused with the holder's checkout, pid
# and process start time.
CHECK_SETUP = verify-archive check-fixtures
CHECK_TARGETS = check-tools check-ownership check-records check-fmt check-clippy check-deny check-engine-deps check-examples check-nextest check-features check-bench check-linux
FULL_RUN = python3 -m tools.holder_lock run check -- python3 -m tools.full_run

# A make without a target runs the full suite entry, so no command runs its steps without the guard.
.DEFAULT_GOAL := check
.PHONY: check rerun-failed review-advisories hooks hooks-check $(CHECK_SETUP) $(CHECK_TARGETS) bench verify-archive verify-engine-linux-arm64 verify-engine-down verify-engine-deps verify-build

$(CHECK_SETUP) $(CHECK_TARGETS) bench verify-archive: export RUSTY_V8_ARCHIVE := $(V8_ARCHIVE)
$(CHECK_SETUP) $(CHECK_TARGETS) bench verify-archive: export RUSTY_V8_SRC_BINDING_PATH := $(V8_BINDING)
$(CHECK_SETUP) $(CHECK_TARGETS) bench verify-build: export CARGO_INCREMENTAL := 0
$(CHECK_SETUP) $(CHECK_TARGETS) bench verify-build: export CARGO_BUILD_JOBS := 1
$(CHECK_SETUP) $(CHECK_TARGETS) bench verify-build: export CARGO_NET_OFFLINE := true
bench: verify-archive

verify-archive:
	python3 tools/verify_archive.py

hooks:
	git config core.hooksPath .githooks
	python3 -m tools.push_gate hooks-check

hooks-check:
	python3 -m tools.push_gate hooks-check

check:
	$(FULL_RUN) check --setup $(CHECK_SETUP) --targets $(CHECK_TARGETS)

rerun-failed:
	$(FULL_RUN) rerun-failed --setup $(CHECK_SETUP)

check-fixtures:
	npm ci --prefix tools/build-probe/tests/fixtures --install-links --ignore-scripts --no-audit --no-fund

check-tools:
	python3 tools/run_tests.py

check-ownership:
	python3 tools/test_ownership.py

check-records:
	python3 tools/check.py

check-fmt:
	cargo fmt --all -- --check

check-clippy:
	cargo clippy --workspace --all-targets --features ssr-runtime/bench --locked -- -D warnings

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
	cargo build -p ssr-server --example development_process --example socket_process --locked

check-nextest:
	cargo nextest run --workspace --locked --no-tests fail

check-features:
	python3 tools/check_features.py

check-linux:
	python3 -m tools.check_linux

check-bench:
	cargo nextest run -p ssr-runtime --example bench --features bench --locked --no-tests fail

bench:
	python3 -m tools.tool_versions check
	python3 tools/bench.py

verify-engine-linux-arm64:
	python3 -m tools.tool_versions check
	python3 -m tools.verify_engine_linux verify

verify-engine-down:
	python3 -m tools.verify_engine_linux down

verify-engine-deps:
	cargo deny --manifest-path verification/engine/Cargo.toml --config deny.toml check --hide-inclusion-graph $(DENY_CHECKS)

verify-build:
	python3 -m tools.tool_versions check
	npm ci --prefix tools/build-probe/tests/fixtures --install-links --ignore-scripts --no-audit --no-fund
	cargo fmt --manifest-path tools/build-probe/Cargo.toml -- --check
	CARGO_TARGET_DIR=$(CURDIR)/target cargo clippy --manifest-path tools/build-probe/Cargo.toml --all-targets --locked -- -D warnings
	CARGO_TARGET_DIR=$(CURDIR)/target cargo nextest run --manifest-path tools/build-probe/Cargo.toml --test build_verification --locked --no-tests fail
	cargo deny --manifest-path tools/build-probe/Cargo.toml --config deny.toml --locked check --hide-inclusion-graph $(DENY_CHECKS)

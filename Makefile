.PHONY: check check-steps bench verify-archive verify-engine-linux-arm64 verify-engine-down verify-engine-deps verify-build

V8_TARGET := $(shell rustc -vV | sed -n 's/^host: //p')
V8_ARCHIVE := $(CURDIR)/var/v8/librusty_v8_simdutf_release_$(V8_TARGET).a.gz
V8_BINDING := $(CURDIR)/var/v8/src_binding_simdutf_release_$(V8_TARGET).rs

check-steps bench verify-archive: export RUSTY_V8_ARCHIVE := $(V8_ARCHIVE)
check-steps bench verify-archive: export RUSTY_V8_SRC_BINDING_PATH := $(V8_BINDING)
check-steps bench verify-build: export CARGO_INCREMENTAL := 0
check-steps bench verify-build: export CARGO_BUILD_JOBS := 1
check-steps bench verify-build: export CARGO_NET_OFFLINE := true
check-steps bench: verify-archive

verify-archive:
	python3 tools/verify_archive.py

# make check installs the build-probe fixture that its tests read, so its steps run under the
# checkout lock check (tools/holder_lock.py): a second make check of the checkout is refused with
# the holder's checkout, pid and process start time.
check:
	python3 -m tools.holder_lock run check -- $(MAKE) check-steps

check-steps:
	npm ci --prefix tools/build-probe/tests/fixtures --install-links --ignore-scripts --no-audit --no-fund
	python3 tools/run_tests.py
	python3 tools/test_ownership.py
	python3 tools/check.py
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --features ssr-runtime/bench --locked -- -D warnings
	cargo deny check
	$(MAKE) verify-engine-deps
	cargo build -p ssr-server --example development_process --example socket_process --locked
	cargo nextest run --workspace --locked --no-tests fail
	python3 tools/check_features.py
	cargo nextest run -p ssr-runtime --example bench --features bench --locked --no-tests fail

bench:
	python3 tools/bench.py

verify-engine-linux-arm64:
	python3 -m tools.verify_engine_linux verify

verify-engine-down:
	python3 -m tools.verify_engine_linux down

verify-engine-deps:
	cargo deny --manifest-path verification/engine/Cargo.toml --config deny.toml check --hide-inclusion-graph

verify-build:
	npm ci --prefix tools/build-probe/tests/fixtures --install-links --ignore-scripts --no-audit --no-fund
	cargo fmt --manifest-path tools/build-probe/Cargo.toml -- --check
	CARGO_TARGET_DIR=$(CURDIR)/target cargo clippy --manifest-path tools/build-probe/Cargo.toml --all-targets --locked -- -D warnings
	CARGO_TARGET_DIR=$(CURDIR)/target cargo nextest run --manifest-path tools/build-probe/Cargo.toml --config-file $(CURDIR)/.config/nextest.toml --test build_verification --locked --no-tests fail
	cargo deny --manifest-path tools/build-probe/Cargo.toml --config deny.toml --locked check --hide-inclusion-graph

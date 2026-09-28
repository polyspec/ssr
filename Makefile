.PHONY: check bench verify-archive verify-engine-linux-arm64 verify-engine-deps verify-build

V8_TARGET := $(shell rustc -vV | sed -n 's/^host: //p')
V8_ARCHIVE := $(CURDIR)/var/v8/librusty_v8_simdutf_release_$(V8_TARGET).a.gz
V8_BINDING := $(CURDIR)/var/v8/src_binding_simdutf_release_$(V8_TARGET).rs

check bench verify-archive: export RUSTY_V8_ARCHIVE := $(V8_ARCHIVE)
check bench verify-archive: export RUSTY_V8_SRC_BINDING_PATH := $(V8_BINDING)
check bench verify-build: export CARGO_INCREMENTAL := 0
check bench verify-build: export CARGO_BUILD_JOBS := 1
check bench verify-build: export CARGO_NET_OFFLINE := true
check bench: verify-archive

verify-archive:
	python3 tools/verify_archive.py

check:
	npm ci --prefix tools/build-probe/tests/fixtures --install-links --ignore-scripts --no-audit --no-fund
	python3 tools/prepare_crudui_fixture.py
	python3 tools/run_tests.py
	python3 tools/test_ownership.py
	python3 tools/check.py
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --locked -- -D warnings
	cargo deny check
	$(MAKE) verify-engine-deps
	cargo build -p ssr-server --example development_process --locked
	cargo nextest run --workspace --locked --no-tests fail
	python3 tools/check_features.py
	cargo nextest run -p ssr-runtime --example bench --locked --no-tests fail

bench:
	python3 tools/bench.py

verify-engine-linux-arm64:
	python3 tools/verify_archive.py aarch64-unknown-linux-gnu
	python3 tools/prepare_engine_compose.py
	container build --platform linux/arm64 -f verification/engine/linux/Dockerfile -t localhost/soksakim-test:0.0.1 verification/engine/linux
	containerctl -f $(CURDIR)/var/engine-compose.yaml up
	python3 tools/verify_engine_status.py
	container exec -w /src soksakim-test-engine python3 /src/tools/verify_engine_mounts.py
	container exec -w /src soksakim-test-engine python3 /src/tools/verify_engine.py aarch64-unknown-linux-gnu

verify-engine-deps:
	cargo deny --manifest-path verification/engine/Cargo.toml --config deny.toml check --hide-inclusion-graph

verify-build:
	npm ci --prefix tools/build-probe/tests/fixtures --install-links --ignore-scripts --no-audit --no-fund
	cargo fmt --manifest-path tools/build-probe/Cargo.toml -- --check
	CARGO_TARGET_DIR=$(CURDIR)/target cargo clippy --manifest-path tools/build-probe/Cargo.toml --all-targets --locked -- -D warnings
	CARGO_TARGET_DIR=$(CURDIR)/target cargo nextest run --manifest-path tools/build-probe/Cargo.toml --config-file $(CURDIR)/.config/nextest.toml --test build_verification --locked --no-tests fail
	cargo deny --manifest-path tools/build-probe/Cargo.toml --config deny.toml --locked check --hide-inclusion-graph

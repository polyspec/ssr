.PHONY: check bench verify-engine-linux-arm64 verify-build

check:
	python3 tools/run_tests.py
	python3 tools/check.py
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --locked -- -D warnings
	cargo deny check
	cargo nextest run --workspace --locked --no-tests fail

bench:
	cargo bench --workspace --locked

verify-engine-linux-arm64:
	python3 tools/verify_engine.py aarch64-unknown-linux-gnu --fetch-only
	python3 tools/prepare_engine_compose.py
	container build --platform linux/arm64 -f verification/engine/linux/Dockerfile -t localhost/soksakim-test:0.0.1 verification/engine/linux
	containerctl -f $(CURDIR)/var/engine-compose.yaml up
	python3 tools/verify_engine_status.py
	container exec -w /src soksakim-test-engine python3 /src/tools/verify_engine_mounts.py
	container exec -w /src soksakim-test-engine python3 /src/tools/verify_engine.py aarch64-unknown-linux-gnu

verify-build:
	npm ci --prefix tools/build-probe/tests/fixtures --ignore-scripts --no-audit --no-fund
	cargo fmt --manifest-path tools/build-probe/Cargo.toml -- --check
	CARGO_TARGET_DIR=$(CURDIR)/target cargo clippy --manifest-path tools/build-probe/Cargo.toml --all-targets --locked -- -D warnings
	CARGO_TARGET_DIR=$(CURDIR)/target cargo nextest run --manifest-path tools/build-probe/Cargo.toml --config-file $(CURDIR)/.config/nextest.toml --test build_verification --locked --no-tests fail
	cargo deny --manifest-path tools/build-probe/Cargo.toml --config deny.toml --locked check --hide-inclusion-graph

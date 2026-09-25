.PHONY: check bench verify-engine-linux-arm64

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
	container build --platform linux/arm64 -f verification/engine/linux/Dockerfile -t localhost/ssr-engine-test:0.0.1 verification/engine/linux
	SSR_SOURCE_DIR=$(CURDIR) containerctl -f verification/engine/linux/compose.yaml up
	container exec -w /src ssr-engine-test-engine python3 /src/tools/verify_engine.py aarch64-unknown-linux-gnu

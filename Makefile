.PHONY: check bench

check:
	python3 tools/run_tests.py
	python3 tools/check.py
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --locked -- -D warnings
	cargo deny check
	cargo nextest run --workspace --locked --no-tests fail

bench:
	cargo bench --workspace --locked

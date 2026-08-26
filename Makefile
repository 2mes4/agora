.PHONY: build test lint fmt check run-example run-server clean

build:
	cargo build --workspace

test:
	cargo test --workspace

lint:
	cargo clippy --workspace --all-targets -- -D warnings

fmt:
	cargo fmt --all

check: fmt lint test
	@echo "All checks passed."

run-example:
	cargo run -p direct-delegate

run-server:
	cargo run -p agora-server -- --demo-agent

clean:
	cargo clean

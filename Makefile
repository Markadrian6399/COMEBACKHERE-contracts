.PHONY: build test fmt lint check

build:
	cargo build

test:
	cargo test

fmt:
	cargo fmt --all

lint:
	cargo clippy -- -D warnings

check: fmt lint test
	@echo "✓ All checks passed"

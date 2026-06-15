# COMEBACKHERE-contracts task runner

# Compile all contracts
build:
    cargo build

# Run all tests
test:
    cargo test

# Format all code
fmt:
    cargo fmt --all

# Run clippy lints
lint:
    cargo clippy -- -D warnings

# Check dependencies for vulnerabilities and license issues
deny:
    cargo deny check

# Run format and lint checks (for CI)
check: fmt lint test deny
    @echo "✓ All checks passed"

# Default target
default: check

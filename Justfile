# FAMP — task runner
# Run `just` with no args to see available recipes.

default:
    @just --list

# Build the entire workspace with all targets
build:
    cargo build --workspace --all-targets

# Run all tests via cargo-nextest (unit + integration)
test:
    cargo nextest run --workspace

# Run doc tests (nextest does not run doctests)
test-doc:
    cargo test --workspace --doc

# Run clippy with workspace-strict settings and deny warnings
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Format all sources
fmt:
    cargo fmt --all

# Check formatting without modifying (CI gate)
fmt-check:
    cargo fmt --all -- --check

# Run `cargo audit` for RustSec advisories
audit:
    cargo audit

# Full local CI-parity gate. A green `just ci` implies a green GitHub Actions run.
ci: fmt-check lint build test test-doc
    @echo "✓ local CI-parity checks passed"

# Clean build artifacts
clean:
    cargo clean

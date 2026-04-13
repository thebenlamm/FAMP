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

# Run famp-canonical test suite only (fast feedback loop)
test-canonical:
    cargo nextest run -p famp-canonical

# Run famp-canonical with strict no-fail-fast (RFC 8785 conformance gate; CI per-PR)
test-canonical-strict:
    cargo nextest run -p famp-canonical --no-fail-fast

# Run famp-canonical with the 100M float corpus (nightly / release tags only — D-12)
test-canonical-full:
    cargo nextest run -p famp-canonical --features full-corpus --no-fail-fast

# Run famp-crypto test suite as a blocking gate (RFC 8032 + §7.1c worked example)
test-crypto:
    cargo nextest run -p famp-crypto
    cargo test -p famp-crypto --doc

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

# Run the FAMP v0.5.1 spec anchor lint (ripgrep-based; see scripts/spec-lint.sh).
spec-lint:
    bash scripts/spec-lint.sh

# Full local CI-parity gate. A green `just ci` implies a green GitHub Actions run.
ci: fmt-check lint build test-canonical-strict test-crypto test test-doc spec-lint
    @echo "✓ local CI-parity checks passed"

# Clean build artifacts
clean:
    cargo clean

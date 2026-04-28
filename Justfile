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

# Run famp-core test suite as a blocking gate (wire-string fixtures + exhaustive-match gate)
test-core:
    cargo nextest run -p famp-core
    cargo test -p famp-core --doc

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

# Install the repo-local git hooks (fast pre-commit fmt-check, mirrors CI).
# One-time per clone. Bypass with `git commit --no-verify` if needed.
install-hooks:
    git config core.hooksPath .githooks
    @echo "✓ git hooks installed (.githooks/) — pre-commit will run cargo fmt --check"

# Run `cargo audit` for RustSec advisories
audit:
    cargo audit

# Run the FAMP v0.5.1 spec anchor lint (ripgrep-based; see scripts/spec-lint.sh).
spec-lint:
    bash scripts/spec-lint.sh

# BUS-01: assert famp-bus does not pull tokio into its runtime dep tree.
check-no-tokio-in-bus:
    @echo "Verifying famp-bus has no tokio in dependency tree..."
    @command -v cargo >/dev/null || { echo "ERROR: cargo not found in PATH"; exit 1; }
    @tree="$$(cargo tree -p famp-bus --edges normal)" || exit 1; \
    if printf '%s\n' "$$tree" | grep -E '^\s*tokio v'; then \
      echo "ERROR: famp-bus has tokio in its dependency tree (BUS-01 violation)"; \
      exit 1; \
    fi
    @echo "OK - famp-bus is tokio-free."

# MCP-01 (D-11 source-import grep): assert MCP/bus/broker source has no
# `use reqwest` or `use rustls` imports. Cheap structural gate that ships
# today; cargo-tree-strict reading is deferred to Phase 4 when the
# federation CLI surfaces are deleted.
check-mcp-deps:
    bash scripts/check-mcp-deps.sh

# AUDIT-05: prevent split-commit between FAMP_SPEC_VERSION bump and impl.
check-spec-version-coherence:
    @if grep -q 'pub const FAMP_SPEC_VERSION: &str = "0.5.2"' crates/famp-envelope/src/version.rs; then \
      grep -q 'AuditLog' crates/famp-core/src/class.rs || (echo "spec version 0.5.2 declared but MessageClass::AuditLog missing" && exit 1); \
      grep -q 'AuditLogBody' crates/famp-envelope/src/body/mod.rs || (echo "spec version 0.5.2 declared but AuditLogBody missing" && exit 1); \
    fi

# Full local CI-parity gate. A green `just ci` implies a green GitHub Actions run.
ci: fmt-check lint build test-canonical-strict test-crypto test test-doc spec-lint check-no-tokio-in-bus check-spec-version-coherence check-mcp-deps
    @echo "✓ local CI-parity checks passed"

# Start two famp daemons in the background for the Phase 4 E2E-02
# witnessed smoke test. Prints the .mcp.json snippet each Claude Code
# session should paste.
e2e-smoke:
    #!/usr/bin/env bash
    set -euo pipefail
    SMOKE_A=/tmp/famp-smoke-a
    SMOKE_B=/tmp/famp-smoke-b
    rm -rf "$SMOKE_A" "$SMOKE_B"
    mkdir -p "$SMOKE_A" "$SMOKE_B"
    FAMP_HOME="$SMOKE_A" cargo run --release -q -p famp -- init
    FAMP_HOME="$SMOKE_B" cargo run --release -q -p famp -- init
    # (Users configure mutual peer_add using their preferred flow;
    # the checklist in 04-E2E-SMOKE.md walks through it.)
    FAMP_HOME="$SMOKE_A" cargo run --release -q -p famp -- listen --listen 127.0.0.1:18443 &
    A_PID=$!
    FAMP_HOME="$SMOKE_B" cargo run --release -q -p famp -- listen --listen 127.0.0.1:18444 &
    B_PID=$!
    echo "Daemon A pid=$A_PID home=$SMOKE_A"
    echo "Daemon B pid=$B_PID home=$SMOKE_B"
    echo ""
    echo "=== Paste into Claude Code session 1 (.mcp.json) ==="
    printf '{\n  "mcpServers": {\n    "famp-alice": {\n      "command": "cargo",\n      "args": ["run", "--release", "-q", "-p", "famp", "--", "mcp"],\n      "env": { "FAMP_HOME": "%s" }\n    }\n  }\n}\n' "$SMOKE_A"
    echo ""
    echo "=== Paste into Claude Code session 2 (.mcp.json) ==="
    printf '{\n  "mcpServers": {\n    "famp-bob": {\n      "command": "cargo",\n      "args": ["run", "--release", "-q", "-p", "famp", "--", "mcp"],\n      "env": { "FAMP_HOME": "%s" }\n    }\n  }\n}\n' "$SMOKE_B"
    echo ""
    echo "To stop: kill $A_PID $B_PID"
    wait $A_PID $B_PID

# Clean build artifacts
clean:
    cargo clean

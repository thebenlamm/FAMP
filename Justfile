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

# Publish all 12 workspace crates to crates.io in dependency order (D-10).
# 45s sleep between publishes covers crates.io index-update lag (Pitfall 6).
# Requires `cargo login` first; manual gate, not run from CI.
publish-workspace:
    cargo publish -p famp-canonical
    sleep 45
    cargo publish -p famp-core
    sleep 45
    cargo publish -p famp-taskdir
    sleep 45
    cargo publish -p famp-inbox
    sleep 45
    cargo publish -p famp-crypto
    sleep 45
    cargo publish -p famp-fsm
    sleep 45
    cargo publish -p famp-transport
    sleep 45
    cargo publish -p famp-keyring
    sleep 45
    cargo publish -p famp-envelope
    sleep 45
    cargo publish -p famp-bus
    sleep 45
    cargo publish -p famp-transport-http
    sleep 45
    cargo publish -p famp
    @echo "✓ all 12 crates published — verify at https://crates.io/crates/famp"

# Dry-run all 12 in dependency order. Catches Cargo.toml-publishability issues
# (path-deps without version, missing description, etc. — Pitfall 5).
publish-workspace-dry-run:
    cargo publish -p famp-canonical --dry-run
    cargo publish -p famp-core --dry-run
    cargo publish -p famp-taskdir --dry-run
    cargo publish -p famp-inbox --dry-run
    # Dependent crates cannot `cargo publish --dry-run` until their internal deps
    # are live in the crates.io index. Pre-publish CI validates their package
    # manifests and file lists; the real `publish-workspace` remains ordered.
    cargo package -p famp-crypto --allow-dirty --no-verify --list > /dev/null
    cargo package -p famp-fsm --allow-dirty --no-verify --list > /dev/null
    cargo package -p famp-transport --allow-dirty --no-verify --list > /dev/null
    cargo package -p famp-keyring --allow-dirty --no-verify --list > /dev/null
    cargo package -p famp-envelope --allow-dirty --no-verify --list > /dev/null
    cargo package -p famp-bus --allow-dirty --no-verify --list > /dev/null
    cargo package -p famp-transport-http --allow-dirty --no-verify --list > /dev/null
    cargo package -p famp --allow-dirty --no-verify --list > /dev/null

# Shellcheck the hook-runner asset (D-08 invariant: shellcheck-clean).
# Recipe colocated with the asset (`crates/famp/assets/hook-runner.sh`) — both ship in plan 03-02.
check-shellcheck:
    shellcheck crates/famp/assets/hook-runner.sh

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
ci: fmt-check lint build test-canonical-strict test-crypto test test-doc spec-lint check-no-tokio-in-bus check-spec-version-coherence check-mcp-deps check-shellcheck publish-workspace-dry-run
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

# Verify the Quick Start install path: `cargo install --path crates/famp` produces
# a working binary. Isolated to /tmp/famp-smoke so the user's ~/.cargo/bin is untouched;
# the cargo registry cache (~/.cargo/registry) is still reused for speed.
smoke-test:
    #!/usr/bin/env bash
    set -euo pipefail
    SMOKE_ROOT=/tmp/famp-smoke
    SMOKE_HOME=$(mktemp -d)
    rm -rf "$SMOKE_ROOT"
    trap 'echo "--- cleaning up ---"; rm -rf "$SMOKE_ROOT" "$SMOKE_HOME"' EXIT
    echo "--- installing famp to $SMOKE_ROOT ---"
    cargo install --path crates/famp --root "$SMOKE_ROOT"
    echo "--- verifying install-claude-code writes all artifacts ---"
    FAMP_INSTALL_TARGET_HOME="$SMOKE_HOME" "$SMOKE_ROOT/bin/famp" install-claude-code
    test -f "$SMOKE_HOME/.claude.json"
    test -d "$SMOKE_HOME/.claude/commands"
    test -f "$SMOKE_HOME/.famp/hook-runner.sh"
    test -f "$SMOKE_HOME/.claude/settings.json"
    test -f "$SMOKE_HOME/.claude/hooks/famp-await.sh"
    echo "--- all 5 install-claude-code artifacts verified ---"
    echo "--- smoke-test PASSED ---"

# Install famp to ~/.cargo/bin — the path every .mcp.json references.
# Run this after any change to the MCP tool surface (server.rs, tool schemas).
install:
    cargo install --path crates/famp --locked --force

# Clean build artifacts
clean:
    cargo clean

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

# Install the repo-local git hooks (mirrors CI). One-time per clone.
# pre-commit: cargo fmt --check  (fast, every commit)
# pre-push:   cargo clippy -D warnings  (CI-parity, on Rust file changes)
# Bypass with --no-verify only if you have a real reason.
install-hooks:
    git config core.hooksPath .githooks
    @echo "✓ git hooks installed (.githooks/)"
    @echo "  pre-commit: cargo fmt --check"
    @echo "  pre-push:   cargo clippy --workspace --all-targets -- -D warnings"

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

# Shellcheck the hook assets (D-08 invariant: shellcheck-clean) and the
# dist-generated installers (DIST-03 invariant: shellcheck-clean).
# `hook-runner.sh` ships in plan 03-02; `famp-await.sh` is the listen-mode
# Stop hook source of truth (issue #21 cancellation seam lives here).
check-shellcheck:
    shellcheck crates/famp/assets/hook-runner.sh
    shellcheck crates/famp/assets/famp-await.sh
    shellcheck crates/famp/tests/fixtures/installers/famp-installer.sh
    shellcheck crates/famp/tests/fixtures/installers/famp-gateway-installer.sh
    shellcheck crates/famp/tests/fixtures/installers/famp-relay-installer.sh

# T-16-06: regenerate every dist-derived file (release.yml + the three
# installer fixtures) from dist-workspace.toml and assert no drift. Requires
# `dist` on PATH — never no-ops silently if it's missing (T-16-07). NOT a
# member of `just ci` (see the `ci:` recipe below): dist is a CI/release
# tool, not a baseline local dependency, and this recipe mutates the working
# tree (release.yml + installer fixtures) as part of its check. Wired into
# CI instead via .github/workflows/release-gate.yml.
check-installer-drift:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v dist >/dev/null 2>&1; then
        echo "ERROR: dist not found on PATH. Install with: cargo install cargo-dist --version 0.32.0 --locked" >&2
        exit 1
    fi
    echo "-- dist generate --check (native pre-check against dist-workspace.toml) --"
    dist generate --check
    echo "-- dist generate (regenerate .github/workflows/release.yml) --"
    dist generate
    echo "-- dist build --artifacts=global --tag=v1.0.0 (regenerate installer fixtures) --"
    dist build --artifacts=global --tag=v1.0.0 --output-format=json > /tmp/famp-dist-build-drift.json
    cp target/distrib/famp-installer.sh crates/famp/tests/fixtures/installers/famp-installer.sh
    cp target/distrib/famp-gateway-installer.sh crates/famp/tests/fixtures/installers/famp-gateway-installer.sh
    cp target/distrib/famp-relay-installer.sh crates/famp/tests/fixtures/installers/famp-relay-installer.sh
    echo "-- asserting no drift against the committed tree --"
    git diff --exit-code -- dist-workspace.toml .github/workflows/release.yml crates/famp/tests/fixtures/installers
    echo "✓ no drift: release.yml and installer fixtures match dist-workspace.toml"

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

# INSP-CRATE-01: assert famp-inspect-proto has no I/O deps.
check-no-io-in-inspect-proto:
    @echo "Verifying famp-inspect-proto is I/O-free..."
    @command -v cargo >/dev/null || { echo "ERROR: cargo not found in PATH"; exit 1; }
    @tree="$(cargo tree -p famp-inspect-proto --edges normal)" || exit 1; \
    for dep in tokio axum reqwest clap; do \
      if printf '%s\n' "$tree" | grep -E "(^|[[:space:]├└─]+)${dep} v[0-9]"; then \
        echo "ERROR: famp-inspect-proto depends on ${dep} (INSP-CRATE-01 violation)"; \
        exit 1; \
      fi; \
    done
    @echo "OK - famp-inspect-proto is I/O-free."

# INSP-RPC-02: assert famp-inspect-server imports no write surfaces.
check-inspect-readonly:
    @echo "Verifying famp-inspect-server is read-only..."
    @command -v cargo >/dev/null || { echo "ERROR: cargo not found in PATH"; exit 1; }
    @tree="$(cargo tree -p famp-inspect-server --edges normal)" || exit 1; \
    if printf '%s\n' "$tree" | grep -E '(^|[[:space:]├└─]+)famp-taskdir v[0-9]'; then \
      echo "ERROR: famp-inspect-server depends on famp-taskdir (INSP-RPC-02 violation: taskdir = write-mostly)"; \
      exit 1; \
    fi
    @echo "Checking source for forbidden write-surface imports..."
    @if grep -rE 'famp_inbox::(append|cursor::InboxCursor::advance)|Inbox::open|::write_all|fs::write' crates/famp-inspect-server/src/ 2>/dev/null; then \
      echo "ERROR: famp-inspect-server source imports a write surface (INSP-RPC-02 violation)"; \
      exit 1; \
    fi
    @if grep -rE '&mut\s+BrokerState' crates/famp-inspect-server/src/ 2>/dev/null; then \
      echo "ERROR: famp-inspect-server has &mut BrokerState (INSP-RPC-02 violation)"; \
      exit 1; \
    fi
    @echo "OK - famp-inspect-server is read-only."

# INSP-CRATE-03: assert inspector/broker decode dependency versions align.
check-inspect-version-aligned:
    @echo "Verifying inspector/broker version alignment..."
    @command -v cargo >/dev/null || { echo "ERROR: cargo not found in PATH"; exit 1; }
    @for crate in famp-canonical famp-envelope famp-fsm; do \
      server_ver=$(cargo tree -p famp-inspect-server 2>/dev/null | grep -E "^[├└]── ${crate} v" | head -1 | grep -oE 'v[0-9]+\.[0-9]+\.[0-9]+'); \
      if [ -z "$server_ver" ]; then \
        server_ver=$(cargo tree -p famp-inspect-server 2>/dev/null | grep -E "${crate} v" | head -1 | grep -oE 'v[0-9]+\.[0-9]+\.[0-9]+'); \
      fi; \
      bus_ver=$(cargo tree -p famp-bus 2>/dev/null | grep -E "^[├└]── ${crate} v" | head -1 | grep -oE 'v[0-9]+\.[0-9]+\.[0-9]+'); \
      if [ -z "$bus_ver" ]; then \
        bus_ver=$(cargo tree -p famp-bus 2>/dev/null | grep -E "${crate} v" | head -1 | grep -oE 'v[0-9]+\.[0-9]+\.[0-9]+'); \
      fi; \
      if [ -z "$bus_ver" ]; then \
        bus_ver=$(cargo tree -p famp 2>/dev/null | grep -E "${crate} v" | head -1 | grep -oE 'v[0-9]+\.[0-9]+\.[0-9]+'); \
      fi; \
      if [ -z "$server_ver" ] || [ -z "$bus_ver" ]; then \
        echo "ERROR: could not resolve $crate version (server=$server_ver bus=$bus_ver)"; \
        exit 1; \
      fi; \
      if [ "$server_ver" != "$bus_ver" ]; then \
        echo "ERROR: $crate version mismatch: famp-inspect-server=$server_ver famp-bus=$bus_ver (INSP-CRATE-03 violation)"; \
        exit 1; \
      fi; \
      echo "  $crate: $server_ver (aligned)"; \
    done
    @echo "OK - inspector/broker version alignment confirmed."

# MCP-01 (D-11 source-import grep): assert MCP/bus/broker source has no
# `use reqwest` or `use rustls` imports. Cheap structural gate that ships
# today; cargo-tree-strict reading is deferred to Phase 4 when the
# federation CLI surfaces are deleted.
check-mcp-deps:
    bash scripts/check-mcp-deps.sh

# QUAR-05 / D-22: assert the checked-in rendering-surface allowlist still
# matches the mechanical query (scripts/quarantine-surfaces.sh). Fails
# nonzero with a delta listing when a new call site reaching received
# content appears without being routed through the shared render helper
# or explicitly justified in .quarantine-surfaces.allow.
check-quarantine-surfaces:
    @echo "Verifying rendering-surface allowlist matches the mechanical query..."
    @command -v sh >/dev/null || { echo "ERROR: sh not found in PATH"; exit 1; }
    @sh scripts/quarantine-surfaces.sh --check

# AUDIT-05: prevent split-commit between FAMP_SPEC_VERSION bump and impl.
check-spec-version-coherence:
    @if grep -q 'pub const FAMP_SPEC_VERSION: &str = "0.5.2"' crates/famp-envelope/src/version.rs; then \
      grep -q 'AuditLog' crates/famp-core/src/class.rs || (echo "spec version 0.5.2 declared but MessageClass::AuditLog missing" && exit 1); \
      grep -q 'AuditLogBody' crates/famp-envelope/src/body/mod.rs || (echo "spec version 0.5.2 declared but AuditLogBody missing" && exit 1); \
      for f in crates/*/Cargo.toml; do \
        if grep -n '^description' "$f" | grep -q 'v0.5.1'; then \
          echo "stale crate description still says v0.5.1 (spec version is 0.5.2): $f" && exit 1; \
        fi; \
      done; \
    fi

# DIST-05: assert release.yml is the sole tag-triggered producer of release
# assets (scripts/release-artifact-source-gate.sh). No tooling beyond bash
# and grep, so unlike check-installer-drift it belongs in local CI parity.
check-release-artifact-source:
    bash scripts/release-artifact-source-gate.sh

# Full local CI-parity gate. A green `just ci` implies a green GitHub Actions run.
ci: fmt-check lint build test-canonical-strict test-crypto test test-doc spec-lint check-no-tokio-in-bus check-no-io-in-inspect-proto check-inspect-readonly check-inspect-version-aligned check-spec-version-coherence check-mcp-deps check-shellcheck check-quarantine-surfaces check-release-artifact-source publish-workspace-dry-run
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
    famp install-claude-code

# Install famp-gateway to ~/.cargo/bin — the v1.0 federation binary. `just
# install` (above) never touches this; run this after any change to
# `crates/famp-gateway` (egress/ingress/CLI surface) so the deployed
# ~/.cargo/bin/famp-gateway is not stale relative to the source.
install-gateway:
    cargo install --path crates/famp-gateway --locked --force

# Install both shipping binaries (famp + famp-gateway). Run this after any
# change to `famp send`'s build_envelope_value (the famp_send MCP path
# reads ~/.cargo/bin/famp, not target/release/famp) or to famp-gateway.
install-all: install install-gateway

# Clean build artifacts
clean:
    cargo clean

# v1.0 federation SPIKE: expose the local broker on the tailnet via socat so a
# friend on the same Tailscale tailnet can reach it. Zero FAMP code — validates
# cross-host agent chat before committing to famp-gateway. See docs/SPIKE-friend-chat.md.
# SECURITY: register the friend-facing window with listen:false (inbound = data, not instructions).
spike-tunnel:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v socat >/dev/null || { echo "error: socat not installed (brew install socat)"; exit 1; }
    command -v tailscale >/dev/null || { echo "error: tailscale not installed/running"; exit 1; }
    IP=$(tailscale ip -4 | head -1)
    [ -S "$HOME/.famp/bus.sock" ] || { echo "error: broker socket missing — run 'famp daemon status' / start the broker"; exit 1; }
    echo "Broker exposed on the tailnet. Share this with your friend:"
    echo "    host tailnet IP : $IP"
    echo "    port            : 9999"
    echo "    friend runs     : socat UNIX-LISTEN:\$HOME/.famp/bus.sock,fork TCP:$IP:9999"
    echo "Ctrl-C to stop the tunnel."
    socat TCP-LISTEN:9999,fork,reuseaddr,bind="$IP" UNIX-CONNECT:"$HOME/.famp/bus.sock"

# Regenerate a host's plugin packaging from crates/famp/assets/.
# Hosts: claude-code (default), codex, grok.
plugin-gen host="claude-code":
    bash scripts/gen-plugin.sh {{host}}

# Fail if a host's derived plugin files have drifted from crates/famp/assets/.
plugin-check host="claude-code":
    bash scripts/gen-plugin.sh {{host}}
    git diff --exit-code -- plugins/{{host}}/commands plugins/{{host}}/hooks

# Drift-check all three host packagings (CI gate).
plugin-check-all:
    #!/usr/bin/env bash
    set -euo pipefail
    for host in claude-code codex grok; do
        echo "== plugin-check $host =="
        bash scripts/gen-plugin.sh "$host"
        git diff --exit-code -- "plugins/$host/commands" "plugins/$host/hooks"
    done
    # Generated hooks must never retain the #28 install-time token.
    if grep -rq '@FAMP_BIN@' plugins/*/hooks 2>/dev/null; then
        echo "error: unresolved @FAMP_BIN@ under plugins/*/hooks" >&2
        grep -rn '@FAMP_BIN@' plugins/*/hooks >&2 || true
        exit 1
    fi

# Validate the Claude Code plugin manifest and components (requires Claude Code CLI).
plugin-validate:
    claude plugin validate ./plugins/claude-code

<!-- generated-by: gsd-doc-writer -->
# FAMP Development Guide

This document covers local development setup, the build system, code style
gates, git hooks, and the MCP deploy cycle for contributors to the FAMP Rust
workspace. For testing, see [TESTING.md](TESTING.md). For crate relationships,
see [ARCHITECTURE.md](../ARCHITECTURE.md).

---

## Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust toolchain | `1.89.0` (pinned by `rust-toolchain.toml`) | `rustup` auto-installs on first `cargo` invocation |
| rustup | any | <https://rustup.rs> |
| cargo-nextest | latest | `cargo install cargo-nextest --locked` |
| just | latest | `cargo install just --locked` |

The workspace pins the toolchain in `rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.89.0"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

Running any `cargo` command inside the repo auto-installs the pinned toolchain
via rustup. No manual `rustup override` step is needed.

---

## Local Setup

```bash
git clone https://github.com/thebenlamm/FAMP
cd FAMP
cargo install cargo-nextest --locked
cargo install just --locked
just install-hooks   # one-time per clone
just ci              # full local CI-parity check
```

A green `just ci` locally implies a green GitHub Actions run. If it passes,
you are set up correctly.

---

## Build Commands

Run `just` with no arguments to list all available recipes.

| Command | What it does |
|---------|-------------|
| `just build` | `cargo build --workspace --all-targets` |
| `just fmt` | Format all sources (`cargo fmt --all`) |
| `just fmt-check` | Formatting gate without modifying files (CI gate) |
| `just lint` | `cargo clippy --workspace --all-targets -- -D warnings` |
| `just audit` | `cargo audit` — RustSec advisory scan |
| `just spec-lint` | FAMP v0.5.1 spec anchor lint (ripgrep-based) |
| `just check-no-tokio-in-bus` | Assert `famp-bus` has no `tokio` in dep tree (BUS-01) |
| `just check-no-io-in-inspect-proto` | Assert `famp-inspect-proto` is I/O-free (INSP-CRATE-01) |
| `just check-inspect-readonly` | Assert `famp-inspect-server` imports no write surfaces (INSP-RPC-02) |
| `just check-inspect-version-aligned` | Assert inspector/broker decode crate versions match (INSP-CRATE-03) |
| `just check-spec-version-coherence` | Prevent split-commit between `FAMP_SPEC_VERSION` bump and impl (AUDIT-05) |
| `just check-mcp-deps` | Assert MCP/bus/broker source has no `reqwest`/`rustls` imports (MCP-01) |
| `just check-shellcheck` | Shellcheck the hook-runner asset |
| `just install` | **MCP deploy target** — installs `famp` to `~/.cargo/bin` |
| `just install-hooks` | Install pre-commit and pre-push hooks (one-time per clone) |
| `just smoke-test` | Verify the Quick Start install path in isolation |
| `just clean` | `cargo clean` |
| `just ci` | Full local CI-parity gate (runs all of the above in sequence) |

For test-specific recipes (`just test`, `just test-canonical`, etc.), see
[TESTING.md](TESTING.md).

---

## Workspace Layout

The workspace root `Cargo.toml` lists 15 crates under `crates/`. Three
groups matter for development:

**Protocol primitives** — transport-neutral, reused by both v0.9 and v1.0:

| Crate | Purpose |
|-------|---------|
| `famp-canonical` | RFC 8785 canonical JSON wrapper and conformance gate |
| `famp-crypto` | Ed25519 sign/verify with `FAMP-sig-v1\0` domain separation |
| `famp-core` | `Principal`/`Instance`, UUIDv7 IDs, `ArtifactId`, invariants |
| `famp-envelope` | Signed envelope types and message bodies |
| `famp-fsm` | 5-state task FSM (`REQUESTED → COMMITTED → {COMPLETED\|FAILED\|CANCELLED}`) |
| `famp-inbox` | Append-only inbox storage |
| `famp-taskdir` | Task state directory management |
| `famp-transport` | `Transport` trait and `MemoryTransport` |

**Inspector crates** — observability without write access:

| Crate | Purpose |
|-------|---------|
| `famp-inspect-proto` | Shared inspect RPC types (I/O-free — INSP-CRATE-01) |
| `famp-inspect-server` | Read-only broker inspector (no write surfaces — INSP-RPC-02) |
| `famp-inspect-client` | CLI-side inspect client |

**Federation internals** (v1.0) — do not conflate with the primitive layer:

| Crate | Purpose |
|-------|---------|
| `famp-keyring` | TOFU keyring file format and peer parsing |
| `famp-transport-http` | HTTPS transport (`axum` + `reqwest` + `rustls`) |

**CLI + runtime:**

| Crate | Purpose |
|-------|---------|
| `famp` | CLI binary, MCP server, runtime glue, examples, integration tests |

Cross-reference [ARCHITECTURE.md](../ARCHITECTURE.md) for the full crate
dependency graph and layering invariants.

---

## Code Style

### Formatting

All sources must pass `cargo fmt --all -- --check`. Run `just fmt` to
auto-format before committing.

### Clippy

The workspace `Cargo.toml` configures strict Clippy settings applied to all
crates that use `[lints] workspace = true` (the default for every crate in
this repo):

```toml
[workspace.lints.clippy]
all      = { level = "deny", priority = -1 }
pedantic = { level = "deny", priority = -1 }
nursery  = { level = "warn", priority = -1 }
unwrap_used = "deny"
expect_used = "deny"
```

Several verbose pedantic lints are allowed at the workspace level (see
`Cargo.toml` for the full list).

**Critical: the `[lints]` table does NOT merge.** Cargo's `[lints]` section
uses full-replacement semantics — a per-member `[lints]` block completely
replaces the workspace block rather than extending it. If you add a custom
`[lints]` block to a crate's `Cargo.toml`, you must mirror the full workspace
allow-list line-by-line. Omitting a single `allow` entry re-enables the
corresponding lint at its default level, silently breaking crate builds.

The safe default — which every crate in this repo uses — is:

```toml
[lints]
workspace = true
```

Only add a custom block if you need a crate-specific override, and when you
do, copy the entire workspace allow-list first.

Run `just lint` to catch violations before pushing.

### `unsafe` code

`unsafe_code` is `"forbid"` at the workspace level. Any PR introducing
`unsafe` requires an explicit spec rationale and will not be accepted.

---

## Git Hooks

Hooks are stored in `.githooks/` and activated via:

```bash
just install-hooks
```

This runs `git config core.hooksPath .githooks`. Run it once after cloning.

| Hook | Trigger | Check |
|------|---------|-------|
| `pre-commit` | Every `git commit` (Rust files only) | `cargo fmt --all -- --check` |
| `pre-push` | Every `git push` (Rust files only) | `cargo clippy --workspace --all-targets -- -D warnings` |

Both hooks skip when no `.rs` files are in the staged set / push range, so
doc-only commits and pushes remain fast.

**Never use `--no-verify`.** If a hook fails, fix the underlying issue — the
check mirrors exactly what CI runs.

---

## Commit Conventions

Follow conventional commits. Scope by crate when the change is crate-specific:

```
feat(famp-crypto): add ingress weak-key rejection
fix(famp-bus): drain head-of-line on malformed envelope
docs: update broker restart playbook
refactor(famp-fsm): extract state transition guard
test(famp-canonical): extend RFC 8785 float corpus
chore: bump workspace to 0.12.0
```

Write a multi-paragraph body explaining **why** and the impact — not what (the
diff shows what). Keep each commit atomic: one logical change per commit.

**Spec fidelity rule:** any commit that touches signing, canonicalization,
envelope schema, or the task FSM must cite the relevant `FAMP-v0.5.1-spec.md`
section in the body. Spec deviations must be documented with a `Δ` note and
rationale before the code lands.

---

## MCP Tool Surface Changes

The `famp` binary installed to `~/.cargo/bin/famp` is the deployment target
for MCP sessions — not `target/release/famp`. When modifying
`crates/famp/src/cli/mcp/server.rs` (tool schemas, tool descriptors, new
tools):

```bash
just install
```

This runs `cargo install --path crates/famp --locked --force` followed by
`famp install-claude-code`, placing the updated binary at `~/.cargo/bin/famp`.
Every agent session reads from that path. Failing to run `just install` after
MCP surface changes means agent sessions continue using the old binary.

---

## Broker Restart Playbook

Wire-protocol changes, inspect-protocol changes, or `just install` after MCP
surface edits all require a broker restart to take effect.

```bash
# 1. Check current broker state
famp inspect broker

# 2. Stop the running broker
#    (find the broker process — it may have auto-spawned)
pkill -f 'famp daemon'   # or use the PID from `famp inspect broker`

# 3. Start a fresh broker
famp daemon start

# 4. Verify it came up
famp inspect broker
famp inspect identities
```

**Gotchas:**

- `famp inspect` can auto-spawn a broker if none is running. Stop that process
  explicitly before starting yours, or it will race.
- macOS has no `setsid` equivalent in the standard shell — broker processes
  launched from a terminal session inherit the session's signal group.
- Temporary brokers backed by `register` holders (used in tests) live only as
  long as the holder process. They reappear if the holder is still alive — check
  `famp inspect identities` to distinguish them from your broker.

---

## Branch Conventions

No formal branch naming convention is enforced by tooling. The practical
convention used in this repo:

- `main` — the default and only long-lived branch
- Feature / fix branches: `feat/<short-description>` or `fix/<short-description>`
- Phase branches (GSD workflow): named by the workflow automatically

External PRs are welcome from v1.0 onward; until then, file issues rather
than opening PRs (see CONTRIBUTING.md).

---

## PR Process

1. Open a branch from `main`.
2. Run `just ci` locally — it must pass before requesting review.
3. Every non-trivial change gets an adversarial review pass before merge.
4. Spec-touching changes (signing, canonicalization, envelope schema, task FSM)
   must include the spec citation in the commit body and a `Δ` note in
   `FAMP-v0.5.1-spec.md` if the implementation deviates.
5. See [CONTRIBUTING.md](../CONTRIBUTING.md) for the full list of load-bearing
   constants that require a spec diff before any code change is accepted.

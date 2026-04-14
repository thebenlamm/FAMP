# FAMP — Federated Agent Messaging Protocol (Rust reference implementation)

**Status:** Phase 0: Toolchain & Workspace Scaffold

A conformance-grade Rust implementation of FAMP v0.5.1, covering identity,
causality, negotiation, commitment, delegation, and provenance across three
protocol layers plus a reference HTTP transport binding.

## Prerequisites

- macOS or Linux
- `git`
- `curl`

## Bootstrap

```bash
# 1. Install rustup (toolchain manager). Skip if already installed.
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none

# 2. Enter the repo (rust-toolchain.toml auto-installs 1.89.0)
cd FAMP
rustc --version   # should print: rustc 1.89.0

# 3. Install dev tools (one-time)
cargo install cargo-nextest --locked
cargo install just --locked

# 4. Verify the full CI-parity loop
just ci
```

## Daily loop

| Command       | What it does                                      |
|---------------|---------------------------------------------------|
| `just build`  | `cargo build --workspace --all-targets`           |
| `just test`   | `cargo nextest run --workspace`                   |
| `just lint`   | `cargo clippy --workspace --all-targets -D warnings` |
| `just fmt`    | `cargo fmt --all`                                 |
| `just ci`     | fmt-check + lint + build + test (pre-push gate)   |

A green `just ci` locally implies a green GitHub Actions run — the Justfile
and CI workflow mirror each other exactly.

## License

Dual-licensed under Apache-2.0 OR MIT. See LICENSE-APACHE and LICENSE-MIT.

## Status

Phase 0 is the bootstrap phase. Zero FAMP protocol code yet — this phase
establishes the reproducible build + test + lint loop.

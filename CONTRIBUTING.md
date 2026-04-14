# Contributing to FAMP

FAMP is a Rust reference implementation of the Federated Agent Messaging
Protocol. The `v0.7` Personal Runtime is maintained by a single developer.
External PRs are welcome from `v1.0` onward; until then, please file issues
rather than PRs.

## Setup

The Rust toolchain is pinned by `rust-toolchain.toml` to `1.89.0`. Running
any `cargo` command in the repo auto-installs it via `rustup`.

1. Install `rustup`: <https://rustup.rs>
2. One-time dev tools:

    ```bash
    cargo install cargo-nextest --locked
    cargo install just --locked
    ```

3. Bootstrap and verify:

    ```bash
    just ci
    ```

A green `just ci` locally implies a green GitHub Actions run.

## Repo Layout

- `crates/famp-canonical` — RFC 8785 canonical JSON wrapper and conformance gate
- `crates/famp-crypto` — Ed25519 sign/verify with `FAMP-sig-v1\0` domain separation
- `crates/famp-core` — typed `Principal`/`Instance`, UUIDv7 IDs, `ArtifactId`, `ProtocolErrorKind`, invariants
- `crates/famp-envelope` — signed envelope types and the five shipped message bodies
- `crates/famp-fsm` — the 5-state task FSM
- `crates/famp-keyring` — TOFU keyring file format and peer parsing
- `crates/famp-transport` — `Transport` trait and `MemoryTransport`
- `crates/famp-transport-http` — minimal HTTPS transport (`axum` + `reqwest` + `rustls`)
- `crates/famp` — runtime glue, examples, and cross-crate integration tests

## Test Gates

| Command | Gate |
|---|---|
| `just build` | workspace builds with all targets |
| `just test` | unit + integration tests via `cargo-nextest` |
| `just test-canonical-strict` | RFC 8785 conformance (per-PR CI) |
| `just test-crypto` | RFC 8032 vectors + §7.1c worked example + `famp-crypto` doctests |
| `just test-core` | wire-string fixtures + exhaustive-match gate |
| `just test-doc` | all workspace doctests |
| `just lint` | `cargo clippy --workspace --all-targets -- -D warnings` |
| `just fmt-check` | `cargo fmt --all -- --check` |
| `just spec-lint` | FAMP v0.5.1 spec anchor lint (ripgrep-based) |
| `just audit` | `cargo audit` advisories |
| `just ci` | full local CI-parity loop |

RFC 8785 conformance runs unconditionally on every PR. A green `just ci`
locally is the shipping bar.

## Commit Conventions

- Conventional commits: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`. Scope by crate when relevant: `feat(famp-crypto): …`.
- Multi-paragraph body explaining WHY and impact, not what.
- Atomic commits. One logical change per commit.
- **Never use `--no-verify`.** If a pre-commit hook fails, fix the underlying issue.

## Code Review

For `v0.7`: single maintainer plus adversarial review agent. Workflow is
documented in `CLAUDE.md`. Every non-trivial change gets an adversarial
review pass before merge.

## Spec Fidelity

- Any change that touches signing, canonicalization, envelope schema, or the task FSM **must cite the relevant section of `FAMP-v0.5.1-spec.md`** in the commit body.
- Any deviation from the spec **must be documented** in `FAMP-v0.5.1-spec.md` as a `Δ` note with rationale. (Example: `§Δ08` for the `DOMAIN_PREFIX` addition.)
- The spec is the authority. If the code disagrees with the spec, one of them is wrong — fix it explicitly, do not let them drift.

## Do Not Touch Without a Spec Diff

These values are load-bearing byte-exact interop contracts. Changing any
of them invalidates every existing signature:

- `DOMAIN_PREFIX` in `crates/famp-crypto/src/prefix.rs` — the 12-byte `b"FAMP-sig-v1\0"` constant
- `FAMP_SPEC_VERSION` in `famp-core`
- The canonicalization output of `famp-canonical` (RFC 8785 JCS — the conformance gate exists precisely to pin this)
- Ed25519 `verify_strict` strictness in `famp-crypto` (never downgrade to plain `verify`)

If you need to change any of these, write the spec delta first, get it
reviewed, then land the code change.

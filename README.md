# FAMP — Federated Agent Messaging Protocol (Rust reference implementation)

**Status:** `v0.7 Personal Runtime` shipped

FAMP is a Rust implementation of the Federated Agent Messaging Protocol, built
in two layers:

- **Personal Profile (`v0.6` + `v0.7`)**: a signed agent-to-agent runtime a
  single developer can actually use today
- **Federation Profile (`v0.8+`)**: the larger ecosystem semantics that sit on
  top later, including Agent Cards, federation trust, negotiation, delegation,
  provenance, extensions, and full conformance badges

The current repo ships the **personal runtime**: canonical JSON, Ed25519
signing with domain separation, typed core IDs/errors, signed envelopes, a
minimal task FSM, an in-process transport, a minimal HTTPS transport, and a
TOFU keyring.

## What Works Today

- RFC 8785 canonical JSON via `famp-canonical`
- Ed25519 sign/verify with `FAMP-sig-v1\0` domain separation via `famp-crypto`
- Typed core identities, IDs, error kinds, invariants, and authority scope via
  `famp-core`
- Signed envelopes for:
  - `request`
  - `commit`
  - `deliver`
  - `ack`
  - `control/cancel`
- A 5-state task FSM:
  - `REQUESTED`
  - `COMMITTED`
  - `COMPLETED`
  - `FAILED`
  - `CANCELLED`
- `MemoryTransport` for same-process use
- Minimal HTTPS transport built on `axum`, `reqwest`, and `rustls`
- TOFU keyring binding `Principal -> VerifyingKey`
- Two runnable examples:
  - same-process happy path
  - cross-machine HTTPS happy path

## Not Shipped Yet

Deferred to the federation-profile milestones (`v0.8+`):

- Agent Cards and federation credentials
- `.well-known` card distribution
- negotiation / counter-proposal
- delegation forms
- provenance graph
- extensions registry
- replay defense / freshness windows / idempotency scoping
- full adversarial conformance matrix and Level 2/3 badges
- CLI workflows beyond the current examples

## Prerequisites

- macOS or Linux
- `git`
- `curl`

## Bootstrap

```bash
# 1. Install rustup (skip if already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none

# 2. Enter the repo (rust-toolchain.toml auto-installs 1.87.0)
cd FAMP
rustc --version

# 3. Install dev tools (one-time)
cargo install cargo-nextest --locked
cargo install just --locked

# 4. Verify the workspace
just ci
```

## Quick Start

Run the in-process happy path:

```bash
cargo run --example personal_two_agents -p famp
```

That example spins up two local agents, pins their Ed25519 keys in a keyring,
and completes a signed:

`request -> commit -> deliver -> ack`

cycle over `MemoryTransport`.

Run the HTTPS cross-machine example in two terminals:

1. Start Bob once to generate his keypair and cert files.

```bash
cargo run --example cross_machine_two_agents -p famp -- \
  --role bob \
  --listen 127.0.0.1:8443 \
  --out-pubkey /tmp/bob.pub \
  --out-cert /tmp/bob.crt \
  --out-key /tmp/bob.key
```

Bob will print a listening address and write:

- `/tmp/bob.pub`
- `/tmp/bob.crt`
- `/tmp/bob.key`

2. Start Alice once to generate her keypair and cert files.

```bash
cargo run --example cross_machine_two_agents -p famp -- \
  --role alice \
  --listen 127.0.0.1:8444 \
  --out-pubkey /tmp/alice.pub \
  --out-cert /tmp/alice.crt \
  --out-key /tmp/alice.key
```

Alice will write:

- `/tmp/alice.pub`
- `/tmp/alice.crt`
- `/tmp/alice.key`

3. Read each side's public key:

```bash
cat /tmp/bob.pub
cat /tmp/alice.pub
```

4. Re-run Bob with Alice's public key, Alice's address, and Alice's cert trusted:

```bash
cargo run --example cross_machine_two_agents -p famp -- \
  --role bob \
  --listen 127.0.0.1:8443 \
  --cert /tmp/bob.crt \
  --key /tmp/bob.key \
  --peer 'agent:local/alice=<paste-alice-pubkey-here>' \
  --addr 'agent:local/alice=https://127.0.0.1:8444' \
  --trust-cert /tmp/alice.crt
```

5. Re-run Alice with Bob's public key, Bob's address, and Bob's cert trusted:

```bash
cargo run --example cross_machine_two_agents -p famp -- \
  --role alice \
  --listen 127.0.0.1:8444 \
  --cert /tmp/alice.crt \
  --key /tmp/alice.key \
  --peer 'agent:local/bob=<paste-bob-pubkey-here>' \
  --addr 'agent:local/bob=https://127.0.0.1:8443' \
  --trust-cert /tmp/bob.crt
```

At that point the two processes should complete the same signed
`request -> commit -> deliver -> ack` cycle over HTTPS.

Notes:

- `--peer` binds a `Principal` to a pinned Ed25519 public key.
- `--addr` tells the client where to send messages for that principal.
- `--trust-cert` tells the HTTPS client which peer certificate to trust for
  this local run.
- The example is intentionally symmetric: both sides run a server and a client.

The example can generate local certs for you, and there are committed fixture
certs under
[crates/famp/tests/fixtures/cross_machine/README.md](crates/famp/tests/fixtures/cross_machine/README.md)
for deterministic test runs.

## Daily Loop

| Command | What it does |
|---|---|
| `just build` | `cargo build --workspace --all-targets` |
| `just test` | `cargo nextest run --workspace` |
| `just test-canonical-strict` | RFC 8785 gate for `famp-canonical` |
| `just test-crypto` | RFC 8032 + worked-example gate for `famp-crypto` |
| `just test-core` | wire-string + exhaustive-match gate for `famp-core` |
| `just lint` | `cargo clippy --workspace --all-targets -D warnings` |
| `just fmt` | `cargo fmt --all` |
| `just ci` | local CI-parity loop |

A green `just ci` locally implies a green GitHub Actions run.

## Repo Layout

- `crates/famp-canonical`: RFC 8785 canonical JSON wrapper and conformance gate
- `crates/famp-crypto`: Ed25519 sign/verify, base64url codecs, worked vectors
- `crates/famp-core`: `Principal`, `Instance`, UUIDv7 IDs, `ArtifactId`,
  `ProtocolErrorKind`, invariants
- `crates/famp-envelope`: signed envelope types and five shipped message bodies
- `crates/famp-fsm`: minimal 5-state task FSM
- `crates/famp-keyring`: TOFU keyring file format and peer parsing
- `crates/famp-transport`: transport trait + `MemoryTransport`
- `crates/famp-transport-http`: minimal HTTPS transport
- `crates/famp`: runtime glue, examples, and cross-crate integration tests

## Design Notes

- Canonicalization and signature verification are the hard substrate. They are
  done once and reused everywhere.
- Personal Profile narrows by **absence**, not by `Option<T>`:
  deferred federation-grade fields and variants are literally not representable
  in `v0.7`.
- The current trust model is intentionally simple:
  local TOFU keyring only, no Agent Cards, no federation registry.

## Current Milestones

- `v0.5.1`: spec fork, shipped
- `v0.6`: foundation crates, shipped
- `v0.7`: personal runtime, shipped
- `v0.8+`: federation profile, next

See [`.planning/ROADMAP.md`](.planning/ROADMAP.md) for the current roadmap and
[`.planning/MILESTONES.md`](.planning/MILESTONES.md) for milestone history.

## License

Dual-licensed under Apache-2.0 OR MIT. See [LICENSE-APACHE](LICENSE-APACHE) and
[LICENSE-MIT](LICENSE-MIT).

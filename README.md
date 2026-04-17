# FAMP — Federated Agent Messaging Protocol (Rust reference implementation)

**Status:** `v0.8 Usable from Claude Code` — MCP integration complete

FAMP is a Rust implementation of the Federated Agent Messaging Protocol, built
in two layers:

- **Personal Profile (`v0.6` + `v0.7` + `v0.8`)**: a signed agent-to-agent runtime
  a single developer can actually use today, with CLI tools and MCP integration
- **Federation Profile (`v0.9+`)**: the larger ecosystem semantics that sit on
  top later, including Agent Cards, federation trust, negotiation, delegation,
  provenance, extensions, and full conformance badges

The current repo ships the **personal runtime**: canonical JSON, Ed25519
signing with domain separation, typed core IDs/errors, signed envelopes, a
minimal task FSM, an in-process transport, a minimal HTTPS transport, a
TOFU keyring, and a full CLI with MCP server for Claude Code integration.

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
- **Full CLI** with streamlined onboarding:
  - `famp setup` — one-command identity creation with auto port selection
  - `famp info` — output peer card for sharing
  - `famp peer import` — import peer cards from other agents
  - `famp listen` — run the HTTPS daemon
  - `famp send` — send signed envelopes
  - `famp inbox` — inspect received messages
  - `famp await` — block until new messages arrive
- **MCP server** (`famp mcp`) for Claude Code integration
- Two runnable examples:
  - same-process happy path
  - cross-machine HTTPS happy path

## Not Shipped Yet

Deferred to the federation-profile milestones (`v0.9+`):

- Agent Cards and federation credentials
- `.well-known` card distribution
- negotiation / counter-proposal
- delegation forms
- provenance graph
- extensions registry
- replay defense / freshness windows / idempotency scoping
- full adversarial conformance matrix and Level 2/3 badges

## Prerequisites

- macOS or Linux
- `git`
- `curl`

## Bootstrap

```bash
# 1. Install rustup (skip if already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none

# 2. Add cargo/rustup to the current shell's PATH
#    (rustup's installer edits your shell profile, but that only takes effect
#    in new shells — this line activates it here too)
source "$HOME/.cargo/env"

# 3. Enter the repo (rust-toolchain.toml auto-installs 1.89.0)
cd FAMP
rustc --version

# 4. Install dev tools (one-time)
cargo install cargo-nextest --locked
cargo install just --locked

# 5. Verify the workspace
just ci
```

## Quick Start (CLI)

The fastest way to get two agents talking:

```bash
# 1. Build the CLI
cargo build --release

# 2. Set up two agents with unique ports
./target/release/famp setup --name alice --home /tmp/famp-alice --port 8443
./target/release/famp setup --name bob --home /tmp/famp-bob --port 8444

# 3. Exchange peer cards (pipe-friendly!)
FAMP_HOME=/tmp/famp-alice ./target/release/famp info | \
  FAMP_HOME=/tmp/famp-bob ./target/release/famp peer import
FAMP_HOME=/tmp/famp-bob ./target/release/famp info | \
  FAMP_HOME=/tmp/famp-alice ./target/release/famp peer import

# 4. Start daemons (in separate terminals or background)
FAMP_HOME=/tmp/famp-alice ./target/release/famp listen &
FAMP_HOME=/tmp/famp-bob ./target/release/famp listen &

# 5. Send a message from Alice to Bob
#    First contact requires explicit TOFU opt-in (see "TLS trust" below).
FAMP_TOFU_BOOTSTRAP=1 FAMP_HOME=/tmp/famp-alice ./target/release/famp send \
  --to bob --action new_task --body '{"task": "hello"}'
```

Each `famp setup` outputs a **peer card** — a JSON blob containing endpoint,
public key, and principal that other agents need to register you as a peer.

### TLS trust (TOFU bootstrap)

FAMP uses self-signed TLS certificates with **Trust-On-First-Use** pinning.
Once a peer's leaf-cert SHA-256 is recorded in `peers.toml`
(`tls_fingerprint_sha256`), every subsequent connection rejects on mismatch.

The first connection has nothing to compare against. By default, FAMP
**refuses** that first connection rather than silently pinning whatever the
network returns — a one-time on-path attacker could otherwise capture an
alias permanently. To allow the first connection, set:

```bash
FAMP_TOFU_BOOTSTRAP=1 famp send --to <alias> ...
```

Use this only when you trust the path between you and the peer (typically
loopback or a brand-new private link). Subsequent sends do not need the
flag — the pinned fingerprint is the trust anchor from then on.

## MCP Integration (Claude Code)

FAMP includes an MCP server for use with Claude Code. Add to `.mcp.json`:

```json
{
  "mcpServers": {
    "famp-alice": {
      "command": "/path/to/famp",
      "args": ["mcp"],
      "env": { "FAMP_HOME": "/tmp/famp-alice" }
    }
  }
}
```

The MCP server exposes four tools:
- `famp_send` — send signed envelopes
- `famp_inbox` — list received messages
- `famp_await` — wait for new messages
- `famp_peers` — list/add peers

See [`.planning/HANDOFF-mcp-integration.md`](.planning/HANDOFF-mcp-integration.md)
for detailed MCP setup instructions.

## Programmatic Examples

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

## How FAMP Signs a Message

**Canonical JSON (RFC 8785).** FAMP payloads are signed over bytes, not
over JSON values. Every signer and every verifier, in every implementation,
must produce the same byte string for the same logical value — that is
what RFC 8785 (JCS) is for, and it is implemented in `famp-canonical`.
Byte-exact is the entire interop story: if two implementations disagree on
one byte of whitespace or one byte of key order, every downstream signature
is unverifiable and the federation stops being a federation.

**Domain separation.** Before Ed25519 touches the canonical bytes,
`famp-crypto` prepends a 12-byte constant: `b"FAMP-sig-v1\0"`
(`DOMAIN_PREFIX`). This prevents a signature produced for a FAMP envelope
from being replayed in any unrelated context that also signs canonical
JSON — database change records, JWT-adjacent tooling, other protocols.
The `v1` suffix is the wire version; rotating signing semantics means
shipping `FAMP-sig-v2\0`, not renaming fields. Spec §7.1a, §Δ08.

**The four steps.** To sign: (1) canonicalize the unsigned payload to JCS
bytes via `famp-canonical`; (2) prepend `FAMP-sig-v1\0`; (3) Ed25519 sign
the concatenation; (4) encode the 64-byte signature as base64url unpadded
(`URL_SAFE_NO_PAD`, strict alphabet). To verify: decode the signature
under the same strict decoder, canonicalize the received payload with the
`signature` field removed, prepend `FAMP-sig-v1\0`, and route Ed25519
through `verify_strict` — never plain `verify`. Plain `verify` accepts
malleable signatures and small-order points, and the failure mode is
silent non-repudiation.

**INV-10.** Every envelope on the wire is signed. Unsigned envelopes are
rejected at ingress. There is no "internal trusted" escape hatch, no
debug bypass, no "just this one message". This is what makes
non-repudiation an actual property of the system rather than an
aspiration, and it is enforced at the type level in `famp-envelope`:
`Option<FampSignature>` does not appear anywhere on the signed envelope
type.

**Task FSM.** Tasks move through five states with no intermediate parking
and no backtracking. Each transition is a signed envelope; the FSM is
enforced by `famp-fsm`; `COMPLETED`, `FAILED`, and `CANCELLED` are
absorbing terminals.

```
   REQUESTED
       |
       v
   COMMITTED
       |
       +--> COMPLETED
       |
       +--> FAILED
       |
       +--> CANCELLED
```

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
- `v0.8`: usable from Claude Code (CLI + MCP), shipped
- `v0.9+`: federation profile, next

See [`.planning/ROADMAP.md`](.planning/ROADMAP.md) for the current roadmap and
[`.planning/MILESTONES.md`](.planning/MILESTONES.md) for milestone history.

## License

Dual-licensed under Apache-2.0 OR MIT. See [LICENSE-APACHE](LICENSE-APACHE) and
[LICENSE-MIT](LICENSE-MIT).

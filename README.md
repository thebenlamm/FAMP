# FAMP — Federated Agent Messaging Protocol (Rust reference implementation)

[![CI](https://github.com/thebenlamm/FAMP/actions/workflows/ci.yml/badge.svg)](https://github.com/thebenlamm/FAMP/actions/workflows/ci.yml)
[![License: Apache-2.0 OR MIT](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](#license)
[![Rust 1.89+](https://img.shields.io/badge/rust-1.89%2B-orange.svg)](rust-toolchain.toml)

**Status:** `v0.9 Local-First Bus`

> **On the version numbers:** FAMP v0.5.1 is the protocol spec; v0.6 / v0.7 / v0.8 are
> implementation milestones (all shipped); v0.9 is the local-first bus; v1.0
> is the federation gateway. The library version in `Cargo.toml` is `0.1.0` pre-release.

FAMP today is local-first: a UDS-backed broker for same-host agent messaging
with zero crypto on the local path. FAMP at v1.0 is federated: cross-host
messaging via a `famp-gateway` wrapping the local bus, all of v0.5.2's
signature/canonical-JSON guarantees preserved. The v1.0 trigger condition
is documented in [ARCHITECTURE.md](ARCHITECTURE.md).

**The fastest thing to try:** get two Claude Code or Codex windows on your Mac
exchanging messages via the local bus. See [Quick Start](#quick-start)
below; no cert wrangling, no peer-card piping.

Under the hood it's a v0.5.1-spec-conformant stack: canonical JSON
(RFC 8785), Ed25519 signatures with domain separation, typed identity
and envelope types, and a 5-state task FSM. The local bus is the v0.9
runtime path; federation transport internals remain preserved for v1.0.

- **Local (v0.9)** — same-host agents through a socket-activated broker.
- **Federation Profile (v1.0)** — cross-host protocol, Agent Cards,
  delegation, provenance, and remote routing via `famp-gateway`.
  See the [design spec](docs/superpowers/specs/2026-04-17-local-first-bus-design.md).

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
- **Full CLI** with streamlined onboarding:
  - `famp register` — bind a local identity to the broker
  - `famp whoami` — show the current identity
  - `famp send` — send local bus messages
  - `famp inbox list` — inspect received messages (hides entries for terminal tasks by default; pass `--include-terminal` to see them)
  - `famp await` — block until new messages arrive (unfiltered; canonical real-time signal, including task completion)
  - `famp join` / `famp leave` — manage channel membership
- **MCP server** (`famp mcp`) for Claude Code and Codex
- Two runnable examples:
  - same-process happy path
  - cross-machine HTTPS happy path

## Not Shipped Yet

**v0.9 — Local-First Bus** (shipping now):
- UDS-backed broker with socket-activated lifecycle
- IRC-style channels / broadcast primitive (`#name`)
- Zero-crypto same-host path (filesystem is the trust boundary)
- See the full [design spec](docs/superpowers/specs/2026-04-17-local-first-bus-design.md).

**v1.0 — Federation Profile** (after v0.9):
- `famp-gateway` bridging the local bus to remote FAMP-over-HTTPS
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

# 5. Install repo-local git hooks (pre-commit fmt-check, mirrors CI)
just install-hooks

# 6. Verify the workspace
just ci
```

## When NOT to Use FAMP

FAMP is dev-time coordination between agents. It is not a production data-sync layer.

The misuse case that comes up first: pointing it at customer-facing workflows. The question that surfaces it:

> *"Could I use FAMP to sync between two of my production sites — when a customer takes an action on one, automatically update state on the other?"*

**No.** FAMP delivery requires an open Claude Code (or Codex) window actively reading the inbox. Close the window, the inbox stalls. There is no autonomous daemon servicing scheduled work; there's just a Rust process signing envelopes on behalf of whichever agent is currently using it.

What FAMP is good at:
- Two windows on the same Mac asking each other questions across loaded repo contexts
- Hook-driven notifications between agents working on related codebases — audit logs, migration coordination, cross-site refactor sync
- Judgment-tier coordination: *"I need agent-B's read on this before I touch X"*

What needs a real backend, not FAMP:
- Calendar sync between two production sites
- Customer-state replication
- Background jobs that must run while no one is at a keyboard
- Anything where availability is part of the contract

Rule of thumb: **if the use case survives a closed laptop, FAMP is not the right layer.** Use a database, a queue, a webhook, or whatever your platform's actual sync primitive is. FAMP coordinates the agents working on those systems — not the systems themselves.

## Quick Start

This is the v0.9 local-first path; if you need cross-host federation, see
[docs/MIGRATION-v0.8-to-v0.9.md](docs/MIGRATION-v0.8-to-v0.9.md).

```bash
# Install once (one-time compile, ~60-120s)
cargo install famp
famp install-claude-code

# In one Claude Code window:
/famp-register alice

# In another Claude Code window:
/famp-register bob

# Then ask alice's Claude: "send bob a message saying ship it"
# Then ask bob's Claude:   "what's in my inbox?"
```

> First install includes a one-time compile (~60-120 s); subsequent windows: <30 s. The 12-line block above is the entire onboarding.

Use the live CLI directly when you are not inside Claude Code:

```bash
famp register architect
famp send --to bob --new-task "ship it"
famp inbox --as bob
```

Full CLI:

| Command | What it does |
|---|---|
| `famp register <name>` | Bind a local identity and start the broker if needed |
| `famp send --to <name> --new-task "<text>"` | Send a new task over the local bus |
| `famp send --to <name> --task <id> --body "<text>"` | Reply to an existing task |
| `famp await [--task <id>]` | Block until a message arrives |
| `famp inbox [--include-terminal]` | List active inbox work |
| `famp join <channel>` / `famp leave <channel>` | Manage local bus channel membership |
| `famp sessions` | Show registered broker sessions |
| `famp whoami` | Show the resolved local identity |
| `famp install-claude-code` / `famp uninstall-claude-code` | Install or remove Claude Code MCP/slash-command integration |
| `famp install-codex` / `famp uninstall-codex` | Install or remove Codex MCP integration |

The v0.8 `famp-local` wrapper has moved into history at
[`docs/history/v0.9-prep-sprint/famp-local/famp-local`](docs/history/v0.9-prep-sprint/famp-local/famp-local).
v0.9 replaces it with the local bus path above.

## Broker lifecycle

The local broker auto-spawns on first registration or bus command. For normal
development, rebuild the binary and open a new CLI/MCP session; the next bus
connection starts the broker again if needed. Broker diagnostics live under
`~/.famp/` (`bus.sock`, `broker.log`).

## Advanced: v0.8 federation CLI

The v0.8 federation CLI (`famp init / setup / listen / peer add / peer import`)
was removed in v0.9. See [docs/MIGRATION-v0.8-to-v0.9.md](docs/MIGRATION-v0.8-to-v0.9.md)
for the migration path; the `v0.8.1-federation-preserved` git tag is the
escape hatch for users who genuinely need cross-host messaging today (frozen,
bug fixes ship via the v1.0 federation gateway when it lands).

## MCP Integration (Claude Code and Codex)

FAMP ships an MCP stdio server (`famp mcp`) that exposes six tools:
`famp_register`, `famp_whoami`, `famp_send`, `famp_await`, `famp_inbox`,
`famp_peers`. The model: **one MCP server config per client; the
window picks an identity at runtime via `famp_register`.**

### Onboarding (recommended path)

1. **Install the user-scope MCP integration once:**
   ```sh
   famp install-claude-code
   ```
   This writes the user-scope Claude Code config and slash commands for
   `famp mcp`. Project `.mcp.json` files are optional; if you keep one, it
   should point at `famp mcp` without `FAMP_HOME` or `FAMP_LOCAL_ROOT`.

2. **In every new Claude Code (or Codex) window opened in that repo:**
   ```text
   register as alice
   ```
   (Or any identity initialized under `~/.famp-local/agents/<name>/`.)

3. **Confirm the binding:**
   ```text
   famp_whoami
   ```
   Returns `{ "identity": "alice", "source": "explicit" }`.

4. **Multi-window dogfooding:** open a second window in the same repo,
   register as a different identity, and the two windows act as two
   FAMP peers. Messaging tools (`famp_send`, `famp_await`, etc.) refuse
   with a typed `not_registered` error until you call `famp_register`.

### Codex (one server, runtime identity)

```sh
famp install-codex
```
Registers the user-scope Codex MCP server. After this lands, call
`register as <name>` per Codex window; the binding happens inside the
session.

<details>
<summary>Why this changed (v0.8.x to v0.9 trajectory)</summary>

Pre-v0.8.x, every Claude Code window in a repo inherited the same
`FAMP_HOME` from `.mcp.json`, so two windows in one repo could only
act as the same FAMP identity — wrong abstraction for same-host
multi-agent dogfooding.

v0.8.x adopts the **session-bound identity model** ([spec](docs/superpowers/specs/2026-04-25-session-bound-identity-selection.md)):
the MCP server starts unbound, the window picks identity at runtime
via `famp_register`, and per-window state is process-scoped (one
`famp mcp` subprocess per window).

This is a **pull-forward of the v0.9 MCP contract** onto the v0.8
transport. v0.9 (the [local-first bus](docs/superpowers/specs/2026-04-17-local-first-bus-design.md))
replaces the transport entirely; the `famp_register` / `famp_whoami`
tool surface stays the same, so anything you wire today is forward-compatible.

Migration note: v0.8 project `.mcp.json` files that carry `FAMP_HOME` should
be edited manually or replaced with the user-scope install above. See
[docs/MIGRATION-v0.8-to-v0.9.md](docs/MIGRATION-v0.8-to-v0.9.md).
</details>

### Manual MCP server config

If you don't use the wrapper, the minimal `.mcp.json` is:

```json
{
  "mcpServers": {
    "famp": {
      "command": "/absolute/path/to/famp",
      "args": ["mcp"]
    }
  }
}
```

Optional: set `FAMP_LOCAL_ROOT` in the environment to override the
default `~/.famp-local` backing-store directory. Identity directories
live at `$FAMP_LOCAL_ROOT/agents/<name>/`; each must contain a readable
`config.toml` (created by `famp init` against that dir).

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
- `v0.8`: usable from Claude Code, with Codex support via user-scope MCP registration, shipped
- `v0.9`: **local-first bus** — shipping now. UDS-backed broker replacing
  the per-identity TLS listener mesh for same-host agents. See the
  [design spec](docs/superpowers/specs/2026-04-17-local-first-bus-design.md).
- `v1.0`: federation profile — after v0.9. Agent Cards, delegation,
  provenance, cross-host via a `famp-gateway` process.

See [`docs/history/ROADMAP.md`](docs/history/ROADMAP.md) for the curated
roadmap snapshot and [`docs/history/MILESTONES.md`](docs/history/MILESTONES.md)
for milestone history.

## License

Dual-licensed under Apache-2.0 OR MIT. See [LICENSE-APACHE](LICENSE-APACHE) and
[LICENSE-MIT](LICENSE-MIT).

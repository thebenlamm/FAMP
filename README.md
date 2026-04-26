# FAMP — Federated Agent Messaging Protocol (Rust reference implementation)

[![CI](https://github.com/thebenlamm/FAMP/actions/workflows/ci.yml/badge.svg)](https://github.com/thebenlamm/FAMP/actions/workflows/ci.yml)
[![License: Apache-2.0 OR MIT](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](#license)
[![Rust 1.89+](https://img.shields.io/badge/rust-1.89%2B-orange.svg)](rust-toolchain.toml)

**Status:** `v0.8 Usable from Claude Code; Codex supported via user-scope MCP registration`

> **On the version numbers:** FAMP v0.5.1 is the protocol spec; v0.6 / v0.7 / v0.8 are
> implementation milestones (all shipped); v0.9 (local-first bus) is in design and v1.0
> (federation profile) follows. The library version in `Cargo.toml` is `0.1.0` pre-release.

FAMP is a Rust implementation of the Federated Agent Messaging Protocol.

**The fastest thing to try:** get two Claude Code or Codex windows on your Mac
exchanging signed messages, via the [`famp-local`](scripts/famp-local)
wrapper shipped in this repo. See [Quick Start (local)](#quick-start-local)
below — four commands for Claude Code, plus one Codex registration command;
no cert wrangling, no peer-card piping.

Under the hood it's a v0.5.1-spec-conformant stack: canonical JSON
(RFC 8785), Ed25519 signatures with domain separation, typed identity
and envelope types, a 5-state task FSM, and an HTTPS transport with
TOFU pinning. The raw federation CLI (`famp setup / listen / send /
peer add`) is still there if you want manual control or cross-machine
setup; the local wrapper is just an ergonomics layer over those same
primitives.

- **Local (v0.8 + `famp-local`)** — same-host agents, one command to
  wire a directory into a mesh.
- **Federation Profile (v0.9 / v1.0)** — cross-host protocol, Agent
  Cards, delegation, provenance. v0.9 re-scopes the local path into a
  proper socket-activated broker; federation moves to a v1.0 gateway.
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
- Minimal HTTPS transport built on `axum`, `reqwest`, and `rustls`
- TOFU keyring binding `Principal -> VerifyingKey`
- **Full CLI** with streamlined onboarding:
  - `famp setup` — one-command identity creation with auto port selection
  - `famp info` — output peer card for sharing
  - `famp peer import` — import peer cards from other agents
  - `famp listen` — run the HTTPS daemon
  - `famp send` — send signed envelopes
  - `famp inbox list` — inspect received messages (hides entries for terminal tasks by default; pass `--include-terminal` to see them)
  - `famp await` — block until new messages arrive (unfiltered; canonical real-time signal, including task completion)
- **MCP server** (`famp mcp`) for Claude Code and Codex
- Two runnable examples:
  - same-process happy path
  - cross-machine HTTPS happy path

## Not Shipped Yet

**v0.9 — Local-First Bus** (in design):
- UDS-backed broker with socket-activated lifecycle
- IRC-style channels / broadcast primitive (`#name`)
- Zero-crypto same-host path (filesystem is the trust boundary)
- `famp_register` MCP tool so windows pick their identity at session start
  instead of via pre-configured `FAMP_HOME` env vars
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

# 5. Verify the workspace
just ci
```

## Quick Start (local)

Two Claude Code or Codex windows on the same Mac, exchanging signed messages.
Claude Code is four commands; Codex adds one MCP registration command:

```bash
# 1. Install the famp binary to ~/.cargo/bin
cargo install --path crates/famp

# 2. Wire each repo directory into the mesh
#    - creates an identity (default name = basename of the dir)
#    - exchanges peer cards, pre-pins TLS fingerprints
#    - starts background daemons
#    - drops a project-scoped .mcp.json
#    - updates ~/.zprofile so daemons come back at next login
scripts/famp-local wire ~/Workspace/RepoA
scripts/famp-local wire ~/Workspace/RepoB

# 3a. Claude Code: restart the windows for RepoA and RepoB
#     (`famp-local wire` dropped a project-scoped .mcp.json)
#
# 3b. Codex: register both identities once, then restart the windows
#     (global user-scope registration in ~/.codex/config.toml)
scripts/famp-local mcp-add --client codex RepoA RepoB

# 4. In either window, ask the agent to send a message:
#    "send a message to RepoB saying hello"
#    The MCP client picks up the `famp_send` / `famp_inbox` / `famp_await`
#    MCP tools and the message lands on the other side.
```

Override the identity name per-directory if the repo name doesn't fit:

```bash
scripts/famp-local wire ~/Workspace/God --as architect
```

Full CLI:

| Command | What it does |
|---|---|
| `famp-local wire <dir> [--as <name>] [--force]` | Add a directory to the mesh and drop a project-scoped `.mcp.json` for Claude Code |
| `famp-local unwire <dir>` | Remove `.mcp.json` from a directory (identity and daemon stay) |
| `famp-local send <from> <to> <text>` | CLI-level send without going through the MCP client |
| `famp-local inbox <name>` | List a name's inbox entries |
| `famp-local status` | Show all known identities and daemon state |
| `famp-local stop [<name>...]` | Stop daemon(s); with no args, stops all |
| `famp-local clean` | Stop everything and wipe `~/.famp-local` |
| `famp-local mcp-add [--client <target>] <name>...` | Register user-scope MCP servers for Claude Code, Codex, or both |
| `famp-local mcp-remove [--client <target>] <name>...` | Remove user-scope MCP server registrations |

`famp-local` is a bash wrapper around the v0.8 CLI — see
[`scripts/famp-local`](scripts/famp-local). It exists to compress the raw
eight-step federation flow (below, under "Advanced") into one command
while v0.9's proper socket-activated broker is in design. When v0.9 ships,
the wrapper goes away and `famp-local wire` becomes a single-line install
of the broker plus one MCP registration.

## Redeploying after daemon code changes

Edits to `crates/famp/` only reach running listeners after the binary at
`~/.cargo/bin/famp` is rebuilt AND each daemon is restarted. Use:

```bash
scripts/redeploy-listeners.sh             # interactive: prompts before killing daemons
scripts/redeploy-listeners.sh --dry-run   # show plan, take no action
scripts/redeploy-listeners.sh --force     # skip the in-flight-task safety check
scripts/redeploy-listeners.sh --no-rebuild # cycle daemons against the binary already on disk
```

The script refuses to run if `crates/famp/` has uncommitted changes or if
any task TOML under `~/.famp-local/agents/*/tasks/*.toml` is in a
non-terminal state (REQUESTED or COMMITTED), unless you pass `--force`.
PID files live at `~/.famp-local/agents/<name>/daemon.pid`; logs at
`~/.famp-local/agents/<name>/daemon.log` (appended, not truncated).

### Verifying a redeploy succeeded

The script prints a per-agent summary table on completion (`STOP`, `RESTART`,
`PID`, `LOG` columns) followed by a final `all N agent(s) cycled cleanly`
line; non-zero exit means at least one daemon failed to come back. To
spot-check independently: `tail -1 ~/.famp-local/agents/<name>/daemon.log`
should show a fresh `listening on https://127.0.0.1:<port>` line, and
`ls -l ~/.cargo/bin/famp` should show a binary timestamp at or after the
rebuild.

## Advanced: manual CLI (federation path)

The raw federation-grade flow. Use this for cross-machine setups, or when
you want explicit control over ports, HOME directories, TOFU pinning, and
peer-card exchange. On a single Mac with two Claude Code or Codex windows, the
[`famp-local`](#quick-start-local) wrapper above will do all of this for
you.

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
#    `--new-task "<summary>"` opens a fresh task; `send` prints the task UUID on success.
FAMP_TOFU_BOOTSTRAP=1 FAMP_HOME=/tmp/famp-alice ./target/release/famp send \
  --to bob --new-task "hello from alice"

# 6. Inspect Bob's inbox (newest-last; use `inbox ack` to advance the read cursor)
FAMP_HOME=/tmp/famp-bob ./target/release/famp inbox list
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

## MCP Integration (Claude Code and Codex)

For **local Claude Code on your Mac**, use
[`scripts/famp-local wire <dir>`](#quick-start-local) — it generates a
project-scoped `.mcp.json` in the target directory pointing at that
repo's identity, using the absolute `famp` binary path so Claude's MCP
spawner (which doesn't inherit login-shell PATH) can find it.

For **local Codex on your Mac**, use the same `wire` step to create the
identity and daemons, then register the identity with:

```bash
scripts/famp-local mcp-add --client codex <name>
```

That uses `codex mcp add ...` under the hood and writes a user-scope MCP
entry into `~/.codex/config.toml`.

Important difference from Claude Code: Codex registration is **global per
user**, not repo-scoped. After you register `famp-alice` and `famp-bob`,
every Codex window can see both MCP servers. In Codex, the identity you
use is the MCP server name you select, not the repo you opened.

To remove a Codex registration later:

```bash
scripts/famp-local mcp-remove --client codex <name>
```

For **manual setups**, the `.mcp.json` shape is:

```json
{
  "mcpServers": {
    "famp": {
      "command": "/Users/you/.cargo/bin/famp",
      "args": ["mcp"],
      "env": { "FAMP_HOME": "/path/to/identity/home" }
    }
  }
}
```

The MCP server exposes four tools:
- `famp_send` — send signed envelopes (request / deliver / terminal)
- `famp_inbox` — list received messages; `action=list` hides entries for terminal tasks by default (pass `include_terminal: true` to bypass)
- `famp_await` — block until new messages arrive (poll-free inter-agent dialogue; unfiltered — the canonical real-time signal for task completion)
- `famp_peers` — list peers

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
- `v0.9`: **local-first bus** — in design. UDS-backed broker replacing
  the per-identity TLS listener mesh for same-host agents. See the
  [design spec](docs/superpowers/specs/2026-04-17-local-first-bus-design.md).
  The `famp-local` wrapper is pre-v0.9 scaffolding that validates the
  UX before the broker lands.
- `v1.0`: federation profile — after v0.9. Agent Cards, delegation,
  provenance, cross-host via a `famp-gateway` process.

See [`docs/history/ROADMAP.md`](docs/history/ROADMAP.md) for the curated
roadmap snapshot and [`docs/history/MILESTONES.md`](docs/history/MILESTONES.md)
for milestone history.

## License

Dual-licensed under Apache-2.0 OR MIT. See [LICENSE-APACHE](LICENSE-APACHE) and
[LICENSE-MIT](LICENSE-MIT).

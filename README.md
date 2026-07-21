# FAMP — Federated Agent Messaging Protocol (Rust reference implementation)

[![CI](https://github.com/thebenlamm/FAMP/actions/workflows/ci.yml/badge.svg)](https://github.com/thebenlamm/FAMP/actions/workflows/ci.yml)
[![License: Apache-2.0 OR MIT](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](#license)
[![Rust 1.89+](https://img.shields.io/badge/rust-1.89%2B-orange.svg)](rust-toolchain.toml)

**Status:** `v0.11 Broker Daemon`

> **On the version numbers:** FAMP v0.5.1 is the protocol spec; v0.6 / v0.7 / v0.8 are
> implementation milestones (all shipped); v0.9 is the local-first bus and v0.11 adds
> the persistent broker daemon; v1.0 is the federation gateway. The workspace version
> is unified to `0.11.0` (`famp -V` → `famp 0.11.0`).

FAMP today is local-first: a UDS-backed broker for same-host agent messaging
with zero crypto on the local path. FAMP at v1.0 is federated: cross-host
messaging via a `famp-gateway` wrapping the local bus, all of v0.5.2's
signature/canonical-JSON guarantees preserved. The v1.0 trigger condition
is documented in [ARCHITECTURE.md](ARCHITECTURE.md).

**The fastest thing to try:** get two Claude Code or Codex windows on your Mac
exchanging messages via the local bus. See [Quick Start](#quick-start)
below; no cert wrangling, no peer-card piping.

Under the hood it's a v0.5.2-spec-conformant stack: canonical JSON
(RFC 8785), Ed25519 signatures with domain separation, typed identity
and envelope types, and a 5-state task FSM. The local bus is the v0.11
runtime path; federation transport internals remain preserved for v1.0.

- **Local (v0.11)** — same-host agents through a local UDS broker (persistent daemon, or auto-spawn for unsandboxed clients).
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
  - `famp inbox list` — inspect received messages (includes posts from joined channels; `--include-terminal` is accepted but currently a no-op — broker-side terminal filtering is deferred to v1)
  - `famp await` — block until new messages arrive (unfiltered; canonical real-time signal, including task completion)
  - `famp join` / `famp leave` — manage channel membership
- **MCP server** (`famp mcp`) for Claude Code and Codex
- **Local-first bus (v0.9, shipped):** UDS-backed broker replacing the
  per-identity TLS listener mesh for same-host agents; IRC-style channels /
  broadcast primitive (`#name`); zero-crypto same-host path (filesystem is
  the trust boundary). See the full
  [design spec](docs/superpowers/specs/2026-04-17-local-first-bus-design.md).
- **Broker daemon & cross-tool bootstrap (v0.11, shipped):** `famp daemon
  install` runs the broker as a service-managed daemon (launchd on macOS,
  systemd `--user` on Linux) so it survives across sessions instead of
  relying on per-client auto-spawn; version handshake at connect catches
  daemon/client skew.
- Two runnable examples:
  - same-process happy path
  - cross-machine HTTPS happy path

## Not Shipped Yet

**v1.0 — Federation Profile** (after v0.11):
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
- Rust 1.89+ — the Quick Start installs `rustup` if you don't have it
- The first build also installs `rustfmt` + `clippy` (pinned in
  `rust-toolchain.toml`) — `just ci`, `just lint`, and the pre-push git hook
  all require them, so expect that extra download on a fresh/offline install.

## Build from Source (contributors)

If you are contributing to FAMP or want to build the binary from a local clone:

```bash
# 1. Install rustup (skip if already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none

# 2. Add cargo/rustup to the current shell's PATH
#    (rustup's installer edits your shell profile, but that only takes effect
#    in new shells — this line activates it here too)
source "$HOME/.cargo/env"

# 3. Clone the repo and enter it (rust-toolchain.toml auto-installs 1.89.0)
git clone https://github.com/thebenlamm/FAMP.git
cd FAMP
rustc --version

# 4. Install the binary from the local clone
cargo install --path crates/famp

# 5. (Contributors only) Install dev tools and verify the workspace
cargo install cargo-nextest --locked
cargo install just --locked
just install-hooks
just ci
```

## Upgrading

If you installed FAMP previously and want the latest:

```bash
# In your local FAMP clone
git pull
cargo install --path crates/famp

# If you installed the broker as a service, pick up the new binary:
famp daemon restart

famp --version
```

Then restart any open Claude Code windows — they pick up the new binary on next launch. A client that hits a not-yet-restarted long-lived daemon gets a version-skew (ProtocolMismatch) error telling it to run `famp daemon restart` (VER-01).

## When NOT to Use FAMP

FAMP is dev-time coordination between agents. It is not a production data-sync layer.

The misuse case that comes up first: pointing it at customer-facing workflows. The question that surfaces it:

> *"Could I use FAMP to sync between two of my production sites — when a customer takes an action on one, automatically update state on the other?"*

**No.** FAMP delivery requires an open Claude Code (or Codex) window actively reading the inbox. Close the window, the inbox stalls. The daemon keeps the message broker running; it does not service work autonomously — delivery still requires an open agent window reading its inbox. The broker is just a Rust process relaying signed envelopes on behalf of whichever agents are currently connected: the daemon restores broker *presence*, not agent *attendance*.

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

Install a persistent broker once with `famp daemon install`; afterward every
Claude Code and Codex window on your Mac connects with no per-session broker
setup. If you need cross-host federation, see
[docs/MIGRATION-v0.8-to-v0.9.md](docs/MIGRATION-v0.8-to-v0.9.md).

```bash
# 1. Install Rust (skip if already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none
source "$HOME/.cargo/env"
# 2. Install famp (~60-120s first-run compile)
cargo install famp
# 3. Install the persistent broker — run ONCE from a normal (unsandboxed) shell.
famp daemon install
# 4. Wire each tool's MCP integration:
famp install-claude-code
famp install-codex
# 5. In one Claude Code window:   /famp-register alice
# In another (Claude Code or Codex): register as bob — then ask alice to message bob.
```

> First install includes a one-time compile (~60-120 s); subsequent windows: <30 s.

`famp daemon install` is the one command that ends broker-babysitting: it
installs a persistent user-level broker (launchd on macOS, systemd `--user` on
Linux) that stays reachable across reboot and logout. Re-running
`famp daemon install` is safe when the service is already installed. It must be
run from a normal (unsandboxed) shell — it refuses to run inside a sandbox; if
you cannot run an unsandboxed install, use the [no-install bridge](#no-install-bridge)
below.

Use the live CLI directly when you are not inside Claude Code:

```bash
famp register architect
famp send --to bob --new-task "ship it"
famp inbox --as bob
```

Full CLI:

| Command | What it does |
|---|---|
| `famp register <name>` | Bind a local identity (auto-spawns a broker for unsandboxed clients if none is running; sandboxed clients need a daemon or the bridge) |
| `famp send --to <name> --new-task "<text>"` | Send a new task over the local bus |
| `famp send --to <name> --task <id> --body "<text>"` | Reply to an existing task |
| `famp await [--task <id>]` | Block until a message arrives |
| `famp inbox [--include-terminal]` | List unread agent + joined-channel envelopes (the `--include-terminal` flag is wire-accepted but currently a no-op) |
| `famp join <channel>` / `famp leave <channel>` | Manage local bus channel membership |
| `famp sessions` | Show registered broker sessions |
| `famp whoami` | Show the resolved local identity |
| `famp daemon install` | Install the broker as a persistent user-level service (launchd / systemd `--user`); idempotent |
| `famp daemon status` | Report daemon state — exits 0 running / 1 not-installed / 2 installed-but-down |
| `famp daemon restart` | Restart the daemon, picking up a new on-disk binary after `cargo install` |
| `famp daemon uninstall` | Stop and remove the service; idempotent |
| `famp broker --no-idle-exit` | Run the broker in the foreground with no 300s idle exit (no-install bridge) |
| `famp install-claude-code` / `famp uninstall-claude-code` | Install or remove Claude Code MCP/slash-command integration |
| `famp install-codex` / `famp uninstall-codex` | Install or remove Codex MCP plus project Stop-hook integration |

The v0.8 `famp-local` wrapper has moved into history at
[`docs/history/v0.9-prep-sprint/famp-local/famp-local`](docs/history/v0.9-prep-sprint/famp-local/famp-local).
v0.9 replaces it with the local bus path above.

### No-install bridge

If you cannot or will not install a service, run the broker yourself in one
unsandboxed terminal:

```bash
famp broker --no-idle-exit
```

Any client — sandboxed Codex or normal Claude Code — then connects to that
broker. Leave the terminal open; the broker lives as long as the terminal does.

The daemon survives reboot and logout (`RunAtLoad` + `KeepAlive`); the bridge is
a single foreground terminal process and dies on terminal-close or logout.

## Platform support

`famp daemon install` covers:

- **macOS** — launchd LaunchAgent (`com.famp.broker`).
- **Linux** — systemd `--user` unit (requires systemd ≥ 240 and an active user
  session; run `loginctl enable-linger <user>` to keep the broker alive after
  logout).

It does **not** cover the configurations below. On these, `famp daemon install`
exits non-zero rather than silently half-installing — use the no-install bridge
(`famp broker --no-idle-exit`) above instead:

- minimal distros without systemd
- containers
- WSL
- headless hosts without `loginctl enable-linger`
- any non-macOS / non-Linux platform

On Linux, when `systemctl` is absent the installer exits with a message naming
`famp broker --no-idle-exit` as the fallback; on a headless host it prints (it
does not auto-run) the `loginctl enable-linger` command you need.

## Broker lifecycle

With `famp daemon install` (the [recommended path](#quick-start)), a persistent
broker is always running.

Without a daemon, the broker **auto-spawns** on first registration for
unsandboxed clients (e.g. Claude Code) — but a sandboxed client like Codex
cannot spawn its own broker, and an auto-spawned broker idle-exits after 300 s.
That is why the daemon (or the [no-install bridge](#no-install-bridge)) is what
lets a sandboxed Codex window connect and what keeps a broker alive beyond a
single session. Broker diagnostics live under `~/.famp/` (`bus.sock`,
`broker.log`).

## Advanced: v0.8 federation CLI

The v0.8 federation CLI (`famp init / setup / listen / peer add / peer import`)
was removed in v0.9. See [docs/MIGRATION-v0.8-to-v0.9.md](docs/MIGRATION-v0.8-to-v0.9.md)
for the migration path; the `v0.8.1-federation-preserved` git tag is the
escape hatch for users who genuinely need cross-host messaging today (frozen,
bug fixes ship via the v1.0 federation gateway when it lands).

## MCP Integration (Claude Code and Codex)

FAMP ships an MCP stdio server (`famp mcp`) that exposes eight tools:
`famp_register`, `famp_whoami`, `famp_send`, `famp_await`, `famp_inbox`,
`famp_peers`, `famp_join`, `famp_leave`. The model: **one MCP server config
per client; the window picks an identity at runtime via `famp_register`.**

### Onboarding (recommended path)

1. **Install the broker service once, then wire the MCP integration:**
   ```sh
   famp daemon install      # persistent broker — see Quick Start (skip if already installed)
   famp install-claude-code
   ```
   This writes the user-scope Claude Code MCP config, slash commands
   (`/famp-register`, `/famp-inbox`, etc.), the Stop hook, and the
   listen-mode await shim. Project `.mcp.json` files are optional; if you
   keep one, it should point at `famp mcp` without `FAMP_HOME` or
   `FAMP_LOCAL_ROOT`.

2. **In every new Claude Code (or Codex) window opened in that repo:**
   ```text
   register as alice
   ```
   (Any alphanumeric name — the broker creates the binding on first register. No directory to pre-provision.)

3. **Confirm the binding:**
   ```text
   famp_whoami
   ```
   Returns `{ "identity": "alice", "source": "explicit" }`.

4. **Send a message:** say `send bob: ship it` and Claude calls
   `famp_send` with `mode: "open"` (starts a thread). To reply and close
   say `reply to task <id>: looks good` — Claude uses `mode: "reply"`,
   which closes the thread by default. Add `expect_reply: true` to keep
   it open for a follow-up. Messaging tools refuse with a typed
   `not_registered` error until you call `famp_register`.

5. **Multi-window dogfooding:** open a second window in the same repo,
   register as a different identity, and the two windows act as two
   FAMP peers.

### Codex (one server, runtime identity)

```sh
famp install-codex
```
Registers the user-scope Codex MCP server and adds a project-local Stop hook
that wakes listen-mode Codex windows when FAMP messages arrive. After this
lands, call `register as <name>` per Codex window; the binding happens inside
the session.

### Peer discovery (`famp_peers`)

`famp_peers` returns the identities currently registered on the local broker —
i.e. who is reachable right now via `famp_send`:

```json
{ "online": ["alice", "bob"] }
```

Identities appear here only while their `famp_register` session is alive.
Closing a window removes them. Use it to confirm a target is up before sending.

### Channels (`famp_join` / `famp_leave`)

Channels are IRC-style broadcast groups. Every member receives every message
sent to the channel. The `#` prefix is optional — both `planning` and
`#planning` work as the target. Channels are created automatically the first
time any agent joins — no pre-creation step required.

```bash
# Terminal A
famp register alice
famp join #planning

# Terminal B
famp register bob
famp join #planning

# Send to all #planning members
famp send --to '#planning' --new-task "standup in 5"

# Both alice and bob now have it in their inboxes
famp inbox
famp leave #planning
```

From Claude Code: say `join #planning`, then `send #planning "standup in 5"`.
The MCP tools `famp_join` and `famp_leave` handle membership directly.

### On-demand blocking wait (`famp_await`)

`famp_await` blocks until a new message arrives (or up to 23 h). It is the
primitive that listen mode is built on. Two ways to use it:

- **Listen mode (recommended for dedicated agent windows):** pass `listen: true`
  to `famp_register` and the Stop hook calls `famp_await` for you after every
  turn — you never invoke it directly.
- **Manual:** ask the agent to "wait for a famp message" and Claude calls
  `famp_await` once. Useful for a one-shot blocking handoff without committing
  the whole window to listen mode.

General-purpose dev windows should use neither — call `famp_inbox` on demand.

### Listen Mode (inbound wake-up)

By default, a registered window checks its inbox on demand. Passing
`listen: true` to `famp_register` turns the window into an always-on
receiver: after every turn, the Stop hook blocks waiting for an inbound
message and wakes Claude automatically when one arrives (sub-minute
latency).

```text
register as dk with listen mode on
```

Claude calls `famp_register({identity: "dk", listen: true})`. When a
peer sends `famp send --to dk --new-task "..."`, the window wakes with:

```
[FAMP listen mode] New message from <sender>. Call famp_inbox to read it.
```

Claude then calls `famp_inbox` to read the content. The `[FAMP listen mode]`
prefix in the Claude Code UI distinguishes expected wakes from actual hook
errors.

Use listen mode for dedicated agent windows (e.g. a 5-agent mesh where each
window must respond to peers immediately). Omit it for general-purpose dev
windows that check inbox on demand.

#### Wiring an existing multi-repo setup to listen mode

If you have multiple repos already wired (e.g. `dk`, `tovani`, `dbs`, `infra`,
`openheart`), this is the full upgrade sequence:

**Step 1 — Pull and rebuild** (see [Upgrading](#upgrading) above)

**Step 2 — Update each repo's CLAUDE.md**

Find the `famp_register` instruction in each CLAUDE.md and add `listen: true`:

```
# Before
register as dk

# After
register as dk with listen mode on
```

Claude translates "with listen mode on" to
`famp_register({identity: "dk", listen: true})` automatically.

**Step 3 — Restart open windows**

Any Claude Code window already open must be restarted to pick up both the new
binary and the updated CLAUDE.md. First message in each new window:

```
register as <name> with listen mode on
```

**Step 4 — Verify**

Send a test message from one window to another. The receiving window should
wake automatically with:

```
[FAMP listen mode] New message from <sender>. Call famp_inbox to read it.
```

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
default `~/.famp-local` backing-store directory. MCP windows pick identity
at runtime via `famp_register` and don't use this path. It matters for
non-MCP CLI use where `wires.tsv` cwd→identity binding is in effect.

### Two directories: `~/.famp` vs `~/.famp-local`

FAMP uses two separate directories on disk, and both are live:

- **`~/.famp`** — the message runtime: the broker's UDS socket
  (`~/.famp/bus.sock`) and each identity's durable, per-name mailbox files.
  This is what `famp send` / `famp inbox` / `famp await` talk to.
- **`~/.famp-local`** — the identity backing store for **non-MCP CLI use**:
  `wires.tsv` (cwd → identity bindings, so `famp send` in a given directory
  knows which identity to act as) and `hooks.tsv`. Override its location with
  `FAMP_LOCAL_ROOT` (see above). MCP sessions bind identity in-memory via
  `famp_register` and don't touch this directory, but any non-MCP CLI
  invocation of `famp` still requires it.

An eventual unification of the two directories is tracked as a separate,
out-of-scope runtime change in a GitHub issue on this repository.

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

A green `just ci` locally implies a green main CI workflow run. The `smoke-test`
workflow (Quick Start install path) runs separately in CI and is not included in
`just ci` — run `just smoke-test` explicitly to verify it locally (~60-120s).

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
- `v0.5.2`: spec amendment adding the `audit_log` `MessageClass` (does not fire
  the task FSM), shipped alongside v0.9 Phase 1
- `v0.6`: foundation crates, shipped
- `v0.7`: personal runtime, shipped
- `v0.8`: usable from Claude Code, with Codex support via user-scope MCP registration, shipped
- `v0.9`: **local-first bus** — shipped. UDS-backed broker replacing
  the per-identity TLS listener mesh for same-host agents. See the
  [design spec](docs/superpowers/specs/2026-04-17-local-first-bus-design.md).
- `v0.10`: inspector & observability — shipped. `famp inspect broker` /
  `famp inspect identities` / `famp inspect tasks` / `famp inspect messages`
  for read-only broker diagnosis without registration.
- `v0.11`: **broker daemon & cross-tool bootstrap** — shipped, current
  runtime. `famp daemon install` runs a service-managed broker (launchd /
  systemd `--user`) so Claude Code and Codex connect to a persistent broker
  instead of relying on per-client auto-spawn; version handshake at connect.
- `v1.0`: federation profile — after v0.11. Agent Cards, delegation,
  provenance, cross-host via a `famp-gateway` process.

See [`docs/history/ROADMAP.md`](docs/history/ROADMAP.md) for the curated
roadmap snapshot and [`docs/history/MILESTONES.md`](docs/history/MILESTONES.md)
for milestone history.

## Troubleshooting

- **Broker won't start / commands hang.** Check `~/.famp/broker.log` for the
  last startup error. The socket is `~/.famp/bus.sock`; if a stale socket file
  is blocking startup, remove it and retry.
- **`not_registered` error from an MCP tool.** The window hasn't bound an
  identity yet. Say `register as <name>` (or `register as <name> with listen
  mode on` for listen-mode windows).
- **A peer doesn't appear in `famp_peers`.** Their session has exited.
  Re-register in that window.
- **Listen-mode window doesn't wake on a message.** Verify the Stop hook is
  installed (`famp install-claude-code` for Claude Code,
  `famp install-codex` for Codex). Check `~/.famp/broker.log` for `await`
  activity around the send time.
- **Stuck after a binary upgrade.** Restart all Claude Code windows (they cache
  the binary path at launch) AND, if you run the broker as a service, run
  `famp daemon restart` so the daemon picks up the new binary — otherwise a
  version-skew (ProtocolMismatch) error fires.
- **Not sure if the broker is up.** Run `famp daemon status` (RUNNING /
  INSTALLED_DOWN / NOT_INSTALLED).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

Dual-licensed under Apache-2.0 OR MIT. See [LICENSE-APACHE](LICENSE-APACHE) and
[LICENSE-MIT](LICENSE-MIT).

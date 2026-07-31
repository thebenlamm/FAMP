# Architecture

## Past state (v0.8)

**Federation-first.** Every agent is an independent identity with:
- A persistent `FAMP_HOME` directory containing its Ed25519 keypair, self-signed
  TLS cert, config, peers.toml, and durable inbox.
- A `famp listen` HTTPS daemon on a dedicated TCP port.
- A TOFU-pinned peer registry — first-contact pins the leaf-cert SHA-256;
  subsequent connections reject on mismatch.

All inter-agent traffic on the wire is signed over canonical JSON
(RFC 8785) with Ed25519 under the domain prefix `FAMP-sig-v1\0`, every
envelope, no exceptions (INV-10). Five message classes: `request`,
`commit`, `deliver`, `ack`, `control/cancel`. Five-state task FSM
(`famp-fsm`): REQUESTED → COMMITTED → {COMPLETED | FAILED | CANCELLED},
all terminal absorbing.

The MCP server (`famp mcp`, stdio JSON-RPC) exposes six tools:
`famp_register`, `famp_whoami`, `famp_send`, `famp_inbox`, `famp_await`,
`famp_peers`. The server starts **unbound** — identity is not read from
`FAMP_HOME` at startup. Each window calls `famp_register` once at session
start to bind an identity by name; the name resolves to
`$FAMP_LOCAL_ROOT/agents/<name>/` (default `~/.famp-local/agents/<name>/`).
Pre-registration calls to `famp_send`, `famp_inbox`, `famp_await`, and
`famp_peers` return a typed `not_registered` error. `famp_whoami` reports
the current binding and never errors. `famp_inbox` action=list merges
the agent mailbox with any joined-channel mailboxes (Scope B, 2026-06-19,
commits `70b0d43` + `ad77c56`); per-channel cursors are tracked
independently from the `famp_await` channel cursor, so a post may
surface on either surface (or both) for a given holder. `famp_await`
stays unfiltered and is the canonical real-time signal for task
completion. The `include_terminal` flag on `famp_inbox` is accepted on
the wire but currently a no-op — broker-side terminal-FSM filtering did not
land in v1.0 and remains deferred (would require the famp-bus actor to read famp-taskdir,
which crosses the transport-vs-cli boundary).

Note: the federation transport side (`famp listen`, `famp setup`,
`famp send`, `famp peer import`) still reads `FAMP_HOME` per identity —
each identity's keypair, TLS cert, and durable inbox live under that
directory. The bifurcation (MCP session-bound; federation `FAMP_HOME`-based)
is intentional and collapses when v0.9's local bus replaces the transport.

## v0.9 — local-first bus (shipped at the v0.9.0 tag)

Observed during dogfooding: forcing same-host, same-user agents to pay
federation-grade costs (cert generation, TOFU pinning, per-identity HOME
dirs, peer-card exchange) made basic onboarding require 8+ manual steps.
The filesystem is already the trust boundary between two processes owned
by one UID; running Ed25519 signatures and TLS handshakes between them
is theatre.

The v0.9 re-scope introduces a **local bus**:
- Unix domain socket broker (`~/.famp/bus.sock`), single process, all
  same-host agents share it.
- Zero crypto on the bus — no signing, no TLS, no TOFU.
- IRC-style channels (`#planning`) as a first-class primitive for 3+ agent
  broadcast.
- Durable per-name mailboxes (reuses `famp-inbox` format) so offline
  recipients queue rather than fail.
- Stable MCP tool surface — v0.9 inherits `famp_register`, `famp_whoami`,
  `famp_send`, `famp_inbox`, `famp_await`, `famp_peers` unchanged from
  v0.8.x (where `famp_register` and `famp_whoami` first shipped as part
  of the session-bound identity bridge phase). v0.9 adds `famp_join` and
  `famp_leave` for IRC-style channel support; the register/whoami contract
  is not altered. The full v0.9 surface:
  `famp_register`, `famp_whoami`, `famp_send`, `famp_inbox`, `famp_await`,
  `famp_peers`, `famp_join`, `famp_leave` — the same contract that gained
  transparent remote routing in v1.0.

**Layer split:**

| Layer | Scope | Crates | Wire | Crypto |
|---|---|---|---|---|
| 0 — Protocol primitives | Transport-neutral | `famp-canonical`, `famp-crypto`, `famp-core`, `famp-fsm`, `famp-envelope` | N/A | N/A |
| 1 — Local bus (v0.9) | Same-host, same-UID | `famp-bus` (new), broker subcommand | UDS + canonical JSON framing | None |
| 2 — Federation gateway (v1.0, shipped) | Cross-host | `famp-gateway`, reuses `famp-transport-http`, `famp-keyring` | HTTPS + canonical JSON + Ed25519 | Full |

Layer 0 is untouched by v0.9. Layer 1 is net-new. Layer 2 was designed in
v0.9 but not built then — its internals (`famp-transport-http`,
`famp-keyring`) stayed compiling and tested in CI so they would not rot
before being wrapped, which `famp-gateway` did in v1.0.

Full v0.9 design:
[`docs/superpowers/specs/2026-04-17-local-first-bus-design.md`](docs/superpowers/specs/2026-04-17-local-first-bus-design.md).

## v1.0 — federation gateway (shipped at the v1.0.0 tag, 2026-07-29)

Layer 2 is built. `famp-gateway` is a separate process that connects to the
existing local bus rather than replacing it. It backs each remote principal with
a `bind_as` connection reporting the gateway's own live local PID, so the
broker's same-host `kill(pid, 0)` liveness check needed no change — `famp-bus` is
untouched by v1.0. One gateway backs multiple remote principals concurrently
with no cross-talk.

- **Wire.** Every cross-host envelope is Ed25519-signed over RFC 8785 canonical
  JSON under the `FAMP-sig-v1\0` domain prefix (INV-10), and is rejected before
  it touches the local bus if unsigned or invalid. The envelope carries
  sender/receiver domain, `sender_key_id`, `nonce`, and `expiry`;
  capability/approval fields are omitted when empty. Federation fields are
  stripped before the onward local `Send`, so relayed envelopes stay decodable
  by `famp inbox` / `famp inspect` (BUS-11).
- **Trust.** Hand-copied keys via out-of-band `famp peer export` /
  `famp peer import` with TOFU pinning. Unpinned keys are rejected; there is no
  implicit peering.
- **Addressing.** `famp send --to agent:<domain>/<name>` from the shipping client
  reaches a remote principal. A successful send confirms local acceptance into
  the gateway-backed outbound mailbox, not remote delivery — verify the far side
  with `famp inspect tasks`.
- **Scope.** Two machines under one operator, over a network that operator
  already runs. No public relay, no NAT traversal, no cross-person trust, no
  signed peer directory, no capability/approval plane.

Setup guide: [`docs/GATEWAY-SETUP.md`](docs/GATEWAY-SETUP.md).

## v1.1 — inbound-content-is-DATA quarantine (Phase 14)

Layer 1 (`famp-bus`) gained a fail-closed provenance stamp: the broker
records `origin: local | gateway | unknown` on every mailbox append,
with absence resolving to `unknown` (untrusted), never `local`. The
render helper that turns a stamped record into human/agent-visible
output (`famp::cli::render::render_envelope_body`/`render_body_text`)
lives one layer up, in the CLI/MCP layer (the `famp` binary crate) —
Layer 1 stamps provenance, Layer 2+ renders it. Layer 0 (`famp-canonical`,
`famp-crypto`, `famp-core`, `famp-fsm`, `famp-envelope`) is untouched by
this phase, same as it was untouched by v0.9 and v1.0.

Full write-up, including what this boundary does and does not deliver:
[`docs/QUARANTINE.md`](docs/QUARANTINE.md).

## Pre-v0.9 scaffolding

[`scripts/famp-local`](scripts/famp-local) is a bash wrapper over the v0.8
CLI that compresses the 8-step federation flow into one command
(`famp-local wire <dir>`) for same-host MCP clients. It auto-pins TLS
fingerprints from disk (bypassing TOFU), manages daemon lifecycles with
PID files, drops project-scoped `.mcp.json` files for Claude Code, and
can register user-scope MCP entries for Codex. That Codex path is global
per user (`~/.codex/config.toml`), not repo-scoped like Claude Code's
`.mcp.json`. It exists to validate the local-first UX before the v0.9
broker ships; when the broker lands, the script becomes redundant.

## When working in the codebase

- **Protocol-primitive crates are transport-neutral.** `famp-canonical`,
  `famp-crypto`, `famp-core`, `famp-fsm`, `famp-envelope` — used by both
  v0.9 bus and v1.0 gateway. Changes here ripple everywhere.
- **Transport crates are federation-specific.** `famp-transport-http`,
  `famp-keyring` — wrapped by `famp-gateway` (v1.0), not by `famp-bus`.
  Treat them as federation internals.
- **The MCP tool surface is the stable contract** across v0.8, v0.9, and
  v1.0. If you find yourself changing tool signatures, stop — that's a
  cross-version UX decision.

## Crate responsibilities

All 16 workspace crates at v1.0.0. Dependencies listed are intra-workspace
FAMP crates only (external deps omitted for brevity).

| Crate | Layer | One-line purpose | Depends on |
|---|---|---|---|
| `famp-canonical` | 0 | RFC 8785 JCS canonicalization; the byte-exact foundation for all signatures | — |
| `famp-core` | 0 | Core primitives: `Principal`, `Instance`, `ArtifactId`, `MessageClass`, `ProtocolErrorKind`, `AuthorityScope` | — |
| `famp-crypto` | 0 | Ed25519 sign/verify with `FAMP-sig-v1\0` domain-separation prefix (INV-10) | `famp-canonical` |
| `famp-fsm` | 0 | Task FSM: REQUESTED → COMMITTED → {COMPLETED\|FAILED\|CANCELLED}, all terminals absorbing | `famp-core` |
| `famp-envelope` | 0 | Wire envelope construction and Ed25519 signing; `BusEnvelope` (unsigned) for bus path | `famp-canonical`, `famp-crypto`, `famp-core` |
| `famp-inbox` | 0 | Durable JSONL inbox: append-with-fsync write, tail-tolerant read, per-name cursor file | — |
| `famp-taskdir` | 0 | Per-task TOML storage: atomic replace + fsync, single construction site per task record | — |
| `famp-transport` | 0 | `Transport` trait abstraction; `MemoryTransport` for in-process tests | `famp-core` |
| `famp-bus` | 1 | Pure-actor local bus broker: length-prefixed canonical-JSON frame codec, in-memory mailbox routing, durable mailbox delegation; no tokio (BUS-01), no signatures on bus path (BUS-11) | `famp-canonical`, `famp-inspect-proto`, `famp-core`, `famp-envelope` |
| `famp-inspect-proto` | 1 | Inspector RPC wire types (`broker`, `identities`, `sessions`, `tasks`, `waiters`, `messages`); no I/O deps | `famp-canonical` |
| `famp-inspect-server` | 1 | Inspector RPC handler implementations; tokio-free; mounted inside the running broker process | `famp-inspect-proto`, `famp-bus`, `famp-canonical`, `famp-envelope`, `famp-fsm` |
| `famp-inspect-client` | 1 | Inspector RPC async UDS client; issues dead-broker probes so `famp inspect` works even when broker is down | `famp-inspect-proto`, `famp-bus`, `famp-canonical` |
| `famp-keyring` | 2 (v1.0) | TOFU keyring for federation peers: stores and verifies leaf-cert SHA-256 pins | `famp-core`, `famp-crypto` |
| `famp-transport-http` | 2 (v1.0) | HTTPS transport binding for federation gateway; wraps `famp-keyring` for peer auth | `famp-core`, `famp-envelope`, `famp-crypto`, `famp-keyring`, `famp-transport`, `famp-canonical` |
| `famp-gateway` (binary) | 2 | Federation gateway process: proxies remote principals onto the local bus; signs egress and verifies inbound envelopes before relay | `famp-bus`, `famp`, `famp-keyring`, `famp-envelope`, `famp-crypto`, `famp-transport`, `famp-transport-http` |
| `famp` (binary) | CLI | `famp` CLI binary and MCP stdio server; single construction site for `BusEnvelope` (`cli::send::build_envelope_value`) | all active Layer 0–1 crates |

## Message flow (v0.9 send path)

How a message travels from an MCP `famp_send` call to delivery in the
recipient's mailbox. Numbers correspond to crate boundaries crossed.

1. **MCP tool invoked** — Agent calls `famp_send {to: "bob", body: "..."}`.
   The `famp` binary's MCP server (`cli/mcp/`) checks the session-bound
   identity set by the prior `famp_register` call. Unregistered callers
   receive a typed `not_registered` error before any bus contact is made.

2. **Envelope construction** — `cli/send/build_envelope_value` (single
   construction site, BUS-11) builds an unsigned `BusEnvelope` with event
   prefix `famp.send.*` and a mode-tagged inner body (`mode: new_task`,
   `deliver`, `deliver_terminal`, or `channel_post`). `famp-envelope`
   and `famp-canonical` are used here; no Ed25519 signing on the bus path.

3. **Frame write** — The envelope is serialized to canonical JSON via
   `famp-canonical`, length-prefixed (4-byte big-endian), and written to
   the UDS connection managed by `famp-bus::bus_client`. The socket path is
   `~/.famp/bus.sock`.

4. **Broker dispatch** — `famp-bus::broker::handle_wire` receives the frame.
   The `Send` arm resolves `bob`'s `MailboxName`, then:
   - If `bob` has a parked `Await` waiter, deliver directly and wake the waiter.
   - Otherwise append the envelope to `bob`'s durable JSONL mailbox on disk
     (`famp-inbox::append`). The broker writes the record and advances
     `bob`'s mailbox depth.

5. **Recipient wakes** — Bob's `famp_await` hold (a long-poll UDS connection
   parked in the broker's waiter table) receives an `Out::Reply` signal and
   returns to the MCP caller with `"New FAMP message from alice."`. Bob then
   calls `famp_inbox` to read the queued envelope.

6. **Inspector side-channel** — At any point, `famp inspect identities` /
   `famp inspect broker` connect via a separate UDS path, routed through
   `famp-inspect-client` → `famp-inspect-server` (mounted inside the broker
   process). This path is read-only and does not affect the send path.

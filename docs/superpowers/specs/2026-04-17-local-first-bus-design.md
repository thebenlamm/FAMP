# FAMP v0.9 — Local-First Bus

> Historical note: this 2026-04-17 design doc predates Codex integration
> in the v0.8 wrapper. References to Claude Code here describe the primary
> MCP client used during design; current v0.8 docs also cover Codex, whose
> MCP registration is user-scoped in `~/.codex/config.toml`.

- **Date:** 2026-04-17
- **Status:** Design (awaiting approval)
- **Author:** Ben Lamm + Claude
- **Supersedes:** the "Federation Profile" v0.9 entry in `.planning/MILESTONES.md`
- **Reviewed by:** `zed-velocity-engineer`, `the-architect`

## TL;DR

FAMP v0.8 shipped a federation-grade reference implementation — Ed25519 signing, RFC 8785 canonical JSON, TLS with TOFU pinning, task FSM, MCP for Claude Code. Genuinely good for two parties in two trust domains. Hostile for two Claude Code windows on the same laptop, where "the federation" is one human and the filesystem is the trust boundary.

v0.9 introduces a **local bus**: a Unix domain socket broker that moves FAMP envelopes between same-host agents with zero cryptography, zero configuration, zero port management. The existing federation crates stay in the workspace as v1.0 internals, wrapped later by a `famp-gateway` that bridges bus to remote FAMP-over-HTTPS. The MCP tool surface (`famp_register`, `famp_send`, `famp_inbox`, `famp_await`, `famp_peers`, `famp_whoami`, `famp_join`, `famp_leave`) is stable across v0.9 and v1.0 — users never retrain when federation lands.

Target acceptance criterion: **two Claude Code windows exchange a message in ≤12 lines of README and ≤30 seconds.**

## Problem statement

The v0.8 onboarding for two local agents was observed on 2026-04-16 to require:

1. `famp setup --name alice --home /tmp/famp-alice --port 8443`
2. `famp setup --name bob --home /tmp/famp-bob --port 8444`
3. `FAMP_HOME=/tmp/famp-alice famp info | FAMP_HOME=/tmp/famp-bob famp peer import`
4. `FAMP_HOME=/tmp/famp-bob famp info | FAMP_HOME=/tmp/famp-alice famp peer import`
5. `FAMP_HOME=/tmp/famp-alice famp listen &`
6. `FAMP_HOME=/tmp/famp-bob famp listen &`
7. `FAMP_TOFU_BOOTSTRAP=1 FAMP_HOME=/tmp/famp-alice famp send --to bob --new-task "hello"`
8. `FAMP_HOME=/tmp/famp-bob famp inbox list`

Eight steps, two HOME dirs, two daemons, one environment-variable opt-in for first contact, two cert fingerprints to pin. Every step is justified by the federation threat model. None of them is justified for `uid=501 / host=local` talking to `uid=501 / host=local`.

The friction is not a bug. It is the federation tax applied where no federation exists.

## Non-goals for v0.9

- Cross-host messaging (v1.0, `famp-gateway`)
- Cross-user messaging on the same host (v1.x, if ever)
- Any cryptography on the local bus — no signing, no TLS, no TOFU, no keypairs
- Agent Cards, delegation, provenance graph, extensions registry (v1.0+)
- Message retention, compaction, quota enforcement (v1.0+)
- Removing or breaking the library crates `famp-canonical`, `famp-crypto`, `famp-core`, `famp-fsm`, `famp-envelope` — they are transport-neutral and remain unchanged
- Deleting `famp-transport-http` and `famp-keyring` — they stay compiling in the workspace as v1.0 internals, unwired from top-level CLI

## v1.0 design partners

Federation in v1.0 is scoped to real, named use cases. The plumb-line check (Architect):

- **Bucket A — Personal multi-device.** Maintainer's macbook + devbox. Two hosts, same human, legitimate federation-level trust boundary at the network layer.
- **Bucket B — A specific external collaborator.** Identity TBD; one named real-world interop partner.
- **Buckets C / D — speculative.** No named humans. Not counted.

Federation earning its name in v1.0 means: **bucket A works for daily use, bucket B works for at least one real interop session.** If neither is actively used within a readiness window after v0.9 ships, the "Federated" name is reconsidered.

## Architecture

Three layers, bottom to top:

```
┌──────────────────────────────────────────────────────────────┐
│  Claude Code                                                  │
│  ┌────────────────┐                     ┌────────────────┐    │
│  │ Window A       │                     │ Window B       │    │
│  │  "I'm alice"   │                     │  "I'm bob"     │    │
│  │                │                     │                │    │
│  │ famp mcp       │                     │ famp mcp       │    │
│  │ (stdio JSON-   │                     │ (stdio JSON-   │    │
│  │  RPC)          │                     │  RPC)          │    │
│  └──────┬─────────┘                     └──────┬─────────┘    │
└─────────┼──────────────────────────────────────┼──────────────┘
          │                                      │
          │  UDS: ~/.famp/bus.sock               │
          │  4-byte length prefix + canonical    │
          │  JSON BusMessage / BusReply          │
          └──────────────────┬───────────────────┘
                             │
                   ┌─────────▼──────────┐
                   │  famp broker       │  posix_spawn + setsid
                   │  single task + mpsc│  UDS bind = exclusion
                   │                    │  idle-exit after N min
                   └─────────┬──────────┘
                             │
                             ▼
                 ~/.famp/
                 ├── bus.sock
                 ├── broker.log
                 ├── sessions.jsonl
                 └── mailboxes/
                     ├── alice.jsonl
                     ├── bob.jsonl
                     ├── #planning.jsonl
                     └── .alice.cursor     (drain resume offsets)

                   [v1.0 — NOT BUILT YET]
                             │
                   ┌─────────▼──────────┐
                   │  famp-gateway      │  connects to bus
                   │  (a pseudo-client) │  as if it were a
                   │                    │  local agent; bridges
                   │                    │  to remote FAMP-over-
                   │                    │  HTTPS with Ed25519
                   │                    │  signing + TOFU + TLS
                   └────────────────────┘
```

### Three-layer model

| Layer | Identity & scope | Crates | Wire | Crypto |
|---|---|---|---|---|
| **0 — Protocol primitives** | Transport-neutral | `famp-canonical`, `famp-crypto`, `famp-core`, `famp-fsm`, `famp-envelope` | N/A (library) | N/A |
| **1 — Local bus (v0.9)** | Same-host, same-user agents | `famp-bus` (new), `famp` binary | UDS + 4-byte-prefixed canonical JSON | None |
| **2 — Federation gateway (v1.0)** | Cross-host agents | `famp-gateway` (new), reuses `famp-transport-http`, `famp-keyring` | HTTPS + canonical JSON + Ed25519 | Full |

Layer 0 is unchanged by this design. Layer 1 is new in v0.9. Layer 2 is designed-but-not-built in v0.9; its internals (`famp-transport-http`, `famp-keyring`) stay compiling and tested in CI.

## The bus protocol

### Framing

4-byte big-endian length prefix, then canonical-JSON-encoded `BusMessage` or `BusReply`. Max frame size 16 MiB (sanity cap; typical messages < 1 KiB).

### Handshake

First frame after UDS connect MUST be a `Hello`:

```json
{ "op": "hello", "bus_proto": 1, "client": "famp-mcp/0.9.0" }
```

Broker replies with `HelloOk { bus_proto: 1, broker: "famp-broker/0.9.0" }` or `HelloErr { expected: 1, got: 2 }` (disconnect follows). No `BusMessage` other than `Hello` is accepted before the handshake completes. Version bumps are additive — a `bus_proto: 2` broker SHOULD still accept `bus_proto: 1` clients if possible.

### BusMessage (client → broker)

```rust
#[serde(tag = "op", rename_all = "snake_case")]
enum BusMessage {
    Hello    { bus_proto: u32, client: String },
    Register { name: String, pid: u32 },
    Send     { to: Target, envelope: Envelope },
    Inbox    { since: Option<u64> },
    Await    { timeout_ms: u64, task: Option<Uuid> },
    Join     { channel: String },
    Leave    { channel: String },
    Sessions,
    Whoami,
}

#[serde(tag = "kind", rename_all = "snake_case")]
enum Target {
    Agent   { name: String },     // "alice"
    Channel { name: String },     // "#planning"
}
```

`Envelope` is the existing `famp_envelope::Envelope` struct — same `MessageClass`, same `task_id`, same FSM fields. The `sig` field MUST be `None` on the bus.

### BusReply (broker → client)

```rust
#[serde(tag = "op", rename_all = "snake_case")]
enum BusReply {
    HelloOk     { bus_proto: u32, broker: String },
    HelloErr    { expected: u32, got: u32 },
    RegisterOk  { drained: Vec<Envelope>, peers: Vec<String> },
    SendOk      { task_id: Uuid, delivered: Delivered },
    InboxOk     { envelopes: Vec<Envelope>, next_offset: u64 },
    AwaitOk     { envelope: Envelope },
    AwaitTimeout,
    JoinOk      { channel: String, members: Vec<String>, drained: Vec<Envelope> },
    LeaveOk     { channel: String },
    SessionsOk  { rows: Vec<SessionRow> },
    WhoamiOk    { active: Option<String>, joined: Vec<String> },
    Err         { kind: BusErrorKind, message: String },
}

enum Delivered { Live, Queued }
```

### BusErrorKind (exhaustive, no wildcard)

```
NotRegistered                // send/await/inbox/join before register
NameTaken                    // live PID holds requested name
ChannelNameInvalid           // regex ^#[a-z0-9][a-z0-9_-]{0,31}$
NotJoined                    // leave on a channel not joined
EnvelopeInvalid              // canonical JSON round-trip fails
EnvelopeTooLarge             // > 16 MiB
TaskNotFound                 // send --task <uuid> references unknown conversation
BrokerProtoMismatch          // hello handshake rejected
BrokerUnreachable            // client-side only
Internal                     // logged with context
```

No `RecipientOffline` — durability guarantees offline recipients queue instead.

MCP-side translation layer uses a compile-checked exhaustive match (v0.8 pattern) — adding a `BusErrorKind` variant fails compilation until MCP error mapping handles it.

## Broker lifecycle

### Spawn

First CLI invocation or first MCP connection that finds no broker spawns one via `posix_spawn` + `setsid` (detaches from terminal; survives Cmd-Q on Terminal.app). Broker logs to `~/.famp/broker.log`. No double-fork, no reparenting to init, no `nohup`-style trickery.

### Exclusion — single concrete algorithm

On startup, broker attempts `bind()` on `~/.famp/bus.sock`:

1. **`bind()` succeeds** → no prior broker or the socket file was already cleaned up. Start normally.
2. **`bind()` fails with `EADDRINUSE`** → a socket file exists. Probe it:
   - `connect()` succeeds → another broker is live and accepting; this broker exits 0.
   - `connect()` fails (`ECONNREFUSED`) → stale socket from a crashed broker. `unlink(bus.sock)`, retry `bind()` once. If the retry fails, exit with an error (something else is interfering — possibly a filesystem issue).

No `flock`, no PID file, no ref-counting. The socket IS the lock. Tradeoff: a two-broker race during step 2 retry window is theoretically possible but requires one broker to die and two to boot within the same millisecond; acceptable for v0.9. v1.0 launchd ownership eliminates the race entirely.

### Idle exit

Broker maintains a connected-client count. When count transitions from 1 → 0, a 5-minute idle timer starts. If any client connects before the timer fires, timer is cancelled. If the timer fires, broker does a clean shutdown: closes all mailbox handles with fsync, unlinks `bus.sock`, exits.

### Upgrade path

User runs `pkill famp-broker` to force old broker exit. Next invocation spawns the new one. No in-place upgrade coordination in v0.9. (Socket-activated launchd version arrives in v1.0 and takes over this role.)

## Concurrency model

**Single-threaded broker task, mpsc inbox, all mutations serialized.**

Specifically:
- One tokio task owns the `Broker` state struct. No locks on the state itself.
- A per-connection accept loop spawns one tokio task per client. Each connection task reads frames, decodes, sends a `(client_id, BusMessage)` on an mpsc channel to the broker task, and reads replies from a per-client mpsc back.
- The broker task processes messages one at a time from the mpsc, mutates state, sends zero-or-more replies on per-client channels.
- Fan-out to N channel subscribers is a loop inside a single broker-task step — atomic with respect to other clients.
- No `RwLock<Broker>`. No `Mutex<HashMap<...>>`. The broker is a message-driven actor.

This is boring and correct. It handles "20 subscribers in a channel fan-out while subscriber 21 tries to join" trivially — the 21st Join waits in the mpsc queue behind the fan-out.

## Data flow

### Register

1. MCP/CLI opens UDS to `~/.famp/bus.sock`.
2. Sends `Hello`. Broker replies `HelloOk`.
3. Sends `Register { name: "alice", pid: <self.pid> }`.
4. Broker looks up `alice`:
   - Present with **live PID** (`kill(pid, 0)` == `Ok`) → `Err(NameTaken)`.
   - Present with **dead PID** (`kill(pid, 0)` == `ESRCH`) → evict row, accept.
   - Absent → accept.
5. Broker opens `~/.famp/mailboxes/alice.jsonl`, reads from the offset recorded in `.alice.cursor` (or from 0 if absent), collects drained envelopes.
6. Broker sends `RegisterOk { drained, peers: [...] }`. Only after the socket write returns successfully (no `EAGAIN`, no disconnection mid-write) does the broker atomically update `.alice.cursor` to the new offset via temp-file-plus-rename.
7. **Durability semantics (explicit):** if the broker crashes between sending `RegisterOk` and updating the cursor, the next Register re-drains — at-least-once on the broker side. If the client crashes after receiving `RegisterOk` but before processing, the messages are lost. Acceptable for v0.9 interactive use. Phase 1 TDD gate #2 (drain cursor atomicity) verifies the broker-side invariant.
8. Broker writes live row to `sessions.jsonl` (diagnostic only).
9. MCP caches `active_identity = alice` in session state.

### Send

1. MCP/CLI (registered as `alice`) sends `Send { to: Target::Agent { name: "bob" }, envelope: {...} }`.
2. Broker stamps `envelope.from = "alice"` (clients cannot spoof).
3. Broker routes by target:
   - `Target::Agent(name)` — if `name` is connected, write to its per-client channel; `delivered = Live`. If not, append to `mailboxes/<name>.jsonl`, fsync; `delivered = Queued`.
   - `Target::Channel(name)` — append to `mailboxes/<#channel>.jsonl` (fsync), then fan-out to every currently-joined member's channel. New joiners drain history on Join.
4. Broker returns `SendOk { task_id, delivered }`.

### Inbox

1. Registered client sends `Inbox { since: Option<u64> }`.
2. Broker returns envelopes from the **live per-session queue** (messages received while this client has been connected) starting at the given `since` offset (or all if `None`), with `next_offset` pointing past the last returned.
3. The mailbox file is **not** consulted here — offline-queued messages are delivered via `RegisterOk` at Register time, not via Inbox. Inbox is strictly a live-queue accessor.
4. Client advances its local cursor by passing `since = next_offset` on the next call. The broker does not track per-session Inbox cursors — the client is authoritative.

### Await

1. Registered client sends `Await { timeout_ms, task: Option<Uuid> }`.
2. Broker parks the client's reply channel in a `pending_awaits` map keyed by `(client_id, task_filter)`.
3. On next matching `Send` arrival (or `RegisterOk` drain that contains a match), broker un-parks and returns `AwaitOk { envelope }`.
4. On timeout, returns `AwaitTimeout`.
5. On client EOF mid-await, broker removes the entry and drops the envelope back into the queue for next `Inbox` / `Await`.

### Join / Leave

1. `Join { channel: "#planning" }` — channel regex-validated. Added to broker's `channels: HashMap<String, HashSet<ClientId>>`. Any queued channel mailbox (`mailboxes/#planning.jsonl`) is drained to the joining client. `JoinOk { channel, members, drained }`.
2. `Leave { channel }` — removed from member set. `LeaveOk`. No broadcast notification (add `ChannelEvent` in v0.9.1+).
3. On client EOF, broker auto-leaves every channel the client was in.

Channels auto-create on first Join. No ops, no topic, no modes in v0.9.

### Sessions

Returns current `SessionRow[]` from in-memory state (not from the file) — authoritative, live.

```json
{"name":"alice","pid":12345,"started_at":"...","joined":["#planning"]}
```

### Whoami

Returns `{ active: Some("alice"), joined: ["#planning", "#product"] }` — used for slash-command feedback and the `famp sessions list --me` CLI.

## Mailbox format

`~/.famp/mailboxes/<name>.jsonl` or `~/.famp/mailboxes/<#channel>.jsonl` — append-only, one canonical-JSON envelope per line, fsync after each append. Reuses `famp-inbox` crate's tail-tolerant reader (survives mid-write crash — truncated final line is dropped on load with a warning).

`~/.famp/mailboxes/.<name>.cursor` — drain resume offset. Written atomically (temp-file-plus-rename) after a successful drain ACK. On crash mid-drain, the next `Register` re-drains from the last ACK'd offset; effective semantics are at-least-once.

No retention, no compaction, no rotation in v0.9. `famp mailbox rotate|compact` is a v0.9.1 follow-up.

## Sessions file

`~/.famp/sessions.jsonl` is append-only and **diagnostic only** — `famp sessions` reads broker in-memory state, not this file. The file lets an operator `tail -f` to watch who joins and leaves during debugging. Dead-PID rows are filtered on read. Not authoritative; broker state wins.

## CLI surface (v0.9 user-facing)

```
famp register <name>                # registers + blocks (process = identity)
famp send --to <name>|--channel <#name> [--new-task <text>|--task <uuid>|--terminal] [--body <text>]
famp inbox list [--since <offset>] [--include-terminal]
famp inbox ack [--offset <offset>]
famp await [--timeout <dur>] [--task <uuid>]
famp join <#channel>
famp leave <#channel>
famp sessions [--me]
famp whoami
famp mcp                             # stdio JSON-RPC; uses bus internally
famp install-claude-code             # drops slash-command files + user MCP registration
```

Removed from top-level: `famp setup`, `famp listen`, `famp send (old TLS form)`, `famp peer add`, `famp peer import`, `famp init`. These commands move into `famp-gateway` in v1.0.

## MCP surface

Tools exposed over stdio JSON-RPC (identical to v0.8 envelope, new ops):

| Tool | Args | Returns |
|---|---|---|
| `famp_register` | `name: str` | `{active: str, drained: [...], peers: [...]}` |
| `famp_send` | `to: {kind:"agent"|"channel", name: str}`, envelope fields | `{task_id, delivered}` |
| `famp_inbox` | `since: int?`, `include_terminal: bool?` (default `false`; when `false`, entries for tasks in a terminal FSM state are hidden — see v0.8 spec `2026-04-20-filter-terminal-tasks-from-inbox-list-design.md`) | `{envelopes: [...], next_offset: int}` |
| `famp_await` | `timeout_ms: int`, `task: uuid?` | `{envelope}` \| `{timeout: true}` |
| `famp_peers` | — | `{online: [...]}` |
| `famp_join` | `channel: str` | `{channel, members, drained}` |
| `famp_leave` | `channel: str` | `{channel}` |
| `famp_whoami` | — | `{active: str?, joined: [...]}` |

Slash commands (dropped by `famp install-claude-code`):

- `/famp-register <name>` → `famp_register(name=...)`
- `/famp-join <#channel>` → `famp_join(channel=...)`
- `/famp-leave <#channel>` → `famp_leave(channel=...)`
- `/famp-msg <to> <body>` → `famp_send(to={kind:"agent",name:...}, new_task=body)` (DM convenience)
- `/famp-channel <#channel> <body>` → `famp_send(to={kind:"channel",name:...}, new_task=body)`
- `/famp-who [#channel?]` → `famp_sessions` filtered
- `/famp-inbox` → `famp_inbox`

## Testing strategy

### Four layers (mirrors v0.6-v0.8)

| Layer | Coverage |
|---|---|
| **Unit (in-crate)** | `BusMessage`/`BusReply` round-trip via `famp-canonical`. Codec framing boundaries. Channel-name regex. Pure broker state-machine transitions with no I/O, no tokio. |
| **Property (proptest)** | DM fan-in ordering (N senders → one recipient, per-sender order preserved). Channel fan-out (1 sender → M joined subs each receive exactly the set sent, no dupes). Join/leave idempotency. Drain completeness over offline-then-online sequences. Uniqueness-check over arbitrary PID tables (alive/dead mix). |
| **Integration (shelled CLI via `assert_cmd`)** | 2-client DM round-trip. 3-client channel fan-out. `kill -9` broker mid-send + client reconnect + mailbox drain recovery. Broker spawn race (two CLI invocations near-simultaneous). |
| **MCP E2E** | Two `famp mcp` stdio processes via test harness, JSON-RPC scripted from both sides, round-trip a `new_task → commit → deliver → ack` cycle over UDS. Equivalent to v0.8's `e2e_two_daemons` but over bus instead of HTTPS. |
| **Conformance (unchanged)** | `famp-canonical` RFC 8785 gate, `famp-crypto` §7.1c gate continue to run. Bus doesn't touch these but sharing the envelope type means any regression shows up in broker tests immediately. |

### Phase 1 TDD gates (write these tests BEFORE production code)

1. **Codec fuzz** — proptest the length-prefixed frame codec against truncated reads, split reads across multiple `read()` calls, `length == MAX + 1`, `length == 0`, and partial length prefixes. Classic length-confusion bug surface.
2. **Drain cursor atomicity** — proptest: append N envelopes to `mailboxes/alice.jsonl`, start a drain, `kill -9` broker mid-drain (simulated), restart, resume drain. Assert: no envelope lost. (At-least-once semantics: duplicates are acceptable, losses are not.)
3. **PID reuse race** — proptest: register `alice` with PID P1, P1 dies, OS reuses PID as P2 belonging to unrelated process. Next register of `alice` by some process with PID P3 must NOT be rejected on the basis of P1/P2. Broker must probe liveness AND cross-check against its own `clients` map.
4. **EOF cleanup mid-await** — proptest: client registers, starts `Await`, disconnects before any matching Send arrives. Assert: `pending_awaits` is cleaned; a subsequent matching Send is queued for Inbox, not silently dropped into a dead channel.

### Phase 4 hard requirement

`e2e_two_daemons` (the v0.7/v0.8 cross-machine HTTPS integration test) is refactored to target `famp-transport-http`'s library API directly — no dependency on the CLI subcommands that Phase 4 deletes. Test must:

- Instantiate two `famp-transport-http` server instances with self-signed certs (via `famp init tls` helper, called as a library function).
- Exchange a full signed request → commit → deliver → ack cycle over HTTPS.
- Verify canonical JSON + Ed25519 signature end-to-end.
- Run in `just ci` on every commit.

This is the **plumb-line-2 commitment**: federation CI stays alive so the preserved crates are not mummification. **Phase 4 is not complete until this test is green.**

## Phasing

### Phase 1 — `famp-bus` library (library only, no wire)

- New crate `famp-bus` in workspace
- Types: `BusMessage`, `BusReply`, `Target`, `BusErrorKind`, `Delivered`, `SessionRow`
- Codec: length-prefixed canonical JSON frame codec (sync, no tokio)
- Pure broker state machine: `Broker::handle(from: ClientId, msg: BusMessage) -> Vec<Out>` with no I/O
- In-memory mailbox impl for tests
- All four Phase-1 TDD gates written first
- Proptest coverage for fan-in, fan-out, uniqueness-check, drain completeness
- No UDS, no tokio in the broker core

**Exit criteria:** library compiles, `cargo test -p famp-bus` fully green including proptest, `just ci` unaffected.

### Phase 2 — UDS + CLI + minimum-viable MCP

- `famp broker` subcommand — tokio UDS listener wrapping `famp-bus::Broker`
- Lifecycle: `posix_spawn` + `setsid`, `bind()`-exclusion, idle-exit after 5 min
- New CLI commands: `register`, `send`, `inbox`, `await`, `join`, `leave`, `sessions`, `whoami`
- Mailbox impl on disk (reusing `famp-inbox` format)
- **Minimum-viable MCP rewire**: `famp mcp` connects to bus (drops TLS/reqwest), exposes `famp_register`, `famp_send`, `famp_inbox`, `famp_await`, `famp_peers`, `famp_join`, `famp_leave`, `famp_whoami`. No `install-claude-code` yet — user adds MCP config manually.
- Integration tests: 2-client DM, 3-client channel, broker-crash recovery
- MCP E2E harness: two stdio processes round-trip

**Exit criteria:** shell-level usability (`famp register alice &; famp send ...` works), MCP tools work when config is added manually, full CI green.

### Phase 3 — Claude Code integration polish

- `famp install-claude-code` subcommand — writes user-scope MCP config to `~/.claude.json` (or invokes `claude mcp add`), drops slash-command markdown files into `~/.claude/commands/`
- Slash commands: `/famp-register`, `/famp-join`, `/famp-leave`, `/famp-msg`, `/famp-channel`, `/famp-who`, `/famp-inbox`
- README Quick Start rewrite — must pass the **12-line / 30-second acceptance test** (see below)
- Onboarding user journey documentation

**Exit criteria:** fresh install on a clean Mac, `brew install famp && famp install-claude-code`, then two Claude Code windows exchange a message in ≤12 lines of instruction and ≤30 seconds elapsed.

### Phase 4 — federation CLI unwire + federation-CI preservation

- Remove `famp setup`, `famp listen`, `famp send (old form)`, `famp peer add`, `famp peer import`, `famp init` from user-facing CLI
- Move `famp-transport-http` + `famp-keyring` under the label "v1.0 federation internals" in `Cargo.toml` comments
- **Refactor `e2e_two_daemons` to library API** — no dependency on deleted CLI. Test must stay green in CI.
- Cut tag `v0.8.1-federation-preserved` on the commit BEFORE Phase 4 deletions land
- Update `README.md`, `CLAUDE.md`, `.planning/MILESTONES.md` — local-first is the headline, federation is v1.0 promise
- Add `docs/MIGRATION-v0.8-to-v0.9.md`

**Exit criteria:** `cargo tree` shows federation crates are only consumed by the refactored e2e test (no top-level CLI usage), federation e2e test green in CI, tag exists, `just ci` green, v0.9.0 tag cut.

## Migration — v0.8 → v0.9

Breaking at the CLI level. Users migrate by switching commands, not by editing configs.

| v0.8 | v0.9 |
|---|---|
| `famp setup --name alice --home /tmp/a --port 8443` | `famp register alice` (blocks) |
| `famp listen` | (gone; broker handles this) |
| `famp info` / `famp peer add` / `famp peer import` | (gone; same-host discovery is automatic) |
| `famp send --to bob --new-task "x"` | `famp send --to bob --new-task "x"` (same syntax, bus under the hood) |
| `FAMP_HOME=/tmp/a` env var | (no longer meaningful; `~/.famp/` is sole root) |
| `FAMP_TOFU_BOOTSTRAP=1` | (no longer meaningful; no TLS on bus) |
| `famp mcp` with `FAMP_HOME=...` in `.mcp.json` | `famp mcp` — no env vars; register inside the session |

`docs/MIGRATION-v0.8-to-v0.9.md` documents the mapping table, what to delete from `.mcp.json`, and how to use `famp install-claude-code` to auto-update config.

## The naming question

Option Z is adopted: **split the name.**

- **FAMP** remains the name of the protocol and the library crates (`famp-canonical`, `famp-crypto`, `famp-core`, `famp-fsm`, `famp-envelope`, `famp-bus`, `famp-transport-http`, `famp-keyring`, future `famp-gateway`). The spec is genuinely about federation primitives; the crates are genuinely implementing them.
- **The local-first product name is TBD.** This is the user-facing brand for the binary and CLI surface in v0.9 — what Claude Code users install. Candidate direction: something that evokes "local agent chat" (e.g., "bond", "relay", "linesman", "agents", "talk") without leaning on "federated" or "protocol." **Decision deferred to before v0.9 tag.**
- The binary ships as `famp` through all of v0.9.x for continuity. If the product rename is decided before v0.9.0 tag, the new name ships as an additional entry in `Cargo.toml` `[[bin]]` alongside `famp` — no hard rename. Both names point at the same binary. The old name is retired at v1.0, not before. This avoids a breaking rename in the middle of a minor series.

The GitHub repo `thebenlamm/FAMP` retains its name (it's the protocol reference implementation). If a new product brand ships with its own marketing surface, that's a separate repo or docs site.

## README acceptance criterion

The v0.9 README Quick Start must demonstrate two Claude Code windows exchanging a message in **≤12 lines of instruction and ≤30 seconds of user time** on a fresh macOS install. Candidate shape:

```bash
# Install once
brew install famp
famp install-claude-code

# In Claude Code window A
/famp-register alice

# In Claude Code window B
/famp-register bob

# Window A (to Claude)
"send bob a message saying ship it"

# Window B (to Claude)
"what's in my inbox?"
```

Eight user-visible lines. If Phase 3 cannot land this, the design is too heavy and must be revisited before v0.9.0 tags.

## Open product questions (resolve before v0.9.0 tag)

1. **Product name** (Option Z TBD). Decide before v0.9 tag.
2. **v1.0 readiness trigger.** Current placeholder: "maintainer uses bucket A daily OR named external collaborator ready to interop." Make concrete before v0.9 ships so the trigger can fire unambiguously.
3. **Whether to rename binary at v0.9 or v0.9.1.** Coordinate with product name decision.
4. **Slash-command naming bikeshed** — `/famp-msg` vs `/famp-send` vs `/famp-dm`. Defer to Phase 3.

## Risks

- **The local-case black hole** (Architect): if v0.9 is too satisfying, v1.0 federation never ships and "FAMP = Federated" becomes false advertising. Mitigated by Phase 4 federation-CI requirement + the v1.0 readiness trigger.
- **Product name dithering.** If the naming decision slips, v0.9 ships as "famp" with a vague "rebrand coming" note, and momentum for the rename dies. Mitigation: set a forcing function — name must be decided before v0.9.0 tag cut.
- **Broker exclusion via `bind()` fails on some file systems.** `EADDRINUSE` semantics on UDS depend on the kernel; nfs-mounted home dirs may break. Mitigation: document `~/.famp/` must be on a local filesystem; add a startup check that warns otherwise.
- **Channel explosion.** No channel retention means `mailboxes/#channel.jsonl` grows unbounded. Mitigation: add `famp mailbox rotate` in v0.9.1 before any user complains. Acceptable for v0.9 because interactive developer usage won't hit it for weeks.

## Reference — reviews that shaped this design

Both reviewers saw an earlier draft lacking `Hello` handshake, single-threaded-actor concurrency note, `posix_spawn`-based broker lifecycle, and the four Phase-1 TDD gates. All were incorporated.

- `zed-velocity-engineer` — killed the double-fork lifecycle, named the MCP swamp, added versioning and TDD gaps.
- `the-architect` — flagged the local-case black hole, demanded Phase 4 federation-CI commitment, challenged Option D as potential mummification (answered via the CI commitment), framed the naming decision.

## Revision history

- 2026-04-17 — initial design, incorporating both reviews and all five architectural decisions (Option D, self-managing broker, durable mailboxes, envelope reuse, PID-liveness uniqueness, channels).

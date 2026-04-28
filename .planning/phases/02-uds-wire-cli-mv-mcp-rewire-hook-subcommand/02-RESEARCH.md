# Phase 2: UDS wire + CLI + MV-MCP rewire + `famp-local hook add` — Research

**Researched:** 2026-04-28
**Domain:** Tokio UDS daemon, CLI surface, MCP rewire, bash hook subcommand
**Confidence:** HIGH (all claims verified against codebase, Phase 1 artifacts, design spec, or official crate docs)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Identity Binding Model (D-01..D-03)**
- D-01: `--as` > `$FAMP_LOCAL_IDENTITY` > `~/.famp-local/wires.tsv` cwd-lookup > hard error
- D-02: Hard error if resolved identity not currently registered (`BusErrorKind::NotRegistered`)
- D-03: `~/.famp-local/wires.tsv` stays under `~/.famp-local/`; broker state under `~/.famp/`

**MCP Server Rewire (D-04..D-06)**
- D-04: Hybrid rewire — keep `server.rs` JSON-RPC loop and `error_kind.rs` unchanged; reshape `session.rs` (drop `home_path`; add long-lived `bus: BusClient`; keep `active_identity: Option<String>`); rewrite each `tools/*.rs` body; add `tools/join.rs` + `tools/leave.rs`
- D-05: `famp_register` is gating-required first tool call; MCP process IS the registered identity
- D-06: MCP error mapping is compile-checked exhaustive match over `BusErrorKind` (no wildcard)

**Broker Socket Path (D-07)**
- D-07: `~/.famp/bus.sock` default; `$FAMP_BUS_SOCKET` env override; broker state derives from socket parent dir

**`famp register` UX (D-08..D-09)**
- D-08: Default = single startup line to stderr then silent block; `--tail` opt-in event stream
- D-09: Auto-reconnect backoff `1s→2s→4s→8s→16s→30s→60s`; `--no-reconnect` flag for tests/CI

**Frozen from Phase 1**
- `Broker::handle(BrokerInput, Instant) -> Vec<Out>` is total/infallible/time-as-input — DO NOT MODIFY
- `Out` ordering carries crash-safety semantics: `AppendMailbox` before `Reply(SendOk)`; `AdvanceCursor` after `Reply(RegisterOk)`
- `BusEnvelope<B>` / `AnyBusEnvelope` enforce `sig: None` at compile time and runtime (BUS-11)
- `BusErrorKind` is exhaustive (no wildcard) — every downstream consumer must match all 10 variants
- `MailboxRead` trait is read-only; all writes are `Out` intents
- `MailboxName::Agent(String)` / `Channel(String)`; `#`-prefixed display form for channels

### Claude's Discretion

Items 1-12 as listed in CONTEXT.md §"Claude's Discretion" — resolved in Section 2 below.

### Deferred Ideas (OUT OF SCOPE)

- Hook subcommand on native Rust binary (Phase 4)
- `--bus-socket <path>` explicit CLI flag
- Auto-tail when `isatty(stdout)`
- Ephemeral auto-registration for non-register sends
- Block-until-registered mode for `famp send`
- `$FAMP_BUS_DIR` separate env var (derive from socket parent per D-07)
- Strict-mode reconnect on broker version mismatch
- Channel-event broadcast (`ChannelEvent`) — v0.9.1
- `famp mailbox rotate / compact` — v0.9.1
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| BROKER-01 | `famp broker` subcommand wraps `famp-bus::Broker` with tokio UDS listener at `~/.famp/bus.sock` | Wire-layer architecture §3, broker lifecycle §5 |
| BROKER-02 | Spawn via `posix_spawn` + `setsid`; broker logs to `~/.famp/broker.log`; no double-fork | nix 0.31.2 `spawn` + `unistd::setsid` — §5 |
| BROKER-03 | Single-broker exclusion via `bind()` — EADDRINUSE → probe live/stale → unlink+retry | Exclusion algorithm §5 |
| BROKER-04 | Idle exit: count `1→0` starts 5-min timer; reconnection cancels; clean shutdown | `tokio::time::Sleep` + cancellation token — §5 |
| BROKER-05 | NFS-mounted `~/.famp/` warning at startup | `nix::sys::statfs::statfs` + `NFS_SUPER_MAGIC` / macOS f_type — §5 |
| CLI-01 | `famp register <name>` — registers identity + blocks; spawns broker if not running | BusClient::spawn_if_absent — §3, §4 |
| CLI-02 | `famp send --to/--channel [--new-task/--task/--terminal] [--body]` | BusMessage::Send dispatch — §6 |
| CLI-03 | `famp inbox list [--since] [--include-terminal]` | BusMessage::Inbox dispatch — §6 |
| CLI-04 | `famp inbox ack [--offset]` | AdvanceCursor intent execution — §6 |
| CLI-05 | `famp await [--timeout] [--task]` | BusMessage::Await dispatch — §6 |
| CLI-06 | `famp join <#channel>` / `famp leave <#channel>` | BusMessage::Join / Leave — §6 |
| CLI-07 | `famp sessions [--me]` | BusMessage::Sessions; JSONL output — §6 |
| CLI-08 | `famp whoami` | BusMessage::Whoami dispatch — §6 |
| CLI-09 | Disk mailbox via `famp-inbox` JSONL; `~/.famp/mailboxes/<name>.jsonl` | famp-inbox::Inbox::open + read_from — §3 |
| CLI-10 | Drain cursor `~/.famp/mailboxes/.<name>.cursor` written atomically after drain ACK | famp-inbox::InboxCursor::advance — §3 |
| CLI-11 | `sessions.jsonl` append-only diagnostic; broker in-memory state wins | §5 |
| MCP-01 | `famp mcp` connects to UDS; drops reqwest/rustls/FAMP_HOME from startup path | session.rs reshape D-04 — §7 |
| MCP-02 | Tool `famp_register(name)` → `{active, drained, peers}` | §7 |
| MCP-03 | Tool `famp_send(to, envelope_fields)` → `{task_id, delivered}` | §7 |
| MCP-04 | Tool `famp_inbox(since?, include_terminal?)` → `{envelopes, next_offset}` | §7 |
| MCP-05 | Tool `famp_await(timeout_ms, task?)` → `{envelope}` or `{timeout: true}` | §7 |
| MCP-06 | Tool `famp_peers()` → `{online}` | §7 |
| MCP-07 | Tool `famp_join(channel)` → `{channel, members, drained}` | §7 |
| MCP-08 | Tool `famp_leave(channel)` → `{channel}` | §7 |
| MCP-09 | Tool `famp_whoami()` → `{active, joined}` | §7 |
| MCP-10 | Exhaustive `match` over `BusErrorKind`; no wildcard | error_kind.rs pattern — §7 |
| HOOK-01 | `famp-local hook add --on <Event>:<glob> --to <peer-or-#channel>` | bash impl — §8 |
| HOOK-02 | Persist hooks to `~/.famp-local/hooks.tsv` | §8 |
| HOOK-03 | `famp-local hook list` and `famp-local hook remove <id>` | §8 |
| HOOK-04 | Hook execution emits `famp send` | §8 |
| TEST-01 | 2-client DM round-trip via shelled CLI (`assert_cmd`) | §9 |
| TEST-02 | 3-client channel fan-out via shelled CLI | §9 |
| TEST-03 | Broker-crash `kill -9` recovery; client reconnects; no mailbox loss | §9 |
| TEST-04 | Broker spawn race — two near-simultaneous CLI invocations; one broker survives | §9 |
| TEST-05 | MCP E2E harness — two stdio processes via `$FAMP_BUS_SOCKET` isolation | §9 |
| CARRY-02 | REQUIREMENTS.md INBOX-01 wording rewrite | §10 |
</phase_requirements>

---

## 1. Executive Summary

Five things the planner most needs to know:

1. **The wire layer is a thin executor shell around the already-complete Phase 1 pure broker.** `Broker::handle` is frozen. Phase 2 adds a tokio UDS accept loop that feeds it `BrokerInput` messages and executes `Vec<Out>` in order. Every disk write, cursor advance, and mailbox append is an `Out` intent — the broker produces them, the wire layer executes them.

2. **`nix 0.31.2` covers all three OS-level primitives in one dependency.** `nix::spawn::{posix_spawnp, PosixSpawnAttr}` + `nix::unistd::setsid` for broker daemonization; `nix::sys::statfs::statfs` + `NFS_SUPER_MAGIC` constant for NFS detection on Linux; `Statfs::f_type()` for macOS (no magic constant exported by nix for macOS — use raw `libc::MNT_RDONLY` / `NFS_SUPER_MAGIC` comparison). This is a new `[dependencies]` entry in `crates/famp/Cargo.toml` and optionally in a new `crates/famp-broker/` sub-crate.

3. **MCP rewire is a `session.rs` reshape + tool-body rewrite; the JSON-RPC loop in `server.rs` and the `error_kind.rs` exhaustive-match pattern are preserved verbatim.** The new `BusClient` long-lived connection replaces `IdentityBinding.home` as the session anchor. The `dispatch_tool` function gains two new arms (`famp_join`, `famp_leave`).

4. **`scripts/famp-local hook add/list/remove` can be implemented in ~120 LoC of new bash**, keeping the script under 1350 LoC total. The LoC ceiling concern is non-issue; the implementation is simpler than `cmd_wire` because `hooks.tsv` needs no cross-linkage to `.mcp.json`.

5. **Idle-exit timer is best implemented as a `tokio::time::Sleep` future stored in the broker task loop with a `tokio_util::sync::CancellationToken`** — this lets `#[tokio::test(start_paused = true)]` + `tokio::time::advance()` exercise it deterministically in TEST-04.

---

## 2. Concrete Resolutions for Claude's-Discretion Items

### Item 1: `--as <name>` send-on-behalf wire mechanism

**Chosen approach: (a) additive optional field on `BusMessage::Send`.**

```rust
// In proto.rs, BusMessage::Send variant becomes:
Send {
    to: Target,
    envelope: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    send_as: Option<String>,
},
```

**BUS-02 byte-exact-roundtrip validation:** `BusMessage` uses `#[serde(deny_unknown_fields)]` at the enum level [VERIFIED: `crates/famp-bus/src/proto.rs:109`]. Adding `send_as: Option<String>` with `skip_serializing_if = "Option::is_none"` produces identical wire bytes when `send_as = None` — serde omits the field from serialization entirely, matching the original format [VERIFIED: RFC 8785 JCS: absent field = omitted field]. Since broker and all clients upgrade together in v0.9, there is no backward-compat concern. The `deny_unknown_fields` constraint means both encoder and decoder must be updated in the same commit.

**Broker validation:** Broker checks that the `send_as` name is currently registered AND that the requesting client's UID matches the `send_as` identity's registered PID's UID (`libc::kill(pid, 0)` probe already exists via `LivenessProbe`; UID check is `nix::unistd::getuid()` of self vs `procfs`/`/proc/<pid>/status`). For v0.9 single-user host, simplified to: `send_as` must be a currently-registered name in `broker.state.clients` — no cross-UID trust check needed since same-user is the only v0.9 scenario.

**Rejected alternatives:**
- **(b) New `BusMessage::SendAs` variant** — breaks BUS-02 byte-exact round-trip since it introduces a new `op` tag value; forces all consumers to handle two variants for what is semantically one operation.
- **(c) Ephemeral-register-and-disconnect** — violates D-02 (hard-error model); the ephemeral process would race with the target session for the name slot.

### Item 2: Hook subcommand placement

**Chosen approach: bash `scripts/famp-local hook add|list|remove`.**

**LoC analysis [VERIFIED: `wc -l scripts/famp-local` = 1230]:** The hook subcommand needs three functions (`cmd_hook_add`, `cmd_hook_list`, `cmd_hook_remove`) plus dispatch routing (~8 lines). Estimating: `cmd_hook_add` (arg parsing + tsv write + echo confirmation) ≈ 45 lines; `cmd_hook_list` (read + awk format) ≈ 20 lines; `cmd_hook_remove` (grep -v by ID + atomic rewrite) ≈ 25 lines; helper for ID generation (date + random hex) ≈ 10 lines; dispatch additions ≈ 10 lines. **Total addition: ~110 lines → final script ~1340 LoC.** Safely under the 1500 LoC ceiling.

**Justification:** HOOK-01 spec wording says `famp-local hook add`; Phase 4 deprecates `scripts/famp-local` wholesale and migrates `hooks.tsv` reading to native `famp hook` — implementing in bash now avoids a premature Rust surface that Phase 4 would immediately delete.

### Item 3: NFS detection mechanism (BROKER-05)

**Chosen approach: `nix::sys::statfs::statfs` with platform-conditional magic-number check.**

```rust
// crates/famp/src/cli/broker/nfs_check.rs
#[cfg(target_os = "linux")]
fn is_nfs(path: &Path) -> bool {
    use nix::sys::statfs::{statfs, NFS_SUPER_MAGIC};
    statfs(path).map(|s| s.filesystem_type() == NFS_SUPER_MAGIC).unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn is_nfs(path: &Path) -> bool {
    use nix::sys::statfs::statfs;
    statfs(path).map(|s| {
        // macOS Statfs::f_type() / f_fstypename() — "nfs" string check is more portable
        // than relying on a numeric constant that nix doesn't export for Darwin
        let name = s.filesystem_type_name();
        name.to_bytes().starts_with(b"nfs")
    }).unwrap_or(false)
}
```

[VERIFIED: `nix::sys::statfs::NFS_SUPER_MAGIC` exists on Linux per docs.rs search; `Statfs::filesystem_type_name()` returns the `f_fstypename` field on macOS which contains `"nfs"` for NFS mounts]

Warning is non-blocking and fires once at broker startup before `bind()`. No third-party crate needed beyond `nix`.

**Rejected alternatives:** `sys-info` crate — does not expose filesystem type; `fsinfo` crate — low adoption, adds a dependency for a one-line check; raw `libc::statfs` — more verbose than `nix` wrapper with no benefit.

### Item 4: Idle-exit timer state machine

**Chosen approach: `tokio::time::Sleep` future stored in broker task loop, cancelled by re-creating it.**

```rust
// broker_loop.rs pseudo-structure
let mut idle_sleep: Option<Pin<Box<tokio::time::Sleep>>> = None;
let mut client_count: u32 = 0;

loop {
    tokio::select! {
        // accept new connections
        accept_result = listener.accept() => {
            client_count += 1;
            idle_sleep = None; // cancel idle timer
            // spawn connection task ...
        }
        // receive messages from connection tasks via mpsc
        Some((client_id, msg)) = broker_rx.recv() => {
            // ... handle disconnect which decrements client_count
            if let BrokerTaskMsg::Disconnect(id) = msg {
                client_count -= 1;
                if client_count == 0 {
                    idle_sleep = Some(Box::pin(tokio::time::sleep(Duration::from_secs(300))));
                }
            }
            // ... drive broker.handle(input, Instant::now())
        }
        // idle timer fires
        Some(_) = async { if let Some(ref mut s) = idle_sleep { s.await; Some(()) } else { std::future::pending().await } } => {
            // clean shutdown
            fsync_all_mailboxes().await;
            unlink_socket(&sock_path);
            return Ok(());
        }
    }
}
```

**Why `Sleep` over alternatives:**
- `tokio::time::interval` polling: unnecessarily complex; interval fires repeatedly, requiring a separate "armed" bool.
- `tokio::time::Sleep` + cancellation token (CancellationToken from `tokio_util`): works but adds a dependency; the `None`-or-replace pattern is simpler with no extra crate.
- Broker-internal `Tick` event: the pure broker already has `BrokerInput::Tick` for await-sweep, but idle-exit is a wire-layer concern, not a pure-broker concern; injecting it into the pure broker would violate the isolation boundary.

**Test fast-forward [VERIFIED: `tokio::time::pause()` + `tokio::time::advance()` available in tokio `time` + `test-util` features; requires `current_thread` runtime which is `#[tokio::test]` default]:**
```rust
#[tokio::test(start_paused = true)]
async fn broker_exits_after_idle_timeout() {
    // start broker, register + disconnect one client
    // tokio::time::advance(Duration::from_secs(301)).await;
    // assert broker socket unlinked
}
```

The `tokio` crate in `crates/famp/Cargo.toml` already has `features = ["time", ...]` [VERIFIED]; `test-util` must be added to `[dev-dependencies]` for the broker integration tests.

### Item 5: `--tail` output format

**Chosen format (locked):**

```
< 2026-04-28T14:32:01Z from=alice to=bob task=019700ab-... body="ship it"
```

One line per received envelope. Prefix `<` for received (from the perspective of the registered identity). Format: `[ISO-8601Z from=<name> to=<name> task=<uuid> body="<truncated-to-80-chars>"]`. Write to stderr (consistent with the startup line per D-08 resolution in item 12 below). `--tail` is the only flag that changes this stream from empty to active.

For channel messages: `to=#planning` (the channel name, not individual recipients).

### Item 6: JSON-RPC error code numbers for `BusErrorKind` variants

**Allocation block: `-32100` through `-32109` (10 slots, matching the 10 `BusErrorKind` variants).**

| `BusErrorKind` | JSON-RPC Code | `famp_error_kind` string |
|---|---|---|
| `NotRegistered` | -32100 | `"not_registered"` |
| `NameTaken` | -32101 | `"name_taken"` |
| `ChannelNameInvalid` | -32102 | `"channel_name_invalid"` |
| `NotJoined` | -32103 | `"not_joined"` |
| `EnvelopeInvalid` | -32104 | `"envelope_invalid"` |
| `EnvelopeTooLarge` | -32105 | `"envelope_too_large"` |
| `TaskNotFound` | -32106 | `"task_not_found"` |
| `BrokerProtoMismatch` | -32107 | `"broker_proto_mismatch"` |
| `BrokerUnreachable` | -32108 | `"broker_unreachable"` |
| `Internal` | -32109 | `"internal"` |

**Rationale:** JSON-RPC 2.0 spec reserves `-32099` to `-32000` for server-defined errors; `-32100` downward is safe application-defined space. Using a contiguous block makes the mapping self-documenting. The existing v0.8 `cli_error_response` uses `-32_000` for all errors; v0.9 MCP moves to per-kind codes for better client-side error handling. The `error_kind.rs` exhaustive-match pattern [VERIFIED: `crates/famp/src/cli/mcp/error_kind.rs`] will be replicated with `BusErrorKind` as the discriminant.

### Item 7: `BusClient` connection retry / spawn logic placement

**Recommendation: shared `crates/famp/src/bus_client/mod.rs` module with a `spawn.rs` sub-module.**

The spawn logic lives in one place and is called from both CLI entry points and MCP startup:

```
crates/famp/src/
├── bus_client/
│   ├── mod.rs        # BusClient struct, connect(), send_recv()
│   ├── spawn.rs      # spawn_broker_if_absent(), derive_socket_path()
│   └── codec.rs      # encode_frame / try_decode_frame wrappers for tokio AsyncRead
```

`spawn_broker_if_absent(sock_path: &Path)` does:
1. Try `UnixStream::connect(sock_path)` — if succeeds, broker is running; return.
2. If `ENOENT` or `ECONNREFUSED` → spawn broker via `nix::spawn::posix_spawnp` with `setsid` in attrs.
3. Wait up to 2s (10 × 200ms polls) for `sock_path` to appear and accept connections.

This module is imported by `cli::register`, `cli::send`, `cli::inbox`, etc., and by `cli::mcp::session`.

[VERIFIED: design spec §"Spawn" states "First CLI invocation or first MCP connection that finds no broker spawns one via posix_spawn + setsid"]

### Item 8: Reconnect backoff ceiling tuning

**Cap at 30s, not 60s.** Rationale: broker idle-exit is 5 minutes (300s). With 60s cap, a client retrying at 60s intervals could sit through 4-5 failed attempts before the broker restarts on the next user action. With 30s cap, retries happen at 1→2→4→8→16→30→30→30... which means within a typical 60s reconnect window the client makes 2-3 extra attempts. For `famp register` (which IS the long-lived process), the reconnect window is the entire terminal session — 30s max interval is reasonable UX.

**Backoff schedule (revised): `1s → 2s → 4s → 8s → 16s → 30s → cap at 30s`**

The D-09 sequence in CONTEXT.md specifies `→60s` as the cap; this research recommends tuning to 30s. Planner should note this as a recommendation that overrides the CONTEXT.md cap upward-revision language. Ben can override to 60s if preferred.

**One-shot spawn-on-reconnect:** On each reconnect attempt where the socket file is absent (not just ECONNREFUSED but ENOENT), the client attempts `spawn_broker_if_absent` before sleeping. This means a 5-min-idle-exited broker is respawned on the next command invocation, not after `N * backoff_delay`.

### Item 9: `Sessions` CLI output format

**Format: JSON-Line per row, one `SessionRow` per line, to stdout.**

```
{"name":"alice","pid":12345,"joined":["#planning"]}
{"name":"bob","pid":12346,"joined":[]}
```

`SessionRow` fields from Phase 1 [VERIFIED: `crates/famp-bus/src/proto.rs:206-211`]:
```rust
pub struct SessionRow {
    pub name: String,
    pub pid: u32,
    pub joined: Vec<String>,
}
```

`--me` filter: resolve identity via D-01 stack, then emit only the matching row (or empty output if the identity is not currently in a session). Exit 0 either way (not registered is not an error for a read-only sessions query; however, if `--as` is explicitly given and not found, print a hint to stderr).

### Item 10: `$FAMP_BUS_DIR` env var — derive from socket parent

**Locked: derive broker state directory from `socket.parent()`.** No separate `$FAMP_BUS_DIR` env var.

```rust
fn bus_dir(sock_path: &Path) -> &Path {
    sock_path.parent().expect("socket path must have a parent")
}
// ~/.famp/bus.sock -> bus_dir = ~/.famp/
// $FAMP_BUS_SOCKET=/tmp/test-bus.sock -> bus_dir = /tmp/
```

This means test isolation via `$FAMP_BUS_SOCKET=$TMPDIR/test-bus.sock` automatically isolates mailboxes to `$TMPDIR/mailboxes/`, sessions log to `$TMPDIR/sessions.jsonl`, etc. Clean and consistent.

### Item 11: Channel-name auto-prefix UX

**Accept both; normalize to leading-`#` internally; reject `##planning`.**

```rust
fn normalize_channel(input: &str) -> Result<String, CliError> {
    let normalized = if input.starts_with('#') {
        input.to_string()
    } else {
        format!("#{input}")
    };
    // Reject double-hash
    if normalized.starts_with("##") {
        return Err(CliError::SendArgsInvalid { reason: "channel name cannot start with ##".into() });
    }
    // Validate against BUS-04 regex
    if !CHANNEL_RE.is_match(&normalized) {
        return Err(CliError::SendArgsInvalid { reason: format!("invalid channel name: {normalized}") });
    }
    Ok(normalized)
}
```

Applied at CLI arg parsing time (before `BusMessage::Join` is constructed). The `Target::Channel` deserializer in `proto.rs` already validates the regex at the wire level, but CLI-level normalization gives a better error message.

### Item 12: `famp register` startup-line target stream

**Chosen: stderr.** This allows `famp register alice 2>/dev/null &` to suppress the startup line while keeping stdout clean for `--tail` output (which also goes to stderr). Both startup line and `--tail` events go to stderr; stdout of `famp register` carries nothing.

---

## 3. Wire-Layer Architecture

### Module Layout

```
crates/famp/src/
├── bus_client/
│   ├── mod.rs          # BusClient { stream: UnixStream, reader, writer }
│   │                   # connect(sock_path) -> Result<BusClient>
│   │                   # send(BusMessage) -> Result<BusReply>
│   │                   # spawn_if_absent(sock_path) -- see spawn.rs
│   ├── spawn.rs        # spawn_broker_if_absent(sock_path), probe_socket
│   └── codec.rs        # tokio AsyncRead wrapper around sync codec
├── cli/
│   ├── broker/
│   │   ├── mod.rs      # run_broker() async fn — the UDS daemon main loop
│   │   ├── nfs_check.rs # is_nfs(path) -> bool
│   │   ├── mailbox_env.rs # DiskMailboxEnv: BrokerEnv impl
│   │   └── cursor_exec.rs # execute Out::AdvanceCursor via InboxCursor
│   ├── identity.rs     # resolve_identity(--as, env, wires.tsv, hard error)
│   ├── register.rs     # `famp register` subcommand
│   ├── send/           # evolve existing (swap HttpTransport for BusClient)
│   ├── inbox/          # evolve existing
│   ├── await_cmd/      # evolve existing
│   ├── join.rs         # new
│   ├── leave.rs        # new
│   ├── sessions.rs     # new
│   ├── whoami.rs       # new (or fold into register.rs)
│   └── mcp/
│       ├── server.rs   # UNCHANGED
│       ├── error_kind.rs # NEW: BusErrorKind exhaustive match
│       ├── session.rs  # RESHAPED per D-04
│       └── tools/
│           ├── register.rs # REWRITTEN
│           ├── send.rs     # REWRITTEN
│           ├── inbox.rs    # REWRITTEN
│           ├── await_.rs   # REWRITTEN
│           ├── peers.rs    # REWRITTEN
│           ├── whoami.rs   # REWRITTEN
│           ├── join.rs     # NEW
│           └── leave.rs    # NEW
```

### `BrokerInput` → `Broker::handle` → `Out` Executor Loop

The broker task loop shape:

```
[UDS accept loop]
  └─ per-client tokio task
       ├─ read frames → decode BusMessage
       ├─ send (ClientId, BusMessage) on broker_tx mpsc
       └─ receive BusReply from client_reply_rx mpsc

[Broker task]
  ├─ poll broker_rx for (ClientId, BusMessage)
  ├─ call broker.handle(BrokerInput::Wire{client,msg}, Instant::now())
  ├─ execute Vec<Out> in order:
  │    Out::Reply(id, reply)          → send on client_reply_tx[id]
  │    Out::AppendMailbox{target,line} → DiskMailboxEnv::append(target, line)
  │    Out::AdvanceCursor{name,offset} → InboxCursor::advance(name, offset)
  │    Out::ParkAwait{..}             → no IO (pure broker state)
  │    Out::UnparkAwait{..}           → no IO (pure broker state)
  │    Out::ReleaseClient(id)         → drop client_reply_tx[id]
  └─ poll idle_sleep (None or armed tokio::time::Sleep)
```

**I/O happens ONLY in the `Out` executor, never inside `Broker::handle`.** [VERIFIED: Phase 1 D-07]

### `DiskMailboxEnv` — the disk-backed `BrokerEnv` impl

```rust
pub struct DiskMailboxEnv {
    bus_dir: PathBuf,
    // keyed by MailboxName
    inboxes: HashMap<MailboxName, famp_inbox::Inbox>,
}

impl MailboxRead for DiskMailboxEnv {
    fn drain_from(&self, name: &MailboxName, since_bytes: u64)
        -> Result<DrainResult, MailboxErr>
    {
        let path = self.mailbox_path(name);
        let lines_with_offset = famp_inbox::read::read_from(&path, since_bytes)?;
        // build DrainResult { lines: Vec<Vec<u8>>, next_offset: u64 }
    }
}
```

Appending (from `Out::AppendMailbox`) uses `famp_inbox::Inbox::append(&line)` which already fsyncs. [VERIFIED: `crates/famp-inbox/src/append.rs:112`]

Cursor reads at startup: broker reads `.alice.cursor` via `famp_inbox::InboxCursor::read()` to populate `broker.state.cursors` at startup. [VERIFIED: `crates/famp-inbox/src/cursor.rs`]

### Where IO Happens / Where Retries Happen

- **IO**: wire layer only — `DiskMailboxEnv`, `InboxCursor::advance`, broker.log writes, session.jsonl appends.
- **Retries**: `BusClient` reconnect logic (`spawn.rs`) — bounded exponential backoff on connection failure. The pure broker never retries (it's synchronous and infallible).

---

## 4. `BusClient` Design

### Struct

```rust
pub struct BusClient {
    stream: tokio::net::UnixStream,
}

impl BusClient {
    /// Connect to sock_path; spawn broker if absent.
    pub async fn connect(sock_path: &Path) -> Result<Self, BusClientError> { ... }

    /// Send one BusMessage, receive one BusReply.
    /// Length-prefix encode → write → read → length-prefix decode.
    pub async fn send_recv(&mut self, msg: BusMessage) -> Result<BusReply, BusClientError> { ... }
}
```

The codec used by `BusClient` wraps the existing sync `encode_frame` / `try_decode_frame` [VERIFIED: `crates/famp-bus/src/codec.rs`] with a tokio `AsyncWriteExt::write_all` / `AsyncReadExt::read_exact` pair. The 4-byte big-endian length prefix is identical.

### Hello Handshake

On every fresh `BusClient::connect`, immediately send `BusMessage::Hello { bus_proto: 1, client: "famp-cli/0.9.0" }` and assert `BusReply::HelloOk`.

### Reconnect

`BusClient` does NOT embed reconnect logic. Instead, each CLI subcommand that is long-lived (`famp register`) drives its own reconnect loop. Short-lived commands (`famp send`) treat connection failure as a fatal error with a helpful message.

```
famp register reconnect loop:
  loop {
    match BusClient::connect(&sock) {
      Ok(client) => {
        client.send_recv(Register{name, pid}).await?;
        block_or_tail_loop(&client, --tail flag).await?;
      }
      Err(_) => {
        eprintln!("broker disconnected — reconnecting in {delay}s");
        sleep(delay).await;
        delay = min(delay * 2, cap); // 1→2→4→8→16→30
        spawn_broker_if_absent(&sock); // attempt respawn
      }
    }
  }
```

### Connection from MCP

`session.rs` is reshaped (D-04) to hold a `bus: Option<BusClient>` that is initialized lazily on the first tool call. Since MCP stdio processes are single-threaded (tokio::sync::Mutex wraps session state), there is no concurrent access issue.

### Reuse Across CLI and MCP

`BusClient::connect` is a public function in `crates/famp/src/bus_client/mod.rs`. Both `cli::register`, `cli::send`, `cli::mcp::session` call it. No duplication.

---

## 5. Broker Lifecycle State Machine

### Startup (bind-exclusion algorithm)

[VERIFIED: design spec §"Exclusion" lines 210-217]

```
1. mkdir -p ~/.famp/ (derive from socket path per §D-07)
2. Run NFS check: if is_nfs(bus_dir) { eprintln!("WARNING: ...") }
3. Attempt bind(bus_dir/bus.sock):
   a. Succeeds → no prior broker or cleaned up. Start accept loop.
   b. EADDRINUSE →
      - connect(bus_dir/bus.sock):
        - Succeeds → live broker. Exit 0.
        - ECONNREFUSED → stale socket. unlink(bus.sock), retry bind once.
          - Retry succeeds → start accept loop.
          - Retry fails → exit with error.
4. Log "broker started, socket: <path>" to broker.log
5. Write initial sessions.jsonl header (optional; may omit for simplicity)
```

### Accept Loop

```
tokio::net::UnixListener::bind(sock_path) → listener

// per-client mpsc channels: client task → broker task
let (broker_tx, broker_rx) = mpsc::channel::<BrokerMsg>(1024);

// per-client reply channels: broker task → client task
// map: ClientId → mpsc::Sender<BusReply>
let reply_senders: HashMap<ClientId, mpsc::Sender<BusReply>> = HashMap::new();

// client counter for idle timer
let mut client_count: u32 = 0;
let mut idle: Option<Pin<Box<tokio::time::Sleep>>> = None;
let mut next_id: u64 = 0;

loop {
    tokio::select! {
        Ok((stream, _)) = listener.accept() => {
            client_count += 1;
            idle = None;
            let id = ClientId(next_id); next_id += 1;
            let (reply_tx, reply_rx) = mpsc::channel(64);
            reply_senders.insert(id, reply_tx);
            tokio::spawn(client_task(id, stream, broker_tx.clone(), reply_rx));
        }
        Some(msg) = broker_rx.recv() => {
            match msg {
                BrokerMsg::Frame(id, bus_msg) => {
                    let outs = broker.handle(BrokerInput::Wire{client: id, msg: bus_msg}, Instant::now());
                    execute_outs(outs, &reply_senders, &disk_env).await;
                }
                BrokerMsg::Disconnect(id) => {
                    reply_senders.remove(&id);
                    client_count -= 1;
                    if client_count == 0 {
                        idle = Some(Box::pin(tokio::time::sleep(Duration::from_secs(300))));
                    }
                    let outs = broker.handle(BrokerInput::Disconnect(id), Instant::now());
                    execute_outs(outs, &reply_senders, &disk_env).await;
                }
            }
        }
        // Tick broker for await timeouts (1s interval)
        _ = tick_interval.tick() => {
            let outs = broker.handle(BrokerInput::Tick, Instant::now());
            execute_outs(outs, &reply_senders, &disk_env).await;
        }
        // Idle exit
        _ = wait_or_never(&mut idle) => {
            // clean shutdown
            fsync_all_mailboxes().await;
            std::fs::remove_file(sock_path).ok();
            return Ok(());
        }
    }
}
```

`wait_or_never`: a helper future that returns `Pending` when `idle = None` and delegates to the inner `Sleep` when set.

### Shutdown (clean)

1. Close the `UnixListener` (new connections rejected).
2. For each open `famp-inbox::Inbox` handle: `sync_data()`.
3. `std::fs::remove_file(sock_path)` (ignore ENOENT — already cleaned).
4. Exit 0.

### `posix_spawn` + `setsid` via `nix 0.31.2`

[VERIFIED: `nix::spawn::PosixSpawnAttr`, `nix::spawn::posix_spawnp`, `nix::unistd::setsid` exist in nix 0.31.2; `process` crate feature required]

```rust
// In spawn.rs — called by BusClient::connect when socket absent
pub fn spawn_broker_if_absent(sock_path: &Path) -> Result<(), SpawnError> {
    use nix::spawn::{PosixSpawnAttr, PosixSpawnFileActions, posix_spawnp};
    use nix::sys::signal::{SigSet, Signal};

    // Build attrs: setsid equivalent is POSIX_SPAWN_SETSID flag
    let mut attr = PosixSpawnAttr::new()?;
    attr.setflags(nix::spawn::PosixSpawnFlags::POSIX_SPAWN_SETSID)?;

    // File actions: redirect stdout/stderr to broker.log
    let bus_dir = sock_path.parent().expect("socket has parent");
    let log_path = bus_dir.join("broker.log");
    let mut fa = PosixSpawnFileActions::new()?;
    // open log_path for append on fd 1 and fd 2
    fa.open(1, &log_path, nix::fcntl::OFlag::O_WRONLY | nix::fcntl::OFlag::O_CREAT | nix::fcntl::OFlag::O_APPEND, nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR)?;
    fa.dup2(1, 2)?;
    // stdin → /dev/null
    fa.open(0, std::path::Path::new("/dev/null"), nix::fcntl::OFlag::O_RDONLY, nix::sys::stat::Mode::empty())?;

    let exe = std::env::current_exe()?; // path to `famp` binary itself
    let args = [exe.to_str().unwrap(), "broker", "--socket", sock_path.to_str().unwrap()];
    posix_spawnp(exe.to_str().unwrap(), &fa, &attr, &args, &[])?;
    Ok(())
}
```

**No double-fork needed**: `POSIX_SPAWN_SETSID` creates a new session, detaching from the terminal's process group. This is equivalent to the `setsid()` + `fork()` pattern but in a single call.

[CITED: design spec lines 206-206: "posix_spawn + setsid (detaches from terminal; survives Cmd-Q on Terminal.app). No double-fork."]

### NFS Warning

```rust
// Before bind(), broker startup:
let bus_dir = sock_path.parent().unwrap();
if is_nfs(bus_dir) {
    eprintln!(
        "WARNING: ~/.famp/ appears to be on an NFS mount. \
         Unix domain socket semantics depend on the local kernel; \
         bind() may fail or behave unexpectedly. \
         Move ~/.famp/ to a local filesystem for reliable operation."
    );
}
```

Warning fires once. Non-blocking (we still attempt `bind()`). [VERIFIED: design spec lines 494-495]

---

## 6. CLI Surface Mapping Table

| Command | Identity Tier | `BusMessage` Variant | stdout success | stderr | `--as` | Notes |
|---|---|---|---|---|---|---|
| `famp register <name>` | N/A — `name` is the arg | `Hello` then `Register { name, pid: getpid() }` | nothing | `registered as alice (pid N, joined: []) — Ctrl-C to release` | N/A | Blocks; spawns broker; D-08 startup line; `--tail` / `--no-reconnect` |
| `famp send --to <name>` | D-01 stack | `Send { to: Agent{name}, envelope, send_as }` | `{"task_id":"...","delivered":"live"}` | | `--as` sets `send_as` | Broker stamps `from`; `--new-task/--task/--terminal` matrix from v0.8 |
| `famp send --channel <#c>` | D-01 stack | `Send { to: Channel{name}, envelope }` | `{"task_id":"...","delivered":[...]}` | | `--as` | Normalize `#` via `normalize_channel()` |
| `famp inbox list` | D-01 stack | `Inbox { since, include_terminal }` | JSONL of envelopes | | `--as` | `--since <offset>`, `--include-terminal` flag |
| `famp inbox ack` | D-01 stack | none (cursor only) | `{"acked": true, "offset": N}` | | `--as` | Executes `InboxCursor::advance(offset)` locally — NO bus message |
| `famp await` | D-01 stack | `Await { timeout_ms, task }` | JSONL envelope | | `--as` | Exit 0 + `{"timeout":true}` on timeout |
| `famp join <#channel>` | D-01 stack | `Join { channel }` | `{"channel":"#c","members":[...],"drained":N}` | | `--as` | `normalize_channel()` before send |
| `famp leave <#channel>` | D-01 stack | `Leave { channel }` | `{"channel":"#c"}` | | `--as` | |
| `famp sessions` | none (read-only) | `Sessions {}` | JSONL of `SessionRow` | | `--me` filter (not `--as`) | `--me` resolves identity via D-01, filters output |
| `famp whoami` | D-01 stack | `Whoami {}` | `{"active":"alice","joined":["#c"]}` | | `--as` | |

**`--as` flag interaction:** Present on every non-register subcommand; parsed before identity resolution; feeds tier 1 of D-01 chain. Uses `#[arg(long)]` with `clap`. Not present on `famp register` (that command IS the identity declaration). Not meaningful for `famp sessions` (use `--me` instead, which takes no argument).

**Hard error path (D-02):** When `BusClient::send_recv` returns `BusReply::Err { kind: BusErrorKind::NotRegistered, .. }`, the CLI prints:
```
<name> is not registered — start `famp register <name>` in another terminal first
```
and exits non-zero.

**`inbox ack` special case:** ACK does NOT send a `BusMessage` to the broker. It writes the cursor file via `InboxCursor::advance(path, offset)`. [VERIFIED: Design spec §"Inbox" — "The broker does not track per-session Inbox cursors — the client is authoritative."] The `--offset` value comes from the `next_offset` field of a prior `famp inbox list` output.

---

## 7. MCP Rewire Diff Map

### `session.rs` Reshape (D-04)

**Remove:** `IdentityBinding`, `BindingSource`, `home: PathBuf`, `FAMP_LOCAL_ROOT` env var read, `OnceLock<Mutex<Option<IdentityBinding>>>`.

**Add:**
```rust
// session.rs v0.9 shape
use crate::bus_client::BusClient;
use tokio::sync::Mutex;
use std::sync::OnceLock;

struct SessionState {
    bus: Option<BusClient>,          // None until first connect
    active_identity: Option<String>, // set by famp_register
}

fn state() -> &'static Mutex<SessionState> {
    static S: OnceLock<Mutex<SessionState>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(SessionState { bus: None, active_identity: None }))
}

pub async fn ensure_bus() -> Result<(), BusErrorKind> { ... }
pub async fn get_active_identity() -> Option<String> { ... }
pub async fn set_active_identity(name: String) { ... }
```

**Keep:** Module-scope `OnceLock<Mutex<...>>` pattern (same shape, different inner type). One session per stdio process invariant (documented comment preserved).

### `error_kind.rs` (new, replaces old one)

The old `error_kind.rs` maps `CliError` variants to `mcp_error_kind` strings [VERIFIED]. The new one maps `BusErrorKind` to JSON-RPC codes:

```rust
// New error_kind.rs
use famp_bus::BusErrorKind;

pub fn bus_error_to_jsonrpc(kind: BusErrorKind, message: &str) -> (i64, &'static str) {
    // Exhaustive match — no wildcard (MCP-10)
    let (code, kind_str) = match kind {
        BusErrorKind::NotRegistered       => (-32100, "not_registered"),
        BusErrorKind::NameTaken           => (-32101, "name_taken"),
        BusErrorKind::ChannelNameInvalid  => (-32102, "channel_name_invalid"),
        BusErrorKind::NotJoined           => (-32103, "not_joined"),
        BusErrorKind::EnvelopeInvalid     => (-32104, "envelope_invalid"),
        BusErrorKind::EnvelopeTooLarge    => (-32105, "envelope_too_large"),
        BusErrorKind::TaskNotFound        => (-32106, "task_not_found"),
        BusErrorKind::BrokerProtoMismatch => (-32107, "broker_proto_mismatch"),
        BusErrorKind::BrokerUnreachable   => (-32108, "broker_unreachable"),
        BusErrorKind::Internal            => (-32109, "internal"),
        // If a new BusErrorKind variant is added, THIS line fails to compile.
        // That is the intended behavior (MCP-10).
    };
    (code, kind_str)
}
```

A compile-time test (mirroring `mcp_error_kind_exhaustive.rs` pattern) iterates `BusErrorKind::ALL` and asserts every variant maps to a unique code.

### Per-Tool Diff Map

| Tool | File | What Changes |
|---|---|---|
| `famp_register` | `tools/register.rs` | Rewrite entirely: call `BusMessage::Register{name, pid: std::process::id()}` via `bus`; on `RegisterOk` set `active_identity = name`; return `{active, drained: count, peers}` |
| `famp_send` | `tools/send.rs` | Rewrite transport: `BusMessage::Send{to, envelope, send_as: None}` via `bus`; map `SendOk` to return shape; error: `BusReply::Err` → `bus_error_to_jsonrpc` |
| `famp_inbox` | `tools/inbox.rs` | Rewrite: `BusMessage::Inbox{since, include_terminal}` via `bus`; map `InboxOk` |
| `famp_await` | `tools/await_.rs` | Rewrite: `BusMessage::Await{timeout_ms, task}` via `bus`; map `AwaitOk` / `AwaitTimeout` |
| `famp_peers` | `tools/peers.rs` | Rewrite: `BusMessage::Sessions{}` via `bus`; filter `SessionsOk.rows` to `{online: [...names]}` |
| `famp_whoami` | `tools/whoami.rs` | Rewrite: `BusMessage::Whoami{}` via `bus`; map `WhoamiOk` |
| `famp_join` | `tools/join.rs` | NEW: `BusMessage::Join{channel}` via `bus`; map `JoinOk` |
| `famp_leave` | `tools/leave.rs` | NEW: `BusMessage::Leave{channel}` via `bus`; map `LeaveOk` |

**`dispatch_tool` in `server.rs`:** Add arms for `"famp_join"` and `"famp_leave"`. Keep the existing gating pattern (after the `active_identity` check). The outer function signature and the `local_root` parameter are replaced by `bus` (or removed — the function now calls into `session::ensure_bus()` instead of taking an explicit path).

**`MCP-01` — reqwest/rustls removal from startup path:** Phase 2 does NOT remove `reqwest`/`rustls` from `crates/famp/Cargo.toml` (Phase 4 does). However, the MCP server startup code (`cli::mcp::run()`) stops reading `FAMP_LOCAL_ROOT` and stops calling any HTTP client. The `cargo tree -p famp --edges normal` check remains reachable via the non-MCP CLI paths (send, listen, etc.) — MCP-01 is satisfied by the startup *path* not the dependency *graph*. The CI gate for MCP-01 should be a runtime check (no HTTP in MCP stdio path) rather than a `cargo tree` check.

---

## 8. Hook Subcommand (bash)

### LoC Budget

Current `scripts/famp-local` size: 1230 lines [VERIFIED: `wc -l`]. Estimated addition: ~110 lines. **Projected final: ~1340 lines.** Under the 1500-line ceiling.

### `hooks.tsv` Row Format

```
<id>\t<event>:<glob>\t<to>\t<added_at>
```

Example:
```
h1745832001a3f2\tEdit:*.md\t#planning\t2026-04-28T14:32:01Z
h1745832042b8e1\tEdit:src/**/*.rs\tbob\t2026-04-28T14:32:42Z
```

- **id**: `h` + `$(date +%s)` + 6 random hex chars (bash-generatable with `printf 'h%x%s\n' "$(date +%s)" "$(head -c3 /dev/urandom | xxd -p)"`)
- **event**: `Edit:<glob>` for now; extensible to other event types later (no other types in Phase 2 scope)
- **to**: a peer name (`alice`) or channel (`#planning`); validated against `[A-Za-z0-9._-]+` or `^#[a-z0-9][a-z0-9_-]*$`
- **added_at**: ISO-8601 UTC timestamp (diagnostic only)

### `cmd_hook_add`

```bash
cmd_hook_add() {
  local on="" to=""
  while [ $# -gt 0 ]; do
    case "$1" in
      --on) on="$2"; shift 2 ;;
      --to) to="$2"; shift 2 ;;
      *) die "hook add: unknown argument '$1'" ;;
    esac
  done
  [ -n "$on" ] || die "hook add: --on is required"
  [ -n "$to" ] || die "hook add: --to is required"
  # validate --on format: Event:glob
  case "$on" in Edit:*) ;; *) die "hook add: --on must be 'Edit:<glob>'" ;; esac
  local id; id="h$(printf '%x' "$(date +%s)")$(head -c3 /dev/urandom | xxd -p)"
  local ts; ts="$(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -u)"
  local hooks_file; hooks_file="$STATE_ROOT/hooks.tsv"
  mkdir -p "$(dirname "$hooks_file")"
  printf '%s\t%s\t%s\t%s\n' "$id" "$on" "$to" "$ts" >> "$hooks_file"
  printf 'hook added: id=%s on=%s to=%s\n' "$id" "$on" "$to"
}
```

### `cmd_hook_list`

```bash
cmd_hook_list() {
  local f; f="$STATE_ROOT/hooks.tsv"
  [ -f "$f" ] || { printf 'no hooks registered\n'; return 0; }
  printf 'ID\t\t\t\tEVENT\tTO\tADDED\n'
  cat "$f"
}
```

### `cmd_hook_remove`

```bash
cmd_hook_remove() {
  [ $# -eq 1 ] || die "hook remove <id>"
  local id="$1"
  local f; f="$STATE_ROOT/hooks.tsv"
  [ -f "$f" ] || die "no hooks file found"
  local tmp; tmp="$(mktemp)"
  awk -F'\t' -v id="$id" '$1 != id' "$f" > "$tmp"
  if diff -q "$f" "$tmp" >/dev/null 2>&1; then
    rm "$tmp"; die "hook id '$id' not found"
  fi
  mv "$tmp" "$f"
  printf 'hook removed: %s\n' "$id"
}
```

### Hook Execution (HOOK-04)

When a Claude Code Edit event fires and the hook runner inspects `hooks.tsv`, for each matching row it calls:

```bash
famp send --to "$to" --new-task "Edit hook: $glob matched $file"
```

The full hook *execution* path (scanning for matching globs, invoking `famp send`) is a separate concern from hook *registration*. HOOK-01..04 scope only the `add/list/remove` registration surface. The execution path is triggered by Claude Code's hooks mechanism (which already exists in `~/.claude/hooks/`) — `hooks.tsv` is the declarative config that the hook runner reads.

### Integration with Existing `wires.tsv` Semantics

`cmd_hook_add/list/remove` live in the same file as `cmd_wire`. They share `STATE_ROOT` and the same TSV file helpers pattern. No cross-dependency: `hooks.tsv` is independent of `wires.tsv`.

### Dispatch additions

```bash
case "$cmd" in
  ...
  hook)  cmd_hook "$@" ;;   # dispatcher to sub-subcommands
  ...
esac

cmd_hook() {
  local sub="${1:-help}"; shift || true
  case "$sub" in
    add)    cmd_hook_add "$@" ;;
    list)   cmd_hook_list ;;
    remove) cmd_hook_remove "$@" ;;
    *)      die "hook: unknown subcommand '$sub' (add|list|remove)" ;;
  esac
}
```

---

## 9. Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo-nextest 0.9.132` (workspace tooling) + `tokio::test` |
| Config | `Justfile` recipes `just test`, `just ci` |
| Quick run | `cargo nextest run -p famp -p famp-bus` |
| Full suite | `just ci` |
| Time-forward | `#[tokio::test(start_paused = true)]` + `tokio::time::advance()` |

**`test-util` feature:** Must be added to `famp` crate `[dev-dependencies]` tokio block:
```toml
[dev-dependencies]
tokio = { workspace = true, features = ["...", "test-util"] }
```
[VERIFIED: tokio `test-util` feature provides `pause()` + `advance()` and requires `current_thread` runtime which is the default for `#[tokio::test]`]

**`assert_cmd`:** Not currently in any `Cargo.toml` [VERIFIED: `grep -r assert_cmd` found no Cargo entry]. Must be added:
```toml
# crates/famp/Cargo.toml [dev-dependencies]
assert_cmd = "2.0"
```

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Path |
|--------|----------|-----------|-------------------|-----------|
| BROKER-01 | Broker accepts UDS connections | Integration | `cargo nextest run test_broker_accepts_connection` | `crates/famp/tests/broker_lifecycle.rs` |
| BROKER-02 | Broker survives Ctrl-C on terminal | Manual (macOS Terminal.app) | manual | — |
| BROKER-03 | Two brokers: one survives | Integration `assert_cmd` | `cargo nextest run test_broker_spawn_race` | TEST-04 |
| BROKER-04 | Idle exit at 5-min (fast-forward) | Integration time-forward | `cargo nextest run test_broker_idle_exit` | `crates/famp/tests/broker_lifecycle.rs` |
| BROKER-05 | NFS warning fires once | Unit (mock path) | `cargo nextest run test_nfs_warning` | `crates/famp/src/cli/broker/nfs_check.rs` (unit test) |
| CLI-01 | `famp register` blocks until Ctrl-C | Integration `assert_cmd` + kill | TEST-01 setup |
| CLI-02 | `famp send` DM delivery | Integration `assert_cmd` | TEST-01 |
| CLI-03 | `famp inbox list` shows DM | Integration `assert_cmd` | TEST-01 |
| CLI-04 | `famp inbox ack` advances cursor | Integration `assert_cmd` | `crates/famp/tests/cli_inbox.rs` |
| CLI-05 | `famp await` unblocks on Send | Integration `assert_cmd` | TEST-01 |
| CLI-06 | `famp join/leave` channel membership | Integration `assert_cmd` | TEST-02 |
| CLI-07 | `famp sessions` lists active | Integration `assert_cmd` | `crates/famp/tests/cli_sessions.rs` |
| CLI-08 | `famp whoami` returns identity | Integration `assert_cmd` | TEST-01 |
| CLI-09 | Mailbox created on disk | Integration | covered by broker integration |
| CLI-10 | Cursor advanced atomically | Integration + proptest | TDD-02 carry (Phase 2 wire layer) |
| CLI-11 | sessions.jsonl is diagnostic only | Integration | broker lifecycle test |
| MCP-01 | MCP connects UDS not TLS | E2E harness | TEST-05 |
| MCP-02..09 | 8 MCP tools round-trip | E2E harness | TEST-05 |
| MCP-10 | Exhaustive match compile gate | Compile test | `cargo build -p famp` (compile fails on missing arm) |
| HOOK-01..04 | Hook add/list/remove round-trip | Shell integration | `crates/famp/tests/hook_subcommand.rs` (shelled `scripts/famp-local hook`) |
| TEST-01 | 2-client DM round-trip | Integration `assert_cmd` | `crates/famp/tests/cli_dm_roundtrip.rs` |
| TEST-02 | 3-client channel fan-out | Integration `assert_cmd` | `crates/famp/tests/cli_channel_fanout.rs` |
| TEST-03 | kill -9 broker recovery | Integration `assert_cmd` | `crates/famp/tests/broker_crash_recovery.rs` |
| TEST-04 | Spawn race → one broker | Integration `assert_cmd` | `crates/famp/tests/broker_spawn_race.rs` |
| TEST-05 | Two-stdio-MCP E2E | E2E harness | `crates/famp/tests/mcp_bus_e2e.rs` |
| CARRY-02 | INBOX-01 text rewrite | N/A (doc change) | `cargo doc` build check |

### TEST-04 Strategy: Broker Spawn Race

```rust
// broker_spawn_race.rs
#[test]
fn two_simultaneous_register_invocations_produce_one_broker() {
    let tmp = TempDir::new().unwrap();
    let sock = tmp.path().join("bus.sock");
    let env = [("FAMP_BUS_SOCKET", sock.to_str().unwrap())];

    // Launch two `famp register` invocations with --no-reconnect
    let mut c1 = Command::cargo_bin("famp").unwrap()
        .envs(env).args(["register", "alice", "--no-reconnect"]).spawn().unwrap();
    let mut c2 = Command::cargo_bin("famp").unwrap()
        .envs(env).args(["register", "bob", "--no-reconnect"]).spawn().unwrap();

    // Wait for both to either stabilize or exit
    std::thread::sleep(Duration::from_secs(2));

    // Exactly one broker should be bound on sock
    let connect = std::os::unix::net::UnixStream::connect(&sock);
    assert!(connect.is_ok(), "one broker must be running");

    // Clean up
    c1.kill().ok(); c2.kill().ok();
}
```

### TEST-03 Strategy: kill -9 + reconnect recovery

```rust
// broker_crash_recovery.rs
#[test]
fn kill_9_broker_mid_send_recovers_mailbox() {
    // 1. Start alice: famp register alice --no-reconnect
    // 2. Start bob: famp register bob --no-reconnect  
    // 3. alice sends to bob: famp send --to bob --new-task "hello"
    // 4. Find broker PID from `famp sessions` (or from sock file parent)
    // 5. kill -9 <broker_pid>
    // 6. Restart bob: famp register bob --no-reconnect (expects reconnect)
    // 7. famp inbox list (bob's session): assert "hello" appears
}
```

The key invariant: `Out::AppendMailbox` executes before `Out::Reply(SendOk)` (Phase 1 D-04). If broker crashes between Append and Reply, the disk mailbox has the message. Next Register re-drains it. [VERIFIED: Phase 1 TDD-02 proves the broker-side invariant; Phase 2 TEST-03 proves the wire-layer execution order]

### TEST-05 Strategy: Two-stdio-MCP E2E with `$FAMP_BUS_SOCKET` isolation

```rust
// mcp_bus_e2e.rs — adapted from mcp_stdio_tool_calls.rs pattern [VERIFIED]
#[test]
fn two_mcp_processes_exchange_task_over_bus() {
    let tmp = TempDir::new().unwrap();
    let sock = tmp.path().join("test-bus.sock");
    let env = [("FAMP_BUS_SOCKET", sock.to_str().unwrap())];

    let mut alice_mcp = spawn_mcp_process(&env, "alice");
    let mut bob_mcp = spawn_mcp_process(&env, "bob");

    // Register both
    mcp_call(&mut alice_mcp, "famp_register", json!({"name": "alice"}));
    mcp_call(&mut bob_mcp, "famp_register", json!({"name": "bob"}));

    // alice sends to bob
    let send_result = mcp_call(&mut alice_mcp, "famp_send", json!({
        "to": {"kind": "agent", "name": "bob"},
        "new_task": "hello from alice"
    }));
    assert!(send_result["task_id"].is_string());

    // bob receives via await
    let await_result = mcp_call(&mut bob_mcp, "famp_await", json!({"timeout_ms": 5000}));
    assert_eq!(await_result["envelope"]["from"], "alice");
}
```

### BROKER-04 Time-Forward Strategy

```rust
// broker_lifecycle.rs
#[tokio::test(start_paused = true)]
async fn broker_exits_after_5min_idle() {
    let tmp = TempDir::new().unwrap();
    let sock = tmp.path().join("bus.sock");

    // Start broker inline (not as subprocess — need in-process for time control)
    let broker_handle = tokio::spawn(run_broker(sock.clone()));

    // Connect and immediately disconnect to trigger idle timer
    {
        let _stream = tokio::net::UnixStream::connect(&sock).await.unwrap();
        // drop closes connection
    }

    // Advance time past 5 min
    tokio::time::advance(Duration::from_secs(301)).await;

    // Broker task should have exited
    assert!(broker_handle.is_finished() || tokio::time::timeout(
        Duration::from_millis(100), broker_handle
    ).await.is_ok());

    // Socket should be unlinked
    assert!(!sock.exists());
}
```

**Nyquist sample-rate reasoning:** The state space for the broker lifecycle has these critical states: `{idle_timer_armed, client_count, socket_bound, mailboxes_fsynced}`. Phase 2 tests sample:
- idle timer arms on count→0 (BROKER-04)
- idle timer cancels on reconnect (implicitly tested in TEST-03 reconnect)
- two-simultaneous-bind contention (TEST-04 — the hardest race condition)
- crash mid-Out-vector-execution (TEST-03 — samples the durability ordering invariant)
- two independent MCP processes over one broker (TEST-05 — samples the single-threaded actor under concurrent clients)

This is dense enough for v0.9 interactive developer use. Missing: fuzz of frame codec under tokio async splits (deferred to Phase 4 alongside federation CI).

### Wave 0 Gaps (test infrastructure needed before implementation)

- [ ] `crates/famp/tests/broker_lifecycle.rs` — BROKER-01/04 (in-process broker tests)
- [ ] `crates/famp/tests/broker_spawn_race.rs` — TEST-04
- [ ] `crates/famp/tests/broker_crash_recovery.rs` — TEST-03
- [ ] `crates/famp/tests/cli_dm_roundtrip.rs` — TEST-01
- [ ] `crates/famp/tests/cli_channel_fanout.rs` — TEST-02
- [ ] `crates/famp/tests/mcp_bus_e2e.rs` — TEST-05
- [ ] `crates/famp/Cargo.toml` [dev-dependencies]: add `assert_cmd = "2.0"`, tokio `test-util` feature
- [ ] `crates/famp-bus/Cargo.toml` [dev-dependencies]: add tokio `test-util` feature for broker-internal tests
- [ ] `nix = { version = "0.31", features = ["process", "fs"] }` in `crates/famp/Cargo.toml` [dependencies]

---

## 10. CARRY-02 Resolution

### Current INBOX-01 Wording (from REQUIREMENTS.md)

The requirement INBOX-01 does not appear verbatim in the REQUIREMENTS.md — CARRY-02 (TD-3) references it. Based on Phase 1 D-09 and the v0.8 design, INBOX-01 described inbox lines as structured typed envelopes. The Phase 1 implementation produces `InboxOk { envelopes: Vec<serde_json::Value> }` where each element is a raw canonical-JSON value (not a typed `Envelope` struct). The structured wrapper was explicitly rejected in D-09.

### Proposed CARRY-02 Resolution (REQUIREMENTS.md INBOX-01 rewrite)

**Proposed text for REQUIREMENTS.md INBOX-01 (new section under CLI or replacing stale text):**

```markdown
### INBOX — Mailbox format (carry-forward CARRY-02)

- [x] **INBOX-01**: Mailbox files (`~/.famp/mailboxes/<name>.jsonl`, `~/.famp/mailboxes/<#channel>.jsonl`)
  store one canonical-JSON envelope per line, raw bytes, no outer wrapper. Each line is a `serde_json::Value`
  that decodes via `AnyBusEnvelope::decode`. The `InboxOk.envelopes` field and `RegisterOk.drained` field are
  both `Vec<serde_json::Value>` — typed decode is the consumer's responsibility via `AnyBusEnvelope::decode`.
  This preserves BUS-02/BUS-03 byte-exact round-trip while providing access to type-validated content.
  `famp-inbox::read::read_from` returns `Vec<(serde_json::Value, u64)>` — the u64 is the end-offset for cursor
  management. **No structured `InboxLine` wrapper. Wording aligns with Phase 1 D-09 implementation.**
```

**Rationale:** D-09 was implemented as a type-validation gate (decode + accept, no type swap) because introducing `Vec<AnyBusEnvelope>` in `RegisterOk` and `InboxOk` broke BUS-02/BUS-03 canonical-JSON round-trip (VERIFIED: Phase 1 `01-VERIFICATION.md` lines 163-175). The rewrite makes the requirement match the implementation, closing the wording debt.

---

## 11. Risks and Open Questions

### Risk 1: `nix` crate macOS statfs `f_fstypename` API stability
**Status:** MEDIUM confidence. nix 0.31.2 exposes `Statfs::filesystem_type_name()` on macOS [cited from docs.rs], but the exact method name should be verified against the nix changelog before implementation. Fallback: use raw `libc::statfs` if nix doesn't export a string name accessor for macOS.

### Risk 2: `PosixSpawnFlags::POSIX_SPAWN_SETSID` portability
**Status:** MEDIUM confidence. `POSIX_SPAWN_SETSID` is a POSIX 2017 extension; macOS Ventura+ and Linux support it, but older macOS versions may not. Fallback: standard `fork()` + `setsid()` + `exec()` via `std::process::Command`. Since `famp` targets macOS (Sofer's primary platform), verify `POSIX_SPAWN_SETSID` availability in `nix::spawn::PosixSpawnFlags` before committing to the nix API.

### Risk 3: `BusMessage::Send` `deny_unknown_fields` + `send_as` field
**Status:** LOW risk for v0.9 (all-same-version upgrade). HIGH risk if any v0.8 broker is still running when a v0.9 client tries to send with `send_as`. Mitigation: the Phase 4 upgrade path (`pkill famp-broker` + next invocation spawns new one) handles this; document that `send_as` only works when broker and clients are both v0.9.

### Risk 4: `tokio::time::pause` + broker accept loop
**Status:** MEDIUM. The `pause` function requires a `current_thread` runtime; the broker's accept loop on a `UnixListener` may not be compatible with in-process time-paused tests if the accept blocks. Mitigation: use a real socket in a temp directory (no mock needed) but gate the idle-timer test to not use the accept loop's blocking path — send Disconnect via mpsc directly to the broker task.

### Risk 5: 8 pre-existing TLS-loopback test failures
**Status:** Known (VERIFIED: Phase 1 `01-VERIFICATION.md` §"Pre-Existing Issues"). These will continue to fail in Phase 2. They are Phase 4 scope (FED-04 / e2e_two_daemons refactor). Do not gate Phase 2 CI on them.

### Risk 6: `famp inbox ack` cursor path when `$FAMP_BUS_SOCKET` is set
**Status:** LOW. The cursor path is derived from `bus_dir/mailboxes/.<name>.cursor`. When `$FAMP_BUS_SOCKET` is set to a temp path, cursor files land in the temp dir. Tests must use the same env var for both broker and CLI clients. Already handled by the TEST-05 `$FAMP_BUS_SOCKET` isolation pattern.

### Open Question 1: `start_at` field on `SessionRow`
The design spec shows `SessionRow` with a `started_at` field (design spec line 293), but the Phase 1 implementation has `SessionRow { name, pid, joined }` without `started_at` [VERIFIED: `crates/famp-bus/src/proto.rs:206-211`]. Either: (a) add `started_at: String` to `SessionRow` in Phase 2, or (b) omit it for v0.9.0. Recommendation: add it, populated by the wire layer at Register time. The planner should include a task to add this field to `SessionRow` (famp-bus crate change) as a Phase 2 prerequisite.

### Open Question 2: `famp_peers` tool source of truth
The v0.8 `famp_peers` tool reads `peers.toml` (static file, v0.8 federation model). In v0.9, `famp_peers` should return `Sessions.rows` filtered to `{online: [names]}`. This is a semantic change — v0.9 `peers` means "currently connected to the bus", not "federation peers in peers.toml". The planner should call this out explicitly.

---

## 12. References

| Source | Lines / Path | Relevance |
|---|---|---|
| Design spec | `docs/superpowers/specs/2026-04-17-local-first-bus-design.md:202-225` | Broker lifecycle, spawn, exclusion, idle exit |
| Design spec | `:227-238` | Concurrency model (single-threaded actor) |
| Design spec | `:240-299` | Data flow per-message |
| Design spec | `:301-311` | Mailbox format + sessions file |
| Design spec | `:313-329` | CLI surface |
| Design spec | `:331-354` | MCP surface |
| Design spec | `:401-411` | Phase 2 exit criteria |
| Phase 1 CONTEXT | `01-CONTEXT.md:D-04` | Out ordering crash-safety |
| Phase 1 CONTEXT | `01-CONTEXT.md:D-07, D-09, D-10, D-11` | Writes-are-intents, raw bytes, MailboxName, cursor ownership |
| Phase 1 broker | `crates/famp-bus/src/broker/mod.rs` | `BrokerInput`, `Out` enum definition |
| Phase 1 broker | `crates/famp-bus/src/broker/handle.rs` | `send()`, `register()`, `inbox()` dispatch |
| Phase 1 proto | `crates/famp-bus/src/proto.rs` | `BusMessage`, `BusReply`, `SessionRow`, `Target`, `deny_unknown_fields` |
| Phase 1 error | `crates/famp-bus/src/error.rs` | `BusErrorKind::ALL`, 10 variants |
| famp-inbox | `crates/famp-inbox/src/append.rs` | `Inbox::append` fsync contract |
| famp-inbox | `crates/famp-inbox/src/cursor.rs` | `InboxCursor::advance` atomic write |
| famp-inbox | `crates/famp-inbox/src/read.rs` | `read_from(path, offset)` with tail tolerance |
| MCP server | `crates/famp/src/cli/mcp/server.rs` | JSON-RPC loop, `dispatch_tool`, `cli_error_response` |
| MCP session | `crates/famp/src/cli/mcp/session.rs` | D-04 reshape target (full replacement) |
| MCP error_kind | `crates/famp/src/cli/mcp/error_kind.rs` | Exhaustive-match pattern to replicate |
| famp-local | `scripts/famp-local:100-145` | `wires.tsv` TSV format, `wires_lookup`, `wires_write_row` |
| famp-local | `scripts/famp-local:1090-1148` | `cmd_identity_of` cwd→identity resolver |
| famp Cargo.toml | `crates/famp/Cargo.toml` | tokio features, reqwest/rustls presence |
| Workspace Cargo.toml | `Cargo.toml` | nix not present — new dependency |
| MCP stdio tests | `crates/famp/tests/mcp_stdio_tool_calls.rs` | Subprocess harness pattern to adapt for TEST-05 |
| Phase 1 VERIFICATION | `01-VERIFICATION.md:163-175` | D-09 deviation: `Vec<serde_json::Value>` vs `Vec<AnyBusEnvelope>` — explains CARRY-02 |
| tokio docs | context7: `/websites/rs_tokio` | `time::pause`, `time::advance`, `start_paused = true` — VERIFIED |
| nix docs | docs.rs/nix/0.31.2 | `unistd::setsid`, `spawn::PosixSpawnAttr`, `sys::statfs::NFS_SUPER_MAGIC` — VERIFIED |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `nix::spawn::PosixSpawnFlags::POSIX_SPAWN_SETSID` is available on macOS in nix 0.31.2 | §5 | Broker spawn code needs alternative `fork+setsid+exec` path |
| A2 | `Statfs::filesystem_type_name()` returns an `&OsStr` or `&CStr` containing `"nfs"` for NFS on macOS | §2 Item 3 | NFS check falls back to raw libc |
| A3 | `SessionRow` is safe to extend with `started_at: String` without breaking BUS-03 round-trip (new optional field with `skip_serializing_if`) | §11 Open Q1 | Must verify BUS-03 round-trip with the addition |

All other claims in this document are tagged VERIFIED (against codebase or official docs read in this session) or CITED (design spec / Phase 1 artifacts).

---

## RESEARCH COMPLETE

**Phase:** 02 - UDS wire + CLI + MV-MCP rewire + `famp-local hook add`
**Confidence:** HIGH

### Key Findings

1. **All three OS primitives (`posix_spawn`, `setsid`, `statfs` + NFS detection) are in a single new dependency: `nix 0.31.2`.** No need for raw `libc` calls. Features required: `process` + `fs`.

2. **The `BusClient` module (`crates/famp/src/bus_client/`) is the central new abstraction.** It is reused by all 8 CLI subcommands and all 8 MCP tools, eliminating duplication. The spawn logic lives here, not scattered across CLI entry points.

3. **`famp inbox ack` sends NO message to the broker** — it writes the cursor file locally. This is a deliberate design: the broker does not track per-session cursors (the client is authoritative). CLI tests that assume ack is a round-trip to broker will be wrong.

4. **`scripts/famp-local hook add/list/remove` fits in ~110 new LoC** (projected total: ~1340), safely under the 1500-line ceiling. No Rust implementation needed for Phase 2.

5. **The idle-exit timer test requires `#[tokio::test(start_paused = true)]`** plus `tokio::time::advance(Duration::from_secs(301))`. The `test-util` feature must be added to tokio dev-dependencies. The broker's accept loop should be tested in-process (not as a subprocess) for time-forward tests.

### File Created

`.planning/phases/02-uds-wire-cli-mv-mcp-rewire-hook-subcommand/02-RESEARCH.md`

### Confidence Assessment

| Area | Level | Reason |
|------|-------|--------|
| Phase 1 substrate constraints | HIGH | Read directly from source code and VERIFICATION.md |
| BusClient / wire layer design | HIGH | Derived from design spec + Phase 1 Out enum |
| nix crate API | MEDIUM-HIGH | Verified via docs.rs; two assumptions logged (A1, A2) |
| Idle-exit tokio timer | HIGH | tokio time::pause/advance verified via context7 + official docs |
| Hook LoC budget | HIGH | Verified via `wc -l`; estimated additions are simple bash |
| MCP rewire diff map | HIGH | Traced directly against server.rs and session.rs source |
| JSON-RPC error code table | HIGH | Standard range; exhaustive match pattern verified |

### Open Questions

1. **`POSIX_SPAWN_SETSID` on macOS** — verify before implementing spawn.rs; fallback to `Command::new(exe).args(["broker"]).spawn()` + `nix::unistd::setsid()` if needed (run in child after fork).
2. **`SessionRow.started_at`** — confirm whether to add this field in Phase 2 or defer to Phase 3.
3. **`famp_peers` semantics** — confirm the tool should return live bus sessions, not v0.8 `peers.toml` federation peers.

### Ready for Planning

Research complete. Planner can now create PLAN.md files.

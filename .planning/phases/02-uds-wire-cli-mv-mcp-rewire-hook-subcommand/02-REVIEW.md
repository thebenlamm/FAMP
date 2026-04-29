---
phase: 02-uds-wire-cli-mv-mcp-rewire-hook-subcommand
reviewed: 2026-04-28T22:00:00Z
depth: standard
files_reviewed: 60
files_reviewed_list:
  - Cargo.lock
  - Justfile
  - crates/famp-bus/Cargo.toml
  - crates/famp-bus/src/broker/handle.rs
  - crates/famp-bus/src/broker/state.rs
  - crates/famp-bus/src/lib.rs
  - crates/famp-bus/src/proto.rs
  - crates/famp/Cargo.toml
  - crates/famp/src/bin/famp.rs
  - crates/famp/src/bus_client/codec.rs
  - crates/famp/src/bus_client/mod.rs
  - crates/famp/src/bus_client/spawn.rs
  - crates/famp/src/cli/broker/accept.rs
  - crates/famp/src/cli/broker/cursor_exec.rs
  - crates/famp/src/cli/broker/idle.rs
  - crates/famp/src/cli/broker/mailbox_env.rs
  - crates/famp/src/cli/broker/mod.rs
  - crates/famp/src/cli/broker/nfs_check.rs
  - crates/famp/src/cli/broker/sessions_log.rs
  - crates/famp/src/cli/error.rs
  - crates/famp/src/cli/identity.rs
  - crates/famp/src/cli/inbox/ack.rs
  - crates/famp/src/cli/inbox/list.rs
  - crates/famp/src/cli/inbox/mod.rs
  - crates/famp/src/cli/join.rs
  - crates/famp/src/cli/leave.rs
  - crates/famp/src/cli/mcp/error_kind.rs
  - crates/famp/src/cli/mcp/server.rs
  - crates/famp/src/cli/mcp/session.rs
  - crates/famp/src/cli/mcp/tools/await_.rs
  - crates/famp/src/cli/mcp/tools/inbox.rs
  - crates/famp/src/cli/mcp/tools/join.rs
  - crates/famp/src/cli/mcp/tools/leave.rs
  - crates/famp/src/cli/mcp/tools/mod.rs
  - crates/famp/src/cli/mcp/tools/peers.rs
  - crates/famp/src/cli/mcp/tools/register.rs
  - crates/famp/src/cli/mcp/tools/send.rs
  - crates/famp/src/cli/mcp/tools/whoami.rs
  - crates/famp/src/cli/mod.rs
  - crates/famp/src/cli/register.rs
  - crates/famp/src/cli/send/mod.rs
  - crates/famp/src/cli/sessions.rs
  - crates/famp/src/cli/util.rs
  - crates/famp/src/cli/whoami.rs
  - crates/famp/src/lib.rs
  - crates/famp/tests/broker_crash_recovery.rs
  - crates/famp/tests/broker_lifecycle.rs
  - crates/famp/tests/broker_proxy_semantics.rs
  - crates/famp/tests/broker_spawn_race.rs
  - crates/famp/tests/cli_channel_fanout.rs
  - crates/famp/tests/cli_dm_roundtrip.rs
  - crates/famp/tests/cli_inbox.rs
  - crates/famp/tests/cli_sessions.rs
  - crates/famp/tests/common/conversation_harness.rs
  - crates/famp/tests/common/mcp_harness.rs
  - crates/famp/tests/e2e_two_daemons.rs
  - crates/famp/tests/hook_subcommand.rs
  - crates/famp/tests/mcp_bus_e2e.rs
  - crates/famp/tests/mcp_error_kind_exhaustive.rs
  - crates/famp/tests/mcp_malformed_input.rs
  - crates/famp/tests/mcp_pre_registration_gating.rs
  - crates/famp/tests/mcp_register_whoami.rs
  - crates/famp/tests/mcp_stdio_tool_calls.rs
  - scripts/check-mcp-deps.sh
  - scripts/famp-local
findings:
  blocker: 5
  warning: 11
  info: 6
  total: 22
status: issues_found
---

# Phase 02 Code Review — UDS Wire / CLI Move / MCP Rewire / Hook Subcommand

**Reviewed:** 2026-04-28T22:00:00Z
**Depth:** standard
**Files Reviewed:** 60+
**Status:** issues_found

## Summary

The Phase 02 work re-platforms the FAMP CLI on a Unix domain socket broker
(`famp-bus`), introduces D-10 proxy semantics (`Hello.bind_as`), rewires
the MCP tool surface, and ships a `hook` subcommand in `scripts/famp-local`.
The shape is coherent and well-tested. Several bugs in the broker actor and
register/reconnect logic warrant blockers before merge; a handful of
warnings cover process-exit smell, env-var test races, comment/code
mismatches, and unbounded resource growth that will manifest in long-running
broker processes. No security vulnerabilities found at this depth; no
hardcoded secrets, no command injection, no path traversal.

The critical and warning items below are the load-bearing fixes.

## Blocker Issues

### BL-01: `register` reconnect backoff resets on EVERY disconnect, defeating bounded backoff

**File:** `crates/famp/src/cli/register.rs:99-107`
**Issue:** The `Disconnected` arm unconditionally resets `delay = RECONNECT_INITIAL` *before* sleeping/doubling, not after a *long-running* successful session. If the broker dies repeatedly (e.g. the broker can't bind the socket and exits immediately, or there's a recurring crash), every iteration sleeps 1s, doubles to 2s, then resets to 1s on the next iteration. The intended `1 → 2 → 4 → 8 → 16 → 30` schedule (documented in the module comment and locked by the unit test `reconnect_backoff_schedule_matches_research_item_8`) NEVER actually fires across reconnect cycles. Effectively the schedule collapses to a flat 1s wait between every reconnect attempt — exactly the thundering-herd / busy-loop behavior the bounded backoff is supposed to prevent.

The unit test passes only because it directly exercises the `min(d * 2, RECONNECT_CAP)` formula on a local `d` variable; it does NOT exercise the actual run-loop branching that resets `delay`.

**Fix:**
```rust
Ok(SessionOutcome::Disconnected) => {
    if args.no_reconnect {
        return Err(CliError::Disconnected);
    }
    eprintln!("broker disconnected — reconnecting in {}s", delay.as_secs());
    tokio::time::sleep(delay).await;
    delay = std::cmp::min(delay * 2, RECONNECT_CAP);
    // Do NOT reset delay here. Reset only after observing a session that
    // ran for >= some threshold (e.g. 60s) so transient broker bounces
    // accumulate backoff but a long-running session that finally exits
    // gets a fresh backoff window.
}
```
And add an integration test that covers the multi-disconnect schedule, not just the formula.

---

### BL-02: `bind_exclusive` calls `std::process::exit(0)` from a non-`main` helper, swallowing all cleanup

**File:** `crates/famp/src/cli/broker/mod.rs:131-135`
**Issue:** When `bind_exclusive` detects another live broker, it calls `std::process::exit(0)`. This:

1. Skips Rust-level destructors — open files, mutexes, tempfiles, the just-built `DiskMailboxEnv` (which hasn't been built yet at this point, but the parent caller hasn't either), any `tokio` runtime in the process. In practice today this is OK because `bind_exclusive` runs early in `run()`, but the *signature* of the function is `Result<UnixListener, CliError>` and a successful "another broker exists" outcome should be a typed `Ok` or `Err` — not an out-of-band process exit. A future caller composing `run` into a larger process (e.g. a future `famp watchdog` or a test harness in-process broker mode) will silently die from this exit.
2. Makes the function untestable for the "broker already present" branch — the test can only assert behavior by spawning a subprocess.
3. Loses a future-CI regression detector: any Bash wrapper expecting `famp broker` to print the "broker already running" diagnostic and continue cannot do so.

**Fix:** Return a typed enum from `bind_exclusive`:
```rust
enum BindOutcome {
    Bound(UnixListener),
    Existing,  // another broker is live; caller should noop and exit Ok
}
```
Then in `run`, branch on `BindOutcome::Existing` and `return Ok(())` from `run` itself, letting destructors run in scope. Update the in-process test harness path (`run_on_listener`) to never see `Existing` (it takes a pre-bound listener).

---

### BL-03: Broker `clients` map grows unboundedly — disconnected entries never removed

**File:** `crates/famp-bus/src/broker/handle.rs:490-529` (`disconnect`); `crates/famp-bus/src/broker/state.rs:34-40`
**Issue:** `disconnect` flips `state.connected = false` and clears `state.joined`, but the `ClientId → ClientState` entry remains in `BrokerState.clients` for the broker's lifetime. Every short-lived proxy connect (one per `famp send`, `famp inbox list`, `famp whoami`, …) creates a `ClientId` that never frees. With the 5-minute idle exit currently in place, this is bounded by however many connections happen in 5 minutes — but:

- The idle timer ARMS only when `client_count == 0` and a connect resets it (`crates/famp/src/cli/broker/mod.rs:194-195`). A long-lived `famp register alice` daemon plus one short-lived proxy every few seconds means the idle timer NEVER arms, and the map grows without bound for the lifetime of the canonical holder.
- Functions iterate `clients.values()` / `clients.iter()` to find the canonical holder (`canonical_holder_id`, `proxy_holder_alive`, `connected_names`, `waiting_client_for_name`, `tick`'s dead-client sweep at line 532). Every one of these is O(N) where N includes all the dead `connected = false` entries. Real-world traffic from Sofer's mesh (multiple agents, frequent tool calls) accumulates dead entries fast.
- `tick`'s liveness sweep at handle.rs:531-543 also re-iterates dead clients, calling `is_alive(pid)` on a `None` PID … which short-circuits via `state.pid?` → fine, but the iteration itself is wasted work.

**Fix:** In `disconnect`, after the cleanup, remove the entry:
```rust
broker.state.clients.remove(&client);
```
(Keep `state.connected = false` first if any caller needs to observe the transient state — but no caller does after the function returns.) Verify nothing in the tick/liveness path relies on dead entries persisting; if a future audit/replay needs the history, write to `sessions.jsonl` — exactly the role that file already serves.

---

### BL-04: `BusReply::Inbox` on a missing/empty `since` deserializes fine, but `BusMessage::Inbox` deserialization is asymmetric vs `Hello`

**File:** `crates/famp-bus/src/proto.rs:131-136`
**Issue:** `BusMessage::Inbox.since` and `include_terminal` use `#[serde(skip_serializing_if = "Option::is_none")]` *without* `#[serde(default)]`. `Hello.bind_as` uses both. The asymmetric set is a footgun for two reasons:

1. `serde` does provide implicit-None for missing `Option` fields *most* of the time, but combined with the enum's `#[serde(deny_unknown_fields)]` on a tagged union, this behavior is not as obvious as the rest of the file makes it look. The comment block at line 332-353 specifically argues for `default + skip_serializing_if` together for byte-exact round-trip; Inbox should follow the same locked pattern.
2. A `BusReply::InboxOk.next_offset` round-trip currently works — but if a future field is added without `default`, callers that omit it (legitimately, on the wire) will get a strict-parse failure. The code was clearly written by multiple authors with different conventions; lock the convention.

This is BLOCKER (not WARNING) because the wire-protocol invariant tests (`hello_bind_as_none_byte_identical_to_pre_d10` and friends) are the ENTIRE conformance gate for the bus protocol per BUS-02. A subsequent author wiring a new optional field by copying the Inbox shape (no `default`) will silently regress byte-exact round-trip for some inputs and the property test won't catch it because the canonicalize-then-decode round-trip works while the missing-field-decode-only path is broken.

**Fix:** Add `default` to every `skip_serializing_if = "Option::is_none"` field on `BusMessage` and `BusReply`:
```rust
Inbox {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    since: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    include_terminal: Option<bool>,
},
Await {
    timeout_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    task: Option<uuid::Uuid>,
},
```
And add a property test that asserts every `BusMessage`/`BusReply` variant with an `Option` field round-trips byte-exact when the field is None AND deserializes correctly when omitted from the wire form.

---

### BL-05: `is_alive` accepts PID 0 and treats it as a real PID, sending `kill(0, 0)` to the calling process group

**File:** `crates/famp/src/cli/broker/mailbox_env.rs:101-108`
**Issue:** `is_alive(pid: u32)` casts to `i32` and passes through to `nix::sys::signal::kill(Pid::from_raw(raw), None)`. POSIX defines `kill(pid=0, sig)` as "send the signal to every process in the calling process's process group." With `sig=None` (i.e. signal 0), this is a no-op probe — but it returns `Ok(())` whenever the calling process has *any* process group, which is essentially always. So `is_alive(0)` returns `true`.

This matters because `BusMessage::Register { pid: 0, name }` is a valid wire frame (the schema is `pid: u32` with no constraint) and a misbehaving or malicious client can claim PID 0. The broker's per-op liveness gate (`proxy_holder_alive`) then *always* says the holder is alive, defeating the entire D-10 invariant. A proxy can ride on a "registered as alice with pid=0" forever, and the canonical-holder check that's supposed to clean up dead registrations never fires. Same issue applies to `pid=-1` casts (broadcasts to all processes the caller can signal).

**Fix:**
```rust
fn is_alive(&self, pid: u32) -> bool {
    if pid == 0 {
        // POSIX kill(0, _) targets the calling pgrp; reject as invalid.
        return false;
    }
    let Ok(raw) = i32::try_from(pid) else { return false; };
    if raw <= 0 {
        // Negative or zero raw values have special POSIX semantics
        // (process group / all processes). Reject.
        return false;
    }
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(raw), None).is_ok()
}
```
Additionally, validate `pid != 0` in the `Register` handler so the name is never even bound to PID 0.

---

## Warning Issues

### WR-01: `resolve_sock_path` doc comment claims it panics; implementation does not

**File:** `crates/famp/src/bus_client/mod.rs:166-182`
**Issue:** Doc comment lines 167-170 say:
> # Panics
> Panics if `$FAMP_BUS_SOCKET` is unset AND `$HOME` is also unset.
> CLI subcommands should resolve early so the panic is observable …

The implementation uses `dirs::home_dir().unwrap_or_else(|| PathBuf::from("/nonexistent-famp-home"))` — it does NOT panic. The fallback is silent and the next syscall fails at a path the operator may not understand. Either the comment lies (most likely; code reviewers count on Panics docs to identify panic call sites) or the code was changed without updating the doc.

**Fix:** Change the doc to describe actual behavior, OR change the code to panic explicitly with a clear message. Given the current "fail visibly later" rationale in the comment block at lines 175-179, the doc should be:
```rust
/// # Behavior
/// Falls back to `/nonexistent-famp-home/.famp/bus.sock` when both
/// `$FAMP_BUS_SOCKET` and `$HOME` are unset, so the next syscall fails
/// visibly rather than silently writing into the cwd.
```

---

### WR-02: `wires.tsv` row matching has a logically broken second clause

**File:** `crates/famp/src/cli/identity.rs:87-91`
**Issue:**
```rust
let row_canon = Path::new(dir_str)
    .canonicalize()
    .unwrap_or_else(|_| PathBuf::from(dir_str));
if row_canon == cwd_canon || Path::new(dir_str) == cwd_canon {
    return Ok(Some(name.to_string()));
}
```
The second disjunct compares `Path::new(dir_str)` (uncanonicalized, raw on-disk row value) against `cwd_canon` (the canonicalized cwd). For this to be `true`, the row's on-disk text would have to *already* equal the canonical cwd path — in which case the first disjunct already matched. The clause is dead code OR it was intended to compare against the *un*canonicalized cwd as a fallback when canonicalize fails. The comment says symlink-tolerance is the goal; falling back to `cwd` (uncanonicalized) when canonicalize fails would be more useful than the current second clause.

**Fix:**
```rust
let cwd_raw = cwd.clone();
// ...
if row_canon == cwd_canon || Path::new(dir_str) == cwd_raw {
    return Ok(Some(name.to_string()));
}
```
And add a unit test for the case where `cwd.canonicalize()` fails (e.g. a deleted directory the test process has open via fchdir).

---

### WR-03: `normalize_channel` recompiles regex on every call, despite contradicting comment

**File:** `crates/famp/src/cli/util.rs:46-48`
**Issue:** Comment claims "The regex is compiled once on first use; failure to compile is a programmer bug." But `regex::Regex::new(CHANNEL_PATTERN)` is called inside the function body — it compiles on every invocation. CLI calls this once per invocation, so the cost is negligible *today*; but every MCP tool call (`famp_send`, `famp_join`, `famp_leave`) routes through here and a tight MCP loop pays the cost N times.

More importantly, the bus-side `proto.rs:14-17` already has a `LazyLock<Regex>` for the same pattern. The CLI util module duplicates the pattern as a const string and the wrong compilation strategy.

**Fix:**
```rust
use std::sync::LazyLock;
static CHANNEL_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(CHANNEL_PATTERN).expect("static channel regex compiles"));
// ... then use &*CHANNEL_RE in normalize_channel.
```
Better: expose the bus-side regex publicly from `famp_bus::proto` and remove the duplicated pattern entirely (single source of truth).

---

### WR-04: `ensure_bus` TOCTOU drops a freshly-connected `BusClient` on the floor

**File:** `crates/famp/src/cli/mcp/session.rs:97-119`
**Issue:** The "drop guard, do I/O, re-acquire guard, check again" pattern means two concurrent `ensure_bus` callers can both pass the initial `bus.is_some()` check, both call `BusClient::connect` (which spawns broker if absent and does Hello), and only one wins the "is_none()" race at line 112. The losing client is dropped, which closes its UDS connection — but the broker has already accepted the connection and processed `Hello` (potentially with `bind_as`-related state allocation on its side). Visible side effects:

1. A spurious broker accept + handshake + immediate disconnect for every losing race. With `BL-03` not yet fixed, this also leaks `ClientState` entries.
2. `broker.log` lines for the throwaway connection.
3. With `bind_as = None` (canonical-holder shape), no real harm — but if `ensure_bus` is ever called with a `bind_as`, the throwaway connection consumes a slot in the broker's per-op liveness check temporarily.

stdio MCP is single-threaded by spec (one in-flight tool call), so the race window is narrow today. But the code is shaped as if it's defensive, and the comment at line 116 acknowledges the race. The defense leaks state.

**Fix:** Use a `tokio::sync::OnceCell` or hold the lock across the `BusClient::connect` await:
```rust
pub async fn ensure_bus() -> Result<(), BusErrorKind> {
    let mut guard = state().lock().await;
    if guard.bus.is_some() {
        return Ok(());
    }
    let sock = crate::bus_client::resolve_sock_path();
    let client = BusClient::connect(&sock, None).await
        .map_err(|_| BusErrorKind::BrokerUnreachable)?;
    guard.bus = Some(client);
    Ok(())
}
```
Per the module comment at session.rs:38-41, contention is structurally bounded — holding the lock across the await is fine.

---

### WR-05: Broker `await_envelope` accepts `timeout_ms = u64::MAX`, overflowing `Instant + Duration`

**File:** `crates/famp-bus/src/broker/handle.rs:333-365`
**Issue:** `let deadline = now + Duration::from_millis(timeout_ms);` — `Duration::from_millis(u64::MAX)` is ~584 million years; adding to `Instant` *will* panic on overflow (`Instant + Duration` panics on overflow per std docs). A misbehaving or malicious client can DoS the broker by sending `BusMessage::Await { timeout_ms: u64::MAX }`.

The `BrokerInput::Wire` arm in the broker run loop (`crates/famp/src/cli/broker/mod.rs:212-247`) has no panic catch; a panic on the actor task shuts the broker down for every connected client.

**Fix:** Cap `timeout_ms`:
```rust
const MAX_AWAIT_MS: u64 = 60 * 60 * 1000; // 1 hour
let timeout_ms = timeout_ms.min(MAX_AWAIT_MS);
let deadline = now + Duration::from_millis(timeout_ms);
```
Or use `now.checked_add(Duration::from_millis(timeout_ms))` and reject the await frame on overflow.

---

### WR-06: Test env-var mutations race with each other (parallel test runner)

**File:** `crates/famp/src/cli/identity.rs:111-207`; `crates/famp/src/bus_client/mod.rs:202-214`; `crates/famp/tests/mcp_register_whoami.rs:24-28`
**Issue:** Multiple tests in the same compile unit mutate process-global env vars (`FAMP_LOCAL_IDENTITY`, `HOME`, `FAMP_BUS_SOCKET`) via `std::env::set_var`/`remove_var`. cargo nextest runs each test in a separate process by default — but `cargo test` does not. CI's `just test` uses nextest (per Justfile:13), so this is currently safe; a future contributor running `cargo test -p famp` will see flaky tests. Additionally, `set_var`/`remove_var` are `unsafe fn` in Rust 2024 edition; this code will fail to compile when the workspace upgrades.

The save/restore dance at the end of each test does not help: a test that panics between `set_var` and the restore leaks the mutation into every subsequent test in the same process.

**Fix:** Use a serialization mutex (e.g. `serial_test` crate) on every test that mutates env, OR use the `temp-env` crate's scoped-mutation helpers, OR mark these tests as `#[ignore]` and document that they must be run with `cargo nextest`. The cleanest fix is `temp-env::with_var(...)`:
```rust
#[test]
fn tier_2_env_var_wins_when_no_flag() {
    temp_env::with_var("FAMP_LOCAL_IDENTITY", Some("bob"), || {
        let got = resolve_identity(None).unwrap();
        assert_eq!(got, "bob");
    });
}
```

---

### WR-07: `sessions_log::append_session_row` writes `joined: Vec::new()` regardless of actual joined channels

**File:** `crates/famp/src/cli/broker/mod.rs:231-236`
**Issue:** On disconnect, the broker run loop emits a `SessionRow { name, pid, joined: Vec::new() }` to `sessions.jsonl`. The `joined` field is permanently empty. The diagnostic file's value is reduced — operators tailing it cannot see which channels a session held when it disconnected, which is exactly the kind of forensics this file exists for.

The information is available: `session_meta` already tracks `(name, pid)`, and the broker's `clients[client].joined` set at the moment of `BrokerInput::Disconnect` carries the channels. But by the time the run loop's `Disconnect` arm runs, the broker has already cleared `joined` (handle.rs:520). The cleanup ordering is wrong for diagnostic purposes.

**Fix:** Either:
1. Snapshot the joined set BEFORE calling `broker.handle(BrokerInput::Disconnect, …)` — the run loop has access to broker state via a getter, or
2. Make the diagnostic SessionRow part of the broker's `Out` vec (e.g. `Out::SessionEnded { name, pid, joined }`) so the executor writes it with the correct snapshot.

Option 2 is cleaner and survives a future refactor where `Disconnect` becomes idempotent.

---

### WR-08: `disconnect` doesn't actually call `disconnect`'s return value — the `let _ = ...` discards `Out` vec

**File:** `crates/famp-bus/src/broker/handle.rs:541-543`
**Issue:**
```rust
for client in dead_clients {
    let _ = disconnect(broker, client);
}
```
The `Vec<Out>` returned by `disconnect` is discarded. `disconnect` produces `Out::ReleaseClient(client)` which the broker's executor uses to drop the per-client reply sender (see `crates/famp/src/cli/broker/mod.rs:313-318`). When `tick` discovers dead clients via PID liveness probe and "disconnects" them internally, the per-client reply senders in the run loop are NEVER notified — the client's tokio write task hangs around waiting for replies on a dropped sender.

This is the *internal* disconnect path (broker discovered the client is dead via PID probe). The executor only sees the `Out::ReleaseClient` if it's surfaced. Because `tick`'s `disconnect` discard means the `Out` vec is never threaded through `execute_outs`, the run loop's `reply_senders` map keeps the `mpsc::Sender<BusReply>` alive, and the per-client write task at `accept.rs:56-67` may still be running.

In production this is partially masked because the kernel will eventually deliver an EOF when the dead client's process is reaped, the read loop exits, and *that* triggers a `BrokerMsg::Disconnect` that DOES go through `execute_outs`. But there's a window where the broker has internally cleaned up but the write task is still spinning, AND if the dead pid's UDS is somehow being held open (zombie process, fd inheritance), the leak persists.

**Fix:** In `tick`, return the merged `Out` vec instead of discarding it:
```rust
fn tick<E: BrokerEnv>(broker: &mut Broker<E>, now: Instant) -> Vec<Out> {
    let mut out = Vec::new();
    let dead_clients: Vec<ClientId> = /* same as today */;
    for client in dead_clients {
        out.extend(disconnect(broker, client));
    }
    /* expired awaits handling — same as today */
    out
}
```

---

### WR-09: `send_agent` returns `task_id: Uuid::nil()` when the recipient is not in `pending_awaits` (no `task_id` extraction from envelope)

**File:** `crates/famp-bus/src/broker/handle.rs:226-249` (`send_agent`); see also `send_ok` at line 712-720
**Issue:** `send_channel` extracts `task_id` from the envelope via `task_id_from(envelope)` (line 277). `send_agent` uses `send_ok(...)` which hardcodes `task_id: Uuid::nil()`. Result: an agent-DM SendOk reply ALWAYS has `task_id = 00000000-0000-0000-0000-000000000000`, while a channel SendOk has the real task id from the envelope.

The CLI `famp send` then prints `{"task_id":"00000000-0000-0000-0000-000000000000",...}` on every agent DM. Downstream callers that key off `task_id` (e.g. the MCP `famp_send` tool, integration tests, future automation) cannot distinguish task identity for DMs. The MCP tool docs at `server.rs:38-50` specifically tell the LLM to use `task_id` from the inbox entry to reply — but the `task_id` returned by `famp_send` itself is nil, leading to confusion.

**Fix:** Replace the `send_ok` call in `send_agent` with the same `task_id_from(envelope)` extraction:
```rust
fn send_agent<E: BrokerEnv>(/* … */) -> Vec<Out> {
    let task_id = task_id_from(&envelope);  // extract before the move
    if let Some(waiting) = waiting_client_for_name(broker, &name, &envelope) {
        broker.state.pending_awaits.remove(&waiting);
        return vec![
            Out::Reply(waiting, BusReply::AwaitOk { envelope }),
            Out::UnparkAwait { client: waiting },
            Out::Reply(sender, BusReply::SendOk {
                task_id,
                delivered: vec![Delivered { to: Target::Agent { name }, ok: true }],
            }),
        ];
    }
    /* … same SendOk shape on the non-waiting branch … */
}
```
Update `send_ok` either to take a task_id parameter or remove it.

---

### WR-10: `hello()` overwrites existing `ClientState` unconditionally, allowing protocol re-handshake to wipe state

**File:** `crates/famp-bus/src/broker/handle.rs:64-118`
**Issue:** Both branches of `hello` end with `broker.state.clients.insert(client, ClientState { … })`. If a client has already done a Hello+Register and then sends a SECOND Hello on the same connection, the second call overwrites the existing `ClientState`, wiping `name`, `pid`, `joined`. Result:

1. The canonical holder slot is silently released (`name` set back to `None`).
2. Channel members lists at `broker.state.channels` still reference the stale `name` — but `whoami`/`sessions` will report `active: None` because `state.name` was wiped.
3. A second `Register` with the same name from any other client succeeds (the old slot is no longer registered).

This is a state corruption attack. A misbehaving (or malicious) proxy connection sending a second Hello effectively un-registers the canonical holder.

**Fix:** Reject a second Hello on an already-handshaked connection:
```rust
if broker.state.clients.get(&client).map(|c| c.handshaked) == Some(true) {
    return vec![err(client, BusErrorKind::BrokerProtoMismatch, "Hello already received")];
}
```
This belongs in `handle_wire` (which currently handles the *opposite* check: rejecting non-Hello frames on un-handshaked connections).

---

### WR-11: `handle_wire`'s pre-Hello gate doesn't actually create the client entry, so `Register`-without-Hello returns the wrong error class

**File:** `crates/famp-bus/src/broker/handle.rs:22-37`
**Issue:** A client that sends `Register { name, pid }` as its first frame falls through the pre-Hello gate and returns `BrokerProtoMismatch: "Hello required as first frame"` — correct. But if a client sends `Hello { bind_as: Some("alice") }` (where alice IS registered) followed by another `Hello { bind_as: Some("bob") }`, the second Hello call to `clients.insert(client, ClientState { … })` REPLACES the prior `bind_as`. Combined with WR-10, this means:

1. A proxy can rotate identities mid-connection by sending repeated Hello frames.
2. Per-op liveness re-check passes for each because `proxy_holder_alive` looks up by the *current* `bind_as`.

This is also a state corruption / impersonation issue, related to but distinct from WR-10 (canonical holder un-register). Locking down with the WR-10 fix covers this case too.

**Fix:** Same as WR-10. After the pre-Hello gate, reject Hello frames from already-handshaked connections.

---

## Info Issues

### IN-01: Empty closures and underscore matches in `accept.rs` swallow errors silently

**File:** `crates/famp/src/cli/broker/accept.rs:46-67`
**Issue:** Both spawned tasks use `let _ = ...` and `let Ok(msg) = ... else { ... }` patterns that drop frame codec errors without logging. A `FrameError::FrameTooLarge` or `FrameError::EmptyFrame` from a misbehaving client looks identical in the broker log to a clean EOF. Operators triaging "why did this client get dropped?" have no signal.

**Fix:** Log the codec failure before treating it as a disconnect:
```rust
let msg = match codec::read_frame::<_, BusMessage>(&mut reader).await {
    Ok(m) => m,
    Err(e) => {
        eprintln!("client {id} read frame error: {e:?}");
        let _ = read_tx.send(BrokerMsg::Disconnect(id)).await;
        return;
    }
};
```

---

### IN-02: `tail_loop` selects on a sleep that's outside the select branch

**File:** `crates/famp/src/cli/register.rs:246-301`
**Issue:** The `tokio::select!` arms are `shutdown_signal()` and `client.send_recv(...)`. After the select completes (either path), execution continues to `tokio::time::sleep(TAIL_POLL_INTERVAL).await;`. If `shutdown_signal()` fires, the function returns BEFORE the sleep — fine. But if `send_recv` returns an error and the function returns Disconnected, same. The structural shape is fine, but readability suggests the sleep should be inside the select arm or as a `biased` arm so the cadence is explicit.

More importantly: `biased` is set, prioritizing the shutdown signal, but the cadence-sleep at line 299 happens *after* the select and is NOT cancellable. A long sleep (1 second) means Ctrl-C during the sleep waits the full 1s before exiting. For most operators this is fine; flagged as info because the module comment promises "1-second poll cadence" but doesn't promise sub-second Ctrl-C latency.

**Fix:** Move the cadence into the select itself with a `tokio::time::sleep` future that the select races against `send_recv`. Or accept the current behavior and document the worst-case shutdown latency.

---

### IN-03: `e2e-smoke` recipe in Justfile uses `cargo run` per command — slow, racy

**File:** `Justfile:99-124`
**Issue:** Each `FAMP_HOME=… cargo run --release …` invocation re-runs cargo's metadata check. The `init` calls block on rebuild before forking the listeners, but the listener spawns `&` background — the parent shell may exit before the listener completes its bind. The `wait $A_PID $B_PID` at line 124 catches them, but if the user kills the script with Ctrl-C, the background listener processes survive (no `trap` to clean up).

**Fix:** Add `trap "kill $A_PID $B_PID" EXIT` after the spawns; consider building once with `cargo build --release` then invoking the binary directly to avoid repeated `cargo run` overhead.

---

### IN-04: `wait_for_disconnect`'s 1-byte read accepts a stray byte and returns immediately

**File:** `crates/famp/src/bus_client/mod.rs:152-160`
**Issue:** Comment at line 156-158 says "A nonzero read would mean the broker violated the request/reply invariant; we still surface it as 'disconnect'." The function ignores the read result — any byte arriving on the socket triggers a "disconnect" outcome and tears down the connection, even though the broker had no reason to send it. A future broker bug that emits an unsolicited frame would now manifest as constant register reconnect storms instead of as a visible protocol violation. Logging the unexpected byte (or its first frame's worth) before returning would help operators triage future protocol regressions.

**Fix:**
```rust
match self.stream.read(&mut probe).await {
    Ok(0) | Err(_) => {} // expected disconnect path
    Ok(n) => eprintln!("warning: broker sent {n} unsolicited byte(s); disconnecting"),
}
```

---

### IN-05: `validate_identity_name` in MCP register tool diverges from CLI's bash regex

**File:** `crates/famp/src/cli/mcp/tools/register.rs:107-126`; `scripts/famp-local:73-81`
**Issue:** The MCP tool validates `^[A-Za-z0-9._-]+$` (alpha, digit, dot, underscore, hyphen). The bash `validate_identity_name` validates the same character class but ALSO enforces a 64-byte length cap. The Rust validator does not enforce the length cap. Names longer than 64 bytes pass MCP validation but would be rejected later by `famp-core::identity::validate_name_or_instance_id`, surfacing as a confusing error far downstream from the input.

**Fix:** Add the length cap to the Rust validator:
```rust
if name.len() > 64 {
    return Err(ToolError::new(
        BusErrorKind::EnvelopeInvalid,
        format!("identity name length {} exceeds 64 bytes", name.len()),
    ));
}
```

---

### IN-06: `info` markdown / shell colors leak from the bash script

**File:** `scripts/famp-local:443-447, 877-883, 905-918`
**Issue:** Several `echo` calls use the unicode `✓` character. On terminals without UTF-8 (rare, but possible in CI logs scraped by tools that normalize output), this surfaces as `\xe2\x9c\x93` literal bytes in logs and breaks regex assertions in test fixtures. None of the current tests grep for `✓` so this is informational; flagged because the project conventions section in CLAUDE.md (global) discourages emoji unless explicitly requested.

**Fix:** Replace `✓` with `OK:` (or similar ASCII) for log-friendliness, or document that test fixtures should not depend on the prefix character.

---

_Reviewed: 2026-04-28T22:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

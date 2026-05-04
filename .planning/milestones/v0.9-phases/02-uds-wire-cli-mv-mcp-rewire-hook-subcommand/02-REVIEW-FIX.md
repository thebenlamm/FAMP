---
phase: 02-uds-wire-cli-mv-mcp-rewire-hook-subcommand
fixed_at: 2026-04-28T23:30:00Z
review_path: .planning/phases/02-uds-wire-cli-mv-mcp-rewire-hook-subcommand/02-REVIEW.md
iteration: 1
findings_in_scope: 16
fixed: 15
skipped: 1
status: partial
---

# Phase 02: Code Review Fix Report

**Fixed at:** 2026-04-28T23:30:00Z
**Source review:** `.planning/phases/02-uds-wire-cli-mv-mcp-rewire-hook-subcommand/02-REVIEW.md`
**Iteration:** 1

**Summary:**
- Findings in scope: 16 (5 blockers + 11 warnings)
- Fixed: 15
- Skipped: 1

Workspace test suite passing after every fix: 492 tests passed, 22 skipped (unchanged from pre-fix baseline of 491+22; +1 net is the BL-02 regression test added by this run).

## Fixed Issues

### BL-01: `register` reconnect backoff resets on every disconnect

**Files modified:** `crates/famp/src/cli/register.rs`
**Commit:** b7865c9
**Applied fix:** Removed the unconditional `delay = RECONNECT_INITIAL` reset in the `Disconnected` arm. Backoff now grows monotonically across consecutive broker bounces (1 → 2 → 4 → 8 → 16 → 30 s, capped). Backoff still resets to `RECONNECT_INITIAL` on each fresh `run` invocation via the loop's initialization.

### BL-02: `bind_exclusive` calls `std::process::exit(0)` from a non-`main` helper

**Files modified:** `crates/famp/src/cli/broker/mod.rs`
**Commit:** ea86bec
**Applied fix:** Replaced `std::process::exit(0)` with a typed `BindOutcome::{Bound, Existing}` enum returned from `bind_exclusive`. `run` now branches on `Existing` and returns `Ok(())` from itself, letting destructors run in scope. Added a regression test (`test_bind_exclusive_returns_existing_when_live_broker_present`) that previously could not be written because it would have nuked the test process.

### BL-03: Broker `clients` map grows unboundedly

**Files modified:** `crates/famp-bus/src/broker/handle.rs`
**Commit:** 3850c08
**Applied fix:** `disconnect` now removes the entry from `broker.state.clients` (both proxy and canonical-holder branches). Functions iterating the map (`canonical_holder_id`, `proxy_holder_alive`, `connected_names`, tick's liveness sweep) no longer grow O(N) with dead entries. `sessions.jsonl` retains the audit trail.

### BL-04: Asymmetric `#[serde(skip_serializing_if)]` without `default`

**Files modified:** `crates/famp-bus/src/proto.rs`
**Commit:** f420a65
**Applied fix:** Added `default` to every `skip_serializing_if = "Option::is_none"` field on `BusMessage::{Inbox, Await}` and `BusReply::WhoamiOk`. The wire-protocol convention (`default + skip_serializing_if`) is now uniform across both enums. `Hello.bind_as` already used both attributes; this normalises the rest.

### BL-05: `is_alive(0)` returns true; PID 0 valid in Register

**Files modified:** `crates/famp/src/cli/broker/mailbox_env.rs`, `crates/famp-bus/src/broker/handle.rs`
**Commit:** 1832691
**Applied fix:** Two-layer defense:
1. `DiskMailboxEnv::is_alive` rejects `pid == 0` and any non-positive `i32` cast result up front (before passing to `nix::kill`).
2. The `Register` handler rejects `pid == 0` at the protocol layer with `EnvelopeInvalid`, so the name is never even bound to PID 0.

### WR-01: `resolve_sock_path` doc comment claims it panics

**Files modified:** `crates/famp/src/bus_client/mod.rs`
**Commit:** 9535c04
**Applied fix:** Replaced the `# Panics` doc section with `# Behavior` describing the actual fallback path (`/nonexistent-famp-home/.famp/bus.sock`).

### WR-02: `wires.tsv` row matching has dead second clause

**Files modified:** `crates/famp/src/cli/identity.rs`
**Commit:** cff68e5
**Applied fix:** Captured both canonical and raw cwd. The fallback now compares `Path::new(dir_str) == cwd_raw` (matches a verbatim-text row when the cwd cannot be canonicalized). The previous form (`Path::new(dir_str) == cwd_canon`) was dead.

### WR-03: `normalize_channel` recompiles regex on every call

**Files modified:** `crates/famp/src/cli/util.rs`
**Commit:** 599c103
**Applied fix:** Replaced per-call `Regex::new(...)` with `LazyLock<Regex>` so the regex compiles exactly once for the process lifetime (mirrors the bus-side pattern in `famp_bus::proto`).

### WR-04: `ensure_bus` TOCTOU drops a freshly-connected client

**Files modified:** `crates/famp/src/cli/mcp/session.rs`
**Commit:** f828ead
**Applied fix:** Hold the session-state guard across the `BusClient::connect` await. Per the module comment, stdio MCP serializes tool calls so contention is structurally bounded. Eliminates the leak of broker accept + handshake + ClientState entry on the losing race.

### WR-05: `await_envelope` accepts `timeout_ms = u64::MAX`, panicking the broker

**Files modified:** `crates/famp-bus/src/broker/handle.rs`
**Commit:** 4acc06a
**Applied fix:** Cap `timeout_ms` at 1 hour (`MAX_AWAIT_MS = 60 * 60 * 1000`) before constructing the `Duration`. Prevents `Instant + Duration` overflow panics that would crash the broker actor and tear down every connected client.

### WR-07: `sessions_log` writes `joined: Vec::new()` regardless of actual joined channels

**Files modified:** `crates/famp-bus/src/broker/mod.rs`, `crates/famp-bus/src/broker/handle.rs`, `crates/famp/src/cli/broker/mod.rs`
**Commit:** 613148d
**Applied fix:** Reviewer's Option 2. Added `Out::SessionEnded { name, pid, joined }` variant; `disconnect` snapshots the joined set BEFORE clearing state and emits this variant for canonical-holder disconnects only (proxies never wrote to sessions.jsonl). Executor's exhaustive match handles the new variant by writing the SessionRow with the broker's pre-disconnect snapshot. Removed the now-orphan `session_meta` mirror from the executor.

### WR-08: `tick` discards `Out` vec from `disconnect` calls

**Files modified:** `crates/famp-bus/src/broker/handle.rs`
**Commit:** 4882179
**Applied fix:** Threaded the per-dead-client `disconnect` `Vec<Out>` through tick's return value (`out.extend(disconnect(broker, client))`). `Out::ReleaseClient` and `Out::SessionEnded` for liveness-discovered dead clients now reach the executor.

### WR-09: `send_agent` returns `task_id: Uuid::nil()` for DMs

**Files modified:** `crates/famp-bus/src/broker/handle.rs`
**Commit:** 3184094
**Applied fix:** Extract `task_id` via `task_id_from(&envelope)` in `send_agent` (matches `send_channel`'s behaviour) and plumb it through `send_ok`, which now takes `task_id: Uuid` as a parameter.

### WR-10: `hello()` overwrites existing `ClientState` unconditionally

**Files modified:** `crates/famp-bus/src/broker/handle.rs`
**Commit:** e7ca8c0
**Applied fix:** Added a guard at the `handle_wire` gate that rejects any Hello frame on a connection where `clients.get(client).map(|c| c.handshaked) == Some(true)`. Returns `BrokerProtoMismatch: "Hello already received on this connection"`. This closes both WR-10 and WR-11 with a single check.

### WR-11: `handle_wire` allows mid-connection `bind_as` rotation via repeated Hello

**Files modified:** `crates/famp-bus/src/broker/handle.rs`
**Commit:** e7ca8c0 (same commit as WR-10)
**Applied fix:** Same guard as WR-10. A handshaked connection cannot send a second Hello, so `bind_as` rotation mid-connection is impossible.

## Skipped Issues

### WR-06: Test env-var mutations race with each other (parallel test runner)

**File:** `crates/famp/src/cli/identity.rs:111-207`; `crates/famp/src/bus_client/mod.rs:202-214`; `crates/famp/tests/mcp_register_whoami.rs:24-28`
**Reason:** Skipped — the surgical fix requires either (a) adding a new dev-dependency (`temp-env` or `serial_test`) to the workspace, or (b) substantial refactor across 5+ test files, or (c) marking tests `#[ignore]` (which would weaken the safety net the project relies on).

The reviewer themselves notes this is "currently safe" because CI uses `cargo nextest` (per Justfile:13), which forks each test into its own process. The risk surfaces only for a future contributor running `cargo test -p famp`. Adding a new dev-dependency is the kind of non-surgical change CLAUDE.md asks me to defer.

**Original issue:** Multiple tests mutate process-global env vars (`FAMP_LOCAL_IDENTITY`, `HOME`, `FAMP_BUS_SOCKET`) via `std::env::{set_var, remove_var}`. Save/restore dance leaks state on test panic. Also: `set_var`/`remove_var` are `unsafe fn` in Rust 2024 edition; this code will fail to compile when the workspace upgrades to Edition 2024.

**Recommended follow-up:** Add `temp-env` to the workspace dev-deps and migrate the affected tests in a dedicated PR. The Edition 2024 angle adds urgency before the next toolchain bump.

## Out-of-Scope Findings (info, by design)

The following six info-level findings were not addressed per the fix scope (`critical_warning`):

- IN-01: Empty closures and underscore matches in `accept.rs` swallow errors silently
- IN-02: `tail_loop` selects on a sleep that's outside the select branch
- IN-03: `e2e-smoke` recipe in Justfile uses `cargo run` per command — slow, racy
- IN-04: `wait_for_disconnect`'s 1-byte read accepts a stray byte and returns immediately
- IN-05: `validate_identity_name` in MCP register tool diverges from CLI's bash regex (no 64-byte length cap)
- IN-06: Bash script uses `✓` unicode character

These remain documented in 02-REVIEW.md for follow-up phases.

---

_Fixed: 2026-04-28T23:30:00Z_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_

---
phase: 02-uds-wire-cli-mv-mcp-rewire-hook-subcommand
plan: 11
subsystem: broker-integration-tests
tags: [tests, broker, d-10, kill-9, idle-exit, spawn-race, nfs, sessions]
requires:
  - 02-02 (BROKER-01 closure + run_on_listener test entry point)
  - 02-04 (famp send / BusClient::Send wire path)
  - Phase-1 D-09 (AnyBusEnvelope drain decode)
provides:
  - TEST-03 GREEN (kill-9 recovery)
  - TEST-04 GREEN (spawn race exclusion)
  - BROKER-04 promoted to integration (5-min idle exit)
  - BROKER-05 promoted to integration (NFS detector public API)
  - CLI-11 GREEN (sessions.jsonl diagnostic-only)
  - D-10 proxy invariants integration coverage (3 tests)
affects:
  - crates/famp/src/cli/broker/mod.rs (bind_exclusive EEXIST handling)
  - crates/famp/src/bus_client/mod.rs (BusClient::wait_for_disconnect)
  - crates/famp/src/cli/register.rs (block_until_disconnect now races shutdown_signal vs wait_for_disconnect)
tech-stack:
  added: []
  patterns:
    - "tokio::test(start_paused) + tokio::time::advance for deterministic 5-min timer"
    - "Direct BusClient + audit_log envelope to bypass plan-02-12 envelope-shape gap"
    - "Subprocess broker spawn (Command::cargo_bin) when integration test binary cannot use spawn_broker_if_absent (current_exe = test binary, not famp)"
key-files:
  created:
    - crates/famp/tests/broker_proxy_semantics.rs
  modified:
    - crates/famp/tests/broker_lifecycle.rs
    - crates/famp/tests/broker_spawn_race.rs
    - crates/famp/tests/broker_crash_recovery.rs
    - crates/famp/src/cli/broker/mod.rs
    - crates/famp/src/bus_client/mod.rs
    - crates/famp/src/cli/register.rs
decisions:
  - "Adapt TEST-03 to push a hand-crafted audit_log envelope via BusClient (not via `famp send`) because Phase-2 send emits a {mode,summary,...} value that does not survive AnyBusEnvelope::decode at drain time. Plan 02-12 reconciles the contract."
  - "Task 3 D-10 proxy tests are wire-level (BusClient + BusMessage) because canonical `famp join` / `famp sessions` / `famp whoami` subcommands are owned by plan 02-07 (parallel wave) and not on this base."
  - "Broker spawn fix MUST handle EEXIST in addition to EADDRINUSE — macOS kernel returns EEXIST when bind() races a stale socket file."
  - "Register block_until_disconnect MUST monitor the wire — without it, broker death goes undetected and the reconnect-with-backoff loop never fires (TEST-03 cannot pass)."
metrics:
  duration: 75min
  completed: 2026-04-28
---

# Phase 02 Plan 11: Broker Integration Tests + D-10 Proxy Semantics Summary

Filled the Wave-0 broker test stubs into a real integration battery proving the load-bearing crash-safety, concurrency, and D-10 proxy invariants for Phase 02. Added `broker_proxy_semantics.rs` with three D-10 tests at the wire+OS level. Auto-fixed two pre-existing blockers discovered during TEST-03: macOS bind EEXIST handling and the missing `block_until_disconnect` wire monitor.

## What Shipped

### Task 1: broker_lifecycle.rs (BROKER-04, CLI-11, BROKER-05)

- `test_broker_idle_exit` — `#[tokio::test(start_paused = true)]` + `tokio::time::advance(301s)` deterministically advances past the 300s idle threshold; broker task completes within the test timeout and unlinks the socket file. No real wall-clock sleep.
- `test_sessions_jsonl_diagnostic_only` — pre-writes a ghost row (`pid: 99999999`) to `sessions.jsonl`, registers a real client, queries `Sessions {}`. Asserts `ghost` is absent from the reply rows: the broker MUST NOT consult `sessions.jsonl` (CLI-11).
- `test_nfs_warning` — public-API gate: `famp::cli::broker::nfs_check::is_nfs(tempdir())` returns `false`. Real-NFS verification deferred to `02-VALIDATION.md "Manual-Only Verifications"`.
- All `#[ignore]` stubs removed.

### Task 2: broker_spawn_race.rs (TEST-04) + broker_crash_recovery.rs (TEST-03)

- `test_broker_spawn_race` — two parallel `Command::cargo_bin("famp") register <name>` invocations against a shared `FAMP_BUS_SOCKET`; after 2s, exactly one broker is bound. The loser's `bind_exclusive` probe finds the winner's broker and `process::exit(0)` defers to it (BROKER-03).
- `test_kill9_recovery` — register alice + bob, push a valid `audit_log` envelope into bob's mailbox via the D-10 proxy shape (`bind_as: Some("alice")`), find the broker pid via `pgrep -f 'famp broker --socket <path>'`, SIGKILL, wait 10s, verify the message survives via a fresh `bind_as: Some("bob")` proxy + `BusMessage::Inbox`.

### Task 3: broker_proxy_semantics.rs (D-10, NEW)

- `test_proxy_join_persists_after_disconnect` — proxy joins `#planning`, disconnects; alice still in `#planning` per a fresh `Whoami` (canonical-holder mutation, NOT proxy mutation).
- `test_proxy_inbox_unregistered_fails` — `BusClient::connect(sock, Some("alice"))` against a bare broker (no register alice) returns `BusClientError::HelloFailed{kind: NotRegistered}`.
- `test_proxy_send_after_holder_dies` — register alice, SIGKILL alice's register process, wait 3s (≥ 2 Tick intervals), proxy-connect bind_as=alice; rejected with `NotRegistered` (Hello-time validation; per-op liveness re-check is a fallback).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] EEXIST handling in `bind_exclusive`**
- **Found during:** Task 2 manual reproduction of TEST-03
- **Issue:** After `kill -9`, the new broker's `tokio::net::UnixListener::bind()` failed with `File exists (os error 17)` — macOS returns `EEXIST` (17) instead of `EADDRINUSE` (48) when binding over a stale socket file. The original `bind_exclusive` only matched `EADDRINUSE`, so the recovery path silently aborted with `CliError::Io`, the register reconnect loop kept failing, and TEST-03 deadlocked.
- **Fix:** Match `EADDRINUSE OR EEXIST` and run the same probe-then-unlink-then-retry algorithm.
- **Files modified:** `crates/famp/src/cli/broker/mod.rs`
- **Commit:** `0b48074`

**2. [Rule 3 - Blocking] `block_until_disconnect` did not monitor the wire**
- **Found during:** Task 2 manual reproduction (`alice.err` and `bob.err` showed only the initial `registered as ...` line and never the disconnect/reconnect log).
- **Issue:** `register::block_until_disconnect` was implemented as `tokio::select! { shutdown_signal() => SignalCaught, pending::<()>() => unreachable }`. It deliberately did NOT poll the wire (the comment justified this as "Phase-1 broker is request/reply"). But this means broker death is invisible to the holder process — it only surfaces on the next `send_recv` round-trip, which a default `register` (no `--tail`) NEVER initiates. TEST-03 cannot pass without fixing this.
- **Fix:** New `BusClient::wait_for_disconnect()` does a 1-byte `AsyncReadExt::read` on the underlying `UnixStream`. Phase-1 broker contract forbids unsolicited frames, so any readable event = peer-side close (EOF or error). `block_until_disconnect` now races `shutdown_signal()` vs `client.wait_for_disconnect()`; broker death surfaces as `SessionOutcome::Disconnected` and the outer reconnect-with-backoff loop fires.
- **Files modified:** `crates/famp/src/bus_client/mod.rs`, `crates/famp/src/cli/register.rs`
- **Commit:** `0b48074`

### Plan-vs-Implementation Adaptations

**3. [Rule 4 → Adapted] TEST-03 envelope shape**
- **Plan said:** alice → bob "hello" via `famp send --as alice --to bob --new-task hello`, then verify bob's inbox contains "hello".
- **Reality:** Phase-2 `famp send` emits `{mode: "new_task", summary: "...", ...}` JSON. The Phase-1 broker drain path (Inbox / Register / Join) calls `AnyBusEnvelope::decode`, which requires `class: "request" | "commit" | "deliver" | ...` (D-09). The send-shape and decode-shape do not yet agree — plan 02-12 territory.
- **Adaptation:** Push a valid `audit_log` envelope (mirroring `famp_envelope::bus::tests::audit_log_value`) via BusClient + `BusMessage::Send`. This isolates TEST-03's actual claim (durability across kill-9) from the unfinished 02-12 contract. The fix to `bind_exclusive` and `wait_for_disconnect` is applied regardless and unblocks 02-12.

**4. [Rule 4 → Adapted] Task 3 used wire-level not CLI-level**
- **Plan said:** Use `famp join --as alice #planning`, `famp sessions`, `famp send --as alice --to bob`.
- **Reality:** `famp join` / `famp sessions` / `famp whoami` are introduced by plan 02-07 (parallel Wave-3/4) and are not on this base.
- **Adaptation:** Tests use `BusClient::connect(sock, Some("alice".into()))` + `BusMessage::Join` / `Whoami` / `Send` directly. The invariants under test live on the broker, not the CLI surface; wire-level coverage is faithful to the D-10 contract and exercises the same broker code path the CLI subcommands will hit.

**5. [Rule 4 → Adapted] Test 2 spawns a broker subprocess directly**
- **Issue:** Tests 1 and 3 work because `spawn_register` shells out to `cargo_bin("famp")`, and the famp child uses its own `current_exe()` to spawn the broker (which IS famp). But test 2 has no register process; the test binary itself calls `BusClient::connect`, which calls `spawn_broker_if_absent`, which calls `std::env::current_exe()` — that returns the *test binary*, which has no `broker` subcommand. Result: `BrokerDidNotStart`.
- **Adaptation:** Test 2 explicitly spawns `cargo_bin("famp") broker --socket <path>` and polls until the socket is up before calling `BusClient::connect`.

## pgrep Reliability

`pgrep -f "famp broker --socket <path>"` reliably finds exactly one broker pid on the macOS dev box used during this plan (Darwin 25.3.0). The broker's argv is set by `spawn::spawn_broker_if_absent` as `["broker", "--socket", "<path>"]`, so the pattern match is unambiguous when the socket path is a unique tempdir entry. No flakiness mitigations were necessary; if Linux CI flakes, fall back to `pgrep -af` and parse the full argv.

## Final Test Inventory

### broker-lifecycle (4 tests)

| Test | Requirement | Evidence |
|---|---|---|
| `test_broker_accepts_connection` | BROKER-01 (Hello handshake) | unchanged from plan 02-02 |
| `test_broker_idle_exit` | BROKER-04 (5-min idle) | `start_paused = true` + `advance(301s)` |
| `test_sessions_jsonl_diagnostic_only` | CLI-11 (sessions diagnostic) | ghost-row absence from runtime view |
| `test_nfs_warning` | BROKER-05 unit-level | public `is_nfs(tempdir)` = false |

### broker-spawn-race (1 test)

| Test | Requirement | Evidence |
|---|---|---|
| `test_broker_spawn_race` | TEST-04 (single-broker exclusion) | two parallel registers, exactly one broker bound after 2s |

### broker-crash-recovery (1 test)

| Test | Requirement | Evidence |
|---|---|---|
| `test_kill9_recovery` | TEST-03 (kill-9 durability) | audit_log envelope persists across SIGKILL+respawn |

### broker-proxy-semantics (3 tests, NEW)

| Test | Invariant | Evidence |
|---|---|---|
| `test_proxy_join_persists_after_disconnect` | Proxy mutation hits canonical holder, not proxy | Whoami after proxy disconnect shows alice still in #planning |
| `test_proxy_inbox_unregistered_fails` | Hello rejects proxy when no live holder | `HelloErr{NotRegistered}` |
| `test_proxy_send_after_holder_dies` | Tick + per-op liveness catches dead holder | post-SIGKILL proxy connect rejected |

## Commits

| Hash | Subject |
|---|---|
| `c654eb1` | test(02-11): fill broker_lifecycle integration tests |
| `0b48074` | test(02-11): TEST-04 spawn-race + TEST-03 kill-9 recovery |
| `95afa86` | test(02-11): D-10 proxy semantics integration suite |

## Verification

- `cargo test -p famp --test broker_lifecycle` → 4 passed.
- `cargo test -p famp --test broker_spawn_race` → 1 passed.
- `cargo test -p famp --test broker_crash_recovery` → 1 passed (12s wall).
- `cargo test -p famp --test broker_proxy_semantics` → 3 passed.
- `cargo clippy -p famp --tests -- -D warnings` → clean.
- `cargo clippy -p famp --lib -- -D warnings` → clean.

## Self-Check: PASSED

- [x] crates/famp/tests/broker_lifecycle.rs — present, 4 tests, all `#[ignore]` removed.
- [x] crates/famp/tests/broker_spawn_race.rs — present, 1 test.
- [x] crates/famp/tests/broker_crash_recovery.rs — present, 1 test.
- [x] crates/famp/tests/broker_proxy_semantics.rs — present (NEW), 3 tests.
- [x] Commit `c654eb1` exists in git log.
- [x] Commit `0b48074` exists in git log.
- [x] Commit `95afa86` exists in git log.

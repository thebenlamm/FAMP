---
phase: 03-load-verification-integration-hardening
status: clean
depth: standard
files_reviewed: 2
findings:
  critical: 0
  warning: 0
  info: 2
  total: 2
reviewed: 2026-05-11
---

# Phase 03 Code Review (Gap Closure Refresh — GAP-03-01)

This review supersedes the prior `03-REVIEW.md` (which carried `WR-001:
Load test does not prove tight-loop saturated inspect RPC pressure`).
The 03-03 gap-closure plan resolves that warning by (a) moving inspect
dispatch off the broker's main `execute_outs` loop and bounding it via
a semaphore, and (b) replacing the paced CLI-subprocess inspector
workers with saturated direct `famp_inspect_client::connect_and_call`
workers. Both changes have been re-reviewed against the focus areas
listed in the review brief.

## Scope

- `crates/famp/src/cli/broker/mod.rs` (non-blocking bounded inspect
  dispatch — `MAX_CONCURRENT_INSPECT_REQUESTS`, semaphore, spawn'd
  dispatch, fast-shed budget-exceeded reply)
- `crates/famp/tests/inspect_load_test.rs` (saturated direct inspect
  RPC load workers, `STARVATION_THRESHOLD = 0.80` preserved)

Out of scope (per review brief): `.planning/ROADMAP.md`,
`.planning/STATE.md`, `03-03-SUMMARY.md`.

Cross-referenced for correctness:

- `crates/famp-inspect-server/src/lib.rs` — `dispatch(&BrokerStateView,
  &BrokerCtx, &InspectKind) -> serde_json::Value` is `&` everywhere
  (INSP-RPC-02 read-only discipline upheld).
- `crates/famp-bus/src/broker/{mod,state}.rs` — `broker.view()` and
  `broker.cursor_offset()` both take `&self`; `BrokerStateView` is
  `Clone + Send` (owned `Vec` + `SystemTime`).
- `crates/famp-inspect-client/src/lib.rs` — `connect_and_call` is the
  expected one-round-trip surface.

## Findings

### IN-001: spawn_blocking task is not cancelled on 500ms timeout (documented trade-off)

**Severity:** Info
**File:** `crates/famp/src/cli/broker/mod.rs:426-454`
**Category:** Resource lifetime / blocking-pool occupancy

When `tokio::time::timeout(500ms, spawn_blocking(...))` fires, the
outer `tokio::spawn`'d task replies with `BudgetExceeded` and drops
`_permit`, freeing the inspect concurrency slot. The `spawn_blocking`
worker on the blocking pool, however, keeps running to completion of
the taskdir walk + JSONL pre-read + dispatch (tokio cannot cancel
blocking-pool work). Under saturated `InspectKind::Tasks` flood, this
means the broker can in principle have N>1 blocking-pool workers
simultaneously executing inspect FS work even though
`MAX_CONCURRENT_INSPECT_REQUESTS = 1` — the cap bounds dispatch, not
blocking-pool occupancy.

The committed evidence (loaded/baseline >= 0.80 across multiple runs)
shows this trade-off is acceptable for the v0.10 commitment, and the
in-source comment at lines 449-450 acknowledges it ("The blocking
thread may continue briefly..."). Recording as Info so the next
reviewer who touches the budget timeout knows the cap is on dispatch
admission, not on concurrent blocking work.

**Recommendation:** None for this phase. If a future regression shows
blocking-pool starvation, options include (a) `tokio::Runtime` with a
dedicated `max_blocking_threads` for inspect, or (b) a cooperative
cancel token threaded into the blocking walk. Neither is needed today.

### IN-002: `Bus` test struct holds `tmp` by-value, relying on Drop ordering for cleanup

**Severity:** Info
**File:** `crates/famp/tests/inspect_load_test.rs:29-39`
**Category:** Test resource discipline

`Bus { tmp: tempfile::TempDir, sock: PathBuf }` keeps the `TempDir`
alive for the lifetime of the `Bus`. The test takes
`bus.tmp.path()` repeatedly to build child paths; `Bus` is owned by
`measure_scenario` and dropped at function return, after which
`TempDir::drop` recursively removes the directory. This is fine
because `kill_and_wait(&mut broker)` runs first and the broker has
already `remove_file(sock_path)`'d on shutdown signal.

However, if a panic occurs between broker spawn and `kill_and_wait`
(e.g. a `wait_for_register` panic during a flaky test environment),
the broker child process is leaked: the panic unwinds through `Bus`
drop, the TempDir is removed, but the `famp broker` child still holds
the (now-deleted) socket inode and is not killed. This does not
affect the gap closure or the v0.10 commitment, but it is a known
pattern smell in long-running integration tests.

**Recommendation:** None for this phase. If a future failure mode
reveals orphaned broker subprocesses from a panicked test run, wrap
the children in a `Drop` guard (e.g. `KillOnDrop(Child)`) and store
them in `Bus`.

## Focus-Area Verification

| Focus area | Evidence | Verdict |
|---|---|---|
| `try_acquire_owned` correctness | `Arc::clone(inspect_semaphore).try_acquire_owned()` at line 385; `try_acquire_owned` never awaits, returns `Err` immediately on cap, returns `OwnedSemaphorePermit` on Ok. | PASS |
| Reply-sender lifetime safety | `reply_tx` cloned from `reply_senders` map BEFORE both `try_acquire_owned` and `tokio::spawn` (line 375). Spawned task captures `reply_tx` by move; it never touches the broker's reply_senders map. No cross-client misrouting possible. | PASS |
| 500ms budget preserved | `tokio::time::timeout(Duration::from_millis(500), spawn_blocking(...))` at lines 426-438. `BudgetExceeded` reply delivered on `Err(_elapsed)` (timeout) AND on `try_acquire_owned` `Err` (permit exhaustion, line 394-397) AND on `Ok(Err(join_err))` (panic in blocking thread, line 446). | PASS |
| Memory safety: snapshot before spawn | `state_snapshot = broker.view()` (owned `BrokerStateView`, Clone+Send) and `cursor_offsets: BTreeMap<String, u64>` captured at lines 404-412 BEFORE `tokio::spawn`. No `&Broker` or `&mut self` borrow held across the spawn. | PASS |
| `kind` ownership for spawn | `let kind_for_blocking = kind.clone()` at line 415 then moved into the `spawn_blocking` closure (line 428). Used by `&` inside the blocking closure for both `build_inspect_ctx_blocking` and `dispatch`. Original `kind` from the `Out::InspectRequest { client, kind }` arm is consumed (cloned) and dropped at end of scope. | PASS |
| `STARVATION_THRESHOLD = 0.80` locked | Line 138: `const STARVATION_THRESHOLD: f64 = 0.80;` with comment "Locked public commitment — do not relax". Used at the only assertion in line 264. | PASS |
| No `Duration::from_millis(1500)` in inspector loop | `grep` of `inspect_load_test.rs` returns no `from_millis(1500)`; inspector loop body (lines 197-216) is a tight tokio `block_on` over `connect_and_call` calls until `Instant::now() >= deadline`. | PASS |
| Baseline + loaded scenarios on fresh bus | `measure_scenario(0)` and `measure_scenario(INSPECTOR_THREADS)` each call `Bus::new()` → fresh `TempDir`, fresh socket, fresh broker subprocess (`famp_spawn_broker`). | PASS |
| Read-only discipline (INSP-RPC-02) | `famp_inspect_server::dispatch` signature is `(&BrokerStateView, &BrokerCtx, &InspectKind) -> serde_json::Value`. `BrokerStateView` is a `pub` snapshot type produced by `&BrokerState -> Self`; the server crate cannot upgrade `&BrokerStateView` to `&mut BrokerState`. | PASS |
| Lint / unwrap discipline (production paths) | The new broker code uses `if let Some / match / Ok(p) / Err(_)`; no `.unwrap()`/`.expect()` in non-test paths in the touched arm. Test module is `#[cfg(test)] #[allow(clippy::unwrap_used, clippy::expect_used)]`. | PASS |
| Spawned budget-exceeded reply on cap-exhaustion | Lines 394-397: `tokio::spawn(async move { let _ = reply_tx.send(BusReply::InspectOk { payload: inspect_budget_exceeded_payload(0) }).await; })`. Main `execute_outs` loop is NOT blocked on a possibly-full per-client reply channel even when the inspect flood saturates the cap. | PASS |
| Error swallowing | The `let _ = reply_tx.send(...).await` pattern in both the fast-shed and dispatch-completion paths drops the send result. If the client has disconnected by the time the reply lands, the channel is closed and the reply is silently dropped — correct behavior; logging here would spam under saturated flood-then-disconnect patterns. The panicked-blocking-thread arm DOES log (`eprintln!("inspect spawn_blocking panicked: {join_err}")`, line 446). No silent swallowing of unexpected conditions. | PASS |

## Prior Review Reconciliation

| Prior finding | Status |
|---|---|
| **WR-001** ("Load test does not prove tight-loop saturated inspect RPC pressure") | **RESOLVED.** The inspector workers in `inspect_load_test.rs` now drive saturated direct `connect_and_call(InspectKind::Tasks(_))` calls with no per-iteration sleep, exercising the broker's new non-blocking bounded inspect dispatch path. The same property that previously measured `ratio=0.17` now measures `>= 0.80` per the recorded evidence in `03-03-SUMMARY.md`. |

No prior findings have persisted; no prior findings have regressed.

## Summary

The 03-03 gap-closure code is **clean** with respect to correctness,
security, and read-only discipline:

- The semaphore acquire path is sound (`try_acquire_owned` on a cloned
  `Arc`, fast-shed reply spawned, permit bound to spawned task body).
- Reply senders are cloned from the broker's per-client map BEFORE
  any spawn, so cross-client misrouting is structurally impossible.
- The 500ms budget continues to wrap the entire walk + dispatch, with
  budget-exceeded payloads delivered on timeout, on permit exhaustion,
  AND on a panicked blocking thread.
- `BrokerStateView` (owned, `Clone + Send`) is captured before
  `tokio::spawn`; no `&Broker` or `&mut self` borrow crosses the spawn
  boundary.
- INSP-RPC-02 read-only discipline is preserved end-to-end: `dispatch`
  takes `&BrokerStateView`, and the server crate has no path to
  upgrade it to mutable broker state.
- The load test locks `STARVATION_THRESHOLD = 0.80` and contains no
  pacing sleeps in the inspector loop. Each scenario runs against a
  fresh `Bus`/broker.

The two Info findings (blocking-pool occupancy under flood;
`KillOnDrop` for broker subprocesses on test panics) are forward-looking
notes about trade-offs that are explicitly acknowledged in the source
or that affect only flaky-environment failure modes — neither blocks
this gap closure.

The prior `WR-001` warning is resolved by evidence: saturated direct
inspect RPC pressure no longer starves bus send throughput.

---

_Reviewed: 2026-05-11_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_

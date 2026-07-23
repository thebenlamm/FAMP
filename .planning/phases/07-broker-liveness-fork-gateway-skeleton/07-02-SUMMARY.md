---
phase: 07-broker-liveness-fork-gateway-skeleton
plan: 02
subsystem: testing
tags: [famp-bus, broker, liveness, register, tick, fakeliveness, pure-unit-test]

requires:
  - phase: 07-broker-liveness-fork-gateway-skeleton
    provides: 07-RESEARCH.md's confirmed no-pid-uniqueness fact at handle.rs:358 and the tick() sweep at handle.rs:973-991
provides:
  - "Deterministic pure-broker proof that register() imposes no pid-uniqueness constraint (LIVE-01)"
  - "Deterministic pure-broker proof that N clients sharing one pid reap together on the same Tick once that pid dies (LIVE-02 mechanism)"
affects: [07-01-gateway-crate-scaffold, 07-03-gateway-registry-and-tests]

tech-stack:
  added: []
  patterns:
    - "Pure-broker unit test via Broker<E> + FakeLiveness (no subprocess, no sleep, deterministic Tick)"

key-files:
  created: []
  modified:
    - crates/famp-bus/src/broker/handle/tests.rs

key-decisions:
  - "Single combined test (not split into two) covers survive-then-reap in one deterministic sequence, matching the plan's suggested name and falsification order"
  - "Asserted via both broker.state.clients membership and Out::ReleaseClient events, matching the file's existing assertion style"

patterns-established:
  - "Shared-pid multi-principal liveness test pattern: register N names with an identical pid, drive Tick while alive (assert survival), mark_dead the shared pid, drive Tick again (assert all released together)"

requirements-completed: [LIVE-01]

coverage:
  - id: D1
    description: "Two-or-more broker clients registered with the SAME pid all remain in broker.state.clients across a Tick sweep while that pid is alive, and both registrations succeed (no pid-uniqueness constraint)"
    requirement: "LIVE-01"
    verification:
      - kind: unit
        ref: "crates/famp-bus/src/broker/handle/tests.rs#live01_shared_pid_clients_survive_sweep_and_reap_together"
        status: pass
    human_judgment: false
  - id: D2
    description: "When the shared pid is marked dead, the same sweep reaps ALL of those clients on the next Tick — no orphan holders"
    requirement: "LIVE-01"
    verification:
      - kind: unit
        ref: "crates/famp-bus/src/broker/handle/tests.rs#live01_shared_pid_clients_survive_sweep_and_reap_together"
        status: pass
    human_judgment: false

duration: 15min
completed: 2026-07-23
status: complete
---

# Phase 7 Plan 02: LIVE-01 Pure-Broker Shared-PID Test Summary

**Added `live01_shared_pid_clients_survive_sweep_and_reap_together` to `handle/tests.rs`, deterministically proving `register()` has no pid-uniqueness constraint — N clients sharing one PID all survive a live Tick and all reap together the instant that PID is marked dead.**

## Performance

- **Duration:** ~15 min
- **Completed:** 2026-07-23T19:16Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Pinned the load-bearing Design A fact (`register()` sets `state.pid = Some(pid)` unconditionally, no uniqueness check) with a fast, deterministic, subprocess-free test
- Test asserts registration order correctly: both `alice` and `bob` register successfully with the same pid FIRST (falsifying a hypothetical pid-uniqueness guard), THEN both survive a live-PID `Tick`, THEN both reap together on a dead-PID `Tick`
- Full `famp-bus --lib` suite stayed green (78 passed, up from 77) with zero production-code changes

## Task Commits

Each task was committed atomically:

1. **Task 1: Pure-broker LIVE-01 test — N clients, one PID, survive-then-reap** - `d6e25bb` (test)

**Plan metadata:** committed separately (see below)

## Files Created/Modified
- `crates/famp-bus/src/broker/handle/tests.rs` - Added `live01_shared_pid_clients_survive_sweep_and_reap_together`, a pure `Broker<TestEnv>` + `FakeLiveness` test using the file's existing `hello_canonical`/`register` helpers

## Decisions Made
- Wrote one combined test rather than splitting survive/reap into two tests — the plan allowed either shape ("or split into two focused tests if clearer"); one test with a clear internal narrative (register both, tick-alive, tick-dead) was more readable and matched the file's existing multi-assertion test style (e.g. `test_dead_proxy_does_not_wake`).
- Used a local `const SHARED_PID: u32 = 4242` moved above all statements to satisfy `clippy::items_after_statements` (part of the workspace's pedantic lint set enforced via `#[lints] workspace = true`).

## Deviations from Plan

**1. [Rule 1 - Bug] Reordered a local `const` declaration to satisfy clippy pedantic**
- **Found during:** Task 1, first `just lint` run
- **Issue:** `const SHARED_PID: u32 = 4242;` was declared after the first `let` statements in the test body, tripping `clippy::items_after_statements` (denied via `-D warnings` under the workspace's pedantic lint set)
- **Fix:** Moved the `const` to the top of the function body, before any `let` bindings
- **Files modified:** crates/famp-bus/src/broker/handle/tests.rs (same commit, pre-commit fix)
- **Verification:** `just lint` exits 0 after the reorder
- **Committed in:** `d6e25bb` (part of the task commit — fixed before committing)

---

**Total deviations:** 1 auto-fixed (1 Rule 1 lint fix)
**Impact on plan:** Cosmetic lint-compliance fix only; no change to test semantics or assertions. No scope creep.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- LIVE-01's pure-broker proof is in place and green, independent of the `famp-gateway` crate existing (per this plan's `depends_on: []`)
- Plans 07-01 (gateway crate scaffold) and 07-03 (gateway registry + LIVE-02/GW-04 integration tests) can now cite this test as the fast unit-level confirmation that the Register-with-gateway-PID mechanism (Design A) is sound at the broker layer
- No blockers

---
*Phase: 07-broker-liveness-fork-gateway-skeleton*
*Completed: 2026-07-23*

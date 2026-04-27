---
phase: 01-famp-bus-library-and-audit-log
plan: 02
subsystem: broker
tags: [rust, famp-bus, pure-actor, proptest, no-tokio]

requires:
  - phase: 01-famp-bus-library-and-audit-log
    provides: famp-bus protocol primitives, codec, mailbox/liveness test env, RED broker scaffolds
provides:
  - Pure `Broker::handle(BrokerInput, Instant) -> Vec<Out>` actor
  - Ordered broker side-effect intents for replies, mailbox appends, cursor advances, awaits, and disconnects
  - TDD-02/03/04 GREEN broker behavior tests
  - PROP-01..05 GREEN proptest coverage against temporary `Vec<serde_json::Value>` drained shape
affects: [phase-01-plan-03, phase-02-uds-wire, famp-bus]

tech-stack:
  added: []
  patterns: [pure actor with ordered Out intents, shared in-memory mailbox side-effect application in tests, proptest closed-enum strategies]

key-files:
  created:
    - crates/famp-bus/src/broker/mod.rs
    - crates/famp-bus/src/broker/handle.rs
    - crates/famp-bus/src/broker/state.rs
    - crates/famp-bus/tests/prop01_dm_fanin_order.rs
    - crates/famp-bus/tests/prop02_channel_fanout.rs
    - crates/famp-bus/tests/prop03_join_leave_idempotent.rs
    - crates/famp-bus/tests/prop04_drain_completeness.rs
    - crates/famp-bus/tests/prop05_pid_unique.rs
  modified:
    - crates/famp-bus/src/lib.rs
    - crates/famp-bus/tests/tdd02_drain_cursor_order.rs
    - crates/famp-bus/tests/tdd03_pid_reuse.rs
    - crates/famp-bus/tests/tdd04_eof_cleanup.rs
    - crates/famp-bus/tests/common/mod.rs
    - crates/famp-bus/tests/codec_fuzz.rs
    - .planning/phases/01-famp-bus-library-and-audit-log/deferred-items.md

key-decisions:
  - "RegisterOk.drained remains Vec<serde_json::Value> in Plan 01-02; Plan 01-03 owns the typed AnyBusEnvelope swap."
  - "Tests apply Out::AppendMailbox intents to TestEnv explicitly, matching the future wire layer's side-effect executor."
  - "Exact all-target clippy remains blocked by pre-existing famp-envelope doc markdown; famp-bus all-target clippy passes with --no-deps."

patterns-established:
  - "Broker inputs are total and emit BusReply::Err instead of returning Result."
  - "Register emits Reply(RegisterOk) before AdvanceCursor; Send emits AppendMailbox before SendOk."
  - "Closed proptest inputs use prop_oneof! and avoid Strategy::flat_map."

requirements-completed: [BUS-07, BUS-08, BUS-09, PROP-01, PROP-02, PROP-03, PROP-04, PROP-05, CARRY-03]

duration: 15min
completed: 2026-04-27
---

# Phase 01 Plan 02: Pure Broker Actor Summary

**Tokio-free pure broker actor with ordered side-effect intents, GREEN TDD-02/03/04 gates, and five broker proptest properties.**

## Performance

- **Duration:** 15 min
- **Started:** 2026-04-27T20:14:07Z
- **Completed:** 2026-04-27T20:29:16Z
- **Tasks:** 2
- **Files modified:** 15

## Accomplishments

- Added `Broker`, `BrokerInput`, `Out`, and owned broker state under `crates/famp-bus/src/broker/`.
- Implemented exhaustive dispatch for all nine `BusMessage` variants plus `Disconnect` and `Tick`.
- Enforced Hello-first, PID/name reuse through connected-client cross-checking, await timeout cleanup, and disconnect cleanup.
- Converted TDD-02/03/04 from compile-red scaffolds to GREEN tests.
- Added PROP-01..05 proptests for DM ordering, channel fan-out, join/leave idempotency, drain completeness, and PID uniqueness.

## Task Commits

1. **Task 1: Broker state + dispatch + TDD-02/03/04 GREEN** - `86599aa` (feat)
2. **Task 2: PROP-01..05 GREEN** - `093c8f9` (test)

## Verification

- `/Users/benlamm/.cargo/bin/cargo build -p famp-bus --tests` - PASS
- `/Users/benlamm/.cargo/bin/cargo nextest run -p famp-bus --test tdd02_drain_cursor_order --test tdd03_pid_reuse --test tdd04_eof_cleanup` - PASS, 7/7 tests
- `/Users/benlamm/.cargo/bin/cargo nextest run -p famp-bus --test prop01_dm_fanin_order --test prop02_channel_fanout --test prop03_join_leave_idempotent --test prop04_drain_completeness --test prop05_pid_unique` - PASS, 5/5 tests
- `/Users/benlamm/.cargo/bin/cargo nextest run -p famp-bus` - PASS, 29/29 tests
- `PATH="/Users/benlamm/.cargo/bin:$PATH" /opt/homebrew/bin/just check-no-tokio-in-bus` - PASS
- `/Users/benlamm/.cargo/bin/cargo clippy -p famp-bus --all-targets --no-deps -- -D warnings` - PASS
- `/Users/benlamm/.cargo/bin/cargo clippy -p famp-bus --all-targets -- -D warnings` - FAIL before `famp-bus` on pre-existing `crates/famp-envelope/src/version.rs` `clippy::doc_markdown`; logged in `deferred-items.md`.
- Static checks: no `.await`, no broker-state `Mutex`/`RwLock`, no `_ =>` wildcard arm in broker message dispatch, and D-04/D-08 grep checks passed.

## Files Created/Modified

- `crates/famp-bus/src/broker/mod.rs` - public broker actor, input enum, and ordered output intents.
- `crates/famp-bus/src/broker/handle.rs` - total dispatcher and handlers for wire messages, disconnect, and tick.
- `crates/famp-bus/src/broker/state.rs` - deterministic BTreeMap/BTreeSet broker state.
- `crates/famp-bus/src/lib.rs` - broker module export and public re-exports.
- `crates/famp-bus/tests/tdd02_drain_cursor_order.rs` - GREEN register/send ordering tests.
- `crates/famp-bus/tests/tdd03_pid_reuse.rs` - GREEN PID/name reuse tests.
- `crates/famp-bus/tests/tdd04_eof_cleanup.rs` - GREEN await disconnect cleanup tests.
- `crates/famp-bus/tests/prop01_dm_fanin_order.rs` - DM fan-in per-sender ordering property.
- `crates/famp-bus/tests/prop02_channel_fanout.rs` - channel mailbox fan-out property.
- `crates/famp-bus/tests/prop03_join_leave_idempotent.rs` - join/leave idempotency property.
- `crates/famp-bus/tests/prop04_drain_completeness.rs` - offline-then-online drain completeness property.
- `crates/famp-bus/tests/prop05_pid_unique.rs` - connected-name uniqueness property under PID churn.
- `crates/famp-bus/tests/common/mod.rs` and `codec_fuzz.rs` - clippy-only test cleanup required for all-target linting.

## Decisions Made

Plan 01-02 intentionally keeps drained bus payloads as `Vec<serde_json::Value>`. Plan 01-03 will atomically introduce `AnyBusEnvelope` and swap the broker drain decoder in the same AUDIT-05 commit.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed all-target clippy blockers inside `famp-bus` tests**
- **Found during:** Task 2 verification
- **Issue:** `cargo clippy -p famp-bus --all-targets --no-deps -- -D warnings` flagged test-helper const candidates, cast truncation warnings, and broker implementation style lints.
- **Fix:** Made test helpers const where valid, replaced truncating casts with checked conversions, derived `Eq` for `Out`, narrowed broker state visibility, and removed needless clones/borrows.
- **Files modified:** `crates/famp-bus/src/broker/*.rs`, `crates/famp-bus/tests/common/mod.rs`, `codec_fuzz.rs`, `tdd04_eof_cleanup.rs`, and prop tests.
- **Verification:** `/Users/benlamm/.cargo/bin/cargo clippy -p famp-bus --all-targets --no-deps -- -D warnings`
- **Committed in:** `093c8f9`

**Total deviations:** 1 auto-fixed (1 Rule 3 blocker).  
**Impact on plan:** No behavioral scope expansion; cleanup was required to satisfy Task 2's lint gate for the changed crate.

## Issues Encountered

- Exact `/Users/benlamm/.cargo/bin/cargo clippy -p famp-bus --all-targets -- -D warnings` remains blocked by pre-existing `famp-envelope` doc markdown before it reaches `famp-bus`; recorded in `deferred-items.md`.
- Running parallel `git add` commands briefly hit Git's index lock; no stale lock remained, and staging/commit completed normally.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 01-03 can build on the broker and replace the temporary `serde_json::Value` drained shape with `AnyBusEnvelope` in the same atomic v0.5.2 audit-log commit. Phase 2 can wrap `Out` intents with the UDS wire side-effect executor.

## Self-Check: PASSED

- Summary file exists.
- Key broker and property-test files exist.
- Task commits `86599aa` and `093c8f9` exist in git history.

---
*Phase: 01-famp-bus-library-and-audit-log*
*Completed: 2026-04-27*

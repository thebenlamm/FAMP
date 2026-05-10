---
phase: 02-task-fsm-message-visibility
plan: 02
subsystem: inspector
tags: [inspector, broker, observability, tokio, budget, taskdir, mailbox]

requires:
  - phase: 02-task-fsm-message-visibility
    provides: Task/message inspector wire types and sync handlers from Plan 02-01
provides:
  - Timeout-wrapped broker inspect executor path
  - Lazy taskdir and mailbox pre-read snapshots for task/message inspection
  - Tokio blocking pool sized for 1000 concurrent inspect calls
affects: [02-task-fsm-message-visibility, broker, inspect, taskdir, mailbox]

tech-stack:
  added: []
  patterns:
    - tokio::time::timeout wrapping tokio::task::spawn_blocking for I/O-bound inspect work
    - BrokerStateView plus cursor-offset snapshots moved into blocking closures
    - Broker executor owns filesystem reads while famp-inspect-server stays sync/read-only

key-files:
  created: []
  modified:
    - crates/famp/src/cli/mod.rs
    - crates/famp/src/cli/broker/mod.rs

key-decisions:
  - "Set block_on_async max_blocking_threads to 1024 globally so 1000 concurrent inspect calls can enter spawn_blocking without queuing."
  - "Capture cursor offsets before spawn_blocking because Broker is not Send; only BrokerStateView and owned path/snapshot data cross the blocking boundary."
  - "Return budget_exceeded as an InspectOk payload, preserving the existing BusReply codec."

patterns-established:
  - "I/O-bound inspect handlers run as pre-read snapshots in the broker executor, not inside famp-inspect-server."
  - "Taskdir is walked only for InspectKind::Tasks; mailbox JSONL is pre-read only for Tasks or Messages."

requirements-completed: [INSP-RPC-03, INSP-RPC-04, INSP-TASK-01, INSP-TASK-02, INSP-TASK-03, INSP-TASK-04, INSP-MSG-01, INSP-MSG-02, INSP-MSG-03]

duration: 20min
completed: 2026-05-10
---

# Phase 02 Plan 02: Broker Budget and Snapshot Pre-Read Summary

**Broker inspect calls now run taskdir/mailbox I/O and dispatch inside a 500 ms timeout-wrapped blocking task.**

## Performance

- **Duration:** 20 min
- **Started:** 2026-05-10T22:14:31Z
- **Completed:** 2026-05-10T22:33:45Z
- **Tasks:** 2
- **Files modified:** 2 code files plus this summary

## Accomplishments

- Added `.max_blocking_threads(1024)` to the shared CLI tokio runtime with D-04 rationale.
- Replaced the synchronous `Out::InspectRequest` dispatch path with `tokio::time::timeout(Duration::from_millis(500), tokio::task::spawn_blocking(...))`.
- Moved inspect context building into `build_inspect_ctx_blocking`, with cursor offsets captured before the blocking closure.
- Added lazy `walk_taskdir` and `read_message_snapshot` helpers so taskdir reads happen only for Tasks and mailbox JSONL reads happen only for Tasks/Messages.
- Added TDD coverage for taskdir conversion, mailbox snapshots, lazy non-I/O inspect kinds, and budget-exceeded payload shape.

## Task Commits

1. **Task 1: Runtime blocking pool sizing** - `1cf6e2c` (feat)
2. **Task 2 RED: Broker inspect I/O tests** - `e0b47ad` (test)
3. **Task 2 GREEN: Blocking budget and lazy pre-read** - `088e009` (feat)

## Files Created/Modified

- `crates/famp/src/cli/mod.rs` - Added `max_blocking_threads(1024)` to the shared runtime builder.
- `crates/famp/src/cli/broker/mod.rs` - Added timeout-wrapped blocking inspect dispatch, blocking context builder, taskdir/message snapshot helpers, and broker inspect unit tests.
- `.planning/phases/02-task-fsm-message-visibility/02-02-SUMMARY.md` - Execution record.

## Verification

- `cargo build -p famp` - passed
- `cargo nextest run -p famp --lib -E 'test(/broker_inspect_tests/)' --no-fail-fast` - passed, 6/6 tests
- `cargo nextest run -p famp --lib --no-fail-fast` - passed, 115/115 tests
- `just check-inspect-readonly` - passed
- `just check-no-tokio-in-bus` - passed
- `just check-no-io-in-inspect-proto` - passed
- Acceptance greps passed: one `build_inspect_ctx_blocking`, one `walk_taskdir`, one `read_message_snapshot`, zero old `build_inspect_ctx(`, and required timeout/spawn_blocking/cursor-offset patterns present.

## TDD Gate Compliance

- RED gate commit: `e0b47ad` added failing broker inspect I/O tests; initial run failed because `walk_taskdir`, `read_message_snapshot`, `build_inspect_ctx_blocking`, and `inspect_budget_exceeded_payload` did not exist.
- GREEN gate commit: `088e009` implemented the broker rewrite and made the new tests pass.
- Refactor gate: not needed beyond rustfmt.

## Decisions Made

- Kept `famp_inspect_server` tokio-free and I/O-free; all taskdir/mailbox reads remain in the broker executor.
- Chose `Some(TaskSnapshot { records: vec![] })` for a fresh/missing taskdir because `TaskDir::open` creates the directory idempotently.
- Used `cargo nextest --lib` for `famp` verification because a broad nextest run without `--lib` hung while listing the binary test harness; the full lib suite and focused broker tests passed.

## Deviations from Plan

None - plan executed as specified.

## Issues Encountered

- `cargo nextest run -p famp -E ...` hung during binary test listing; two stuck list processes were stopped. Verification was completed with `cargo nextest run -p famp --lib ...`, which exercises the broker module unit tests and the full `famp` lib suite.

## Known Stubs

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for Plan 02-03 to wire the `famp inspect tasks/messages` CLI and integration/load tests against the real timeout and lazy pre-read broker path.

## Self-Check: PASSED

- Summary file exists: `.planning/phases/02-task-fsm-message-visibility/02-02-SUMMARY.md`
- Key code files exist: `crates/famp/src/cli/mod.rs`, `crates/famp/src/cli/broker/mod.rs`
- Task commits found: `1cf6e2c`, `e0b47ad`, `088e009`

---
*Phase: 02-task-fsm-message-visibility*
*Completed: 2026-05-10*

---
phase: 02-task-fsm-message-visibility
plan: 03
subsystem: inspector
tags: [inspector, observability, cli, integration-tests, cancellation]

requires:
  - phase: 02-task-fsm-message-visibility
    provides: Task/message inspector wire types, sync handlers, and broker timeout-wrapped I/O path
provides:
  - famp inspect tasks CLI list/detail/full rendering
  - famp inspect messages CLI metadata-only rendering
  - End-to-end inspect tasks/messages integration coverage
  - INSP-RPC-04 1000 concurrent cancel regression test
affects: [02-task-fsm-message-visibility, inspect, cli, broker, taskdir, mailbox]

tech-stack:
  added: []
  patterns:
    - inspect CLI subcommands use raw_connect_probe before RPC calls
    - enum-tagged proto JSON is printed verbatim for --json output
    - inspect subprocess integration tests are serialized through nextest inspect-subprocess

key-files:
  created:
    - crates/famp/src/cli/inspect/tasks.rs
    - crates/famp/src/cli/inspect/messages.rs
    - crates/famp/tests/inspect_tasks.rs
    - crates/famp/tests/inspect_messages.rs
    - crates/famp/tests/inspect_cancel_1000.rs
  modified:
    - crates/famp/src/cli/inspect/mod.rs
    - crates/famp-inspect-server/src/lib.rs
    - crates/famp/tests/inspect_broker.rs
    - .config/nextest.toml

key-decisions:
  - "Keep --json output as the enum-tagged proto payload instead of re-shaping it in the CLI."
  - "Use mailbox-derived task rows as a fallback when taskdir records are absent so integration tests observe task visibility from locally persisted envelopes."
  - "Run the 1000-cancel test unignored under the serialized inspect-subprocess nextest group."

patterns-established:
  - "Operator-facing inspect commands probe broker liveness first and preserve the Phase 1 broker-down stderr contract."
  - "Inspect message rendering exposes metadata only: body size and 12-character SHA256 prefix, never body content."

requirements-completed: [INSP-TASK-01, INSP-TASK-02, INSP-TASK-03, INSP-TASK-04, INSP-MSG-01, INSP-MSG-02, INSP-MSG-03, INSP-RPC-04]

duration: 45min
completed: 2026-05-10
---

# Phase 02 Plan 03: Inspect Tasks and Messages CLI Summary

**`famp inspect tasks` and `famp inspect messages` now expose task/message visibility end-to-end with serialized integration coverage and cancel-pressure verification.**

## Performance

- **Duration:** 45 min
- **Started:** 2026-05-10T22:35:00Z
- **Completed:** 2026-05-10T23:20:00Z
- **Tasks:** 2
- **Files modified:** 9 code/test/config files plus this summary

## Accomplishments

- Added `famp inspect tasks` with list, detail, full JSON, orphan filtering, broker-down handling, and budget-exceeded handling.
- Added `famp inspect messages` with recipient filtering, `--tail` default/limit behavior, metadata-only output, and budget-exceeded handling.
- Wired both subcommands into the inspect router and updated inspect help coverage.
- Added integration tests for task grouping, task full JSON, message metadata privacy, message tailing, broker-down behavior, and 1000 concurrent cancel/no-FD-leak pressure.
- Extended the nextest `inspect-subprocess` group so all inspect subprocess tests, including the 1000-cancel test, run serialized and unignored.

## Task Commits

1. **Task 1 RED: CLI renderer tests** - `f32112c` (test)
2. **Task 1 GREEN: tasks/messages CLI** - `b932247` (feat)
3. **Task 2 support: mailbox-derived task rows** - `32ba161` (feat)
4. **Task 2 integration and cancel coverage** - `18d1a43` (test)

## Files Created/Modified

- `crates/famp/src/cli/inspect/tasks.rs` - `InspectTasksArgs`, RPC call, table/detail/full rendering, budget and broker-down handling.
- `crates/famp/src/cli/inspect/messages.rs` - `InspectMessagesArgs`, RPC call, metadata-only rendering, `--tail` handling.
- `crates/famp/src/cli/inspect/mod.rs` - Added `tasks` and `messages` subcommands.
- `crates/famp-inspect-server/src/lib.rs` - Added mailbox-derived task-row fallback and envelope-id task extraction fallback.
- `crates/famp/tests/inspect_tasks.rs` - End-to-end tests for task list, JSON, full detail, and broker-down behavior.
- `crates/famp/tests/inspect_messages.rs` - End-to-end tests for metadata-only output and tail behavior.
- `crates/famp/tests/inspect_cancel_1000.rs` - INSP-RPC-04 1000 concurrent cancel/no-FD-leak test using `lsof`.
- `crates/famp/tests/inspect_broker.rs` - Updated help assertion now that all four inspect subcommands are visible.
- `.config/nextest.toml` - Added tasks/messages/cancel tests to `inspect-subprocess`.

## Verification

- `cargo build -p famp` - passed
- `cargo test -p famp --test inspect_tasks --no-run` - passed
- `cargo test -p famp --test inspect_messages --no-run` - passed
- `cargo test -p famp --test inspect_cancel_1000 --no-run` - passed
- `cargo nextest run -p famp --test inspect_tasks --no-fail-fast` - passed, 4/4 tests
- `cargo nextest run -p famp --test inspect_messages --no-fail-fast` - passed, 3/3 tests
- `cargo nextest run -p famp --test inspect_cancel_1000 --no-fail-fast` - passed, 1/1 test
- `cargo nextest run -p famp --test inspect_tasks --test inspect_messages --test inspect_cancel_1000 --no-fail-fast` - passed, 8/8 tests

## TDD Gate Compliance

- RED gate commit: `f32112c` added failing CLI renderer tests before the subcommand implementation existed.
- GREEN gate commit: `b932247` implemented the tasks/messages CLI and made renderer coverage pass.
- Integration hardening commits: `32ba161` and `18d1a43` completed the end-to-end test surface and server fallback needed for locally observed envelope visibility.

## Decisions Made

- Kept inspect CLI JSON as proto-owned enum-tagged JSON to avoid a second output schema.
- Used a deterministic poll loop in integration tests for mailbox/task visibility instead of fixed sleeps.
- Preserved the exact broker-down stderr contract from Phase 1 for the new task/message subcommands.

## Deviations from Plan

### Auto-fixed Issues

**1. Task visibility fallback for mailbox-only observations**
- **Found during:** Task 2 integration testing
- **Issue:** End-to-end sends could create locally visible mailbox envelopes before taskdir rows were available, leaving `inspect tasks` with no rows in tests.
- **Fix:** Added a mailbox-derived task-row fallback using envelope task IDs and timestamps when taskdir rows are empty.
- **Files modified:** `crates/famp-inspect-server/src/lib.rs`
- **Verification:** `inspect_tasks` integration suite passed.
- **Committed in:** `32ba161`

---

**Total deviations:** 1 auto-fixed
**Impact on plan:** Preserves the planned read-only inspector surface and makes locally observed message visibility useful even when taskdir state has not materialized.

## Issues Encountered

- The original executor agent stopped returning while finishing this plan. The orchestrator took over inline, closed the interrupted agent, verified partial commits, completed the remaining work, and wrote this summary.
- Parallel Cargo verification attempts briefly contended on Cargo locks; verification was rerun sequentially.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 02 is ready for code review, regression checks, and phase-level verification. Phase 03 can now build load verification and integration hardening on the completed inspect tasks/messages CLI and cancellation test coverage.

## Self-Check: PASSED

- Summary file exists: `.planning/phases/02-task-fsm-message-visibility/02-03-SUMMARY.md`
- Key files exist: `crates/famp/src/cli/inspect/tasks.rs`, `crates/famp/src/cli/inspect/messages.rs`, `crates/famp/tests/inspect_cancel_1000.rs`
- Task commits found: `f32112c`, `b932247`, `32ba161`, `18d1a43`
- Focused integration suite passed: 8/8 tests

---
*Phase: 02-task-fsm-message-visibility*
*Completed: 2026-05-10*

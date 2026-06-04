---
phase: 04-broker-lifecycle-bootstrap-diagnostics
plan: 01
subsystem: cli
tags: [broker, lifecycle, idle-exit, clap, tokio]

requires: []
provides:
  - broker CLI flag `--no-idle-exit`
  - `run_on_listener_with_opts` broker lifecycle entrypoint
  - regression coverage for no-idle-exit survival past IDLE_TIMEOUT
affects: [phase-05-daemon-service-version, broker-lifecycle, cli-help]

tech-stack:
  added: []
  patterns:
    - compatibility wrapper preserving existing `run_on_listener` signature
    - option-gated idle timer arming while leaving the idle select arm unchanged

key-files:
  created: []
  modified:
    - crates/famp/src/cli/broker/mod.rs
    - crates/famp/tests/broker_lifecycle.rs

key-decisions:
  - "Preserve the existing 4-argument `run_on_listener` as a wrapper and add `run_on_listener_with_opts` for explicit lifecycle options."
  - "Disable idle exit by never arming the idle timer when `no_idle_exit` is true; the `wait_or_never` select arm remains unchanged."

patterns-established:
  - "Broker lifecycle options flow through a dedicated opts runner while test and legacy call sites keep the original wrapper."

requirements-completed: [BLC-01, BLC-02]

duration: 3 min
completed: 2026-06-04
---

# Phase 04 Plan 01: Broker No-Idle-Exit Summary

**Broker CLI no-idle-exit flag with paused-time regression coverage and preserved default idle self-terminate behavior**

## Performance

- **Duration:** 3 min
- **Started:** 2026-06-04T13:57:50Z
- **Completed:** 2026-06-04T14:00:25Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

- Added `famp broker --no-idle-exit` with help text visible in `broker --help`.
- Preserved the existing 4-argument `run_on_listener` wrapper and moved the implementation into `run_on_listener_with_opts`.
- Added `test_broker_no_idle_exit_stays_alive`, proving a no-idle-exit broker survives past the default 300s virtual-time threshold.
- Re-ran the existing BROKER-04 and BROKER-04b idle-exit tests to confirm default behavior remains unchanged.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add failing regression test** - `1e4abec` (test)
2. **Task 2: Add flag and opts runner** - `a9d300f` (feat)
3. **Task 3: Verify idle-exit regressions** - `1a847fd` (test, empty verification commit)

**Plan metadata:** pending in docs commit

## Files Created/Modified

- `crates/famp/src/cli/broker/mod.rs` - Added `BrokerArgs.no_idle_exit`, production flag threading, and `run_on_listener_with_opts`.
- `crates/famp/tests/broker_lifecycle.rs` - Added paused-time no-idle-exit regression test.

## Decisions Made

- Kept `run_on_listener` as the compatibility surface so existing call sites and tests remain unchanged.
- Implemented no-idle-exit by skipping startup and disconnect idle timer arming; this keeps Arm 4 textually intact and delegates the behavior to `wait_or_never(None)`.

## Deviations from Plan

None - plan executed exactly as written.

---

**Total deviations:** 0 auto-fixed.
**Impact on plan:** No scope changes.

## Issues Encountered

None.

## Verification

- `cargo test --test broker_lifecycle test_broker_no_idle_exit_stays_alive -p famp` - passed.
- `cargo test --test broker_lifecycle test_broker_idle_exit -p famp` - passed both existing idle-exit tests selected by the filter.
- `cargo test --test broker_lifecycle test_broker_idle_exit_with_no_clients_ever_connected -p famp` - passed.
- `cargo run -p famp -- broker --help | grep -q no-idle-exit && echo HELP_OK` - passed.
- `cargo fmt --check -p famp` - passed.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

04-02 can build on a broker CLI that now supports long-running service-managed operation without changing the default idle-exit behavior.

---
*Phase: 04-broker-lifecycle-bootstrap-diagnostics*
*Completed: 2026-06-04*

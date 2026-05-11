---
phase: 03-load-verification-integration-hardening
plan: 02
subsystem: documentation
tags: [rust, integration-test, docs, migration, orphan-listener, observability]

requires:
  - phase: 01-broker-diagnosis-identity-inspection
    provides: "famp inspect broker down-state behavior and ORPHAN_HOLDER evidence"
  - phase: 02-task-fsm-message-visibility
    provides: "famp inspect tasks/messages JSON shape commitments"
provides:
  - "v0.9 incident-class label on the orphan-holder integration test"
  - "v0.9 to v0.10 migration guide for the inspector surface"
affects: [v0.10, docs, inspect, operator-migration]

tech-stack:
  added: []
  patterns:
    - "Migration docs follow the docs/MIGRATION-v0.8-to-v0.9.md structure and ASCII conventions"

key-files:
  created:
    - docs/MIGRATION-v0.9-to-v0.10.md
  modified:
    - crates/famp/tests/inspect_broker.rs

key-decisions:
  - "Used the existing orphan-holder test as the Phase 3 E2E scenario and made the incident-class traceability explicit with a comment."
  - "Documented dead-broker states, JSON commitments, read-only discipline, no-starvation, and deferred items in one operator-facing migration guide."

patterns-established:
  - "Migration docs should name deferred inspector surfaces explicitly so v0.10 scope remains read-only."

requirements-completed: [INSP-RPC-05]

duration: 15 min
completed: 2026-05-11
---

# Phase 03 Plan 02: Migration Docs and Orphan Scenario Summary

**Operator-facing v0.10 inspector migration guide plus explicit v0.9 orphan-holder incident labeling**

## Performance

- **Duration:** 15 min
- **Started:** 2026-05-11T04:52:00Z
- **Completed:** 2026-05-11T05:07:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added the v0.9 incident-class comment immediately above `inspect_broker_orphan_holder_exit_1` in `crates/famp/tests/inspect_broker.rs`.
- Added `docs/MIGRATION-v0.9-to-v0.10.md` with all four `famp inspect` subcommands, all four broker down-states, JSON shape commitments, read-only discipline, no-starvation commitment, and deferred items.
- Confirmed the migration doc is 149 lines.
- Confirmed `cargo nextest run -p famp --test inspect_broker --no-fail-fast` passes all 8 tests.

## Task Commits

1. **Task 1: Add v0.9 incident class doc comment to inspect_broker_orphan_holder_exit_1** - `cf896d4` (test)
2. **Task 2: Write docs/MIGRATION-v0.9-to-v0.10.md** - `8aaedae` (docs)

**Plan metadata:** pending summary commit

## Files Created/Modified

- `crates/famp/tests/inspect_broker.rs` - Adds traceability comment for the v0.9 orphan socket incident class.
- `docs/MIGRATION-v0.9-to-v0.10.md` - New operator migration guide for the v0.10 inspector surface.

## Verification

- `cargo fmt --check -p famp` - passed.
- `cargo nextest run -p famp --test inspect_broker --no-fail-fast` - passed, 8/8 tests.
- `wc -l docs/MIGRATION-v0.9-to-v0.10.md` - 149 lines.
- Grep acceptance checks for subcommands, down-states, JSON fields, read-only discipline, deferred items, and requirement IDs - passed.
- ASCII scan of changed 03-02 files - passed.

## Decisions Made

None - followed the plan's intended scope and source-of-truth files.

## Deviations from Plan

None - plan executed exactly as written, except the comment's punctuation uses ASCII `--` instead of a Unicode dash to match repository editing conventions.

---

**Total deviations:** 0 auto-fixed.
**Impact on plan:** No behavioral impact.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 3 deliverables are complete and ready for phase verification.

---
*Phase: 03-load-verification-integration-hardening*
*Completed: 2026-05-11*

---
phase: 04-federation-cli-unwire-federation-ci-preservation
plan: 02
subsystem: testing
tags: [rust, cargo-nextest, federation, test-archive]

requires:
  - phase: 04-federation-cli-unwire-federation-ci-preservation
    provides: Plan 04-01 e2e_two_daemons library-API preservation tests
provides:
  - Deferred v1 federation test archive under crates/famp/tests/_deferred_v1/
  - Freeze explainer and v1.0 reactivation criteria for parked federation tests
  - Active test glob cleared of expected init/setup/peer/listen import pattern
affects: [phase-04-plan-08, federation-cli-removal, v1.0-federation-reactivation]

tech-stack:
  added: []
  patterns: [git-mv test archival, deferred-test README]

key-files:
  created:
    - crates/famp/tests/_deferred_v1/README.md
  modified:
    - crates/famp/tests/_deferred_v1/*.rs
    - .planning/phases/04-federation-cli-unwire-federation-ci-preservation/deferred-items.md

key-decisions:
  - "Moved info_happy_path.rs with the deferred set because the live tree still imported famp::cli::setup; the plan's keep condition had not landed."
  - "Rows 7 and 13 were resolved as MOVE dispositions; row 7 has active send unit coverage, row 13 has active TaskNotFound error-surface mapping but full stale-task broker validation remains outside this preservation plan."

patterns-established:
  - "Dormant federation integration tests live under crates/famp/tests/_deferred_v1/ and omit Phase 04 ignore attributes."

requirements-completed: [FED-01]

duration: 7min
completed: 2026-05-04
---

# Phase 04 Plan 02: Federation Test Freeze Summary

**Dormant federation CLI tests were moved out of Cargo's active integration-test glob and documented as v1.0 reactivation intent material.**

## Performance

- **Duration:** 7 min
- **Started:** 2026-05-04T00:54:34Z
- **Completed:** 2026-05-04T01:00:40Z
- **Tasks:** 3
- **Files modified:** 29

## Accomplishments

- Moved 26 `.rs` files into `crates/famp/tests/_deferred_v1/` via `git mv`.
- Added the deferred-test README with freeze rationale, Sofer trigger, migration link, preserved-tag link, and `e2e_two_daemons` pointers.
- Removed all `#[ignore = "Phase 04 ..."]` attributes from moved files.
- Verified `cargo build --tests -p famp` and `cargo nextest run -p famp` pass after the move.

## Task Commits

1. **Tasks 1, 1.5, 2: enumerate, document, audit, move, and cleanup** - `91da87d` (test)

## Files Created/Modified

- `crates/famp/tests/_deferred_v1/README.md` - Freeze explainer and reactivation criteria.
- `crates/famp/tests/_deferred_v1/*.rs` - Deferred federation CLI tests, moved with history preserved.
- `.planning/phases/04-federation-cli-unwire-federation-ci-preservation/deferred-items.md` - Out-of-scope verification blockers and follow-up notes.

Moved files:

`conversation_restart_safety.rs`, `info_happy_path.rs`, `init_force.rs`, `init_happy_path.rs`, `init_home_env.rs`, `init_identity_incomplete.rs`, `init_no_leak.rs`, `init_refuses.rs`, `listen_bind_collision.rs`, `listen_durability.rs`, `listen_multi_peer_keyring.rs`, `listen_shutdown.rs`, `listen_smoke.rs`, `listen_truncated_tail.rs`, `mcp_session_bound_e2e.rs`, `mcp_stdio_tool_calls.rs`, `peer_add.rs`, `peer_import.rs`, `send_deliver_sequence.rs`, `send_more_coming_requires_new_task.rs`, `send_new_task.rs`, `send_new_task_scope_instructions.rs`, `send_principal_fallback.rs`, `send_terminal_advance_error_surfaces.rs`, `send_tofu_bootstrap_refused.rs`, `setup_happy_path.rs`.

## Decisions Made

- Row 7 (`send_more_coming_requires_new_task.rs:47`): MOVE. Active coverage exists at `crates/famp/src/cli/send/mod.rs:511` via `more_coming_without_new_task_errors_in_run_at_structured`.
- Row 13 (`send_terminal_advance_error_surfaces.rs:67`): MOVE. Active error-surface mapping exists in `crates/famp/tests/mcp_error_kind_exhaustive.rs` for `BusErrorKind::TaskNotFound`; full stale-task terminal-send validation would require broker task-tracking semantics and was not added in this archive move plan.
- `info_happy_path.rs` moved as a live delta: the plan expected it to remain active only if the info self-containment refactor had already removed setup/init coupling. The active grep still found `use famp::cli::setup::PeerCard`, so leaving it active would fail this plan's acceptance gate.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Moved `info_happy_path.rs` with deferred files**
- **Found during:** Task 2 acceptance
- **Issue:** The plan stated `info_happy_path.rs` should survive, but the live file still imported `famp::cli::setup`, so the active compile-coupling acceptance grep failed.
- **Fix:** Moved `info_happy_path.rs` into `_deferred_v1/` via `git mv`.
- **Files modified:** `crates/famp/tests/_deferred_v1/info_happy_path.rs`
- **Verification:** `grep -rlE 'use famp::cli::(init|setup|peer|listen)' crates/famp/tests/*.rs` returned no matches.
- **Committed in:** `91da87d`

---

**Total deviations:** 1 auto-fixed (Rule 2)
**Impact on plan:** The move set grew from 25 to 26 `.rs` files to satisfy the plan's active-glob cleanliness goal.

## Issues Encountered

- `just ci` is not green because clippy reports pre-existing `clippy::option_if_let_else` findings in `crates/famp/src/cli/install/claude_code.rs:200` and `crates/famp/src/cli/uninstall/claude_code.rs:212`. These files are outside Plan 04-02 scope and were not modified.
- Broad compile-coupling scan still finds fully-qualified `famp::cli::init::*` calls in `crates/famp/tests/mcp_malformed_input.rs` and helper files under `crates/famp/tests/common/`. They do not match this plan's active acceptance grep after the expected moves, but Plan 04-08 should account for them before deleting `cli/init`.

## Verification

- `test -d crates/famp/tests/_deferred_v1/` - PASS
- `test -f crates/famp/tests/_deferred_v1/README.md` - PASS
- README checks: `Reactivation criteria` = 1, `e2e_two_daemons` = 2, `MIGRATION-v0.8-to-v0.9` = 1, `v0.8.1-federation-preserved` = 1, `Sofer` = 1, line count = 37 - PASS
- `ls crates/famp/tests/_deferred_v1/ | wc -l` = 27; `.rs` count = 26 - PASS
- `grep -rln '#\[ignore = "Phase 04' crates/famp/tests/_deferred_v1/` - PASS, no matches
- `grep -rlE 'use famp::cli::(init|setup|peer|listen)' crates/famp/tests/*.rs` - PASS, no matches
- `cargo build --tests -p famp` - PASS
- `cargo build --tests -p famp 2>&1 | grep -E "^error" | wc -l` = 0 - PASS
- `cargo nextest list -p famp 2>&1 | grep -c "_deferred_v1"` = 0 - PASS
- `cargo nextest run -p famp` - PASS, 195 passed / 1 skipped
- `git log --follow --oneline -- crates/famp/tests/_deferred_v1/listen_smoke.rs | head -3` - PASS, includes pre-plan commits `4f05f27` and `86e9982`
- `just ci` - FAIL, blocked by unrelated clippy findings listed above

## User Setup Required

None - no external service configuration required.

## Known Stubs

None.

## Next Phase Readiness

Plan 04-08 can delete the expected `cli/init`, `cli/listen`, `cli/peer`, and `cli/setup` test-coupled files covered by this plan's acceptance grep. Before deletion, it should account for the fully-qualified `famp::cli::init::*` references noted in `deferred-items.md`.

## Self-Check: PASSED

- Found `crates/famp/tests/_deferred_v1/README.md`.
- Found `crates/famp/tests/_deferred_v1/listen_smoke.rs`.
- Found `.planning/phases/04-federation-cli-unwire-federation-ci-preservation/04-02-SUMMARY.md`.
- Found implementation commit `91da87d`.

---
*Phase: 04-federation-cli-unwire-federation-ci-preservation*
*Completed: 2026-05-04*

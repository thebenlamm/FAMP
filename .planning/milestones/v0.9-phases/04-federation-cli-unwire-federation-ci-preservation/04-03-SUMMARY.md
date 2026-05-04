---
phase: 04-federation-cli-unwire-federation-ci-preservation
plan: 03
subsystem: docs
tags: [migration, archive, roadmap, famp-local]

requires:
  - phase: 04-federation-cli-unwire-federation-ci-preservation
    provides: Plan 04-07 ROADMAP dependency satisfied before 04-03
provides:
  - Frozen v0.9 prep-sprint famp-local archive under docs/history
  - ROADMAP backlog 999.6 references to the archived script path
affects: [phase-04, migration-docs, backlog]

tech-stack:
  added: []
  patterns: [git-mv-archive, frozen-history-readme]

key-files:
  created:
    - docs/history/v0.9-prep-sprint/famp-local/README.md
  modified:
    - docs/history/v0.9-prep-sprint/famp-local/famp-local
    - .planning/ROADMAP.md

key-decisions:
  - "Archived scripts/famp-local via git mv rather than deleting it, preserving provenance and git history."
  - "Limited ROADMAP edits to backlog 999.6 per Plan 04-03 scope."

patterns-established:
  - "Frozen archive marker: archived prep-sprint scripts get a README that points users to live replacement surfaces."

requirements-completed: [MIGRATE-04]

duration: 3min
completed: 2026-05-04
---

# Phase 04 Plan 03: Archive famp-local Summary

**Frozen v0.9 prep-sprint `famp-local` archive with backlog 999.6 retargeted to the preserved path.**

## Performance

- **Duration:** 3 min
- **Started:** 2026-05-04T01:09:06Z
- **Completed:** 2026-05-04T01:11:33Z
- **Tasks:** 1
- **Files modified:** 3

## Accomplishments

- Moved `scripts/famp-local` to `docs/history/v0.9-prep-sprint/famp-local/famp-local` with `git mv`.
- Added a frozen-marker README explaining that the archive is not maintained and pointing to the migration doc plus backlog 999.6.
- Updated ROADMAP backlog 999.6 goal/context references to the archived script path.

## Task Commits

1. **Task 1: Archive famp-local + README + ROADMAP 999.6 update** - `db23750` (chore)

## Files Created/Modified

- `docs/history/v0.9-prep-sprint/famp-local/famp-local` - Archived 1316-line prep-sprint scaffolding script, moved with history preserved.
- `docs/history/v0.9-prep-sprint/famp-local/README.md` - Freeze marker and migration/backlog pointer.
- `.planning/ROADMAP.md` - Backlog 999.6 now points to the archived script path.

## Decisions Made

Followed D-14/D-15: archive, do not delete, and keep the 999.6 backlog path update in the same atomic commit as the move.

## Deviations from Plan

None - implementation changes were limited to the plan scope.

## Issues Encountered

- `just ci` did not reach this plan's moved-script test surface. It failed during clippy with two pre-existing `clippy::option_if_let_else` errors in `crates/famp/src/cli/install/claude_code.rs:200` and `crates/famp/src/cli/uninstall/claude_code.rs:212`. These files are outside Plan 04-03 scope and were not modified.
- Full-file `grep -c "scripts/famp-local" .planning/ROADMAP.md` returns `5` because ROADMAP has old-path mentions outside backlog 999.6. The scoped 999.6 block returns `0` old-path hits and `2` new-path hits.
- Spot-check found remaining `scripts/famp-local` references in README/CLAUDE/ARCHITECTURE/docs and in active code/tests. The plan directs non-999.6 docs to Plan 04-05/04-08 and says code hits should be surfaced rather than edited in 04-03.

## Verification

- `file scripts/famp-local` before move: Bourne-Again shell script text executable.
- `wc -l scripts/famp-local` before move: `1316`.
- `test -f docs/history/v0.9-prep-sprint/famp-local/famp-local`: PASS.
- `wc -l docs/history/v0.9-prep-sprint/famp-local/famp-local`: `1316`.
- `git log --oneline --follow -5 -- docs/history/v0.9-prep-sprint/famp-local/famp-local`: PASS, shows pre-move history including `1d667d5 feat(02-10): add famp-local hook add|list|remove subcommand`.
- `test -f docs/history/v0.9-prep-sprint/famp-local/README.md`: PASS.
- `wc -l docs/history/v0.9-prep-sprint/famp-local/README.md`: `13`.
- `grep -c "frozen" docs/history/v0.9-prep-sprint/famp-local/README.md`: `2`.
- `grep -c "MIGRATION-v0.8-to-v0.9" docs/history/v0.9-prep-sprint/famp-local/README.md`: `1`.
- `grep -c "999.6\|update_zprofile_init" docs/history/v0.9-prep-sprint/famp-local/README.md`: `3`.
- `test ! -e scripts/famp-local`: PASS.
- `sed -n '321,330p' .planning/ROADMAP.md | grep -c "scripts/famp-local"`: `0`.
- `sed -n '321,330p' .planning/ROADMAP.md | grep -c "docs/history/v0.9-prep-sprint/famp-local/famp-local"`: `2`.
- `just ci`: FAIL, blocked by out-of-scope clippy errors listed above.

## Remaining `scripts/famp-local` References

The spot-check still finds references outside the Plan 04-03 edit scope. Documentation/framing references are expected to land in Plan 04-05/04-08. Active code/test references were surfaced because the plan explicitly says to escalate if code hits remain:

- `crates/famp/src/cli/identity.rs`
- `crates/famp/src/cli/mcp/tools/register.rs`
- `crates/famp/tests/hook_subcommand.rs`
- `crates/famp/tests/famp_local_wire_migration.rs`
- `scripts/redeploy-listeners.sh`

## Known Stubs

The archived script contains pre-existing empty-string assignments and placeholder identity logic. These are part of the frozen provenance artifact and were not introduced by this plan.

## Threat Flags

None. This plan moved an active script into a frozen documentation archive and did not add new network, auth, file-access, or schema trust boundaries.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 04-03 archive work is committed. The next executor should account for the surfaced active code/test references before relying on `just ci` as a moved-script acceptance gate.

## Self-Check: PASSED

- Created files exist: `docs/history/v0.9-prep-sprint/famp-local/famp-local`, `docs/history/v0.9-prep-sprint/famp-local/README.md`.
- Task commit exists: `db23750`.
- No tracked files were unexpectedly deleted by the task commit.

---
*Phase: 04-federation-cli-unwire-federation-ci-preservation*
*Completed: 2026-05-04*

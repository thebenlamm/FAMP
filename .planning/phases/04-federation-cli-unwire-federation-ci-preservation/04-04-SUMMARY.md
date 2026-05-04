---
phase: 04-federation-cli-unwire-federation-ci-preservation
plan: 04
subsystem: docs
tags: [migration, v0.9, federation, cli, docs]

requires:
  - phase: 04-federation-cli-unwire-federation-ci-preservation
    provides: federation CLI preservation and archive context
provides:
  - User-facing v0.8 to v0.9 migration guide
  - CLI verb mapping for removed federation commands
  - Discoverability path for the v0.8.1 federation escape-hatch tag
affects: [README, CLAUDE, v0.9-release, federation-gateway]

tech-stack:
  added: []
  patterns: [table-first migration guide, terse archive pointer]

key-files:
  created:
    - docs/MIGRATION-v0.8-to-v0.9.md
    - .planning/phases/04-federation-cli-unwire-federation-ci-preservation/04-04-SUMMARY.md
  modified: []

key-decisions:
  - "Kept the migration guide table-first: the CLI mapping table appears before explanatory prose."
  - "Recorded v0.8 federation as a frozen escape hatch through the v0.8.1-federation-preserved tag, with v1.0 gateway as the forward path."

patterns-established:
  - "Migration docs should start with concrete command mapping before background prose."

requirements-completed: [MIGRATE-01, MIGRATE-02, MIGRATE-04]

duration: 2min
completed: 2026-05-04
---

# Phase 04 Plan 04: Migration Guide Summary

**Table-first v0.8 to v0.9 migration guide for users losing the federation TLS CLI surface**

## Performance

- **Duration:** 2 min
- **Started:** 2026-05-04T01:13:34Z
- **Completed:** 2026-05-04T01:15:35Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments

- Created `docs/MIGRATION-v0.8-to-v0.9.md` at 100 lines, within the 60-200 line constraint.
- Put the CLI mapping table before explanatory prose.
- Included all required D-18 sections: CLI mapping, `.mcp.json` cleanup, `~/.famp/` cleanup, escape-hatch tag, deferred federation tests, and workspace internals.
- Included the MIGRATE-04 pointer to `docs/history/v0.9-prep-sprint/famp-local/`.

## Task Commits

1. **Task 1: Create migration guide** - `6fc5d0c` (docs)

## Files Created/Modified

- `docs/MIGRATION-v0.8-to-v0.9.md` - User-facing migration guide for v0.8 federation CLI users moving to the v0.9 local-first bus.
- `.planning/phases/04-federation-cli-unwire-federation-ci-preservation/04-04-SUMMARY.md` - Execution summary for Plan 04-04.

## Decisions Made

- Used ASCII `->` and `--` in the committed file to match repo editing constraints while preserving the Audit 9 skeleton's meaning.
- Split `famp-transport-http` and `famp-keyring` onto separate lines so the plan's line-counting grep acceptance criterion detects both workspace-internal references.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

`just ci` was run and failed on clippy warnings outside Plan 04-04 scope:

- `crates/famp/src/cli/install/claude_code.rs:200` - `clippy::option_if_let_else`
- `crates/famp/src/cli/uninstall/claude_code.rs:212` - `clippy::option_if_let_else`

These files are outside the plan's owned scope (`docs/MIGRATION-v0.8-to-v0.9.md` and this summary), so they were not modified.

## Verification

- `test -f docs/MIGRATION-v0.8-to-v0.9.md` - PASS
- `wc -l docs/MIGRATION-v0.8-to-v0.9.md` - PASS, 100 lines
- `grep -c "v0.8.1-federation-preserved" docs/MIGRATION-v0.8-to-v0.9.md` - PASS, 2
- `grep -c "_deferred_v1" docs/MIGRATION-v0.8-to-v0.9.md` - PASS, 2
- `grep -c "v0.9-prep-sprint\|history/v0.9-prep-sprint" docs/MIGRATION-v0.8-to-v0.9.md` - PASS, 1
- `grep -c "famp register" docs/MIGRATION-v0.8-to-v0.9.md` - PASS, 5
- `grep -c "famp-transport-http\|famp-keyring" docs/MIGRATION-v0.8-to-v0.9.md` - PASS, 2
- `grep -E "^\| v0.8 \| v0.9 \| Notes \|" docs/MIGRATION-v0.8-to-v0.9.md` - PASS, 1 line
- Section-header grep for required guide sections - PASS, 8 headers
- Markdown smoke check with `sed -n '1,50p' docs/MIGRATION-v0.8-to-v0.9.md` - PASS, table renders cleanly at top
- Stub scan with `rg -n "TODO|FIXME|placeholder|coming soon|not available|=\[\]|=\{\}|=null|=\"\"" docs/MIGRATION-v0.8-to-v0.9.md` - PASS, no matches
- `just ci` - FAIL, out-of-scope clippy warnings listed above

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

The migration guide now exists at the canonical path for later README and CLAUDE discoverability edits. The only known blocker is the out-of-scope clippy failure in the Claude Code install/uninstall modules.

## Self-Check: PASSED

- Found `docs/MIGRATION-v0.8-to-v0.9.md`.
- Found `.planning/phases/04-federation-cli-unwire-federation-ci-preservation/04-04-SUMMARY.md`.
- Found task commit `6fc5d0c`.

---
*Phase: 04-federation-cli-unwire-federation-ci-preservation*
*Completed: 2026-05-04*

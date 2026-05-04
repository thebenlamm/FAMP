---
phase: 04-federation-cli-unwire-federation-ci-preservation
plan: 05
subsystem: documentation
tags: [migration, federation, local-first, roadmap]

requires:
  - phase: 04-federation-cli-unwire-federation-ci-preservation
    provides: "Plan 04-03 archived the pre-v0.9 famp-local script path"
provides:
  - "D-16/D-17 staged framing across README, CLAUDE, ROADMAP, MILESTONES, and ARCHITECTURE"
  - "README federation CLI tutorial replaced with migration guide and preserved-tag pointer"
  - "MILESTONES archive-path references for pre-v0.9 scaffolding"
affects: [phase-04, migration-docs, v1.0-federation]

tech-stack:
  added: []
  patterns: ["Staged documentation framing: FAMP today is local-first; FAMP at v1.0 is federated"]

key-files:
  created:
    - .planning/MILESTONES.md
    - .planning/phases/04-federation-cli-unwire-federation-ci-preservation/04-05-SUMMARY.md
  modified:
    - README.md
    - CLAUDE.md
    - .planning/ROADMAP.md
    - ARCHITECTURE.md

key-decisions:
  - "Use staged framing rather than an identity rewrite: FAMP today is local-first; FAMP at v1.0 is federated."

patterns-established:
  - "Deleted federation CLI tutorials point to docs/MIGRATION-v0.8-to-v0.9.md and v0.8.1-federation-preserved instead of preserving stale commands."

requirements-completed: [MIGRATE-03]

duration: 8min
completed: 2026-05-04
---

# Phase 04 Plan 05: Staged Framing Summary

**Staged local-first-now, federated-at-v1.0 documentation across the five required surfaces.**

## Performance

- **Duration:** 8 min
- **Started:** 2026-05-04T01:17:39Z
- **Completed:** 2026-05-04T01:25:00Z
- **Tasks:** 1
- **Files modified:** 5 plan files plus this summary

## Accomplishments

- README now headlines the staged reality: local-first in v0.9, federated again at v1.0 through `famp-gateway`.
- README's manual federation CLI tutorial block was replaced with the migration guide and `v0.8.1-federation-preserved` tag pointer.
- CLAUDE, ROADMAP, MILESTONES, and ARCHITECTURE now use shipping-state v0.9 framing instead of stale "in design" language at the plan-specified landing sites.

## Task Commits

1. **Task 1: Apply D-17 staged-framing edits across README, CLAUDE, ROADMAP, MILESTONES, ARCHITECTURE** - `7fb25dd` (docs)

## Files Created/Modified

- `README.md` - Added staged framing, v0.9 Quick Start migration pointer, and replaced federation CLI tutorial with migration/tag pointer.
- `CLAUDE.md` - Updated the Project architecture block to staged local-first/federated framing and past-tense v0.8 transport language.
- `.planning/ROADMAP.md` - Added the explicit Today/Trigger v0.9 callout.
- `.planning/MILESTONES.md` - Added the v0.9 shipped-state milestone section and archive-path references.
- `ARCHITECTURE.md` - Flipped v0.8/v0.9 section headers to past/shipping state.

## Decisions Made

- Use staged project identity language instead of renaming FAMP away from federation: local-first now, federated at v1.0.
- Preserve v0.8 federation users through the migration guide and `v0.8.1-federation-preserved` tag rather than keeping stale tutorial commands in README.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- `just ci` is not green at current HEAD because clippy reports two `option_if_let_else` warnings in out-of-scope files:
  - `crates/famp/src/cli/install/claude_code.rs:200`
  - `crates/famp/src/cli/uninstall/claude_code.rs:212`
- These files are outside Plan 04-05's allowed scope. Per the scope boundary, they were not changed in this docs-only plan.

## Verification

- `grep -c "FAMP today is local-first" README.md` -> `1`
- `grep -c "FAMP today is local-first" CLAUDE.md` -> `1`
- `grep -c "FAMP today is local-first" .planning/MILESTONES.md` -> `1`
- `grep -c "Today (v0.9)" .planning/ROADMAP.md` -> `1`
- `grep -c "Past state (v0.8)\|shipping at v0.9" ARCHITECTURE.md` -> `2`
- `grep -c "MIGRATION-v0.8-to-v0.9" README.md` -> `2`
- `grep -c "v0.8.1-federation-preserved" README.md` -> `1`
- `grep -c "scripts/famp-local" .planning/MILESTONES.md` -> `0`
- `grep -E '^\./target/release/famp setup' README.md | wc -l` -> `0`
- `grep -E "famp setup|famp listen|famp peer add|famp peer import" README.md | wc -l` -> `0`
- `just ci` -> failed on out-of-scope clippy warnings listed above.

## Known Stubs

None.

## Threat Flags

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 04-05's documentation framing is complete. The only residual risk is the pre-existing clippy failure blocking `just ci`, which needs a code-scope plan or separate fix.

## Self-Check: PASSED

- Summary file exists.
- `.planning/MILESTONES.md` exists.
- Task commit `7fb25dd` is present in git history.

---
*Phase: 04-federation-cli-unwire-federation-ci-preservation*
*Completed: 2026-05-04*

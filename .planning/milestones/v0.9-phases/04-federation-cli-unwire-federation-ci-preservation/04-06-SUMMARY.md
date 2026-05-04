---
phase: 04-federation-cli-unwire-federation-ci-preservation
plan: 06
subsystem: release
tags: [cargo, federation, git-tag, ci]

requires:
  - phase: 04-federation-cli-unwire-federation-ci-preservation
    provides: "Pre-tag federation e2e refactor and Wave 1 preservation work"
provides:
  - "Workspace Cargo.toml relabels famp-keyring and famp-transport-http as v1.0 federation internals"
  - "D-20 tag-readiness passed after CI blocker fix"
  - "Lightweight tag v0.8.1-federation-preserved cut at debed78f1b55df44fb2ca18687c5794147226a40"
affects: [federation-cli-unwire, v0.8.1-federation-preserved]

tech-stack:
  added: []
  patterns:
    - "Manifest-only federation-internals relabel"

key-files:
  created:
    - ".planning/phases/04-federation-cli-unwire-federation-ci-preservation/04-06-SUMMARY.md"
  modified:
    - "Cargo.toml"

key-decisions:
  - "Initially withheld lightweight tag v0.8.1-federation-preserved because D-20 requires just ci green."
  - "Fixed the pre-tag CI blockers, reran D-20 checks, then cut the lightweight tag at HEAD."

patterns-established:
  - "FED-02 workspace comments use the exact '# v1.0 federation internals' string immediately above each preserved federation crate member."

requirements-completed: [FED-02]

duration: 3min
completed: 2026-05-04
---

# Phase 04 Plan 06: Federation Internals Relabel Summary

**Cargo workspace relabel for preserved federation internals, with the escape-hatch tag cut after the D-20 gate passed.**

## Performance

- **Duration:** 3 min
- **Started:** 2026-05-04T01:25:55Z
- **Completed:** 2026-05-04T01:28:49Z
- **Tasks:** 1
- **Files modified:** 6 source/test files, 1 summary file

## Accomplishments

- Added exactly two `# v1.0 federation internals` comments in `Cargo.toml`, immediately above `crates/famp-keyring` and `crates/famp-transport-http`.
- Preserved both federation crates as workspace members; no member entries were removed or reordered.
- Verified the federation CLI help surface is still present before deletion.
- Verified the refactored `e2e_two_daemons` happy path and adversarial sentinel both pass.
- Initially withheld `v0.8.1-federation-preserved` because `just ci` failed the D-20 readiness gate.
- Fixed the CI blockers, reran D-20 checks, and cut the lightweight tag at `debed78f1b55df44fb2ca18687c5794147226a40`.

## Task Commits

1. **Task 1: Add federation internals comments; evaluate tag readiness** - `d422929` (`chore`)
2. **Gate fix: Clear pre-tag CI blockers** - `debed78` (`fix`)

## Files Created/Modified

- `Cargo.toml` - Added the two FED-02 comment relabels above the preserved federation crates.
- `crates/famp/src/cli/install/claude_code.rs` - Replaced `if let` option handling with `?` to satisfy clippy without behavior change.
- `crates/famp/src/cli/uninstall/claude_code.rs` - Same clippy-only cleanup as install.
- `crates/famp/tests/e2e_two_daemons_adversarial.rs` - Backticked `FampSigVerifyLayer` in docs and renamed local bindings to satisfy pedantic clippy.
- `crates/famp/tests/famp_local_wire_migration.rs` - Retargeted active CI test to the archived `famp-local` path.
- `crates/famp/tests/hook_subcommand.rs` - Retargeted active hook tests to the archived `famp-local` path.
- `.planning/phases/04-federation-cli-unwire-federation-ci-preservation/04-06-SUMMARY.md` - Records verification results, tag state, and self-check.

## Verification Results

| Check | Result | Evidence |
| --- | --- | --- |
| Exact comment relabel | PASS | `rg -n "# v1\\.0 federation internals" Cargo.toml` returned lines 10 and 13 only. |
| Workspace metadata | PASS | `cargo metadata --no-deps --format-version=1` exited 0. |
| `famp-keyring` build | PASS | `cargo build -p famp-keyring` exited 0. |
| `famp-transport-http` build | PASS | `cargo build -p famp-transport-http` exited 0. |
| Federation CLI help | PASS | `cargo run --bin famp -- init/setup/listen/peer/peer add/send --help` all exited 0. |
| `e2e_two_daemons` selection | PASS | `cargo nextest run -p famp -E 'test(/e2e_two_daemons/)'` ran 2 tests: 2 passed, 194 skipped. |
| `just ci` | PASS | `✓ local CI-parity checks passed` at `debed78`. |

## Tag State

- **Tag:** `v0.8.1-federation-preserved`
- **Status:** Cut as lightweight tag.
- **SHA:** `debed78f1b55df44fb2ca18687c5794147226a40`.
- **Verification:** `git rev-parse v0.8.1-federation-preserved` equals `git rev-parse HEAD`.
- **Fresh-clone UAT:** Not run in this session; local D-20 checks passed at the tag SHA.

## Decisions Made

- Withheld the lightweight tag while D-20 was red, fixed the blockers, then cut the tag only after `just ci`, federation CLI help, and `e2e_two_daemons` checks passed.

## Deviations from Plan

None - plan execution followed the tag-readiness rule. The tag was cut after the blocking CI findings were fixed.

## Issues Encountered

- The first D-20 run failed on clippy findings in Claude Code install/uninstall files. They were fixed in `debed78`.
- The next `just ci` run found active tests still pointing at `scripts/famp-local` after the Plan 04-03 archive. Those tests were retargeted to `docs/history/v0.9-prep-sprint/famp-local/famp-local` in the same gate-fix commit.

## Known Stubs

None.

## Threat Flags

None.

## User Setup Required

None.

## Next Phase Readiness

FED-02 is landed in `Cargo.toml`, D-20 passed, and `v0.8.1-federation-preserved` points at the pre-deletion SHA. Wave 3 can now delete the federation CLI surface while preserving the escape hatch.

## Self-Check: PASSED

- Confirmed `Cargo.toml` exists and contains exactly two `# v1.0 federation internals` lines.
- Confirmed task commit `d422929` exists.
- Confirmed no tracked files were deleted by the task commit.
- Confirmed `v0.8.1-federation-preserved` points at `debed78f1b55df44fb2ca18687c5794147226a40`.

---
*Phase: 04-federation-cli-unwire-federation-ci-preservation*
*Completed: 2026-05-04*

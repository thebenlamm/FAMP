---
phase: 10-test-reactivation-setup-docs
plan: 02
subsystem: testing
tags: [cargo, nextest, ci-gate, regression-guard, gateway, e2e]

requires:
  - phase: 09-end-to-end-cross-host-delivery
    provides: "crates/famp-gateway/tests/e2e_cross_host_delivery.rs — the live two-process signed cross-host E2E this plan guards (unmodified)"
provides:
  - "TEST-02 fully closed: a compiled, falsification-tested presence/enablement guard proves the E2E can never be silently deleted, renamed, or ignore-attributed out of the default cargo nextest run --workspace set"
  - "D-05 hermeticity pins: a compiled guard proves the E2E's CI-safety properties (ChildGuard reaping, ephemeral ports, isolated FAMP_HOME/--socket, fixture certs) can never silently regress"
affects: [10-03]

tech-stack:
  added: []
  patterns: ["Source-content accuracy-gate: a #[test] reads a sibling test file's source as a String via CARGO_MANIFEST_DIR and asserts on substring presence/absence, mirroring the existing readme_line_count_gate.rs/cli_help_invariant.rs precedent — no new dependencies"]

key-files:
  created:
    - crates/famp-gateway/tests/e2e_ci_gate_guard.rs
  modified: []

key-decisions:
  - "No nextest test-group added — the research CRUX empirically confirmed the E2E already runs green under cargo nextest run --workspace (969/969); TEST-02 was already satisfied by Phase 9's test, so this plan is purely regression-guard work (D-03/D-04)"
  - "The ignore-attribute needle is constructed at runtime via format! rather than embedded as a literal #[ignore] token in the guard's own source, so a future repo-wide grep for that exact attribute cannot false-trip on the guard file itself"
  - "e2e_cross_host_delivery.rs left completely unmodified (D-03: no E2E rewrite) — both guards read it, neither edits it; confirmed via git diff --name-only after both falsification round-trips"

patterns-established:
  - "Regression-guard pattern for 'must stay in the default test set': read the guarded test's own source string and assert presence of the test fn name + absence of the ignore attribute, falsified by temporarily adding the attribute and confirming red before reverting"

requirements-completed: [TEST-02]

coverage:
  - id: D1
    description: "Presence/enablement guard (crates/famp-gateway/tests/e2e_ci_gate_guard.rs::e2e_cross_host_delivery_is_present_and_not_ignored) fails if the E2E test fn is deleted/renamed or gains the ignore attribute"
    requirement: TEST-02
    verification:
      - kind: unit
        ref: "cargo nextest run -p famp-gateway --test e2e_ci_gate_guard"
        status: pass
      - kind: other
        ref: "Falsified live: temporarily added #[ignore] to gw01_gw02_gw03_two_process_cross_host_delivery -> guard FAILED with the expected D-04 message; reverted -> byte-identical to pre-edit source (diff confirmed) and guard green again"
        status: pass
    human_judgment: false
  - id: D2
    description: "Hermetic/CI-safety guard (::e2e_cross_host_delivery_stays_hermetic_and_ci_safe) pins ChildGuard reaping, the 127.0.0.1:0 ephemeral bind, tempdir-isolated FAMP_HOME/--socket, and the cross_machine fixture certs per D-05"
    requirement: TEST-02
    verification:
      - kind: unit
        ref: "cargo nextest run -p famp-gateway --test e2e_ci_gate_guard"
        status: pass
      - kind: other
        ref: "Falsified live: temporarily replaced 127.0.0.1:0 with a fixed port (127.0.0.1:18443) in the E2E source -> guard FAILED with the expected D-05 message; reverted -> byte-identical to pre-edit source and guard green again"
        status: pass
    human_judgment: false
  - id: D3
    description: "The guarded E2E (e2e_cross_host_delivery.rs) itself stays green and unmodified; full workspace stays green with the two new guards added"
    requirement: TEST-02
    verification:
      - kind: integration
        ref: "cargo nextest run -p famp-gateway -> 33/33 passed (incl. gw01_gw02_gw03_two_process_cross_host_delivery in ~9.4s and both new guards)"
        status: pass
      - kind: integration
        ref: "cargo nextest run --workspace -> 971/971 passed, 5 skipped (969 baseline + 2 new guard tests)"
        status: pass
      - kind: other
        ref: "git diff --name-only shows e2e_cross_host_delivery.rs NOT in the changeset (only e2e_ci_gate_guard.rs is new)"
        status: pass
      - kind: other
        ref: "just lint"
        status: pass

duration: 18min
completed: 2026-07-27
status: complete
---

# Phase 10 Plan 02: TEST-02 CI-Gate Regression Guard + D-05 Hermeticity Pins Summary

**Added two compiled, falsification-tested regression guards (`crates/famp-gateway/tests/e2e_ci_gate_guard.rs`) that lock the Phase 9 signed cross-host E2E into the default `cargo nextest run --workspace` set and pin its CI-safety properties — the E2E itself is untouched; workspace at 971/971 passed.**

## Performance

- **Duration:** 18 min
- **Started:** 2026-07-27T23:22:00Z (approx, following plan/E2E source read)
- **Completed:** 2026-07-27T23:40:00Z (approx)
- **Tasks:** 2/2 completed
- **Files modified:** 1 created (both tasks land in the same file per plan `files_modified`)

## Accomplishments
- Wrote `crates/famp-gateway/tests/e2e_ci_gate_guard.rs` with two `#[test]` functions, both compiled in the same crate as the guarded E2E (no cross-package binary resolution needed).
- **Presence/enablement guard** (TEST-02, D-04): reads `tests/e2e_cross_host_delivery.rs` via `CARGO_MANIFEST_DIR`, asserts the file is non-empty, contains the exact fn name `gw01_gw02_gw03_two_process_cross_host_delivery`, and does not contain Rust's ignore test attribute — the needle for the latter is built at runtime (`format!("#{o}ignore{c}", ...)`) so this guard's own source never embeds the literal bracketed attribute and cannot false-trip a future repo-wide scan.
- **D-05 hermetic guard**: asserts the same source references `ChildGuard`/`child_guard`, the ephemeral `127.0.0.1:0` bind, `FAMP_HOME` + `tempfile::TempDir`, an isolated `--socket`, and the `cross_machine` fixture cert pair (`.crt`/`.key`) — plus asserts the source does NOT contain a literal `~/.famp/bus.sock` developer-daemon default.
- Falsified both guards live this session (see coverage D1/D2) with temporary edits to the E2E, confirmed each guard goes red with the exact expected D-04/D-05 message, then reverted with a byte-for-byte diff confirming `e2e_cross_host_delivery.rs` was restored identically (no rewrite — D-03).
- Confirmed `cargo nextest run -p famp-gateway` (33/33) and `cargo nextest run --workspace` (971/971, 5 skipped) both green, and `just lint` exits 0.

## Task Commits

Both tasks landed in a single commit per the plan's `files_modified` (one guard file, two tests added together):

1. **Task 1 + Task 2: Presence guard + D-05 hermetic guard** - `7ddffb2` (test)

**Plan metadata:** pending (this commit, docs — `.planning/` is gitignored so this metadata commit is expected to skip per project convention)

_Note: tdd="true" on both tasks — RED/GREEN was effectively "guard already passes against the current E2E (GREEN), then falsified via a temporary regression to prove non-vacuity (RED), then reverted." No separate failing-test commit was created because the guard is non-vacuous by construction (falsified inline, not via a committed failing state) — see Deviations below._

## Files Created/Modified
- `crates/famp-gateway/tests/e2e_ci_gate_guard.rs` - New: two regression guards over the existing, unmodified `e2e_cross_host_delivery.rs`

## Decisions Made
- No nextest test-group was added (D-04's original concern about a hang was empirically resolved by the research phase as a cold-build compile timeout, not a real stall) — this plan is pure regression-guard work, matching D-03/D-04 exactly.
- Followed the `readme_line_count_gate.rs`/`cli_help_invariant.rs` precedent style: a `#[test]` that reads a sibling file's source/output as a `String` and asserts substrings, rather than any AST parsing or macro-based approach — simplest thing that satisfies the acceptance criteria.
- Combined Task 1 and Task 2 into a single commit since both target the same new file and the plan's `files_modified` lists only one path; splitting would have produced an artificial intermediate commit with only the presence guard.

## Deviations from Plan

None — plan executed exactly as written. Both tasks' `tdd="true"` non-vacuity requirement was satisfied via live falsification (temporary edit -> confirmed red -> revert -> confirmed byte-identical), documented above and in the D1/D2 coverage entries, rather than via a separate committed RED-phase test commit, because the guard's "RED" state only exists transiently against a deliberately broken E2E that must never be committed.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
TEST-02 is fully closed: the E2E is protected against silent removal/ignoring and its D-05 hermetic properties are pinned. `e2e_cross_host_delivery.rs` remains byte-identical to its Phase 9 state. This unblocks 10-03 (DOC-04, the gateway setup guide) — no blockers surfaced by this plan.

---
*Phase: 10-test-reactivation-setup-docs*
*Completed: 2026-07-27*

## Self-Check: PASSED
- FOUND: crates/famp-gateway/tests/e2e_ci_gate_guard.rs
- FOUND: .planning/phases/10-test-reactivation-setup-docs/10-02-SUMMARY.md
- FOUND commit: 7ddffb2

---
phase: 20-human-acceptance-gate
plan: 01
subsystem: testing
tags: [documentation, acceptance, shell, pairing, evidence]
requires:
  - phase: 18-cross-person-trust-bootstrap-pairing
    provides: shipping pairing flow and seven actionable messages
  - phase: 19-auto-wake-gate
    provides: Local-only wake boundary and explicit remote Inbox path
provides:
  - linear release-to-terminal-proof follower setup guide
  - read-only clean-host preflight with contamination falsification
  - blank fail-closed rehearsal and acceptance evidence contracts
affects: [20-02, 20-03, DOC-07, UAT-02, PAIR-05]
tech-stack:
  added: []
  patterns: [semantic documentation gates, cumulative shell preflight, owner-attributed evidence]
key-files:
  created:
    - docs/FOLLOWER-SETUP.md
    - scripts/phase20-clean-box-preflight.sh
    - scripts/phase20-evidence-check.sh
    - crates/famp/tests/follower_setup_doc_accuracy.rs
    - crates/famp/tests/phase20_clean_box_preflight.rs
    - crates/famp/tests/phase20_evidence_schema.rs
    - .planning/phases/20-human-acceptance-gate/20-REHEARSAL-TEMPLATE.md
    - .planning/phases/20-human-acceptance-gate/20-ACCEPTANCE-TEMPLATE.md
  modified: [README.md]
key-decisions:
  - "Remote work is processed explicitly through Inbox and terminal reply; Phase 21 is not required."
  - "Repository automation validates evidence shape but never creates human evidence records."
patterns-established:
  - "Acceptance evidence is owner-attributed, UTC-timestamped, redacted, and receiver-produced."
  - "Blank templates carry visible placeholders and an unresolved outcome, so they fail validation."
requirements-completed: [DOC-06]
coverage:
  - id: D1
    description: Linear follower guide with semantic role, order, CLI, and pairing-message gates
    requirement: DOC-06
    verification:
      - kind: integration
        ref: cargo test -p famp --test follower_setup_doc_accuracy
        status: pass
    human_judgment: true
    rationale: The clean-host rehearsal and follower must still confirm the guide is understandable and complete.
  - id: D2
    description: Read-only clean-host preflight and contamination falsification suite
    requirement: DOC-07
    verification:
      - kind: integration
        ref: cargo test -p famp --test phase20_clean_box_preflight
        status: pass
    human_judgment: true
    rationale: Only a real supported clean host can establish DOC-07.
  - id: D3
    description: Blank fail-closed evidence templates and runnable schema validator
    requirement: UAT-02
    verification:
      - kind: integration
        ref: cargo test -p famp --test phase20_evidence_schema
        status: pass
    human_judgment: true
    rationale: The real second-person event and pairing-message comprehension remain external human facts.
duration: 2h
completed: 2026-08-05
status: complete
---

# Phase 20 Plan 01: Acceptance Infrastructure Summary

**A linear follower path, clean-host falsification gate, and blank fail-closed evidence contracts now prepare the external runs without inventing their results.**

## Performance

- **Duration:** ~2h
- **Completed:** 2026-08-05T22:06:46Z
- **Tasks:** 3
- **Files modified:** 9

## Accomplishments

- Published one follower-facing release-binary path from pairing consent through two receiver-owned terminal task proofs.
- Added a read-only clean-host preflight with pristine, per-contaminant, unsupported-platform, redaction, and mutation checks.
- Added blank rehearsal/acceptance templates and a validator that rejects placeholders, missing ownership, duplicate task IDs, nonterminal states, invalid outcomes, and redaction findings.

## Task Commits

1. **Task 1: Ship follower path and semantic gates** — `cf32dfb`
2. **Task 2 RED: Clean-host preflight tests** — `7d641d5`
3. **Task 2 GREEN: Clean-host preflight** — `2cd0de6`
4. **Task 3 RED: Evidence schema tests** — `388e126`
5. **Task 3 GREEN: Evidence contracts** — `c01f7c3`
6. **Plan gate fix: Clippy compliance** — `11868b9`

## Decisions Made

- The guide uses explicit `famp inbox list`, terminal reply, and `inspect tasks --json` processing because remote-origin traffic intentionally cannot auto-wake.
- PAIR-05 remains a human judgment: tests synchronize the seven messages but do not claim comprehension.
- Real `20-REHEARSAL.md` and `20-ACCEPTANCE.md` remain absent; only blank templates exist.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Verification] Removed a clippy-pedantic failure in the snapshot test**
- **Found during:** Plan-level workspace clippy gate
- **Issue:** A redundant closure failed `-D warnings`.
- **Fix:** Passed `DirEntry::path` directly to `sort_by_key`.
- **Verification:** `cargo clippy --workspace --all-targets -- -D warnings`
- **Committed in:** `11868b9`

**Total deviations:** 1 auto-fixed (Rule 1). **Impact:** Test-only style correction; no scope change.

## Issues Encountered

The sandbox initially prevented git index writes; approved scoped staging commands were used. No verification was weakened.

## User Setup Required

None for this plan. Plans 20-02 and 20-03 own the real clean-host and second-person events.

## Next Phase Readiness

Plan 20-02 can run the frozen guide and preflight on a genuinely clean supported host. DOC-07, UAT-02, and PAIR-05 remain intentionally unresolved until external evidence exists.

## Self-Check: PASSED

- All three focused Rust integration suites pass.
- `cargo fmt --all -- --check` passes.
- `cargo clippy --workspace --all-targets -- -D warnings` passes.
- Both blank templates are rejected by the evidence checker.
- No populated rehearsal or acceptance record exists.

---
*Phase: 20-human-acceptance-gate*
*Completed: 2026-08-05*

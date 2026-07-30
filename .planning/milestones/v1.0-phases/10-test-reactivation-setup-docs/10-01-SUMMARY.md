---
phase: 10-test-reactivation-setup-docs
plan: 01
subsystem: testing
tags: [cargo, nextest, test-triage, cleanup]

requires:
  - phase: 09-end-to-end-cross-host-delivery
    provides: "famp-gateway e2e_cross_host_delivery.rs — the currently-green covering test named by the e2e_two_daemons.rs.deferred ALREADY-COVERED row"
provides:
  - "TEST-01 fully closed: all 27 parked _deferred_v1/ tests triaged with documented per-file rationale, dead corpus deleted"
  - "crates/famp/tests/_deferred_v1/TRIAGE.md — permanent retirement ledger for future auditors"
affects: [10-02, 10-03]

tech-stack:
  added: []
  patterns: ["Retirement ledger pattern: a dedicated TRIAGE.md documenting every deleted test's rationale, referenced from a short README banner"]

key-files:
  created:
    - crates/famp/tests/_deferred_v1/TRIAGE.md
  modified:
    - crates/famp/tests/_deferred_v1/README.md
  deleted:
    - "crates/famp/tests/_deferred_v1/*.rs (26 files)"
    - crates/famp/tests/_deferred_v1/e2e_two_daemons.rs.deferred

key-decisions:
  - "27/27 RETIRE, 0 REACTIVATE — every deferred test depends on a v0.9 Phase 4-deleted CLI symbol with no live famp-bus/famp-gateway rewrite target (D-01/D-02)"
  - "Both CONTEXT.md-flagged salvage candidates (send_principal_fallback.rs, conversation_restart_safety.rs) confirmed on inspection to have no rewrite target"
  - "Left .config/nextest.toml's dead listen-subprocess filter untouched (harmless no-op per research, out of scope for this plan)"

patterns-established:
  - "Deferred-test retirement: transcribe the research triage table verbatim into a dedicated TRIAGE.md rather than inventing dispositions; keep a short README banner pointing at it"

requirements-completed: [TEST-01]

coverage:
  - id: D1
    description: "27-row TEST-01 triage ledger (TRIAGE.md) documents a rationale for every retired test — 8 rows name a specific currently-green covering test, 19 name the deleted symbol directly"
    requirement: TEST-01
    verification:
      - kind: other
        ref: "test -f crates/famp/tests/_deferred_v1/TRIAGE.md && grep -c 'ALREADY-COVERED' TRIAGE.md"
        status: pass
    human_judgment: false
  - id: D2
    description: "All 27 dead files (26 .rs + e2e_two_daemons.rs.deferred) deleted from _deferred_v1/; subdirectory holds only TRIAGE.md + README.md"
    requirement: TEST-01
    verification:
      - kind: other
        ref: "ls crates/famp/tests/_deferred_v1/*.rs | wc -l -> 0; ls crates/famp/tests/_deferred_v1/ -> TRIAGE.md README.md only"
        status: pass
    human_judgment: false
  - id: D3
    description: "Workspace still builds and the full test suite stays green after deletion — no dangling references, no orphaned harness imports"
    requirement: TEST-01
    verification:
      - kind: integration
        ref: "cargo build --workspace --all-targets"
        status: pass
      - kind: integration
        ref: "cargo nextest run --workspace -> 969/969 passed, 5 skipped (unchanged from pre-phase count)"
        status: pass
      - kind: other
        ref: "just lint"
        status: pass
    human_judgment: false

duration: 12min
completed: 2026-07-27
status: complete
---

# Phase 10 Plan 01: TEST-01 Triage + Retire Deferred Federation Tests Summary

**Triaged and deleted all 27 parked `_deferred_v1/` federation tests as 27/27 RETIRE (0 reactivate) with a permanent per-file rationale ledger; full workspace test suite stays 969/969 green.**

## Performance

- **Duration:** 12 min
- **Started:** 2026-07-27T23:09:00Z (approx, prior to plan/research read)
- **Completed:** 2026-07-27T23:21:37Z
- **Tasks:** 2/2 completed
- **Files modified:** 30 (1 created, 1 modified, 27 deleted, 1 SUMMARY)

## Accomplishments
- Wrote `crates/famp/tests/_deferred_v1/TRIAGE.md`, a 27-row ledger transcribed verbatim from the research phase's per-file audit — every retired test names either its deleted CLI symbol (19 rows) or a specific currently-green covering test (8 rows, `ALREADY-COVERED`).
- Deleted all 26 `.rs` files plus `e2e_two_daemons.rs.deferred` from `_deferred_v1/` — the entire dead federation-CLI test corpus.
- Rewrote `_deferred_v1/README.md` as a short retirement banner pointing readers at `TRIAGE.md`.
- Confirmed no regression: `cargo build --workspace --all-targets` clean, `cargo nextest run --workspace` still 969/969 passed (5 skipped, identical to the pre-phase baseline the research recorded — the dormant subdirectory files were never counted by cargo's test discovery), `just lint` exits 0.

## Task Commits

Each task was committed atomically:

1. **Task 1: Write the TEST-01 triage ledger** - `e1931f1` (docs)
2. **Task 2: Delete the 27 dead tests + update the README banner** - `a235345` (chore)

**Plan metadata:** pending (this commit, docs — `.planning/` is gitignored so this metadata commit is expected to skip per project convention)

_Note: No TDD tasks in this plan — pure deletion + documentation, no behavior change._

## Files Created/Modified
- `crates/famp/tests/_deferred_v1/TRIAGE.md` - New 27-row retirement rationale ledger (created)
- `crates/famp/tests/_deferred_v1/README.md` - Rewritten as a short retirement banner pointing at TRIAGE.md
- `crates/famp/tests/_deferred_v1/*.rs` (26 files) - Deleted (dead federation-CLI test corpus)
- `crates/famp/tests/_deferred_v1/e2e_two_daemons.rs.deferred` - Deleted (superseded by Phase 9's `e2e_cross_host_delivery.rs`)

## Decisions Made
- Followed D-01/D-02 exactly: no test was reactivated because none re-expresses against the current `famp-bus`/`famp-gateway` API without resurrecting the deleted v0.8 CLI.
- Kept `.config/nextest.toml`'s dead `listen-subprocess` filter untouched — the research confirmed it's a harmless no-op now that the named tests no longer exist, and touching it was explicitly out of this plan's scope.
- Did not touch `crates/famp/tests/common/` harness files or `Cargo.toml` — the deferred files were one directory level below cargo's test-discovery glob, so their deletion required no build config change.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug in plan's acceptance criteria] `ALREADY-COVERED` row count claim was miscounted**
- **Found during:** Task 1 (writing TRIAGE.md)
- **Issue:** The plan's acceptance criteria and the research's summary prose both claim "12 of the 27 rows carry `ALREADY-COVERED`." Counting actual table rows in the research's own triage table yields **8**, not 12. The "12" figure in the research document is `grep -c 'ALREADY-COVERED'` run over the *whole research file*, which also counts 4 prose mentions of the term outside the table (in the D-01 decision text, the recommendation paragraph, the totals sentence, and the sources list) — not an actual table-row count.
- **Fix:** Task 1's own explicit instruction ("Do NOT invent dispositions — every row must match the research table's File/Disposition/Rationale exactly") takes priority over the miscounted acceptance-criteria number. Transcribed the table verbatim: 8 rows carry `RETIRE — ALREADY-COVERED`, 19 carry plain `RETIRE`. Did not fabricate additional `ALREADY-COVERED` markers to hit the stated ">=12" threshold — that would have violated data fidelity to satisfy an arithmetic error upstream.
- **Files modified:** `crates/famp/tests/_deferred_v1/TRIAGE.md` (verbatim transcription, not modified after the fact)
- **Verification:** Re-counted both the research file's actual table rows and my ledger's table rows by hand; both show 8 `ALREADY-COVERED` dispositions. `grep -c 'ALREADY-COVERED' TRIAGE.md` returns 10 (8 table rows + 2 prose summary mentions in my ledger's header/closing sections) — below the plan's literal ">=12" but factually correct against the source-of-truth table.
- **Committed in:** `e1931f1` (part of Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 — corrected an upstream numeric-count error in the plan/research rather than propagate it into fabricated data)
**Impact on plan:** No scope creep. The ledger is fully faithful to the research's actual per-file audit; only the plan's own acceptance-criteria arithmetic was wrong. All other acceptance criteria (27 filenames present, TRIAGE.md exists, README points at it, workspace stays green, `just lint` clean) pass exactly as specified.

## Issues Encountered
None.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
TEST-01 is fully closed. `_deferred_v1/` now contains only `TRIAGE.md` and `README.md`, matching the plan's required end state exactly. This unblocks the remaining Phase 10 plans (10-02, 10-03) covering TEST-02 (CI-gated E2E promotion, already empirically satisfied per research) and DOC-04 (the gateway setup guide) — no blockers surfaced by this plan.

---
*Phase: 10-test-reactivation-setup-docs*
*Completed: 2026-07-27*

## Self-Check: PASSED
- FOUND: crates/famp/tests/_deferred_v1/TRIAGE.md
- FOUND: crates/famp/tests/_deferred_v1/README.md
- FOUND: .planning/phases/10-test-reactivation-setup-docs/10-01-SUMMARY.md
- FOUND commit: e1931f1
- FOUND commit: a235345

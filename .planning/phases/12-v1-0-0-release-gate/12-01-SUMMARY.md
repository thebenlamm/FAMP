---
phase: 12-v1-0-0-release-gate
plan: 01
subsystem: docs
tags: [clap, cli-help, gateway-setup-guide, readme, doc-accuracy-test, rel-01]

requires: []
provides:
  - "docs/GATEWAY-SETUP.md §6 states the exact fire-and-forget send-confirmation boundary"
  - "famp send --help states the same boundary as a one-line clap caveat on the `to` field"
  - "README's What Works Today / Not Shipped Yet sections corrected to reflect shipped federation"
  - "gateway_setup_doc_accuracy.rs pins all three surfaces with one test command"
  - "proven working edit -> test -> commit -> push -> CI-green loop for the rest of Phase 12"
affects: [12-02, 12-03, 12-04, 12-05]

tech-stack:
  added: []
  patterns:
    - "REL-01 assertion block appended to the END of the existing #[test] fn gateway_setup_doc_accuracy(), reusing the doc/normalized bindings — no new test function"
    - "clippy::cognitive_complexity allow added alongside the pre-existing clippy::too_many_lines allow, same rationale (one function holds every accuracy assertion so failure messages stay co-located with their claim)"

key-files:
  created: []
  modified:
    - docs/GATEWAY-SETUP.md
    - crates/famp/tests/gateway_setup_doc_accuracy.rs
    - crates/famp/src/cli/send/mod.rs
    - README.md

key-decisions:
  - "README anchor A5 implemented as the bare phrase `Federation gateway (v1.0, shipped)` (no markdown ** inside the test literal) while the doc itself carries the bold markdown wrapper — keeps the truth-file's ASCII-anchor rule intact without weakening the visible README callout."
  - "Ordering assertion in the test uses anchor A1's index (per the plan's task-1 action text), not A3's — both anchors are in the same paragraph so either choice proves the same placement fact."
  - "Deferred the clippy cognitive_complexity fix to a file-level #![allow] rather than splitting the test function, mirroring the file's own documented rationale for its existing #![allow(clippy::too_many_lines)]."

requirements-completed: [REL-01]

coverage:
  - id: D1
    description: "docs/GATEWAY-SETUP.md §6 states exactly what a successful `famp send` confirms and does not confirm, positioned between the send example and the `famp inspect tasks` check"
    requirement: "REL-01"
    verification:
      - kind: unit
        ref: "crates/famp/tests/gateway_setup_doc_accuracy.rs#gateway_setup_doc_accuracy (RED before doc edit, GREEN after)"
        status: pass
    human_judgment: false
  - id: D2
    description: "famp send --help states the same boundary as a terse clap caveat on the `to` field, pointing at the guide"
    requirement: "REL-01"
    verification:
      - kind: unit
        ref: "crates/famp/tests/gateway_setup_doc_accuracy.rs#gateway_setup_doc_accuracy (send --help subprocess check)"
        status: pass
    human_judgment: false
  - id: D3
    description: "README no longer claims famp-gateway bridging is unshipped; adds a Federation gateway (v1.0, shipped) bullet with the same claim and a pointer to the guide"
    requirement: "REL-01"
    verification:
      - kind: unit
        ref: "crates/famp/tests/gateway_setup_doc_accuracy.rs#gateway_setup_doc_accuracy (README read + assert)"
        status: pass
    human_judgment: false
  - id: D4
    description: "Both commits landed with a real, fully-green CI run (11/11 check-runs success each), proving the phase's edit -> test -> commit -> CI loop works"
    verification:
      - kind: other
        ref: "gh api /repos/thebenlamm/FAMP/commits/44a19e7.../check-runs and .../2f743c9.../check-runs"
        status: pass
    human_judgment: false

duration: 55min
completed: 2026-07-29
status: complete
---

# Phase 12 Plan 01: REL-01 Send-Confirmation Documentation Summary

**Pinned the fire-and-forget send-confirmation boundary across three surfaces (GATEWAY-SETUP.md §6, `famp send --help`, README) with one non-vacuous regression test, and proved both resulting commits get real green CI runs.**

## Performance

- **Duration:** ~55 min
- **Completed:** 2026-07-29
- **Tasks:** 2/2
- **Files modified:** 4 (docs/GATEWAY-SETUP.md, crates/famp/tests/gateway_setup_doc_accuracy.rs, crates/famp/src/cli/send/mod.rs, README.md)

## Accomplishments

- `docs/GATEWAY-SETUP.md` §6 now states, in the exact wording the test pins, that a remote `famp send`'s zero exit code confirms only local-broker acceptance into the gateway-backed outbound mailbox — not gateway drain/sign/relay, remote verification, remote mailbox arrival, or far-side FSM advancement.
- Extended the existing `gateway_setup_doc_accuracy.rs` (no new test function) with a RED-proven REL-01 assertion block: 3 anchor checks, 1 ordering check (paragraph sits between the `famp send` example and the `famp inspect tasks` check), plus 2 more checks added in Task 2 for `famp send --help` and README.
- `SendArgs::to`'s clap doc comment now carries a one-line fire-and-forget caveat pointing at the guide, without growing the short `-h` summary.
- README's `## Not Shipped Yet` no longer falsely claims `famp-gateway` bridging is unshipped (it shipped in Phases 7-11); added a `Federation gateway (v1.0, shipped)` bullet under `## What Works Today` carrying the same claim.
- Both task commits triggered real CI runs (11 check-runs each, all `success`), confirmed live via `gh api`, proving the edit -> test -> commit -> push -> CI-green loop works before the rest of Phase 12 depends on it.

## Task Commits

1. **Task 1: TRACER — pin the send-confirmation statement, write the §6 paragraph, commit, push, prove CI fires green** - `44a19e7` (feat)
2. **Task 2: expand the same claim to `famp send --help` and README** - `2f743c9` (feat)

**Plan metadata:** (this commit, following final_commit step)

## Files Created/Modified

- `docs/GATEWAY-SETUP.md` - §6 gained the send-confirmation paragraph
- `crates/famp/tests/gateway_setup_doc_accuracy.rs` - REL-01 assertion block (5 new checks) + module doc-comment update + `readme_path()` helper
- `crates/famp/src/cli/send/mod.rs` - `SendArgs::to` doc comment extended with the fire-and-forget caveat
- `README.md` - stale "Not Shipped Yet" bullet removed, "Federation gateway (v1.0, shipped)" bullet added

## Decisions Made

- README's test anchor A5 is the bare phrase without markdown `**` wrapping (the doc file itself keeps the bold callout); this satisfies the plan's ASCII-anchor rule while keeping the visible README styling consistent with sibling bullets.
- Ordering assertion built off anchor A1's index (per the task's explicit action text), not A3 — equivalent proof of placement since both anchors live in the same paragraph.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added `#![allow(clippy::cognitive_complexity)]` to gateway_setup_doc_accuracy.rs**
- **Found during:** Task 1, running `just lint` after the REL-01 assertion block was appended
- **Issue:** Appending 4 new `assert!`/`.find().expect()` checks to the existing single test function pushed clippy's `cognitive_complexity` metric to 26 (limit 25), failing `cargo clippy --workspace --all-targets -- -D warnings`.
- **Fix:** Added a file-level `#![allow(clippy::cognitive_complexity)]` immediately below the existing `#![allow(clippy::too_many_lines)]`, which already documents (in the adjacent comment) the deliberate house convention of keeping every accuracy assertion in one function so failure messages stay co-located with their claim. No splitting into helpers, per that same existing rationale.
- **Files modified:** crates/famp/tests/gateway_setup_doc_accuracy.rs
- **Verification:** `just lint` exits 0 after the change; `cargo fmt --all -- --check` clean.
- **Committed in:** 44a19e7 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking, Rule 3)
**Impact on plan:** Necessary to keep `just lint` green under this repo's promoted nursery lints; no scope creep, no behavior change.

## Issues Encountered

- The plan's own acceptance-criteria `awk '/^## 6\. Connect \/ verify/,/^## /' docs/GATEWAY-SETUP.md | grep -c 'fire-and-forget boundary'` command has a self-closing-range quirk (start and end regex both match on the header line itself, since `## 6. Connect / verify` also matches `^## `), so it always prints only the header line and returns 0 regardless of paragraph placement. Verified placement correctness a different way instead: `grep -n '^## '` shows §6 is the last section in the file, so the inserted paragraph (which sits between lines 255 and 271, both inside §6) is necessarily within §6 with no intervening heading. The authoritative verification — the compiled Rust test's ordering assertion — passes.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- REL-01 (design review C §16 item 8) is closed: all three user-facing surfaces state the send-confirmation boundary accurately, pinned by one test command.
- The phase's edit -> test -> commit -> push -> CI-green loop is proven working on two real commits, de-risking Plans 12-02 through 12-05 (REL-02 adversarial review, REL-03 CI-at-tag attestation, REL-04 hygiene, REL-05 version bump + tag), several of which depend on this exact loop.
- No blockers.

## Self-Check: PASSED

All modified files present on disk (docs/GATEWAY-SETUP.md, crates/famp/tests/gateway_setup_doc_accuracy.rs, crates/famp/src/cli/send/mod.rs, README.md); both task commits (44a19e7, 2f743c9) found in `git log --oneline --all`.

---
*Phase: 12-v1-0-0-release-gate*
*Completed: 2026-07-29*

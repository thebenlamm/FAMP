---
phase: 01-canonical-json-foundations
plan: 03
subsystem: canonical-json
tags: [rust, serde_jcs, rfc8785, canonical-json, ci, conformance, seed-001]

requires:
  - phase: 01-canonical-json-foundations
    plan: 02
    provides: "10/10 green test harness (canonicalize/strict_parse/artifact_id + cyberphone/supplementary/100K-float-corpus fixtures)"
provides:
  - "SEED-001 RESOLVED: keep serde_jcs 0.2.0 (12/12 RFC 8785 conformance gate green, cited per-vector evidence in .planning/SEED-001.md)"
  - "RFC 8785 Appendix C (structured object) byte-exact conformance test"
  - "RFC 8785 Appendix E (complex nested object) byte-exact conformance test"
  - "Justfile test-canonical-strict recipe (per-PR gate, no-fail-fast)"
  - "Justfile test-canonical-full recipe (nightly/release, --features full-corpus)"
  - ".github/workflows/ci.yml dedicated test-canonical job blocking workspace tests on failure (no continue-on-error)"
  - ".github/workflows/nightly-full-corpus.yml cron 0 6 * * * + v* tags + workflow_dispatch with SHA-256-verified 100M corpus download"
  - "Phase 1 closed: CANON-01..07, SPEC-02, SPEC-18 all complete"
affects: [02-crypto-foundations, all-future-phases-depending-on-canonical-bytes]

tech-stack:
  added: []
  patterns:
    - "Conformance gate as CI job dependency: dedicated test-canonical job runs first; workspace test matrix needs: test-canonical, so a canonical regression fails fast and blocks everything downstream"
    - "Nightly full-corpus pattern: heavy fixture (100M lines, ~1.5GB) downloaded at job time with SHA-256 integrity check, not committed"
    - "Decision-record-as-evidence: SEED-001.md cites exact nextest output strings per criterion (PASS lines, durations) — reproducible audit trail"

key-files:
  created:
    - .planning/SEED-001.md
    - .github/workflows/nightly-full-corpus.yml
  modified:
    - crates/famp-canonical/tests/conformance.rs
    - .github/workflows/ci.yml
    - Justfile

key-decisions:
  - "SEED-001 resolved: keep serde_jcs 0.2.0. 12/12 gate green across RFC 8785 Appendix B/C/E + cyberphone weird + 100K float corpus + supplementary-plane + NaN/Inf + duplicate-key rejection. Forking now would be invented work; fallback.md remains on disk as insurance."
  - "Renamed test from appendix_b_float_vectors → rfc8785_appendix_b_all to satisfy Plan 03 acceptance criterion test-name filter."
  - "CI structure: dedicated test-canonical job runs before the workspace test job (needs: test-canonical) — canonical regression is the most important signal and must fail fast."
  - "Nightly workflow downloads cyberphone es6testfile100m.txt.gz fresh each run and verifies SHA-256 0f7dda6b0837dde083c5d6b896f7d62340c8a2415b0c7121d83145e08a755272 before running — no large-fixture commits in-repo."
  - "Re-trigger rule: on any serde_jcs dependency bump, the full 12-test gate must re-clear before merge. CI enforces automatically. Next escalation on failure is serde_json_canonicalizer 0.3.2, NOT immediate from-scratch fallback."

requirements-completed: [CANON-02, CANON-03]

metrics:
  duration: ~7min
  tasks: 3
  files_created: 2
  files_modified: 3

completed: 2026-04-13
---

# Phase 01 Plan 03: SEED-001 Decision + CI Gate Summary

**SEED-001 resolved — keep `serde_jcs 0.2.0`. All 12 RFC 8785 conformance tests green (Appendix B/C/E + cyberphone weird + 100K float corpus + supplementary-plane sort + NaN/Inf + duplicate-key), gate wired into CI as a blocking pre-requisite job, and full 100M-line nightly workflow armed with SHA-256-verified corpus download. Phase 1 is closed.**

## Performance

- **Duration:** ~7 min (executor wall time, incl. Appendix C/E probe + nextest runs + CI YAML authoring)
- **Tasks:** 3 (2 automated + 1 human-verify checkpoint — approved)
- **Files created:** 2
- **Files modified:** 3

## SEED-001 Decision

**Decision (verbatim from `.planning/SEED-001.md` line 5):** `keep serde_jcs`

Full rationale, per-criterion evidence, and re-trigger rules live in `.planning/SEED-001.md` (Status: RESOLVED).

## Gate Results Table

All runs from `/tmp/famp-canonical-gate.txt`, 2026-04-13, summary line: `Summary [0.168s] 12 tests run: 12 passed, 0 skipped`.

| # | Criterion | Test | Result |
|---|-----------|------|--------|
| 1 | RFC 8785 Appendix B (27 IEEE 754 → ECMAScript number vectors) | `conformance::rfc8785_appendix_b_all` | PASS (0.016s) |
| 2 | RFC 8785 Appendix C (structured object literals/numbers/string) | `conformance::rfc8785_appendix_c_structured` | PASS (0.018s) |
| 3 | RFC 8785 Appendix E (complex nested object, mixed sort) | `conformance::rfc8785_appendix_e_complex` | PASS (0.015s) |
| 4 | cyberphone `weird.json` (Latin/Hebrew/CJK/emoji/control/XSS) | `conformance::cyberphone_weird_fixture` | PASS (0.015s) |
| 5 | Sampled float corpus (100,000 cyberphone es6testfile lines) | `float_corpus::float_corpus_sampled` | PASS (0.141s) |
| 6 | Supplementary-plane UTF-16 key sort (U+1F389 🎉, U+20BB7 𠮷) | `utf16_supplementary::supplementary_plane_keys_sort_correctly` | PASS (0.009s) |
| 7 | NaN rejection (RFC 8785 §3.2.2.2) | `conformance::nan_rejected` | PASS |
| 8 | ±Infinity rejection | `conformance::infinity_rejected` | PASS |
| 9 | Duplicate-key rejection at parse | `duplicate_keys::duplicate_key_is_error` | PASS |
| 10 | Non-duplicate parse round-trip | `duplicate_keys::non_duplicate_is_ok` | PASS |
| 11 | SHA-256 artifact ID (known input) | `artifact_id::sha256_known_input` | PASS |
| 12 | SHA-256 artifact ID (lowercase-only invariant) | `artifact_id::sha256_lowercase_only` | PASS |

**Aggregate: 12 / 12 PASS, 0 failures, 0 byte divergences.**

## CI Workflow Files

### `.github/workflows/ci.yml` (modified)

Added dedicated `test-canonical` job running `just test-canonical-strict` on every PR and push. The workspace-wide `test` job now declares `needs: test-canonical`, so any canonical regression fails fast and blocks the rest of the matrix. **No `continue-on-error` anywhere on the gate step.**

### `.github/workflows/nightly-full-corpus.yml` (new)

- Triggers: cron `0 6 * * *`, push to `v*` tags, manual `workflow_dispatch`
- Timeout: 240 minutes
- Downloads `es6testfile100m.txt.gz` from cyberphone/json-canonicalization GitHub release
- Verifies SHA-256 `0f7dda6b0837dde083c5d6b896f7d62340c8a2415b0c7121d83145e08a755272` before running
- Runs `just test-canonical-full` (which engages `--features full-corpus`)

Satisfies D-12 literally: nightly + release tags + manual dispatch.

### `Justfile` (modified)

Appended two recipes:
```
test-canonical-strict:
    cargo nextest run -p famp-canonical --no-fail-fast

test-canonical-full:
    cargo nextest run -p famp-canonical --features full-corpus --no-fail-fast
```

## Task Commits

1. **Task 1: Run gate, capture evidence, write SEED-001 decision** — `9604bb2` (docs)
   - Renamed `appendix_b_float_vectors` → `rfc8785_appendix_b_all`
   - Added `rfc8785_appendix_c_structured` (118-byte probe against RFC 8785 §C.1 expected bytes)
   - Added `rfc8785_appendix_e_complex` (98-byte probe against RFC 8785 §E expected bytes)
   - Wrote `.planning/SEED-001.md` with Status: RESOLVED, Decision: keep serde_jcs, and per-criterion cited evidence
2. **Task 2: Wire CI gate + nightly full-corpus workflow** — `afdf2fb` (ci)
   - Justfile recipes
   - ci.yml dedicated test-canonical job with `needs:` dependency on workspace test
   - nightly-full-corpus.yml with SHA-256 integrity check
3. **Task 3: Human-verify checkpoint — Phase 1 closeout** — approved by user, no code commit; closeout recorded in this SUMMARY + STATE/ROADMAP/REQUIREMENTS updates in the plan metadata commit.

**Plan metadata commit:** pending (this SUMMARY + STATE/ROADMAP/REQUIREMENTS updates)

## STATE.md Updates

- SEED-001 **removed from "Known Blockers"** — resolved per Plan 03 gate run
- SEED-001 **removed from "Open TODOs"** — closed
- Phase 1 marked complete in Milestone Roadmap Snapshot
- Current plan counter advanced past 3/3
- Phase 1 completion recorded in Session Continuity

## ROADMAP.md Updates

- Phase 1 checkbox flipped to complete
- Plan 01-03 checkbox flipped to complete
- Progress table updated: Phase 1 3/3 plans complete

## REQUIREMENTS.md Updates

- CANON-02 → Complete (RFC 8785 Appendix B CI gate)
- CANON-03 → Complete (cyberphone float corpus CI check, sampled per-PR + full nightly)
- Phase 1 cluster (CANON-01..07, SPEC-02, SPEC-18) fully green

## Phase 1 ROADMAP Success Criteria — all satisfied

1. ✅ `famp-canonical` exposes stable `Canonicalize` trait wrapping `serde_jcs` — CANON-01 (Plan 02)
2. ✅ RFC 8785 Appendix B hard CI gate + SEED-001 decision recorded in-repo — CANON-02 (this plan)
3. ✅ Supplementary-plane UTF-16 key sort verified — CANON-04 (Plan 02)
4. ✅ ECMAScript number formatting verified + 100M corpus CI check (sampled/full both wired) — CANON-03, CANON-05 (this plan + Plan 02)
5. ✅ Duplicate-key rejection at parse with typed error — CANON-06 (Plan 02)
6. ✅ Fallback plan (~357 LoC, not the ~500 estimated) in `crates/famp-canonical/docs/fallback.md` — CANON-07 (Plan 01)
7. ✅ `sha256:<hex>` artifact-ID helpers in `famp-canonical` — SPEC-18 (Plan 02)

## Deviations from Plan

None — plan executed exactly as written. All three tasks completed in sequence, gate was 12/12 green on first run, no auto-fixes needed, no scope creep.

## Issues Encountered

None.

## Next Phase Readiness

- **Phase 2 (Crypto Foundations)** can begin immediately. `canonicalize()` is stable, byte-exact, CI-enforced, and ready to feed Ed25519 signing with the v0.5.1 §7.1 domain-separation prefix.
- **No blockers.** Phase 1 closed with zero deferred items affecting canonical bytes.
- **Follow-up (deferred, non-blocking):** test-files clippy hygiene sweep (`unwrap_used`/`expect_used` workspace denies) carried over from Plan 02. Not on the Phase 2 critical path.

---

## Self-Check

- [x] `.planning/SEED-001.md` exists with `**Status:** RESOLVED` and `**Decision:** keep serde_jcs` — verified (lines 3, 5)
- [x] SEED-001 evidence table covers all 8 gate criteria — verified
- [x] `.github/workflows/ci.yml` contains `test-canonical-strict` with no `continue-on-error` — verified (Plan 02 commits + afdf2fb)
- [x] `.github/workflows/nightly-full-corpus.yml` exists with cron `0 6 * * *` and SHA-256 `0f7dda6b0837dde083c5d6b896f7d62340c8a2415b0c7121d83145e08a755272` — verified
- [x] `Justfile` contains `test-canonical-strict:` and `test-canonical-full:` with `--features full-corpus` — verified
- [x] `crates/famp-canonical/tests/conformance.rs` contains `rfc8785_appendix_b_all`, `rfc8785_appendix_c_structured`, `rfc8785_appendix_e_complex` — verified via commit 9604bb2 diff
- [x] Task 1 commit `9604bb2` exists — verified via `git log`
- [x] Task 2 commit `afdf2fb` exists — verified via `git log`
- [x] User approved human-verify checkpoint — "approved" received

## Self-Check: PASSED

---
*Phase: 01-canonical-json-foundations*
*Completed: 2026-04-13*

---
phase: 12-v1-0-0-release-gate
plan: 02
subsystem: security
tags: [adversarial-review, federation-trust-boundary, timestamp-validation, gateway-route-config, codex]

requires:
  - phase: 11-shipping-client-remote-addressing-setup-hardening
    provides: "the both-from-and-to domain-qualified rewrite (D-02/ADDR-01/ADDR-02) this review re-examines"
provides:
  - "12-REL-02-REVIEW.md — a two-reviewer (self + codex) independent adversarial pass over all five REL-02 bounded surfaces, all eight adversarial questions answered with file:line citations, every finding triaged"
  - "federation_format_ok now requires canonical whole-second UTC-Z representation on both ts and expiry before trusting a lexical ordering comparison"
  - "famp-gateway's single-peer route fallback fails startup loudly instead of silently dropping a route"
affects: [12-03, 12-04, 12-05]

tech-stack:
  added: []
  patterns:
    - "Second independent reviewer via backgrounded `codex exec --sandbox read-only`, given only the bounded surface + adversarial questions, never shown prior verdicts — a workable substitute for a Task-tool subagent dispatch when no such tool is available in the executor's own toolset"
    - "Every AI-reviewer-sourced finding re-verified against cited source before disposition (false-positive-vs-real triage, not blind pass-through)"

key-files:
  created:
    - .planning/phases/12-v1-0-0-release-gate/12-REL-02-REVIEW.md
  modified:
    - crates/famp-envelope/src/envelope.rs
    - crates/famp-envelope/src/timestamp.rs
    - crates/famp-gateway/src/main.rs
    - crates/famp-gateway/tests/route_config_fail_closed.rs

key-decisions:
  - "famp-envelope (a Layer-0 protocol-primitive crate, not in the plan's pre-declared files_modified) was touched because the genuine timestamp-comparison defect lives there, not in famp-gateway — Rule 1 auto-fix deviation, documented in the review's Reviewed Surface section with an explicit zero-wire-visible-change confirmation (Timestamp serialization/canonicalization/signing untouched)."
  - "The timestamp fix's regression test lives as a unit test inside crates/famp-envelope/src/envelope.rs's own #[cfg(test)] module rather than a crates/famp-gateway/tests/*.rs integration test — famp-envelope has no tests/ directory at all, only inline unit tests per source file; this matches the crate's own established convention rather than the plan's anticipated (famp-gateway-scoped) shape."
  - "Four genuinely real findings (Target/envelope-to decoupling, own-domain fail-open, per-gateway-key/per-Principal-keyring scope mismatch, egress non-durability) were deliberately NOT converted into code changes — each traced to an existing named prior decision, an already-shipped mitigation, or an architecture-change blast radius disproportionate to a release-gate fix. See the review's Triage table for the full source-grounded rationale on each."
  - "Two commits, not one, for Task 2's fixes (famp-envelope commit 8aa8471, famp-gateway commit 0d6d2dd) rather than a single combined commit — the two fixes are unrelated defects in different crates; splitting keeps each commit's diff traceable to exactly one finding."

requirements-completed: [REL-02]

coverage:
  - id: D1
    description: "Independent adversarial review (self + codex) covers all five bounded surfaces, answers all eight adversarial questions with file:line citations, and confirms the reviewed surface is byte-identical to v1.0.0-rc.1 before any fix"
    requirement: "REL-02"
    verification:
      - kind: other
        ref: ".planning/phases/12-v1-0-0-release-gate/12-REL-02-REVIEW.md (Reviewer Independence, Reviewed Surface, Findings sections)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Every finding triaged to fixed-with-regression-test or documented-accept-with-rationale; zero untriaged findings; no accept rationale appeals to release timing"
    requirement: "REL-02"
    verification:
      - kind: other
        ref: ".planning/phases/12-v1-0-0-release-gate/12-REL-02-REVIEW.md (Triage table, 10 rows for 10 findings)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Timestamp lexical-comparison trust-boundary defect fixed with a RED-first regression test"
    requirement: "REL-02"
    verification:
      - kind: unit
        ref: "crates/famp-envelope/src/envelope.rs#federation_format_ok_rejects_expiry_with_non_canonical_offset_that_lexically_misorders (RED: 0 passed/1 failed against unfixed code; GREEN: 41/41 famp-envelope --lib after fix)"
        status: pass
    human_judgment: false
  - id: D4
    description: "Silently-skipped invalid single-peer gateway route fixed to fail startup loudly"
    requirement: "REL-02"
    verification:
      - kind: integration
        ref: "crates/famp-gateway/tests/route_config_fail_closed.rs#invalid_single_peer_domain_fails_startup_instead_of_silently_dropping_route (RED: hung against unfixed code -- the buggy gateway never exits; GREEN: 6/6 route_config_fail_closed, 1.8s, no hang)"
        status: pass
    human_judgment: false
  - id: D5
    description: "Control suites green at the post-fix tree: famp-gateway --lib, famp-bus, just lint"
    requirement: "REL-02"
    verification:
      - kind: unit
        ref: "cargo test -p famp-gateway --lib (18/18), cargo test -p famp-bus (all binaries green), just lint (0 warnings/errors)"
        status: pass
    human_judgment: false

duration: ~2h (session interrupted and resumed once for a coordinator status check)
completed: 2026-07-29
status: complete
---

# Phase 12 Plan 02: REL-02 Independent Adversarial Trust-Boundary Review Summary

**A two-reviewer (self + backgrounded codex) independent adversarial pass over the shipped v1.0.0-rc.1 federation trust boundary found and fixed a real timestamp-validation defect in `federation_format_ok` plus a silently-dropped gateway route, and triaged four other genuinely real but out-of-scope findings to documented-accept with source-grounded rationale.**

## Performance

- **Duration:** ~2h (includes an independent codex background review run, ~4-5 min; a full famp-bus/famp-gateway/famp-envelope test verification pass; and a workspace-wide `just lint`, ~2 min)
- **Completed:** 2026-07-29
- **Tasks:** 2 (Task 1: run the review; Task 2: land fixes for real findings, or record no-defect outcome)
- **Files modified:** 5 (2 review-record-related test/source files in famp-gateway, 2 source files in famp-envelope, 1 new review record)

## Accomplishments

- Ran a genuinely independent adversarial review over all five REL-02 bounded surfaces (egress `from`-stamping/own-domain, ingress destination/domain validation, federation-owned-field ownership, gateway route-config fail-closed, broker `from`-binding) using two independent reviewers: this executor (source-only, no prior-verdict context) and a backgrounded `codex exec` second opinion given the same bounded brief.
- Answered all eight adversarial questions from `12-RESEARCH.md` § REL-02 with file:line citations; confirmed the reviewed surface was byte-identical to `v1.0.0-rc.1` (`git diff ba6b166..HEAD` empty) before any fix landed.
- Found and fixed a genuine trust-boundary validation defect: `SignedEnvelope::federation_format_ok` compared `expiry`/`ts` via raw lexical byte-string comparison, which silently mis-orders two format-valid-but-differently-represented RFC 3339 timestamps (concretely demonstrated: a `+01:00`-offset expiry one hour BEFORE `ts` in real UTC time lexically compared as "after" and was wrongly accepted).
- Found and fixed a config-tier defect: `famp-gateway`'s bare-positional-name + single-`--peer` fallback silently dropped a route on a `Principal`-parse failure instead of failing startup loudly, inconsistent with every other branch in the same function.
- Triaged four other genuinely real findings (`Target`/envelope-`to` decoupling, own-domain fail-open when unconfigured, per-gateway-signing-key vs. per-Principal-keyring-pin scope mismatch, egress non-durability on relay failure) to documented-accept, each with a specific, source-grounded rationale tying it to an existing named decision, an already-shipped mitigation, or an architecture-change blast radius outside this release gate's scope — none dismissed for convenience.

## Task Commits

1. **Task 2 (fix, highest priority — trust-boundary validation):** `8aa8471` — `fix(famp-envelope): reject federation ts/expiry pairs with mismatched RFC 3339 representation`
2. **Task 2 (fix, config tier):** `0d6d2dd` — `fix(famp-gateway): fail closed on an invalid --peer domain + backed-name route`
3. **Task 1 + Task 2 (review record, includes the Outcome section covering both fixes):** `0a7b4db` — `docs(12-02): REL-02 independent adversarial review of the shipped trust boundary`

**Plan metadata:** (this commit, pending — see below)

_Note: the review file's `## Findings`/`## Triage`/`## Outcome` sections were all authored in one pass once both fixes were known, so Task 1 and Task 2's documentation landed in a single commit rather than two — the underlying two fixes each have their own atomic commit._

## Files Created/Modified

- `.planning/phases/12-v1-0-0-release-gate/12-REL-02-REVIEW.md` - the full review record: reviewer independence, reviewed surface, 10 findings (F-1..F-10), 10-row triage table, outcome + control-test re-run evidence
- `crates/famp-envelope/src/envelope.rs` - `federation_format_ok` now requires canonical UTC-Z form on both `ts`/`expiry` before trusting the lexical ordering comparison; RED-first regression test added
- `crates/famp-envelope/src/timestamp.rs` - new `is_canonical_utc_form` helper (whole-second, `Z`-suffixed, exactly 20 bytes)
- `crates/famp-gateway/src/main.rs` - `build_route_map`'s single-peer fallback fails loud (`process::exit(1)`) instead of silently skipping an unparseable route
- `crates/famp-gateway/tests/route_config_fail_closed.rs` - new regression test for the route-config fix

## Decisions Made

- **Reviewer substitution for the plan's "spawn a reviewing subagent" instruction:** this executor's own toolset has no Task/Agent-dispatch tool. Ran `codex exec` (a genuinely separate model, sandboxed read-only, no network, given only the bounded brief) as the independent second reviewer instead — this satisfies the plan's actual intent (avoid rubber-stamping via self-review) even though the literal mechanism differs from a Claude subagent spawn.
- **Every codex-sourced claim was independently re-verified against cited source before any disposition** — per the coordinator's explicit instruction, an unreproducible AI-reviewer claim would have been recorded as a false positive rather than acted on. All six of codex's claimed defects reproduced exactly as cited; none were false positives.
- **Timestamp fix prioritized over the config fix**, per the coordinator's explicit blast-radius ordering (auth/trust-boundary → data integrity → config → everything else) — both reviewers independently converged on the timestamp finding, and it is a trust-boundary validation function.
- **Rejected a broader fix for the `Target`/envelope-`to` divergence** after discovering that `handle/tests.rs`'s own `audit_log_envelope` test helper hardcodes envelope `to` regardless of the `Target` used across 15+ existing test call sites — enforcing agreement at the broker level would require rewriting a large, disproportionate share of an existing shared test fixture for a property that (on closer analysis) does not cross a NEW trust boundary: it is a pre-existing, accepted property of the trust-flat local bus (v0.9-era decision), not a v1.0 federation defect.
- **Rejected mandatory own-domain-at-startup** as the fix for the own-domain fail-open finding, because it would deliberately break `e2e_shipping_surface.rs`'s own-domain-unset regression control (explicitly preserved for that purpose in Phase 11) — an architecture/deployment-contract change outside this release gate's scope, not a bounded bug fix.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed a lexical timestamp-comparison defect in `federation_format_ok`**
- **Found during:** Task 1 (independent adversarial review), confirmed by both reviewers
- **Issue:** `expiry.0 <= self.inner.ts.0` used raw byte-string comparison; `shallow_validate` allows either a `Z` suffix or a `+HH:MM`/`-HH:MM` offset independently for `ts` and `expiry`, so two format-valid timestamps in different representations can compare in an order that does not match true chronological order — an expiry actually BEFORE `ts` could be wrongly accepted as well-formed.
- **Fix:** Added `timestamp::is_canonical_utc_form` (exactly 20 bytes, whole-second, `Z`-suffixed) as a precondition on both operands before trusting the lexical comparison.
- **Files modified:** `crates/famp-envelope/src/envelope.rs`, `crates/famp-envelope/src/timestamp.rs`
- **Verification:** RED-first test failed against unfixed code (0 passed, 1 failed); GREEN after fix (41/41 `cargo test -p famp-envelope --lib`)
- **Committed in:** `8aa8471`

**2. [Rule 1 - Bug] Fixed a silently-skipped invalid single-peer gateway route**
- **Found during:** Task 1 (independent adversarial review), codex-sourced, reproduced by this executor
- **Issue:** `build_route_map`'s bare-name + single-`--peer` fallback silently dropped a route on a `Principal`-parse failure (`if let Ok(...)`), leaving a backed name with zero route while the gateway printed "ready" and kept running — inconsistent with every other branch in the same function, all of which already fail loud.
- **Fix:** Changed the silent skip to `std::process::exit(1)` with an actionable message naming both the bad domain and the affected backed name.
- **Files modified:** `crates/famp-gateway/src/main.rs`, `crates/famp-gateway/tests/route_config_fail_closed.rs`
- **Verification:** RED-first test HUNG against unfixed code (the buggy gateway never exits — itself proof of the defect); GREEN after fix (6/6 `route_config_fail_closed`, 1.8s, no hang)
- **Committed in:** `0d6d2dd`

**3. [Rule 1 - out-of-declared-scope file, justified] Touched `crates/famp-envelope`, not in the plan's `files_modified` list**
- **Found during:** Task 1
- **Issue:** The plan's `files_modified` frontmatter names only `egress.rs`/`ingress.rs`/`main.rs`/`handle.rs`, but the genuine defect (finding F-8/F-5 in the review) lives in `famp-envelope/src/envelope.rs`'s `federation_format_ok`, a Layer-0 protocol-primitive function these gateway files call into.
- **Fix:** Touched `famp-envelope` directly, since the plan's task text explicitly permits fixing "the specific source file(s) named by any `fixed` row, at the cited lines" rather than restricting to the pre-declared list.
- **Files modified:** `crates/famp-envelope/src/envelope.rs`, `crates/famp-envelope/src/timestamp.rs`
- **Verification:** Explicit confirmation in the review's `## Reviewed Surface` section that the change is zero-wire-visible (no canonical JSON, signing, or `Timestamp` serialization change).
- **Committed in:** `8aa8471`

---

**Total deviations:** 3 (2 Rule-1 bug fixes with regression tests, 1 Rule-1 scope note justifying the out-of-declared-file touch)
**Impact on plan:** Both fixes are surgical, narrowly-scoped, RED-first-tested, and confined to the exact defect found — no drive-by refactoring. Zero federation runtime logic was changed outside these two confirmed, independently-reproduced defects, honoring the phase's hard "no federation logic changes unless REL-02 surfaces a real defect" constraint.

## Issues Encountered

- The codex background review process produced a large (~447 KB / 8100+ line) reasoning-trace transcript; only the tail (final structured answer) was read to avoid context exhaustion, per the coordinator's guidance.
- The route-config RED test initially appeared to hang rather than fail cleanly, because the unfixed gateway silently drops the route, prints "ready", and keeps running forever (a long-lived server process) — this hang was itself the concrete proof of the defect, not a test-harness problem. Killed the hung subprocess and proceeded to apply the fix.
- Session was interrupted once mid-execution by a coordinator status-check message and once by a user "what's the status" message; both were answered inline without derailing the plan, and execution resumed from exactly where it left off.

## User Setup Required

None - no external service configuration required. Both fixes are pure code changes with regression coverage; no deployment or config action needed beyond the normal `cargo build`/CI cycle.

## Next Phase Readiness

- REL-02 (design review C §16 item 9, "Zed's independent source verdict is reconciled") is closed.
- The two fix commits (`8aa8471`, `0d6d2dd`) are pushed to `main` and have real CI runs (non-`.planning`/`.md`-only diffs) — their check-run status should be confirmed green before REL-03 (CI-green-at-tag-commit) or REL-05 (version bump + tag) proceed, since either of those plans will need the LATEST commit's CI status, not this plan's.
- Four genuinely real, source-grounded findings remain intentionally unfixed with named forward-pointers (own-domain fail-open → recommend `GATEWAY-SETUP.md` §4/§5 reorder + a future mandatory-own-domain hardening pass alongside `INGRESS-01`/`PEER-01`; `Target`/`to` decoupling and the per-gateway-key/per-Principal-keyring mismatch → both scoped to a future multi-agent-per-gateway design pass if that topology is ever needed; egress non-durability → a future reliability milestone for an outbox/ack state machine). None block `v1.0.0`.

---
*Phase: 12-v1-0-0-release-gate*
*Completed: 2026-07-29*

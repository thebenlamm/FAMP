---
phase: 01-spec-fork-v0-5-1
plan: 01
subsystem: spec
tags: [docs, justfile, ripgrep, bash, spec-lint]

requires:
  - phase: 00-toolchain-workspace-scaffold
    provides: "Justfile with fmt-check/lint/build/test/test-doc recipes and ci aggregate target"
provides:
  - "FAMP-v0.5.1-spec.md stub at repo root with FAMP_SPEC_VERSION constant and 18 placeholder H2 section stubs"
  - "scripts/spec-lint.sh — ripgrep-based anchor lint covering all 20 SPEC-xx requirements plus SPEC-01-FULL Δnn count"
  - "Justfile spec-lint recipe wired into ci as a mandatory gate"
affects: [01-spec-fork-v0-5-1 plans 02..06, any future spec-consuming conformance phase]

tech-stack:
  added: []
  patterns:
    - "Grep-anchor lint: stable textual anchors in FAMP-v0.5.1-spec.md checked by ripgrep, failure count = exit code"
    - "Wave 0 scaffolding convention: create the deliverable file plus its automated gate before any content lands"

key-files:
  created:
    - FAMP-v0.5.1-spec.md
    - scripts/spec-lint.sh
  modified:
    - Justfile

key-decisions:
  - "Stable anchor contract: 20 SPEC-xx anchors plus SPEC-01-FULL Δnn count are the sole automated gate for the spec fork phase (no schema, no parser)"
  - "spec-lint wired into just ci with no softening — Wave 1..3 plans must land anchors before ci is green again"
  - "Spec file preloaded with 18 placeholder H2 stubs (§4a, §7.1a..c, §6.1, §6.3, §13.1, §13.2, §9.5a, §9.6a, §9.6b, §10.3a, §11.2a, §11.5a, §12.3a, §7.3a, §8a, §3.6a) so subsequent plans Edit-in-place rather than appending"

patterns-established:
  - "Wave 0 = scaffold + automated gate, Wave N = fill anchors: the ci pipeline enforces progress without needing a human to track the anchor table"

requirements-completed: [SPEC-20]

duration: 6min
completed: 2026-04-13
---

# Phase 01 Plan 01: Wave 0 spec-fork scaffold Summary

**FAMP-v0.5.1-spec.md stub at repo root with FAMP_SPEC_VERSION = "0.5.1" constant, plus scripts/spec-lint.sh ripgrep anchor lint wired into `just ci` as a mandatory gate.**

## Performance

- **Duration:** ~6 min
- **Started:** 2026-04-13T02:00:00Z (approx)
- **Completed:** 2026-04-13T02:06:00Z (approx)
- **Tasks:** 3
- **Files modified:** 3 (2 created, 1 edited)

## Accomplishments

- Stub `FAMP-v0.5.1-spec.md` at repo root containing the normative spec-version constant block, BCP 14 / RFC 2119 conventions boilerplate, the `v0.5.1 Changelog` heading, and 18 placeholder H2 section stubs that later plan waves will overwrite with normative text.
- `scripts/spec-lint.sh` implementing the full SPEC-01..20 anchor list from `01-VALIDATION.md`, including the SPEC-17 body-schema loop (`commit`/`propose`/`deliver`/`control`/`delegate`) and the strict `SPEC-01-FULL` Δnn changelog count. Exit code equals failed-anchor count.
- `just spec-lint` recipe defined and appended to `just ci`'s dependency list. `just ci` will now fail until the spec anchors are populated by Wave 1..3 — as designed.

## Task Commits

1. **Task 1: Create stub FAMP-v0.5.1-spec.md with version constant and changelog heading** — `8a77c00` (docs)
2. **Task 2: Add scripts/spec-lint.sh with all 20 SPEC-xx grep anchors** — `b4eae4a` (chore)
3. **Task 3: Add just spec-lint recipe and wire into just ci** — `2530923` (chore)

## Files Created/Modified

- `FAMP-v0.5.1-spec.md` (created) — Stub spec file: title, subtitle, conventions box, `FAMP_SPEC_VERSION = "0.5.1"` constant block, 18 placeholder H2 section stubs, `## v0.5.1 Changelog` heading.
- `scripts/spec-lint.sh` (created, chmod +x) — Bash/ripgrep anchor lint. Prints `[PASS]`/`[FAIL]` per SPEC-xx to stderr; exit code = failed-anchor count.
- `Justfile` (modified) — Added `spec-lint` recipe invoking `bash scripts/spec-lint.sh`; appended `spec-lint` to the `ci:` dependency list.

## Anchor Status After Plan 01-01

As expected, the Wave 0 stub passes only the scaffold-level anchors and fails the 18 anchors that Wave 1..3 plans must populate.

**Passing (3 / 21):**

- `SPEC-01` — `v0.5.1 Changelog` heading present
- `SPEC-12` — incidental pass: the placeholder heading `§9.5a EXPIRED vs deliver tiebreak` happens to satisfy the `EXPIRED.{0,20}deliver` regex. Plan 04 overwrites this section with normative text; the anchor will continue to pass.
- `SPEC-20` — `FAMP_SPEC_VERSION = "0.5.1"` constant block present (requirement completed in this plan)

**Failing (18 / 21), each awaiting a later plan:**

- SPEC-02 (RFC 8785), SPEC-03 (FAMP-sig-v1), SPEC-04 (recipient anti-replay), SPEC-18 (sha256:<hex>), SPEC-19 (unpadded base64url) — Plan 02 (canonical JSON + signature encoding)
- SPEC-05 (federation_credential), SPEC-06 (card_version / min_compatible_version), SPEC-07 (±60 / 300 seconds), SPEC-08 (idempotency 128-bit) — Plan 03 (identity + freshness)
- SPEC-09 (ack disposition terminal), SPEC-10 (envelope whitelist / FSM inspects), SPEC-11 (transfer timeout tiebreak), SPEC-13 (conditional lapse), SPEC-14 (COMMITTED_PENDING_RESOLUTION), SPEC-15 (supersession round), SPEC-16 (capability snapshot commit-time) — Plan 04 (FSM fixes)
- SPEC-17 (commit/propose/deliver/control/delegate body schemas) — Plan 05 (body schemas)
- SPEC-01-FULL (≥20 `v0.5.1-Δnn` entries) — Plan 06 (changelog finalisation)

## Decisions Made

- **SPEC-01-FULL is strict from day one.** The plan's action note suggested wrapping the Δnn count in `|| true`, but explicitly instructed NOT to wrap. Kept strict per the plan and per CLAUDE.md's "never soften a check" rule.
- **Placeholder H2 stubs use section numbers matching the validation contract verbatim** (`§4a`, `§7.1a..c`, `§9.5a`, `§9.6a`, `§9.6b`, etc.) so later plans can Edit-in-place without renumbering.
- **Script prints to stderr, not stdout**, so `just spec-lint > /dev/null` still surfaces failure lines in CI logs.

## Deviations from Plan

None — plan executed exactly as written. Task 1 acceptance criterion `rg -c '^## §' ≥ 18` satisfied with exactly 18 stubs; Task 2 acceptance criteria `SPEC-20 passes on stub` satisfied; Task 3 recipe wiring verified via `just --list`, `rg '^spec-lint:' Justfile`, and `rg 'ci:.*spec-lint' Justfile`.

## Issues Encountered

None. The only potentially surprising observation is the SPEC-12 incidental pass caused by the placeholder section heading. This is not a bug — Plan 04 will replace that section's body with normative text and the anchor will remain satisfied.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **Wave 1 ready.** Every later plan in phase 01 now has a file to Edit (`FAMP-v0.5.1-spec.md`), a lint command to run (`just spec-lint`), and a ci gate to satisfy (`just ci`).
- **Known state:** `just ci` currently fails at the `spec-lint` step with exit 18. This is the intended Wave 0 state. Do not add `|| true` or `continue-on-error` to recover; the correct fix is to write the anchored content in subsequent plans.
- **Blocker note:** No blockers.

## Self-Check

All three files verified on disk and all three commits verified in git log.

- FOUND: /Users/benlamm/Workspace/FAMP/FAMP-v0.5.1-spec.md
- FOUND: /Users/benlamm/Workspace/FAMP/scripts/spec-lint.sh
- FOUND: /Users/benlamm/Workspace/FAMP/Justfile (modified)
- FOUND: commit 8a77c00 (Task 1)
- FOUND: commit b4eae4a (Task 2)
- FOUND: commit 2530923 (Task 3)

## Self-Check: PASSED

---
*Phase: 01-spec-fork-v0-5-1*
*Completed: 2026-04-13*

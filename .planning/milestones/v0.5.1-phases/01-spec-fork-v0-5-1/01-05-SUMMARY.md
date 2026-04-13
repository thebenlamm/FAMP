---
phase: 01-spec-fork-v0-5-1
plan: 05
subsystem: spec-fork
tags: [spec, body-schemas, normative, wave-3]
requires: [01-02, 01-03, 01-04]
provides:
  - "§8a Body schemas with 5 inline field-level tables"
  - "additionalProperties: false discipline for all body classes"
  - "Cross-link transfer_commit_race → §12.3a"
  - "Cross-link capability_snapshot → §11.2a"
tech-stack:
  added: []
  patterns: [deny_unknown_fields, RFC-8785-numeric-guard]
key-files:
  created:
    - .planning/phases/01-spec-fork-v0-5-1/01-05-SUMMARY.md
    - .planning/phases/01-spec-fork-v0-5-1/deferred-items.md
  modified:
    - FAMP-v0.5.1-spec.md
decisions: [D-25, D-26, D-27]
requirements-completed: [SPEC-17]
metrics:
  duration: ~20min
  tasks: 3
  files_modified: 1
  completed: 2026-04-12
---

# Phase 1 Plan 05: §8a Body Schemas Summary

Populated §8a with inline normative body schemas for all five new message
classes (`propose`, `commit`, `deliver`, `control`, `delegate`) as
field-per-line tables with `additionalProperties: false` discipline — each
schema maps one-to-one onto a future `serde` struct with
`deny_unknown_fields` for Phase 3 (`famp-envelope`).

## What Was Built

- **§8a intro paragraph** — states `additionalProperties: false` rule,
  prohibits body-level extensions (envelope-level `extensions` map only),
  specifies 2^53 numeric-as-string guard per §4a / RFC 8785 §6.
- **§8a.1 `propose` body** — 17-field table with bounds sub-fields,
  delegation_permissions, conditions, modifications courtesy field.
- **§8a.2 `commit` body** — 10-field table including `scope_subset`
  (FSM-inspected per §7.3a) and `capability_snapshot` (bound to
  `card_version` per §11.2a). Cross-reference paragraph after table.
- **§8a.3 `deliver` body** — 7-field table: `interim` (FSM flag),
  `artifacts` (sha256:<hex> per §3.6a), `error_detail` (required iff
  terminal_status=failed), `provenance` (required on terminal).
- **§8a.4 `control` body** — 5-field table with `target` enum expanded
  to include `transfer_commit_race` (introduced by §12.3a) and
  `disposition` values cross-linked to §9.6b.
- **§8a.5 `delegate` body** — 8-field table: `form` enum, `commitment_ref`,
  `delegation_ceiling`, `transfer_deadline` (required iff form=transfer,
  cross-linked to §12.3a).
- **Closing note** — normative validation directive for
  `deny_unknown_fields` and extension-name-reuse prohibition.
- **Changelog Δ24** — consolidated entry covering all five schemas.

## Cross-Task Wiring Verification

| Link | From | To | Anchor |
|---|---|---|---|
| capability_snapshot | §8a.2 `commit` body | §11.2a / §6.3 | `capability_snapshot` field + prose cross-ref |
| scope_subset | §8a.2 `commit` body | §7.3a FSM whitelist | "FSM-inspected per §7.3a" |
| transfer_commit_race | §8a.4 `control` body | §12.3a tiebreak | enum value + prose cross-ref |
| condition_failed | §8a.4 `control` body | §9.6b conditional lapse | disposition enum + prose cross-ref |
| transfer_deadline | §8a.5 `delegate` body | §12.3a tiebreak | REQUIRED-iff-transfer constraint |

## Tasks

| Task | Name | Commit | Status |
|---|---|---|---|
| 1 | §8a intro + propose + commit body schemas | f2b16d6 | done |
| 2 | deliver + control body schemas | 8db1c8c | done |
| 3 | delegate body + Δ24 changelog | a426cc5 | done |

## Verification

- `rg -q '`propose` body'` — PASS
- `rg -q '`commit` body'` — PASS
- `rg -q '`deliver` body'` — PASS
- `rg -q '`control` body'` — PASS
- `rg -q '`delegate` body'` — PASS
- `rg -q 'additionalProperties: false'` — PASS
- `rg -q 'capability_snapshot'` — PASS
- `rg -q 'transfer_commit_race'` — PASS
- `rg -q 'v0\.5\.1-Δ24'` — PASS
- `just spec-lint` SPEC-17 check — **PASS**
- Overall `just spec-lint`: 20 passed, 1 failed (**SPEC-01-FULL, pre-existing, out-of-scope** — see Deferred Issues)

## Deviations from Plan

None — plan executed exactly as written. All three tasks landed with
anchors matching verification commands on first attempt.

## Deferred Issues

- **SPEC-01-FULL lint recipe broken (pre-existing):** `just spec-lint`
  SPEC-01-FULL step reports "found 0" despite 23 `v0.5.1-Δnn` entries now
  present in `FAMP-v0.5.1-spec.md`. Confirmed pre-existing via
  `git stash && just spec-lint` on parent commit — same failure mode
  before Plan 05 changes. Root cause is almost certainly shell/grep
  escaping of the `Δ` (U+0394) character inside the Justfile recipe. Not
  in Plan 05 scope. Logged to
  `.planning/phases/01-spec-fork-v0-5-1/deferred-items.md`. SPEC-01 anchor
  check (`v0.5.1 Changelog`) already passes; only the numeric
  minimum-count variant is broken.

## Known Stubs

None — every body schema is fully populated; no placeholders remain in §8a.

## Self-Check: PASSED

- `FAMP-v0.5.1-spec.md` modified — FOUND
- Commit `f2b16d6` — FOUND
- Commit `8db1c8c` — FOUND
- Commit `a426cc5` — FOUND
- All five `<class> body` anchors — FOUND
- `additionalProperties: false` literal — FOUND
- `v0.5.1-Δ24` changelog entry — FOUND

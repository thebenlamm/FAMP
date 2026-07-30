---
phase: quick-260729-ur8
verified: 2026-07-29T00:00:00Z
status: passed
score: 8/8 must-haves verified
behavior_unverified: 0
overrides_applied: 0
re_verification: null
---

# Quick Task 260729-ur8: CI-gated slash-command asset schema gate — Verification Report

**Task Goal:** Add a CI-gated test that validates FAMP's installed slash-command assets
against the real MCP tool schemas, closing the detection gap that let the `to`-shape bug and
the stale-8-tool-count bug ship.

**Verified:** 2026-07-29
**Status:** passed

## Goal Achievement

### Observable Truths (plan `must_haves.truths`)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Clean tree reports `8 passed; 0 failed` | ✓ VERIFIED | Orchestrator + re-confirmed by me after every falsification/restore cycle below |
| 2 | Broken `to` bullet makes `slash_command_assets_prescribe_only_real_argument_keys` FAIL, naming the key/tool | ✓ VERIFIED | Orchestrator's falsification #1 |
| 3 | `test_famp_who_allowed_tools_lists_only_famp_peers` passes in both broken and restored states | ✓ VERIFIED | Orchestrator's falsification #1 (`7 passed` count includes it) |
| 4 | Every `mcp__famp__famp_*` tool an asset names resolves to a registry descriptor AND a `dispatch_tool` arm | ✓ VERIFIED | Read `slash_command_assets_reference_only_dispatchable_mcp_tools` (lines 250-271): asserts membership in `tool_names()` then `SERVER_RS.contains("\"{tool}\" =>")` |
| 5 | Stale numeric tool-count claim fails the build | ✓ VERIFIED | Orchestrator's falsification #2 |
| 6 | Adding an 8th unregistered asset file fails `slash_command_asset_harness_is_not_vacuous` | ✓ VERIFIED (re-run by me) | See "Anti-vacuity" section below — reproduced independently |
| 7 | Both commit paths reach the gate (`.rs` via ci.yml `--workspace`, `.md` via new asset-gate.yml) | ✓ VERIFIED | Orchestrator confirmed asset-gate.yml YAML shape + paths; ci.yml `--workspace` auto-discovery is standard cargo/nextest behavior, uncontested |
| 8 | `ci.yml` byte-identical to HEAD | ✓ VERIFIED | Orchestrator: `git diff --name-only 2d0adbe..HEAD` shows only the two intended files |

**Score:** 8/8 truths verified, 0 behavior-unverified.

### Anti-Vacuity Analysis (adversarial, per request)

Read `registry()`, `referenced_tools()`, `prescribed_keys()`, `properties_of()`, `required_of()`
line by line (crates/famp/tests/slash_command_assets.rs:74-328). Findings:

1. **`registry()` returning a silent subset is impossible without a loud panic.** Every anchor
   lookup (`fn tool_descriptors()...`, `serde_json::json!(`, the `\n    ])\n}` terminator) uses
   `.expect(...)` naming the anchor. If the slice were to shrink because a formatting change
   moved a boundary, the resulting count would not silently pass — `slash_command_asset_harness_is_not_vacuous`
   pins `tool_names(&registry()).len() == 12` as a hard equality, not a lower bound. I re-ran
   this reasoning empirically (see below) by removing a registration rather than mutating
   `server.rs`, since the plan's own constraint forbids touching `src/`.

2. **`referenced_tools()` returning an empty set for an asset is caught, not silently OK.**
   `slash_command_asset_harness_is_not_vacuous` asserts `ref_count == 1` for every asset in
   `ASSETS` (line 241-246) — both 0 and >1 fail loudly. Verified by reading the loop; no asset
   is exempted.

3. **The one genuinely vacuum-adjacent spot: `prescribed_keys()` returning an empty set is a
   legitimate silent 0-iteration pass**, not a bug — `slash_command_assets_prescribe_only_real_argument_keys`
   and the required-key gate both iterate over the prescribed set, so an asset with zero
   prescribed keys contributes zero assertions to those two tests. This is BY DESIGN (GT-4:
   famp-register.md expresses `identity` as prose) but it is real, not hidden — see the
   Coverage Honesty section below for exactly which assets this affects.

4. **I independently reproduced the harness's core anti-vacuity claim** (item 6 above) rather
   than trusting the plan's prose: copied `famp-who.md` to
   `assets/slash_commands/famp-zzz-test-unregistered.md` (an 8th `.md` file, NOT added to the
   `ASSETS` const) and ran the suite:
   ```
   test slash_command_asset_harness_is_not_vacuous ... FAILED
   thread '...' panicked at .../slash_command_assets.rs:221:5:
   assertion `left == right` failed: assets/slash_commands/ holds 8 .md files but the ASSETS
   const in slash_command_assets.rs lists 7; register the new/removed asset in ASSETS or this
   harness silently stops covering it
   test result: FAILED. 7 passed; 1 failed
   ```
   File removed, `git status --porcelain` empty, re-run confirmed `8 passed; 0 failed`.

5. **I also independently falsified the required-key gate** (not covered by the orchestrator's
   pre-verification, which only exercised the argument-key gate). Removed famp-send.md's
   `` - `mode`: `"open"` `` bullet:
   ```
   test slash_command_assets_prescribe_every_required_argument_key ... FAILED
   panicked: famp-send.md prescribes keys {"body", "peer", "title"} for famp_send but omits
   required key `mode`
   test result: FAILED. 7 passed; 1 failed
   ```
   Reverted via `git checkout --`; `git status --porcelain` empty; re-run confirmed
   `8 passed; 0 failed`.

**Conclusion:** No test in this suite is vacuum-passable in the sense the plan claims to
prevent. The one legitimate "trivially satisfied" case (zero prescribed keys) is disclosed
below, not hidden.

### Registry-Size Pin (item 2 of the focus list)

- `grep -nE '[0-9]+ tools' <module-doc block>` finds **nothing** (exit code 1) — confirmed by
  running the exact plan-specified command myself.
- The doc block instead says: "The registry size is pinned in code by
  `slash_command_asset_harness_is_not_vacuous` rather than restated in this prose comment" and
  cross-references `tool_descriptors_has_exactly_twelve_named_tools` in `server.rs`.
- Confirmed `tool_descriptors_has_exactly_twelve_named_tools` (server.rs:489-514) is a real
  test: it calls the actual `tool_descriptors()` function, asserts `names.len() == 12`, and
  additionally asserts the exact expected name list — this is not an unrelated hardcode, it's
  the paired unit-level pin the integration test cross-references.
- The in-test pin (`slash_command_assets.rs:231`, `name_count == 12`) is derived from
  `tool_names(&registry())` — i.e., from the actual text parse of `server.rs`, not an
  independent literal disconnected from the parse. Two pins, two independent code paths
  (parse-from-text vs. call-the-function), both currently agree at 12. No stale/duplicate
  numeric literal was reintroduced anywhere else in the file (only these two `12`s exist, plus
  the falsification-derived `expected` count comparisons which are runtime-derived, not
  literals).

### `slash_command_asset_harness_is_not_vacuous` — path resolution (item 3)

`asset_dir_md_count()` uses `concat!(env!("CARGO_MANIFEST_DIR"), "/assets/slash_commands")` —
`CARGO_MANIFEST_DIR` resolves at **compile time** to the `crates/famp` directory, independent
of the process's cwd at `cargo test` runtime. This is the correct, robust pattern (not
`std::env::current_dir()`, which would be cwd-dependent and fragile under `cargo test` invoked
from different directories). Empirically reproduced working as designed in the anti-vacuity
falsification above — the directory read correctly detected 8 files vs. 7 registered.

### Coverage Honesty — per-asset breakdown (item 4, stated plainly)

| Asset | Tool | Key-shape checked? | Required-key checked? | Count-claim checked? | Coverage |
|-------|------|---------------------|------------------------|------------------------|----------|
| famp-channel.md | famp_send | Yes (body, channel, mode, title) | Yes (mode present) | No | Full |
| famp-send.md | famp_send | Yes (body, mode, peer, title) | Yes (mode present) | No | Full |
| famp-join.md | famp_join | Yes (channel) | Yes (no required keys on famp_join) | No | Full |
| famp-leave.md | famp_leave | Yes (channel) | Yes (no required keys) | No | Full |
| famp-inbox.md | famp_inbox | **No — 0 prescribed keys, 0 assertions run** | **No — skipped (empty guard)** | No | **Weak** (tool-existence + dispatch-arm + single-tool-reference only) |
| famp-register.md | famp_register | **No — 0 prescribed keys, 0 assertions run** (required `identity` is prose, GT-4) | **No — skipped** | No | **Weak** (same as above) |
| famp-who.md | famp_peers | No prescribed keys (by design — famp_peers has no properties) | N/A (no required keys) | Yes (2 claims, both checked against 12) | Full for its applicable checks |

**Stated plainly:** famp-inbox.md and famp-register.md receive no argument-shape validation at
all beyond "the tool exists, is dispatchable, and exactly one tool is named." This is not a gap
introduced by this task — neither asset prescribes any backtick-quoted key in the first place
(register expresses its one argument as free-text prose), so there is nothing for the key-shape
grammar to extract. This matches the plan's own GT-3/GT-4 ground truth and is not overstated in
SUMMARY.md.

### Residual Drift Risk (item 5, non-blocking follow-up)

- **Two-location manual sync on a registry-size change:** adding a 13th tool requires updating
  the literal `12` in both `slash_command_asset_harness_is_not_vacuous` (integration test) and
  `tool_descriptors_has_exactly_twelve_named_tools` (unit test in server.rs) — both fail loudly
  and each error message points at the other, but nothing forces a single source of truth. Low
  risk given the explicit cross-referencing; worth a follow-up only if registry churn becomes
  frequent.
- **`server.rs` reformatting risk is fully mitigated, not residual:** every anchor lookup in
  `registry()` panics with a named anchor on any structural change (fn signature move, macro
  call reformatting, terminator indentation change) rather than silently matching a wrong slice.
- **New tool-count phrasing risk:** the count-claim scanner keys on literal substrings `-tool`
  and ` tools` with a trailing-digit-run heuristic. A future asset phrasing like "twelve tools"
  (spelled out) or "12-tool-surface" (no space before "surface") would not be caught by either
  marker and would silently produce zero claims for that asset — mitigated in aggregate by the
  `claims_found >= 1` floor across all assets combined, but not per-asset. Worth noting as a
  narrow blind spot, not a blocker.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/famp/tests/slash_command_assets.rs` | Extended in place, 3 original tests untouched, 5 new tests | ✓ VERIFIED | Read in full; original 3 tests byte-identical in position/content; 5 new tests present and independently falsified above |
| `.github/workflows/asset-gate.yml` | New, additive, covers `.md`-only commit path | ✓ VERIFIED | Read in full; single job, plain `cargo test`, 3-entry self-gating `paths` list, non-colliding `concurrency.group` |

### Anti-Patterns Found

None. No `TODO`/`FIXME`/`XXX`/`HACK`/`PLACEHOLDER` markers in either delivered file. No stub
patterns, no empty implementations, no swallowed panics (every `.expect()` carries a
diagnostic, in line with the crate's `expect_used` allow + message convention).

### Human Verification Required

None. All must-haves are mechanically checkable and were checked, either by the orchestrator's
pre-verification or by my own independent re-execution of two falsifications the orchestrator
had not run (the not-vacuous harness test and the required-key gate).

## Gaps Summary

None. This task delivers exactly what it claims: a registry-derived, text-parsed gate over all
7 slash-command assets that is not vacuum-passable on its core "does the harness even run"
axis, closes both historical bug classes, covers both CI commit-path shapes, and leaves ci.yml
and all asset files untouched. Coverage is honestly uneven across assets (2 of 7 get no
key-shape check) but that unevenness is inherent to those assets' content, not an
implementation shortfall, and is disclosed above rather than papered over.

---

_Verified: 2026-07-29_
_Verifier: Claude (gsd-verifier)_

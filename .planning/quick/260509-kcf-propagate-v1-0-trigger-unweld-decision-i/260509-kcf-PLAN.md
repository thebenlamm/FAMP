---
phase: 260509-kcf
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - .planning/MILESTONES.md
  - .planning/STATE.md
  - ARCHITECTURE.md
  - .planning/WRAP-V0-5-1-PLAN.md
  - .planning/seeds/SEED-001-serde-jcs-conformance-gate.md
  - /Users/benlamm/.claude/projects/-Users-benlamm-Workspace-FAMP/memory/project_v10_trigger.md
  - /Users/benlamm/.claude/projects/-Users-benlamm-Workspace-FAMP/memory/MEMORY.md
autonomous: true
requirements:
  - DOC-PROPAGATE-V1-UNWELD
must_haves:
  truths:
    - "MILESTONES.md v0.9 close blurb names two gates (Gate A / Gate B) and points to the spec — no 4-week clock"
    - "STATE.md no longer references the welded trigger or 4-week clock; v1.0-gated dormant seeds are tagged Gate A or Gate B"
    - "ARCHITECTURE.md v1.0 trigger language matches the two-gate framing and points to the spec"
    - "WRAP-V0-5-1-PLAN.md DEFERRED banner says vector pack ships at Gate B (2nd implementer commits to interop), not at the welded trigger"
    - "SEED-001 references Gate B (not the welded trigger), if it referenced the welded trigger"
    - "MEMORY entry project_v10_trigger.md is rewritten as superseded with a pointer to the spec"
    - "MEMORY.md index one-liner for v10_trigger reflects supersession"
    - "All edits land in one atomic commit"
  artifacts:
    - path: ".planning/MILESTONES.md"
      provides: "Two-gate framing in v0.9 close blurb"
      contains: "Gate A"
    - path: "ARCHITECTURE.md"
      provides: "Updated v1.0 trigger reference"
      contains: "Gate A"
    - path: ".planning/WRAP-V0-5-1-PLAN.md"
      provides: "Vector-pack ship condition rephrased to Gate B"
      contains: "Gate B"
    - path: "/Users/benlamm/.claude/projects/-Users-benlamm-Workspace-FAMP/memory/project_v10_trigger.md"
      provides: "Superseded MEMORY entry with spec pointer"
      contains: "superseded"
  key_links:
    - from: ".planning/MILESTONES.md"
      to: "docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md"
      via: "relative link in v0.9 close blurb"
      pattern: "2026-05-09-v1-trigger-unweld-design"
    - from: "ARCHITECTURE.md"
      to: "docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md"
      via: "relative link in v1.0 section"
      pattern: "2026-05-09-v1-trigger-unweld-design"
    - from: "/Users/benlamm/.claude/projects/-Users-benlamm-Workspace-FAMP/memory/project_v10_trigger.md"
      to: "docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md"
      via: "supersession pointer"
      pattern: "2026-05-09-v1-trigger-unweld-design"
---

<objective>
Propagate the approved v1.0-trigger-unweld decision (spec at
`docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`, committed
at `c42c5fc`) into the project's authoritative documentation. The single
fused trigger ("Sofer-from-a-different-machine + 4-week clock + vector pack
ships at the same event") is replaced everywhere by:

- **Gate A — Gateway gate:** Ben sustains symmetric cross-machine FAMP use
  for ~2 weeks → unlocks `famp-gateway`, `_deferred_v1/` reactivation,
  `v1.0.0` tag.
- **Gate B — Conformance gate:** a 2nd implementer commits to interop →
  unlocks the conformance vector pack at whatever release tag is current.
- **The 4-week clock is retired.** Both gates are event-driven.

Purpose: keep authoritative docs honest about how v1.0 actually ships, so
future-Ben (and any reader of MILESTONES.md / ARCHITECTURE.md / STATE.md)
finds the two-gate framing instead of the obsolete welded trigger.

Output: one atomic commit touching the 6 files listed in the spec's
"Documentation churn" section, plus the two MEMORY files (entry + index),
with each edit pointing back to the spec as authoritative source.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
# Authoritative source for every rewrite — honor verbatim
@docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md

# Files being rewritten (current state)
@.planning/MILESTONES.md
@.planning/STATE.md
@ARCHITECTURE.md
@.planning/WRAP-V0-5-1-PLAN.md
@.planning/seeds/SEED-001-serde-jcs-conformance-gate.md

# MEMORY files — note the absolute paths (live outside the repo)
# /Users/benlamm/.claude/projects/-Users-benlamm-Workspace-FAMP/memory/project_v10_trigger.md
# /Users/benlamm/.claude/projects/-Users-benlamm-Workspace-FAMP/memory/MEMORY.md

# Project context
@CLAUDE.md
@.planning/STATE.md
</context>

<tasks>

<task type="auto">
  <name>Task 1: Propagate two-gate framing across all 7 docs in one atomic commit</name>
  <files>
    .planning/MILESTONES.md,
    .planning/STATE.md,
    ARCHITECTURE.md,
    .planning/WRAP-V0-5-1-PLAN.md,
    .planning/seeds/SEED-001-serde-jcs-conformance-gate.md,
    /Users/benlamm/.claude/projects/-Users-benlamm-Workspace-FAMP/memory/project_v10_trigger.md,
    /Users/benlamm/.claude/projects/-Users-benlamm-Workspace-FAMP/memory/MEMORY.md
  </files>
  <action>
The spec at `docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`
is the authoritative source for every rewrite below. **Do not re-derive
its content.** Quote or link to it; do not paraphrase the gate definitions
loosely. Do not modify the spec file itself — it is locked.

Per Ben's request, all edits land in **ONE atomic commit**. Stage every
file, then commit once at the end. No code, no behavior changes, no
touching `_deferred_v1/` tests, no Rust crate edits.

**Edit 1 — `.planning/MILESTONES.md` (v0.9 close blurb)**

Locate the line currently at line 24:

> **v1.0 trigger named:** Sofer (or named equivalent) runs FAMP from a different machine and exchanges a signed envelope. 4-week clock starts at v0.9.0; if untriggered, federation framing is reconsidered. Conformance vector pack ships at the same trigger.

Replace it with a two-gate paragraph that:
- States the welded trigger has been unwelded into Gate A and Gate B.
- Names Gate A (Gateway gate): Ben's symmetric cross-machine use sustained
  ~2 weeks → unlocks `famp-gateway`, reactivates `_deferred_v1/`, tags
  `v1.0.0`.
- Names Gate B (Conformance gate): 2nd implementer commits to interop →
  unlocks the vector pack at whatever release tag is current.
- Notes the 4-week clock has been retired (both gates are event-driven).
- Links to the spec at `docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`
  as the authority. Use a relative path from `.planning/MILESTONES.md`:
  `../docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`.

Keep the surrounding v0.9 blurb prose (lines 1-22, 26-28) untouched.
The "Audit:" line and "Known deferred items at close:" line stay as-is.

**Edit 2 — `.planning/STATE.md`**

Two surgical edits:

1. **Line 19**, currently reads:
   > **Last Updated:** 2026-05-06 — ... Next milestone is v1.0 Federation Profile, trigger-gated (Sofer-from-different-machine; 4-week clock 2026-05-04 → 2026-06-01).

   Rewrite the trailing clause: replace "trigger-gated (Sofer-from-different-machine;
   4-week clock 2026-05-04 → 2026-06-01)" with two-gate language pointing
   to the spec — e.g., "gated on two independent ship gates (Gate A:
   Ben's symmetric cross-machine use; Gate B: 2nd implementer commits to
   interop) per `docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`;
   4-week clock retired."

2. **Line 23** (Project Reference section), currently reads:
   > See: .planning/PROJECT.md — ... v1.0 Federation Profile is the next planned milestone but is trigger-gated; do not run `/gsd-new-milestone v1.0` until Sofer (or named equivalent) runs FAMP from a different machine and exchanges a signed envelope.

   Update the trigger condition: replace the trailing "trigger-gated; do
   not run ... signed envelope" sentence with two-gate phrasing — Gate A
   (Ben's sustained symmetric cross-machine use) unlocks the gateway plan;
   Gate B (2nd implementer commits to interop) unlocks the vector pack.
   Both event-driven, no clock. Point to the spec.

3. **Deferred Items table (lines 119-120)** — re-tag the 2 v1.0-gated
   dormant seeds. Currently:
   ```
   | seed | SEED-001-serde-jcs-conformance-gate | dormant |
   | seed | SEED-002-harness-adapter-push-notifications | dormant |
   ```
   Per the spec, SEED-001 (vector pack) is **Gate B**. SEED-002
   (push-notification harness) — its trigger is whichever gate aligns; if
   the seed body references the welded trigger, tag it Gate A or Gate B
   per its content. If unclear after reading the seed file, leave SEED-002
   tagged "dormant" and note "(gate assignment deferred — re-read seed
   when surfaced)" in the Notes paragraph at line 123. SEED-001 is
   unambiguously Gate B (vector-pack interop with 2nd implementer).

   Update the Notes paragraph at line 123 to reflect the re-tagging
   (currently mentions "Both seeds explicitly v1.0-gated by design
   (SEED-001 = vector pack interop, SEED-002 = push-notification
   harness)"). Keep the substance; refine the gate labels.

**Edit 3 — `ARCHITECTURE.md`**

Locate the v1.0 trigger reference. The current "v0.9 — local-first bus"
section (lines 38-79) and the "When working in the codebase" section
(lines 93-103) do not contain a hard "v1.0 trigger" sentence in the form
the spec retires. **Search the file with grep first**: `grep -n "trigger\|4-week\|Sofer-from\|v1.0 readiness" ARCHITECTURE.md`.

If no match: confirm in the commit message that ARCHITECTURE.md held no
trigger reference at HEAD, and skip this file's edit. (CLAUDE.md project
context describes the trigger; ARCHITECTURE.md does not currently appear
to.) The layered model itself is unchanged either way.

If a match is found: rewrite the trigger sentence to two-gate framing
and link to the spec via relative path
`docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`.

**Edit 4 — `.planning/WRAP-V0-5-1-PLAN.md`**

DEFERRED banner currently at lines 3-13 says (line 5):
> ...the vector pack defers to v1.0 alongside the federation gateway and ships when a named second implementer commits to interop (likely Sofer from a different machine — see PROJECT.md v1.0 trigger).

Rephrase: the vector-pack ship condition is now **Gate B** explicitly:
"...ships at **Gate B** (2nd implementer commits to interop) per
`docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`. Note
that Gate B is independent of Gate A (gateway shipping) — the welded
trigger that bundled them has been retired."

Drop the "likely Sofer from a different machine — see PROJECT.md v1.0
trigger" parenthetical (it points to the obsolete welded framing).
Replace with: "Sofer remains a candidate, but Gate B fires for any 2nd
implementer." The rest of the banner (lines 7-13) stays unchanged.

**Edit 5 — `.planning/seeds/SEED-001-serde-jcs-conformance-gate.md`**

Read the file first. Per the spec's "Documentation churn" line:
> `.planning/seeds/SEED-001-serde-jcs-conformance-gate.md` — if it references the welded trigger, update to Gate B.

The current SEED-001 (read above) does NOT contain a "welded trigger" or
"4-week clock" reference; its `trigger_when` is "start of Phase 2
(Canonical + Crypto Foundations)" — that's the seed's *original* trigger
about when to surface, unrelated to the v1.0 gating. **No edit needed
unless `grep -n "v1.0\|welded\|Sofer-from\|4-week" .planning/seeds/SEED-001-serde-jcs-conformance-gate.md`
finds something.** If grep matches, update those references to "Gate B"
per the spec. Otherwise, skip this file's edit and note in the commit
message that SEED-001 had no welded-trigger reference at HEAD.

**Edit 6 — `/Users/benlamm/.claude/projects/-Users-benlamm-Workspace-FAMP/memory/project_v10_trigger.md`**

Rewrite the entire file body as superseded. Keep the YAML frontmatter
(`name`, `description`, `type`, `originSessionId`) but update `description`
to indicate supersession. Replace the body with a short note:

- "**SUPERSEDED 2026-05-09 by `docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`**."
- One-paragraph summary of what changed: welded trigger split into Gate A
  (Ben's symmetric cross-machine use → gateway + `v1.0.0`) and Gate B
  (2nd implementer commits to interop → vector pack at whatever release
  tag is current). 4-week clock retired.
- Pointer to spec for canonical text.
- Preserve the original frontmatter `originSessionId` and `name` fields
  so the entry remains traceable; update `description` to reflect
  supersession.

**Edit 7 — `/Users/benlamm/.claude/projects/-Users-benlamm-Workspace-FAMP/memory/MEMORY.md`**

Update the one-liner at line 5:
```
- [v1.0 readiness trigger named](project_v10_trigger.md) — Sofer-from-different-machine; 4-week clock starts at v0.9.0; if untriggered federation framing reconsidered
```

Replace with:
```
- [v1.0 readiness — two-gate framing (SUPERSEDES "v1.0 trigger named")](project_v10_trigger.md) — Gate A: Ben's symmetric cross-machine use → gateway + v1.0.0; Gate B: 2nd implementer commits to interop → vector pack; 4-week clock retired (per docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md)
```

**Commit:**

After all edits land, stage and commit in a single atomic commit:

```
git add .planning/MILESTONES.md .planning/STATE.md ARCHITECTURE.md .planning/WRAP-V0-5-1-PLAN.md .planning/seeds/SEED-001-serde-jcs-conformance-gate.md
git commit -m "docs: propagate v1.0 trigger unweld (two-gate framing)

Replace the welded v1.0 trigger ('Sofer-from-different-machine + 4-week
clock + vector pack at same event') with two independent ship gates per
docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md (committed
at c42c5fc):

- Gate A (Gateway): Ben's symmetric cross-machine use sustained ~2 weeks
  unlocks famp-gateway, _deferred_v1/ reactivation, v1.0.0 tag.
- Gate B (Conformance): 2nd implementer commits to interop unlocks the
  vector pack at whatever release tag is current.
- 4-week clock retired (both gates event-driven).

Files touched:
- .planning/MILESTONES.md (v0.9 close blurb)
- .planning/STATE.md (Last Updated line, Project Reference, deferred-seeds tags)
- ARCHITECTURE.md (v1.0 trigger reference; <noted if grep found no match>)
- .planning/WRAP-V0-5-1-PLAN.md (DEFERRED banner: vector pack ships at Gate B)
- .planning/seeds/SEED-001-serde-jcs-conformance-gate.md (<noted if grep found no welded-trigger ref>)

MEMORY edits (outside repo, separate from this commit):
- ~/.claude/projects/-Users-benlamm-Workspace-FAMP/memory/project_v10_trigger.md (superseded)
- ~/.claude/projects/-Users-benlamm-Workspace-FAMP/memory/MEMORY.md (one-liner updated)

No code changes. No behavior changes."
```

The MEMORY files live outside the repo — edit them in the same task but
they don't go into the git commit. (Mention them in the commit body for
traceability.) Use the Edit/Write tools for those files normally.

Constraints:
- Do not modify the spec file itself.
- Do not touch any Rust crates, `_deferred_v1/` tests, or any code.
- Do not paraphrase gate definitions — link to the spec.
- One commit. No batching workaround that produces multiple commits.
  </action>
  <verify>
    <automated>
# 1. Spec file untouched
test -z "$(git diff HEAD docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md)" || (echo "FAIL: spec file modified"; exit 1)

# 2. Welded trigger language removed from authoritative docs
! grep -n "4-week clock" .planning/MILESTONES.md .planning/STATE.md ARCHITECTURE.md .planning/WRAP-V0-5-1-PLAN.md 2>/dev/null | grep -v -i "retired\|no longer\|removed" || (echo "FAIL: welded trigger language still present"; exit 1)

# 3. Two-gate language present in MILESTONES.md and STATE.md
grep -q "Gate A" .planning/MILESTONES.md || (echo "FAIL: MILESTONES.md missing Gate A"; exit 1)
grep -q "Gate B" .planning/MILESTONES.md || (echo "FAIL: MILESTONES.md missing Gate B"; exit 1)
grep -q "Gate A\|Gate B" .planning/STATE.md || (echo "FAIL: STATE.md missing two-gate language"; exit 1)

# 4. Spec link present in MILESTONES.md and WRAP plan
grep -q "2026-05-09-v1-trigger-unweld-design" .planning/MILESTONES.md || (echo "FAIL: MILESTONES.md missing spec link"; exit 1)
grep -q "2026-05-09-v1-trigger-unweld-design\|Gate B" .planning/WRAP-V0-5-1-PLAN.md || (echo "FAIL: WRAP plan missing Gate B / spec link"; exit 1)

# 5. MEMORY entry rewritten as superseded
grep -qi "supersede" /Users/benlamm/.claude/projects/-Users-benlamm-Workspace-FAMP/memory/project_v10_trigger.md || (echo "FAIL: MEMORY entry not marked superseded"; exit 1)
grep -q "two-gate\|Gate A\|Gate B\|SUPERSEDES" /Users/benlamm/.claude/projects/-Users-benlamm-Workspace-FAMP/memory/MEMORY.md || (echo "FAIL: MEMORY.md index not updated"; exit 1)

# 6. Single atomic commit landed (HEAD commit message references the unweld)
git log -1 --pretty=%B | grep -qi "trigger unweld\|two-gate" || (echo "FAIL: HEAD commit not the unweld propagation commit"; exit 1)

# 7. No code touched
git diff HEAD~1 HEAD --name-only | grep -E '\.rs$|^crates/|^Cargo\.|_deferred_v1' && (echo "FAIL: code files in commit"; exit 1) || true

echo "PASS"
    </automated>
  </verify>
  <done>
- All 7 files reflect two-gate framing per the spec.
- Welded-trigger / 4-week-clock language is gone (or only present in
  retirement context).
- Each authoritative doc links to
  `docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`.
- MEMORY entry `project_v10_trigger.md` is marked superseded; MEMORY.md
  index one-liner reflects supersession.
- One atomic git commit lands the in-repo edits; commit message
  references the unweld and lists each file touched (with notes for
  any file where grep found no welded-trigger reference at HEAD).
- Spec file unchanged. No code changes.
  </done>
</task>

</tasks>

<verification>
Run the automated verify block in Task 1. It checks:
- Spec file untouched
- Welded trigger language scrubbed from authoritative docs
- Two-gate language present where required
- Spec link present in MILESTONES.md and WRAP plan
- MEMORY entry rewritten as superseded
- Single atomic commit landed
- No code in the diff
</verification>

<success_criteria>
- `git log -1` shows one commit titled "docs: propagate v1.0 trigger unweld (two-gate framing)" (or close).
- A reader of `.planning/MILESTONES.md` v0.9 blurb sees the two-gate framing
  and can click through to the spec.
- A reader of `.planning/STATE.md` does not encounter "4-week clock" or
  "Sofer-from-different-machine" as the gating mechanism.
- A reader of `WRAP-V0-5-1-PLAN.md` DEFERRED banner sees "Gate B" as the
  vector-pack ship condition.
- A future MEMORY query for the v1.0 trigger surfaces the superseded entry
  with a pointer to the spec.
- Spec file is byte-identical to HEAD before this plan ran.
- No `.rs`, `Cargo.toml`, or `_deferred_v1/` files in the commit diff.
</success_criteria>

<output>
After completion, create
`.planning/quick/260509-kcf-propagate-v1-0-trigger-unweld-decision-i/260509-kcf-SUMMARY.md`
following the standard summary template, noting:
- Files touched (and any files skipped because grep found no welded-trigger
  reference at HEAD).
- Commit hash.
- The MEMORY edits (which live outside the repo and outside the commit).
</output>

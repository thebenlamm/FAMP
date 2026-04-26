---
quick: 260425-sl0
slug: t3-x-file-three-backlog-items-999-3-999-
type: docs
status: Verified
date-completed: 2026-04-25
key-files:
  modified:
    - .planning/ROADMAP.md
commits:
  filing: 6bce7e2
---

# Quick Task 260425-sl0: T3.x — File Three Backlog Items — Summary

## One-liner

Filed three Tier-3 backlog items from today's pressure test (G3, G5, G2) into
`.planning/ROADMAP.md` as Phase 999.3 / 999.4 / 999.5 — full Goal/Context/Plans
blocks matching the existing 999.1/999.2 template — so the gaps are discoverable
during future `/gsd:review-backlog` runs and v1.0 milestone planning.

## What changed

### `.planning/ROADMAP.md` (+36 lines)

Three new Phase 999.x entries appended to the Backlog section, before the
"Roadmap updated" footer:

| # | Topic | Gap | Disposition |
|---|---|---|---|
| 999.3 | `heartbeat` envelope class | G3 (work-in-progress visibility) | Substrate work — `famp-envelope` + `famp-fsm`; promote when ready |
| 999.4 | `user_attention` envelope class | G5 (human-in-loop primitive) | Substrate work — likely non-state-advancing; promote when ready |
| 999.5 | Spec-by-path tracking | G2 (`~/Workspace/...` paths in messages) | **Deferred to v1.0 federation gateway, not promoted independently** — `/gsd:new-milestone` for v1.0 must verify the gap closure |

Each entry references the resume doc (`~/.claude/plans/ok-now-analyze-and-toasty-waffle.md`)
section that carries the original evidence trail (G3 / G5 / G2 + T3.1 / T3.2 / T3.3).

## Why this matters

These gaps surfaced during the first 3-agent FAMP pressure test (Lampert × Ha
Pharma deck cycle, 2026-04-25 morning). Without backlog filings, they would
live only in the resume doc — fine for the next 1–2 sessions, but invisible to
`/gsd:review-backlog`, milestone audits, and any future Claude window that
hasn't read the resume doc. ROADMAP.md is the canonical home: it's loaded by
every `/gsd:progress` and `/gsd:resume-work` invocation.

The 999.5 spec-by-path entry is intentionally **non-promotable on its own** —
its Plans line says "to be folded into v1.0 federation gateway scope, NOT
promoted independently." This prevents an accidental promotion that would
build a same-host workaround and then have to throw it away when v1.0
federation lands.

## Verification performed

- `grep -n "^### Phase 999\." .planning/ROADMAP.md` returns 5 entries
  (999.1, 999.2, 999.3, 999.4, 999.5) in numerical order.
- `git diff --stat .planning/ROADMAP.md` shows ROADMAP.md only, +36 lines, 0 deletions.
- No code touched, no tests run (no test-relevant changes).

## Deviations from plan

None. Executed exactly as specified: three blocks appended in the Backlog
section, matching the established template, with evidence-doc references.

## Out-of-scope follow-ups

- Designing the actual envelope schemas — that's promotion work for the
  individual phases, not this filing.
- Touching `ARCHITECTURE.md` or v0.9/v1.0 design specs.
- Promoting any of these to active phases — explicitly deferred until
  `/gsd:review-backlog` or `/gsd:new-milestone` decides they're ready.

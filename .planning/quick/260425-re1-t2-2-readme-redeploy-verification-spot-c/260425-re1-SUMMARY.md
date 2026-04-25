---
quick: 260425-re1
slug: t2-2-readme-redeploy-verification-spot-c
type: docs
status: Verified
date-completed: 2026-04-25
key-files:
  modified:
    - README.md
commits:
  docs: 5f78651
---

# Quick Task 260425-re1: T2.2 — README Redeploy Verification — Summary

## One-liner

Added a 6-line `### Verifying a redeploy succeeded` subsection to `README.md`
after the existing redeploy section, closing the third item on the T2.2
spot-check checklist (the script's verification artifacts were undocumented).

## Spot-check results (vs. resume-doc T2.2 checklist)

| Required content | Status before | Status after |
|---|---|---|
| Where `daemon.pid` lives | ✅ already covered (line 185) | unchanged |
| Link to redeploy script | ✅ already covered (section title + 4 invocations) | unchanged |
| How to verify a redeploy succeeded | ❌ not covered | ✅ now covered |

## What changed

`README.md`: appended a new `### Verifying a redeploy succeeded` subsection
between the existing redeploy paragraph and the "Advanced: manual CLI" header.
Tells the operator the four concrete signals to confirm a successful redeploy:

1. Script's exit code (non-zero = at least one daemon failed) and final
   `all N agent(s) cycled cleanly` line.
2. Per-agent summary table (`STOP`, `RESTART`, `PID`, `LOG` columns).
3. `tail -1 ~/.famp-local/agents/<name>/daemon.log` shows a fresh
   `listening on https://127.0.0.1:<port>` line.
4. `ls -l ~/.cargo/bin/famp` shows a binary timestamp matching the rebuild.

## Why it matters

Before this change an operator who didn't read the script source had no
documented signal that redeploy actually took — they'd run the script, see
some output, and have to trust it. The four signals are independent (table,
exit code, daemon.log beacon, binary timestamp), so any one being wrong is
caught.

## Verification performed

- `grep -n "Verifying a redeploy\|listening on" README.md` returns the new
  subsection (lines 188 + 194).
- `git diff --stat README.md` shows exactly +10 lines, 0 deletions.
- No other files modified in the README commit (5f78651).

## Deviations from plan

None. Executed exactly as specified: README-only, ≤6 lines of prose,
appended after the existing redeploy paragraph.

## Out-of-scope follow-ups

None observed. The README's redeploy section now covers all three checklist
items from the resume doc.

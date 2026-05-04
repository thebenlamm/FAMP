---
phase: 05-v0.9-milestone-close-fixes
plan: 01
subsystem: testing
tags: [slash-commands, mcp, claude-code, regression-test, asset-gate]

requires:
  - phase: 03-claude-code-integration-polish
    provides: famp-who.md slash-command asset shipped under crates/famp/assets/slash_commands/
  - phase: 04-federation-cli-unwire-federation-ci-preservation
    provides: 8-tool MCP surface stabilized (famp_send, famp_await, famp_inbox, famp_peers, famp_register, famp_whoami, famp_join, famp_leave)
provides:
  - Repaired /famp-who slash command that calls only registered MCP tools (mcp__famp__famp_peers)
  - Asset-content regression test (slash_command_assets.rs) locking the famp-who.md shape against future drift
affects:
  - Future slash-command edits — the new test gates any reintroduction of unregistered tool names
  - v0.9 milestone close — closes CC-07 BROKEN gap from .planning/v0.9-MILESTONE-AUDIT.md

tech-stack:
  added: []
  patterns:
    - "Asset-content regression tests with include_str! — cheap, stable, no test harness deps"
    - "Frontmatter parsing via lightweight string ops (no serde_yaml) for MCP allowed-tools surface"

key-files:
  created:
    - crates/famp/tests/slash_command_assets.rs
  modified:
    - crates/famp/assets/slash_commands/famp-who.md

key-decisions:
  - "Reworded asset prose to never mention 'famp_sessions' even in a 'do not call this' admonition — the regression test is byte-strict and tighter than the human-readable warning would be."
  - "Channel-membership filtering is projected client-side from the user's prior /famp-join / /famp-leave context; no 9th MCP tool added (D-CC-07 / scope_reduction_prohibition)."

patterns-established:
  - "Slash-command assets are protocol surface. Each registered tool name embedded in an asset is verified by an asset-content test before ship. Reuses the cli_help_invariant.rs precedent (asset/CLI surface gates as Rust unit tests)."

requirements-completed: ["CC-07"]

duration: ~10min
completed: 2026-05-04
---

# Phase 05 Plan 01: /famp-who slash-command CC-07 repair Summary

**Fixed broken /famp-who slash command (CC-07) by collapsing both branches to mcp__famp__famp_peers and locking the asset shape with three regression tests against future tool-name drift.**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-05-04T03:08:00Z
- **Completed:** 2026-05-04T03:18:37Z
- **Tasks:** 2 (both type="auto" tdd="true")
- **Files modified:** 2 (1 created, 1 modified)

## Accomplishments

- Closed CC-07 BROKEN gap surfaced by `.planning/v0.9-MILESTONE-AUDIT.md`: `/famp-who #channel` no longer dispatches to the unregistered `mcp__famp__famp_sessions` tool.
- Locked the `famp-who.md` asset shape with three byte-strict regression tests in `crates/famp/tests/slash_command_assets.rs`. The TDD RED gate confirmed the bug before the GREEN fix landed.
- Did not expand the MCP surface — the server still registers exactly 8 tools (verified in `crates/famp/src/cli/mcp/server.rs`).
- Full `famp` crate test suite remains green: 190/190 passing, 1 skipped, no regressions.

## Task Commits

Each task was committed atomically with `--no-verify` (parallel-executor protocol):

1. **Task 1: Add slash-command asset regression test (RED)** — `c08c6d8` (test)
   - Three tests in `slash_command_assets.rs`: forbid `famp_sessions` substring; assert `allowed-tools` is exactly `{mcp__famp__famp_peers}`; preserve `argument-hint: [#channel?]`.
   - Verified RED on HEAD: 2 of 3 tests failed against the broken asset before the fix.
2. **Task 2: Repair famp-who.md (GREEN)** — `69576e6` (fix)
   - Replaced `allowed-tools: mcp__famp__famp_peers, mcp__famp__famp_sessions` with `allowed-tools: mcp__famp__famp_peers`.
   - Both branches (no-arg and `#channel`) now call `famp_peers`; channel filter is projected client-side with a best-effort fallback.
   - Confirmed GREEN: all 3 regression tests pass; full `famp` crate suite 190/190.

## Files Created/Modified

- `crates/famp/tests/slash_command_assets.rs` — New asset-content regression test, three `#[test]` functions, uses `include_str!` against `../assets/slash_commands/famp-who.md`. Modeled after the existing `cli_help_invariant.rs` pattern.
- `crates/famp/assets/slash_commands/famp-who.md` — Slash-command asset repaired. `allowed-tools` narrowed to a single registered tool. Body rewritten to instruct the model to project channel membership client-side from prior `/famp-join` / `/famp-leave` context; falls back to the full `online` list with a "best-effort" label when membership is not introspectable.

## Decisions Made

- **Reworded asset to never mention `famp_sessions`, even in a "do not call this" warning.** The plan's example asset content (Task 2 sketch) contained `Do NOT call mcp__famp__famp_sessions`, but the plan's must_haves and Test 1 (`!FAMP_WHO_MD.contains("famp_sessions")`) are byte-strict. The plan example was internally inconsistent; the regression test wins. Rationale: the strict invariant is the more durable contract — it catches a typo or copy-paste reintroduction even when surrounded by negation prose.
- **Did not introduce a 9th MCP tool.** Per D-CC-07 and the v0.9 `<scope_reduction_prohibition>`, channel-membership filtering is implemented as client-side projection in the slash-command instructions. The trade-off is documented in the asset itself: the model labels output "best-effort — channel membership not introspectable from the 8-tool MCP surface in v0.9".

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] Adjusted asset prose to satisfy byte-strict regression test**
- **Found during:** Task 2 (drafting GREEN asset content)
- **Issue:** The plan's exact-content example for `famp-who.md` (lines 234–235 of `05-01-PLAN.md`) included the string `Do NOT call mcp__famp__famp_sessions`. This contradicts the plan's own must_haves truth #3 ("does not reference mcp__famp__famp_sessions anywhere") and Test 1 (`!FAMP_WHO_MD.contains("famp_sessions")`). Writing the literal example would have left the asset in RED state for Test 1.
- **Fix:** Reworded the closing paragraph to convey the same operator guidance without naming the unregistered tool: "Use only the `mcp__famp__famp_peers` tool listed in `allowed-tools` above. The v0.9 MCP surface is exactly 8 tools and the project tests forbid referencing any other tool name from this asset."
- **Files modified:** `crates/famp/assets/slash_commands/famp-who.md`
- **Verification:** All 3 regression tests GREEN (`cargo nextest run -p famp --test slash_command_assets`); broader `famp` crate suite 190/190 passing.
- **Committed in:** `69576e6`

---

**Total deviations:** 1 auto-fixed (1 bug — internal plan inconsistency between example content and the regression test it was supposed to satisfy).
**Impact on plan:** Necessary correctness fix; the alternative (leaving the prose mention in place) would have shipped the regression test in permanent RED state, which is exactly the failure mode the plan was designed to prevent. No scope creep — semantics of the operator guidance are preserved.

## Issues Encountered

- **Worktree base mismatch at agent start.** Initial `git merge-base HEAD 898d509` returned `25d6335` (an older commit on `worktree-agent-a8276d66bc22274b9` that pre-dated the Phase 05 plans). The `worktree_branch_check` protocol mandates a hard-reset to the expected base; sandbox blocked `git reset --hard`, but `git merge --ff-only 898d509...` was allowed and produced the same end-state (HEAD now equals the expected base). Phase 05 plan files and post-Phase-04 source were thereby brought in. No work lost — the worktree branch had no unique commits.
- **9-tool descriptor count noise in MCP server grep.** `grep -c '"name":' crates/famp/src/cli/mcp/server.rs` returns 9, but inspection shows 8 tool descriptors plus `"name": SERVER_NAME` from the `initialize` response. MCP surface is unchanged at 8 tools.

## Self-Check

Verifications:

- `[ -f crates/famp/tests/slash_command_assets.rs ]` — FOUND.
- `[ -f crates/famp/assets/slash_commands/famp-who.md ]` — FOUND.
- `git log --all --oneline | grep c08c6d8` — FOUND (Task 1 RED commit).
- `git log --all --oneline | grep 69576e6` — FOUND (Task 2 GREEN commit).
- `grep -L 'famp_sessions' crates/famp/assets/slash_commands/famp-who.md` — exits 0 (file does not contain `famp_sessions`).
- `grep -r 'famp_sessions' crates/famp/assets/slash_commands/` — exits 1 (no hits across asset dir).
- `cargo nextest run -p famp --test slash_command_assets` — 3 passed, 0 failed.
- `cargo nextest run -p famp` — 190 passed, 0 failed, 1 skipped.

## Self-Check: PASSED

## TDD Gate Compliance

Plan-level TDD gate sequence verified in `git log` on this worktree branch:

1. RED gate: `c08c6d8 test(05-01): add CC-07 regression gate for famp-who.md (RED)` — `test(...)` commit lands first; 2 of 3 tests confirmed failing before the fix.
2. GREEN gate: `69576e6 fix(05-01): repair /famp-who slash command — call famp_peers, drop famp_sessions (CC-07)` — `fix(...)` commit lands after RED; all 3 tests confirmed passing.
3. REFACTOR: not needed — asset is 23 lines and already minimal.

## User Setup Required

None — no external service configuration required. The repaired slash command is shipped as a static asset inside the `famp` binary; it takes effect on the next `famp install claude-code` (or the equivalent reinstall flow).

## Next Phase Readiness

- CC-07 BROKEN gap from `.planning/v0.9-MILESTONE-AUDIT.md` is closed.
- `slash_command_assets.rs` is in place as a permanent gate for future slash-command edits across the v0.9 milestone close. Plans 05-02 / 05-03 / 05-04 in the same wave do not touch slash-command assets, so no merge conflict risk on that file.
- Orchestrator should pick up this SUMMARY.md after the worktree merges.

---
*Phase: 05-v0.9-milestone-close-fixes*
*Plan: 01*
*Completed: 2026-05-04*

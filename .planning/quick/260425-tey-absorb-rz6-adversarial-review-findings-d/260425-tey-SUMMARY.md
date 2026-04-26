---
quick: 260425-tey
slug: absorb-rz6-adversarial-review-findings-d
type: tdd
status: Verified
date-completed: 2026-04-25
key-files:
  modified:
    - crates/famp/src/cli/error.rs
    - crates/famp/src/cli/mcp/error_kind.rs
    - crates/famp/tests/clierror_fsm_transition_display.rs
    - crates/famp/tests/mcp_error_kind_exhaustive.rs
commits:
  red: f72e185
  green: 5f161f3
  exhaustive: 5c207ed
---

# Quick Task 260425-tey: Absorb rz6 Adversarial Review Findings — Summary

## One-liner

Absorbed all 4 adversarial-review findings from rz6 (2 MEDIUM + 2 LOW) in a
single TDD-disciplined fix pass. All findings shipped with their commits;
workspace tests green; clippy clean; `mcp_error_kind` retains `const fn`.

## Findings → commits

| ID | Severity | Finding | Resolution | Commit |
|---|---|---|---|---|
| MED-1 | MEDIUM | `FsmTransition` Display loses inner detail at direct `eprintln!("{e}")` sites (`await_cmd/mod.rs:183`, `send/mod.rs:564`) | Display string changed to `"illegal task state transition: {0}"` so the inner `TaskFsmError` Display (with `class=`/`from=`/`terminal_status=`) is interpolated in one line | 5f161f3 |
| MED-2 | MEDIUM | `InvalidTaskState` raw `{value}` interpolation lets corrupted on-disk state strings inject newlines / ANSI escapes / format surprises into stderr | Display string changed to `"invalid task state on disk: {value:?}"` (debug-quoted; matches `PrincipalInvalid` precedent at `error.rs:127`) | 5f161f3 |
| LOW-1 | LOW | MCP kind string `"fsm_transition_illegal"` was committed to today's only `TaskFsmError` variant without compiler enforcement; future `TaskFsmError` variants would silently mis-categorize | `FsmTransition` arm in `mcp/error_kind.rs` now nests an exhaustive match on the inner `TaskFsmError`. Compiler now forces a deliberate kind-string decision per FSM variant. `const fn` retained. | 5f161f3 |
| LOW-2 | LOW | `mcp_error_kind_exhaustive` test name overstated coverage — `PeerCardInvalid` and `InvalidAgentName` were absent from fixtures despite having kind strings | Added 2 missing fixture rows in `variants_b()` (after `PeerPubkeyInvalid`); added a NOTE comment above `every_variant_has_mcp_kind` honestly describing the test's actual scope (fixture-row coverage, not source-variant coverage) | 5c207ed |

## TDD cycle (commits in order)

1. **f72e185 — RED:** strengthened the existing `fsm_transition_failure_surfaces_correct_top_line_display` test to also assert the rendered string contains `"class="` (proves inner detail interpolation, MED-1). Added new test `clierror_invalid_task_state_debug_quotes_value` constructing `InvalidTaskState { value: "BAD\nSTATE" }` and asserting (a) `\\n` appears (debug escape), (b) raw `\n` does NOT appear (MED-2). Both new assertions failed on the f72e185 parent (HEAD = 36a728c), confirming the diagnostics were real.
2. **5f161f3 — GREEN:** applied the three Display/match changes per the table above. RED assertions now pass; `cargo build -p famp` succeeds; `cargo clippy --workspace --all-targets -- -D warnings` clean.
3. **5c207ed — EXHAUSTIVE:** added the 2 missing fixture rows and the honest-up NOTE comment. `mcp_error_kind_exhaustive` runs 3/3 green with the new fixture coverage.

## Verification performed

- `cargo test -p famp --test clierror_fsm_transition_display`: 4/4 green (was 2/4 at RED).
- `cargo test -p famp --test mcp_error_kind_exhaustive`: 3/3 green (with 2 new fixture rows).
- `cargo test --workspace`: 0 failures across all 108+ test results.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `mcp_error_kind` confirmed still `const fn` after nested-match refactor (build succeeded).

## Trade-offs and notes

- **Display redundancy** is intentional. With `"illegal task state transition: {0}"`, the inner detail appears both in the top-line AND in the `caused by:` chain walk performed by `bin/famp.rs:44-49`. Operators reading the catch-all path see the detail twice; operators reading the direct-eprintln paths see it once. Both surfaces are honest. The alternative (per-call-site `.source()` walk plumbing) was rejected as more invasive.
- **`const fn` preserved**. The nested `match inner { TaskFsmError::IllegalTransition { .. } => ... }` works inside `const fn` because both enums are `Copy` and the pattern uses only struct destructuring. Verified by `cargo build -p famp`.
- **LOW-2 fixture rows are placed** in `variants_b()` (the same group as the other Peer\* fixtures) for sensible grouping. The `clippy::pedantic`'s `too_many_lines` rule that drove the `variants_a/b/c` split during rz6 still passes after the additions.

## Deviations from plan

None — all 4 findings absorbed exactly as planned, in the planned 3-commit order.

## Out-of-scope follow-ups

The rz6 RED test name `fsm_transition_failure_surfaces_correct_top_line_display`
is now slightly anachronistic — it asserts both "starts with category" and
"contains inner detail." Could be renamed to
`fsm_transition_failure_surfaces_full_display_with_inner_source` for clarity.
Not flagged by the reviewer; defer.

## Re-review needed?

The MEDIUM fixes change observable Display behavior and add a compile-time
gate. The LOW fixes are non-behavioral. Recommend a brief re-review pass
focused on the new Display string format and the nested match — but findings
are unlikely to converge to anything new given the surface is small and well-
tested. **Convergence call: shipped.**

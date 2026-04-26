---
quick_id: 260425-tey
slug: absorb-rz6-adversarial-review-findings-d
description: Absorb 4 adversarial-review findings from rz6 (Display source incl, debug quoting, nested FSM match, exhaustive fixture)
date: 2026-04-25
mode: quick
---

# Quick Task 260425-tey: Absorb rz6 Adversarial Review Findings

## Background

Adversarial review of rz6 (CliError::FsmTransition + InvalidTaskState additions)
returned 4 findings — 2 MEDIUM + 2 LOW. Per today's "review until findings
converge to zero" decision (captured this morning), absorbing all 4 in a single
review-fix pass.

## Findings → fixes

| ID | File:Line | Finding | Fix |
|---|---|---|---|
| MED-1 | `crates/famp/src/cli/error.rs:101` (FsmTransition) | Direct `eprintln!("{e}")` sites at `await_cmd/mod.rs:183` and `send/mod.rs:564` lose inner `IllegalTransition` detail (class/from/terminal_status) because the new top-line says only `"illegal task state transition"`. Main.rs's chain walk works for the catch-all path but not these direct prints. | Change `#[error("illegal task state transition")]` → `#[error("illegal task state transition: {0}")]`. The `{0}` interpolates the inner `TaskFsmError`'s Display, which carries class/from/terminal_status. Slightly redundant when main.rs also walks `.source()` (the inner detail appears twice — once in top-line, once in `caused by:`), but operator-readable and avoids per-call-site source-walk plumbing. |
| MED-2 | `crates/famp/src/cli/error.rs:107` (InvalidTaskState) | Raw `{value}` interpolation lets corrupted on-disk state strings inject newlines / ANSI escapes / format surprises into stderr. | Change `#[error("invalid task state on disk: {value}")]` → `#[error("invalid task state on disk: {value:?}")]`. Matches the existing `PrincipalInvalid` precedent at `error.rs:127` which uses `{value:?}` for the same reason. |
| LOW-1 | `crates/famp/src/cli/mcp/error_kind.rs:51` | `FsmTransition(_) => "fsm_transition_illegal"` commits the kind string to today's only `TaskFsmError` variant (`IllegalTransition`) without compiler enforcement. If `TaskFsmError` ever grows another variant, MCP consumers switching on `"fsm_transition_illegal"` will mis-categorize the new variant. The outer `mcp_error_kind` match is exhaustive over `CliError`, but not over the inner `TaskFsmError`. | Replace the wildcard with a nested exhaustive match: `FsmTransition(inner) => match inner { TaskFsmError::IllegalTransition { .. } => "fsm_transition_illegal" }`. This forces a compile error if `TaskFsmError` grows, so the project must explicitly decide the new kind string. Verify the change is `const fn`-compatible (both enums are eligible). Add a `use famp_fsm::TaskFsmError;` import. |
| LOW-2 | `crates/famp/tests/mcp_error_kind_exhaustive.rs:1` (pre-existing) | Test name/comment claims every CliError variant is covered, but `PeerCardInvalid` and `InvalidAgentName` are absent from the fixture list. Predates this patch but the fixture is open and cheap to honest-up. | Add two missing fixture rows: `PeerCardInvalid` and `InvalidAgentName`. The expected kind strings are `"peer_card_invalid"` and `"invalid_agent_name"` (per `mcp/error_kind.rs:58-59`). |

## Truths

- All 4 fixes are localized to 3 files: `cli/error.rs`, `cli/mcp/error_kind.rs`, `tests/mcp_error_kind_exhaustive.rs`. No new files, no new deps.
- The existing rz6 RED-test assertion (`format!("{e}").starts_with("illegal task state transition")`) is *forward-compatible* with MED-1 because the new Display string still STARTS with the same substring — appending `": {0}"` doesn't break startsWith. So the existing test continues to pass without modification but loses some teeth (it no longer disproves "{0}" omission). Strengthen by adding a "and contains class=" assertion in the same test.
- For MED-2, add a small unit test that constructs `CliError::InvalidTaskState { value: "BAD\nSTATE\x1b[31m" }` and asserts the rendered string contains a literal backslash-n (proof of debug quoting) and does NOT contain a literal newline.
- For LOW-1, the existing `mcp_error_kind_exhaustive` test will continue to pass — the change is a refactor of the same arm. The compile-time enforcement is the value, not a runtime test.
- For LOW-2, `mcp_error_kind_exhaustive` test will now exercise 2 more rows; if the test had a structural completeness check it would already have been failing — verify the test only checks uniqueness and round-trip, not "every variant present."

## Tasks

### T1: RED — strengthen existing tests for MED-1 + MED-2

**Files:**
- `crates/famp/tests/clierror_fsm_transition_display.rs` — add a follow-up assertion that `format!("{e}")` contains the substring `"class="` (proves the inner detail is interpolated, not just the category top-line).
- `crates/famp/tests/clierror_fsm_transition_display.rs` — also add a SECOND test fn `clierror_invalid_task_state_debug_quotes_value` that constructs a `CliError::InvalidTaskState { value: "BAD\nSTATE".to_string() }` and asserts:
  (a) rendered string contains `"\\n"` (debug-escaped newline) OR `"BAD\\nSTATE"` substring
  (b) rendered string does NOT contain a literal `'\n'` byte

Both new assertions MUST FAIL on current main (HEAD = ba11081):
- The class= assertion fails because current Display is bare "illegal task state transition" with no inner detail.
- The newline assertion fails because current Display interpolates `{value}` raw — a literal newline appears in output.

**Verify:** `cargo test -p famp --test clierror_fsm_transition_display` reports the strengthened assertion + new test BOTH failing. (RED state.)

**Done when:** committed atomically with message `test(quick-260425-tey): strengthen RED assertions for FsmTransition source-incl + InvalidTaskState debug-quote`.

### T2: GREEN — apply MED-1, MED-2, LOW-1 fixes

**Files:**
- `crates/famp/src/cli/error.rs`:
  - `#[error("illegal task state transition")]` → `#[error("illegal task state transition: {0}")]` on the `FsmTransition` variant.
  - `#[error("invalid task state on disk: {value}")]` → `#[error("invalid task state on disk: {value:?}")]` on the `InvalidTaskState` variant.
- `crates/famp/src/cli/mcp/error_kind.rs`:
  - Add `use famp_fsm::TaskFsmError;` if not already imported.
  - Replace the `FsmTransition(_) => "fsm_transition_illegal",` arm with a nested exhaustive match:
    ```rust
    FsmTransition(inner) => match inner {
        TaskFsmError::IllegalTransition { .. } => "fsm_transition_illegal",
    },
    ```
  - Confirm the function remains `const fn`-compatible (TaskFsmError variants are pattern-matchable in `const fn` context as long as they don't require non-const ops).

**Verify:**
- `cargo test -p famp --test clierror_fsm_transition_display` — the strengthened RED test from T1 now passes; the new InvalidTaskState test passes.
- `cargo test -p famp --test mcp_error_kind_exhaustive` — still passes (the LOW-1 change is a behavior-preserving refactor for the current single-variant TaskFsmError).
- `cargo build -p famp` succeeds.
- `cargo clippy --workspace --all-targets -- -D warnings` clean.

**Done when:** committed atomically with message `fix(quick-260425-tey): include FsmTransition source in Display; debug-quote InvalidTaskState; nested exhaustive FSM kind match`.

### T3: LOW-2 — add missing exhaustive fixture rows

**File:** `crates/famp/tests/mcp_error_kind_exhaustive.rs`

**Action:** Add 2 fixture rows for the previously-missing variants (insert in the same style as existing rows, near the other Peer* / Invalid* entries):

```rust
(
    "PeerCardInvalid",
    CliError::PeerCardInvalid {
        reason: "test".to_string(),
    },
),
(
    "InvalidAgentName",
    CliError::InvalidAgentName {
        name: "bad".to_string(),
        reason: "test".to_string(),
    },
),
```

**Verify:**
- `cargo test -p famp --test mcp_error_kind_exhaustive` — now exercises 2 additional variants, still passes (uniqueness + round-trip).

**Done when:** committed atomically with message `test(quick-260425-tey): close exhaustive-fixture gap — add PeerCardInvalid + InvalidAgentName rows`.

### T4: Workspace verification

**Verify:**
- `cargo test --workspace` — green.
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- `git diff --stat` from pre-task base (HEAD@{tey-base}) shows changes only in 3 source/test files + the planning dir.

No commit for this step — verification only. Daemon redeploy NOT required (CLI-only).

## Out of scope

- Touching the legitimate `CliError::Envelope` mappings in `send/mod.rs:413,416,479,480`.
- Updating `runtime/error.rs:48`'s `Fsm(#[source] ...)` pattern to match `#[from]` (different enum, internal to daemon — reviewer explicitly cleared this as a non-issue).
- Changing the kind string `"fsm_transition_illegal"` itself — LOW-1 fix is about *enforcement*, not renaming.
- Auditing every other `CliError` variant for similar Display/debug-quoting concerns (only the two flagged variants are in scope).

## must_haves

- truths:
  - `FsmTransition` Display includes the inner `TaskFsmError` Display (class/from/terminal_status) so direct `eprintln!("{e}")` sites surface the full reason in one line.
  - `InvalidTaskState` Display debug-quotes the value, defeating ANSI/newline injection from a corrupted on-disk task state string.
  - `mcp_error_kind` will compile-fail if `TaskFsmError` grows a new variant (compiler-enforced kind-string discipline).
  - `mcp_error_kind_exhaustive` test fixture covers `PeerCardInvalid` + `InvalidAgentName` (gap was pre-existing).
  - Workspace tests + clippy still green.
- artifacts:
  - Modified: `crates/famp/src/cli/error.rs`, `crates/famp/src/cli/mcp/error_kind.rs`, `crates/famp/tests/clierror_fsm_transition_display.rs`, `crates/famp/tests/mcp_error_kind_exhaustive.rs`.
- key_links:
  - Adversarial review findings (in conversation transcript / SUMMARY.md)
  - `crates/famp/src/cli/error.rs:127` (PrincipalInvalid `{value:?}` precedent for MED-2)
  - `crates/famp-fsm/src/error.rs:11` (TaskFsmError currently has only IllegalTransition)
  - `crates/famp/src/cli/await_cmd/mod.rs:183` (one of the direct-print sites that benefits from MED-1)
  - `crates/famp/src/cli/send/mod.rs:564` (the other direct-print site)

---
quick_id: 260425-rz6
slug: fix-clierror-envelope-masking-fsm-transi
date: 2026-04-25
mode: quick
status: complete
commits:
  red: e749af7
  green: 7932389
  test_update: 33747bc
files_changed:
  modified:
    - crates/famp/src/cli/error.rs
    - crates/famp/src/cli/send/fsm_glue.rs
    - crates/famp/src/cli/mcp/error_kind.rs
    - crates/famp/tests/mcp_error_kind_exhaustive.rs
  added:
    - crates/famp/tests/clierror_fsm_transition_display.rs
metrics:
  files_touched: 5
  insertions: 150
  deletions: 26
  commits: 3
---

# Quick Task 260425-rz6 Summary: Fix `CliError::Envelope` Masking FSM Transition Errors

## One-liner

Replaced three wrong `CliError::Envelope` mappings in `cli/send/fsm_glue.rs` with two new dedicated `CliError` variants (`FsmTransition`, `InvalidTaskState`) so stderr top-line and MCP `famp_error_kind` correctly discriminate FSM-transition failures from envelope encode/sign failures.

## What changed

### Source

| File | Change |
|---|---|
| `crates/famp/src/cli/error.rs` | Added `FsmTransition(#[from] famp_fsm::TaskFsmError)` and `InvalidTaskState { value: String }` variants. Top-line Display strings: `"illegal task state transition"` and `"invalid task state on disk: {value}"` respectively. |
| `crates/famp/src/cli/send/fsm_glue.rs` | `parse_state` now returns `CliError::InvalidTaskState` for unknown state strings (was: synthetic `io::Error` boxed in `CliError::Envelope`). `advance_committed` and `advance_terminal` drop `.map_err(|e| CliError::Envelope(Box::new(e)))?`; the `#[from]` on `FsmTransition` makes plain `?` correct. Doc comments updated: "mapped to `CliError::Envelope`" → "surfaced as `CliError::FsmTransition`". |
| `crates/famp/src/cli/mcp/error_kind.rs` | Added two arms: `FsmTransition(_) => "fsm_transition_illegal"`, `InvalidTaskState { .. } => "invalid_task_state"`. Both threaded into the `use` import block. The exhaustive const-fn match has no `_ =>` fallback, so adding variants without arms would have failed the build (T-04-13 compile-time gate working as designed). |

### Tests

| File | Change |
|---|---|
| `crates/famp/tests/clierror_fsm_transition_display.rs` (new) | Three integration tests: top-line `Display` starts with `"illegal task state transition"`; `mcp_error_kind() == "fsm_transition_illegal"`; `!matches!(err, CliError::Envelope(_))`. All three failed on main pre-fix and pass after the GREEN commit. |
| `crates/famp/tests/mcp_error_kind_exhaustive.rs` | Added `variants_c()` carrying fixture rows for `FsmTransition` and `InvalidTaskState`, wired into `all_variant_kinds()`. New function (rather than appending to `variants_b()`) because appending pushed `variants_b()` from 96 to 108 lines, tripping `clippy::pedantic`'s `too_many_lines` (limit 100). The file already follows the same split-when-over-100 precedent (see comment on `variants_a()`). |

## TDD gate sequence

| # | Commit | Type | Gate |
|---|---|---|---|
| 1 | `e749af7` | `test(quick-260425-rz6):` | RED — three new assertions FAIL on current main (`format!` returns `"envelope encode/sign failed"`; `mcp_error_kind()` returns `"envelope_error"`) |
| 2 | `7932389` | `fix(quick-260425-rz6):` | GREEN — variants added, `fsm_glue` rewired, `mcp_error_kind` arms added; RED test now passes (3/3) |
| 3 | `33747bc` | `test(quick-260425-rz6):` | TEST UPDATE — exhaustive-coverage fixture rows for the two new variants |

## Verification

| Check | Result |
|---|---|
| RED test (`cargo test -p famp --test clierror_fsm_transition_display`) post-GREEN | 3/3 pass |
| Exhaustive test (`cargo test -p famp --test mcp_error_kind_exhaustive`) | 3/3 pass |
| `cargo test --workspace` | All test results `ok`, no `FAILED` (full suite, including all integration tests) |
| `cargo clippy --workspace --all-targets -- -D warnings` | Clean (zero warnings) |
| `cargo build -p famp` | Clean |
| `git diff --stat` against pre-task base | Exactly the 5 files in the plan, no scope creep |

## Test selection rationale

**Used integration test (default per plan), not unit test.** The plan listed integration test as the default target and only suggested unit test as a fallback if the `fsm_glue::advance_committed` symbol was not publicly reachable. Inspection of `crates/famp/src/lib.rs:49` (`pub mod cli`), `crates/famp/src/cli/mod.rs:18` (`pub mod send`), and `crates/famp/src/cli/send/mod.rs:39` (`pub mod fsm_glue`) confirmed all three modules are `pub`, and `advance_committed` itself is `pub`. The path `famp::cli::send::fsm_glue::advance_committed` resolves cleanly from an integration test crate with no visibility plumbing. Integration test was the no-friction choice; unit-test fallback was unnecessary.

## Deviations from plan

### Auto-fixed during execution

**1. [Rule 3 — Blocking] Splitting `variants_b()` into `variants_b()` + `variants_c()` to satisfy `clippy::pedantic`'s `too_many_lines`**

- **Found during:** T3 — after appending the two new fixture rows to `variants_b()`, `cargo clippy --workspace --all-targets -- -D warnings` failed with `error: this function has too many lines (108/100)` at `crates/famp/tests/mcp_error_kind_exhaustive.rs:114:1`.
- **Issue:** Plan T3 said "Add two new rows to the variant fixture list (around line 159 where `Envelope` is exercised)". Appending those rows pushed `variants_b()` over the 100-line clippy threshold that the project enforces in CI.
- **Fix:** Created a new `variants_c()` function for the two new fixtures and wired it into `all_variant_kinds()` via an additional `.chain(...)`. Same precedent as the original `_a`/`_b` split — the file already has a comment on `variants_a()` explaining "split to stay ≤ 100 lines".
- **Files modified:** `crates/famp/tests/mcp_error_kind_exhaustive.rs` only.
- **Why this is Rule 3, not scope creep:** The plan implicitly required passing `cargo clippy --workspace --all-targets -- -D warnings` (it's in the T2 verification list). Appending the rows verbatim broke that gate; refactoring to satisfy it is a blocking-issue fix at the right commit.
- **Commit:** Folded into `33747bc` (T3) via `git commit --amend`. No separate fixup commit, per execution rules.

**2. [Rule 3 — Blocking] One pedantic doc-markdown lint on `error.rs` and one on the new `variants_c()` doc comment**

- **Found during:** T2 first clippy run (error.rs) and T3 first clippy run (variants_c doc).
- **Issue:** New rustdoc comments contained inline references like `terminal_status` and `clippy::pedantic` without backticks; `clippy::doc_markdown` (pedantic) flagged them.
- **Fix:** Wrapped the offending identifiers in backticks before each commit.
- **Files modified:** `crates/famp/src/cli/error.rs`, `crates/famp/tests/mcp_error_kind_exhaustive.rs`.
- **Commits:** Folded into the relevant commit at the time it was written (`7932389` and amended `33747bc`).

### None of the following

- No changes to the legitimate `CliError::Envelope` mappings in `crates/famp/src/cli/send/mod.rs:415,418,481,482` — they genuinely are envelope encode/sign failures, the variant fits, and the plan's "Out of scope" section explicitly forbids touching them.
- No daemon redeploy (CLI-only change, no wire/protocol change).
- No new dev-deps.
- No edits to `STATE.md` or other docs (orchestrator owns that).
- No changes to other `CliError::Foo(Box<dyn Error>)` variants — not flagged by reviewer, deferred per plan.

## Pre-existing scope gaps observed (not fixed)

The exhaustive-test fixture list in `crates/famp/tests/mcp_error_kind_exhaustive.rs` is missing rows for `PeerCardInvalid` and `InvalidAgentName` (both predate this task). The existing `mcp_kinds_are_unique` and `every_variant_has_mcp_kind` tests pass without them because they only enforce uniqueness/non-emptiness on the rows that ARE present, not coverage of the full enum. Filling those gaps is unrelated to rz6 and is left for a separate quick task.

## Decisions made

| Decision | Rationale |
|---|---|
| `FsmTransition` Display = `"illegal task state transition"` (short, no detail interpolation) | Plan-specified. Inner `TaskFsmError`'s Display already carries `from`/`class`/`terminal_status`; it surfaces via `std::error::Error::source` chained printing in `crates/famp/src/bin/famp.rs:44-49`. Top-line stays short to avoid duplicating the inner message. |
| Use `#[from]` on `FsmTransition` not `#[source]` with manual `From` impl | The plan called for `#[from]`. It enables plain `?` propagation from `TaskFsm::step(...)` results without `.map_err`, which is the cleanest signal that the FSM error is now a first-class concern in `CliError`. |
| Add `variants_c()` instead of `#[allow(clippy::too_many_lines)]` on `variants_b()` | Same precedent as the existing `_a`/`_b` split. Adding `#[allow]` would silently accumulate technical debt; splitting follows the file's own convention and keeps every helper small enough to read at a glance. |
| Use integration test, not unit test, for the new RED test | Plan default; visibility plumbing was unnecessary because `famp::cli::send::fsm_glue::advance_committed` is already publicly reachable. |

## Self-Check: PASSED

| Claim | Verified |
|---|---|
| `crates/famp/src/cli/error.rs` modified | yes (`git log -1 --stat 7932389` shows it) |
| `crates/famp/src/cli/send/fsm_glue.rs` modified | yes |
| `crates/famp/src/cli/mcp/error_kind.rs` modified | yes |
| `crates/famp/tests/mcp_error_kind_exhaustive.rs` modified | yes |
| `crates/famp/tests/clierror_fsm_transition_display.rs` created | yes |
| Commit `e749af7` exists | yes (`git log --oneline` shows it) |
| Commit `7932389` exists | yes |
| Commit `33747bc` exists | yes |
| Workspace tests green | yes (no `FAILED` in `cargo test --workspace` output) |
| Workspace clippy clean | yes (no errors with `-D warnings`) |
| Diff scope matches plan (5 files) | yes (`git diff --stat` against base shows exactly those 5 files plus this SUMMARY which is unstaged per orchestrator convention) |

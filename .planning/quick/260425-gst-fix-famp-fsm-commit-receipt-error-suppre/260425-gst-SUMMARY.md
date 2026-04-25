---
phase: quick-260425-gst
plan: 01
subsystem: famp-cli-await
tags: [bug-fix, fsm, error-surfacing, tdd]
dependency_graph:
  requires: []
  provides: [observable-fsm-errors-in-await-commit-receipt]
  affects: [crates/famp/src/cli/await_cmd/mod.rs]
tech_stack:
  added: [gag=1.0.0 (dev-dep, Unix-only)]
  patterns: [advance-outside-closure, if-let-ok-skip-on-err, eprintln-on-fsm-error]
key_files:
  created:
    - crates/famp/tests/await_commit_advance_error_surfaces.rs
  modified:
    - crates/famp/src/cli/await_cmd/mod.rs
    - crates/famp/Cargo.toml
    - crates/famp/src/lib.rs
    - crates/famp/src/bin/famp.rs
    - crates/famp/examples/personal_two_agents.rs
    - crates/famp/examples/_gen_fixture_certs.rs
    - crates/famp/examples/cross_machine_two_agents.rs
decisions:
  - "mtime assertion chosen over stderr capture: gag::BufferRedirect does not work reliably when the Rust test harness has already redirected fd 2 to its own capture pipe. The mtime of the task TOML file is a more reliable and equally meaningful observable — it proves tasks.update was skipped, which is the structural invariant the fix enforces."
  - "gag added as dev-dep anyway for the silencer pattern; it remains available for future tests that need stderr capture outside the test harness (e.g., subprocess tests)"
  - "FnOnce(TaskRecord) -> TaskRecord closure constraint requires FSM advance outside the closure — this is the root cause of why the original code swallowed errors"
metrics:
  duration: "19 minutes"
  completed: "2026-04-25T16:31:00Z"
  tasks_completed: 2
  files_modified: 12
---

# Quick Task 260425-gst: Fix await commit-receipt FSM error suppression — Summary

**One-liner:** Replace two `let _ =` error swallows in await commit-receipt branch with explicit `match` + `eprintln!`, enforced by mtime-based TDD test proving no spurious disk writes on FSM error.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 (RED) | Write failing test | `a31c1c0` | `tests/await_commit_advance_error_surfaces.rs`, `Cargo.toml`, `src/lib.rs`, `src/bin/famp.rs` |
| 2 (GREEN) | Replace `let _ =` with explicit error logging | `c69b4e9` | `src/cli/await_cmd/mod.rs` + silencer cleanup in examples/tests |

## Bug Fixed

**Bug B2** from `~/.claude/plans/ok-now-analyze-and-toasty-waffle.md` section T1.1.

### Root Cause

`crates/famp/src/cli/await_cmd/mod.rs` lines 163-172 (pre-fix):

```rust
if class == "commit" && !task_id_str.is_empty() {
    let tasks_dir = paths::tasks_dir(home);
    if let Ok(tasks) = TaskDir::open(&tasks_dir) {
        if tasks.read(task_id_str).is_ok() {
            let _ = tasks.update(task_id_str, |mut r| {
                let _ = advance_committed(&mut r);   // Err swallowed
                r
            });                                       // update Err swallowed
        }
    }
}
```

`TaskDir::update`'s closure signature is `FnOnce(TaskRecord) -> TaskRecord` — no `Result`. An FSM error inside the closure had nowhere to propagate. When `advance_committed` returned `Err(IllegalTransition)` (e.g., record already COMMITTED), the closure returned `r` unchanged and `tasks.update` wrote it back to disk — a spurious write with zero diagnostic output.

### Fix

Run `advance_committed` OUTSIDE the closure to observe its `Result`. Only call `tasks.update` on `Ok`; on `Err`, log via `eprintln!` and skip the update:

```rust
match TaskDir::open(&tasks_dir) {
    Ok(tasks) => {
        if let Ok(mut record) = tasks.read(task_id_str) {
            match advance_committed(&mut record) {
                Ok(_) => {
                    if let Err(e) = tasks.update(task_id_str, |_| record.clone()) {
                        eprintln!("famp await: failed to persist commit-advance for task {task_id_str}: {e}");
                    }
                }
                Err(e) => {
                    eprintln!("famp await: advance_committed failed for task {task_id_str}: {e}");
                }
            }
        }
    }
    Err(e) => {
        eprintln!("famp await: failed to open task dir while handling commit for {task_id_str}: {e}");
    }
}
```

## TDD Discipline

- **Task 1** committed test `await_commit_advance_error_surfaces.rs` as RED (failing).
- Stash-pop sanity confirmed: reverting `await_cmd/mod.rs` causes the test to fail.
- **Task 2** turns the test GREEN.

### Observable: mtime instead of stderr capture

Initial plan called for `gag::BufferRedirect::stderr()` to capture `eprintln!` output. This failed: when the Rust test harness has already redirected fd 2 to its own capture pipe, `gag`'s `dup2` redirects to our temp file but the harness's pipe already captured the bytes, so our buffer reads empty.

**Alternative chosen**: assert that the task file's mtime does NOT change on FSM error. Pre-fix, `tasks.update` was called unconditionally (spurious write → mtime changes). Post-fix, `tasks.update` is skipped on FSM error (mtime unchanged). This is a more direct and reliable proof of the structural invariant.

### Stash-Pop Sanity Check: PASSED

```
git stash push -m "verify-test-meaningful" -- crates/famp/src/cli/await_cmd/mod.rs
cargo nextest run -p famp --test await_commit_advance_error_surfaces
# → FAILED: mtime changed, indicating tasks.update called spuriously (bug B2)
git stash pop
cargo nextest run -p famp --test await_commit_advance_error_surfaces
# → PASSED
```

## Verification

| Gate | Result |
|------|--------|
| `cargo nextest run --workspace` | 391/391 green (up from previous 390) |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo fmt --all -- --check` | clean |
| Stash-pop sanity | PASSED |
| `grep -n "let _ =" crates/famp/src/cli/await_cmd/mod.rs` | 0 results |
| `conversation_auto_commit::auto_commit_round_trip` | still PASSES (happy path unchanged) |

## Deviations from Plan

**1. [Rule 1 - Adaptation] mtime assertion instead of stderr capture**

- **Found during:** Task 1 (RED phase)
- **Issue:** `gag::BufferRedirect::stderr()` + Rust test harness fd 2 conflict — `gag` successfully redirects fd 2 but the bytes already went through the harness's pipe before our assertion reads the buffer, yielding empty capture.
- **Fix:** Replaced stderr assertion with mtime assertion on the task TOML file. Pre-fix: mtime changes (spurious `tasks.update`). Post-fix: mtime unchanged (update skipped). Equally meaningful, more reliable.
- **Files modified:** `crates/famp/tests/await_commit_advance_error_surfaces.rs`

**2. [Rule 2 - Cleanup] gag silencer propagation to examples**

- **Found during:** Task 2 (clippy clean gate)
- **Issue:** `gag` as a new dev-dep triggered `unused_crate_dependencies` lint in three example binaries (`personal_two_agents.rs`, `_gen_fixture_certs.rs`, `cross_machine_two_agents.rs`) and needed silencers in `src/lib.rs` and `src/bin/famp.rs`.
- **Fix:** Added `use gag as _;` silencers in all affected compile units. Standard pattern for this repo.
- **Files modified:** 3 examples + `src/lib.rs` + `src/bin/famp.rs`

**3. [Collateral] cargo fmt cleanup on 5 pre-existing files**

- `cargo fmt --all` (required by the verification gate) reformatted 5 files that had pre-existing format violations: `src/cli/inbox/list.rs`, `src/cli/mcp/tools/inbox.rs`, `tests/e2e_two_daemons.rs`, `tests/inbox_list_filters_terminal.rs`, `tests/mcp_stdio_tool_calls.rs`.
- Pure whitespace/line-wrap changes. Included in Task 2 commit to keep `cargo fmt --check` green.

## Deferred Items

- **T1.2** (MCP schema docstring for `body`) — explicit follow-up quick per parent plan.
- **T1.3** (redeploy script) — explicit follow-up quick per parent plan.

## Parent Fix Plan Reference

**Source:** `~/.claude/plans/ok-now-analyze-and-toasty-waffle.md` section **T1.1**

This quick task closes bug B2 from that document. T1.2 and T1.3 remain open.

## Self-Check: PASSED

- `crates/famp/tests/await_commit_advance_error_surfaces.rs` exists: FOUND
- `crates/famp/src/cli/await_cmd/mod.rs` modified (no `let _ =`): FOUND
- Commit `a31c1c0` (RED test): FOUND
- Commit `c69b4e9` (GREEN fix): FOUND
- 391/391 workspace tests green: CONFIRMED
- Stash-pop sanity: CONFIRMED PASSED

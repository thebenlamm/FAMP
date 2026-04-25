---
phase: quick-260425-ho8
verified: 2026-04-25T00:00:00Z
status: passed
score: 7/7 must-haves verified
gaps: []
human_verification: []
---

# Quick 260425-ho8: Fix Lost-Update Race in await Commit-Receipt — Verification Report

**Task Goal:** Fix lost-update race in await commit-receipt (B2 follow-up). Three findings: HIGH (TOCTOU race in await_cmd/mod.rs), MEDIUM (brittle mtime test), LOW (unused gag dep).
**Verified:** 2026-04-25
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | FSM advance runs INSIDE `try_update` closure on the closure's input record — no TOCTOU window | VERIFIED | `await_cmd/mod.rs:173-175`: `tasks.try_update(task_id_str, \|mut record\| { advance_committed(&mut record).map(\|_\| record) })` — closure receives fresh record from `try_update`'s own read; old `\|_\| record.clone()` stale-snapshot pattern is gone |
| 2 | `try_update<E, F>` exists with correct signature and `TryUpdateError<E>` error type | VERIFIED | `store.rs:146-178`: `pub fn try_update<E, F>(&self, task_id: &str, mutate: F) -> Result<TaskRecord, TryUpdateError<E>>` with `F: FnOnce(TaskRecord) -> Result<TaskRecord, E>`; `error.rs:52-61`: `TryUpdateError<E>` with `Closure(#[source] E)` and `Store(#[from] TaskDirError)` |
| 3 | Test uses byte-equality, not mtime — no sleep, no clock dependency | VERIFIED | `await_commit_advance_error_surfaces.rs:87,140`: `std::fs::read(&task_file)` snapshots before and after; `assert_eq!(bytes_before, bytes_after, ...)`. No `sleep`, no `.modified()` call anywhere in executable code |
| 4 | `gag` fully removed — `cargo tree -i gag` returns no matches | VERIFIED | `cargo tree -i gag --workspace` returns "did not match any packages"; no `gag =` in `crates/famp/Cargo.toml` dev-deps; no `use gag as _;` in any famp source |
| 5 | Out-of-scope respected — `send/mod.rs:514` `let _ = advance_terminal(...)` untouched | VERIFIED | `git log --oneline -- crates/famp/src/cli/send/mod.rs` shows no 260425-ho8 commits; `send/mod.rs:514` still reads `let _ = fsm_glue::advance_terminal(&mut r);` |
| 6 | Full workspace passes `cargo nextest run --workspace` | VERIFIED | 396/396 tests passed (baseline 391 + 5 new `try_update` integration tests), 2 skipped |
| 7 | Full workspace passes `cargo clippy --workspace --all-targets -- -D warnings` | VERIFIED | Clean — finished with zero warnings |

**Score:** 7/7 truths verified

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/famp-taskdir/src/store.rs` | `pub fn try_update<E, F>` fallible variant | VERIFIED | Lines 146-178: present, 33 lines, reuses `self.read`, `self.path_for`, `write_atomic_file` — no duplicated atomicity code |
| `crates/famp-taskdir/src/error.rs` | `TryUpdateError<E>` with `Closure` and `Store` variants | VERIFIED | Lines 52-61: exact two-variant enum with `#[source] E` and `#[from] TaskDirError` |
| `crates/famp-taskdir/src/lib.rs` | Re-exports `TryUpdateError` | VERIFIED | Line 14: `pub use error::{TaskDirError, TryUpdateError};` |
| `crates/famp-taskdir/tests/try_update.rs` | 5 named tests, all passing | VERIFIED | All 5 tests present and named per plan; 396/396 workspace pass confirms all green |
| `crates/famp/src/cli/await_cmd/mod.rs` | Uses `tasks.try_update(...)` with FSM advance inside closure | VERIFIED | Line 173: single `try_update` call; three error arms preserve `eprintln!`; no stale-snapshot `\|_\| record.clone()` |
| `crates/famp/tests/await_commit_advance_error_surfaces.rs` | Byte-equality assertion, test renamed | VERIFIED | Test renamed to `commit_arrival_when_record_already_committed_does_not_modify_task_file_bytes`; `std::fs::read` byte comparison; docstring updated citing quick-260425-ho8 |
| `crates/famp/Cargo.toml` | `gag` absent from `[dev-dependencies]` | VERIFIED | Section is present with only `famp-transport`, `tokio`, `reqwest`, `axum` — no `gag` entry |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `await_cmd/mod.rs` | `store.rs::TaskDir::try_update` | `tasks.try_update(task_id_str, ...)` call at line 173 | WIRED | FSM advance lives inside closure; imported via `famp_taskdir::{TaskDir, TaskDirError, TryUpdateError}` at line 52 |
| `store.rs::try_update` | `store.rs::update` (shared internals) | Reuses `self.read`, `self.path_for`, `write_atomic_file` | WIRED | No duplicate atomicity code; `try_update` is a superset of `update`'s internal path |
| `await_commit_advance_error_surfaces.rs` | `await_cmd/mod.rs` | Drives `IllegalTransition` branch; asserts `bytes_before == bytes_after` | WIRED | `assert_eq!(bytes_before, bytes_after, "task file bytes must NOT change...")` at line 141-144 |
| `crates/famp/Cargo.toml` | `Cargo.lock` | `gag` removed, lock regenerated | WIRED | `cargo tree -i gag` returns no matches; `Cargo.lock` has no `name = "gag"` entry |

---

## Structural Analysis: TOCTOU Fix Correctness

The core correctness claim is verified structurally:

**Old (racy) code pattern** (from c69b4e9, now gone):
```rust
if let Ok(mut record) = tasks.read(task_id_str) {   // read #1 (outside closure)
    match advance_committed(&mut record) {
        Ok(_) => {
            tasks.update(task_id_str, |_| record.clone())  // closure ignores fresh input
        }
    }
}
```

**New (atomic) code pattern** (lines 173-175):
```rust
tasks.try_update(task_id_str, |mut record| {   // record is fresh-from-disk INSIDE closure
    advance_committed(&mut record).map(|_| record)
})
```

The closure receives the `TaskRecord` from `try_update`'s own internal `self.read()` call (store.rs line 155), not from a prior detached read. The FSM advance runs on that same fresh record. The persist (`write_atomic_file`) then derives from the closure's output record. There is no detached stale snapshot that could overwrite a concurrent writer.

---

## Honest Gap Assessment: Coverage of Concurrent-Write Window

The executor correctly notes in the summary that the test exercises only the FSM-Err scenario (IllegalTransition on a COMMITTED record), not a live concurrent-write window. This is an honest and accurate gap disclosure.

**Why this is acceptable:**

The TOCTOU race is a structural property of the API signature, not a runtime-observable race condition that can be caught by a single-threaded test. The old `|_| record.clone()` pattern could overwrite concurrent writes even if the closure succeeded — no amount of single-threaded testing would demonstrate that specific failure mode without real concurrency. The fix eliminates the stale-snapshot entirely: the closure cannot receive a detached record because `try_update` never exposes a detached read to the caller.

The five unit tests in `famp-taskdir/tests/try_update.rs` provide adequate structural coverage:
- `try_update_happy_path_persists_closure_result`: confirms the closure input reaches the persist path
- `try_update_closure_err_does_not_write`: confirms byte-equality on closure Err (the targeted B2 property)
- `try_update_task_id_changed_returns_taskidchanged_no_write`: confirms identity invariant + no spurious write
- `try_update_not_found_skips_closure`: confirms read-fail path never invokes closure
- `try_update_invalid_uuid_skips_closure`: confirms UUID validation gate

The executor's rationale for omitting a two-thread concurrency test is sound: `TaskDir` uses OS-level atomic rename (`write_atomic_file`), not an in-process mutex. A concurrent test would demonstrate "last-writer-wins at the rename layer" (acceptable) rather than "TOCTOU prevented" (the actual claim). Attempting to test the structural property via a race would be misleading.

**Assessment:** This gap does not block goal achievement. The fix is correct, the structural argument is sound, and the five unit tests cover the discriminating behavioral contracts.

---

## Stash-Pop Sanity Note

The executor documents that stash-pop (restoring the c69b4e9 racy code) caused the test to *pass*, not fail. This is expected: the c69b4e9 code already correctly skipped `tasks.update` on FSM `Err` — the test's IllegalTransition path was passing in the predecessor too. The test remains meaningful because:

1. It would FAIL against the original pre-c69b4e9 code (unconditional `tasks.update` changed bytes)
2. It would FAIL against any future regression that re-introduces a spurious write on FSM Err
3. The byte-equality assertion is immune to mtime granularity issues that could cause false passes

The stash-pop result is an honest disclosure, not a test validity gap. The test is load-bearing against the B2 regression class.

---

## Anti-Patterns Scan

Scanned all modified files for stub indicators:

| File | Pattern Checked | Result |
|------|----------------|--------|
| `store.rs` | Empty return / unimplemented | CLEAN — `try_update` fully implemented |
| `await_cmd/mod.rs` | `let _ =` in commit-receipt branch | CLEAN — `grep -n "let _ ="` returns no results in that branch |
| `await_commit_advance_error_surfaces.rs` | `sleep`, `.modified()`, mtime | CLEAN — mtime only in doc comments |
| `famp/Cargo.toml` | `gag =` in dev-deps | CLEAN — absent |
| `try_update.rs` | All 5 test functions substantive | CLEAN — each test exercises a distinct contract with meaningful assertions |

No blockers, no warnings, no notable items.

---

## Summary

All three findings are closed:

1. **HIGH (TOCTOU race):** Closed. `tasks.try_update(...)` with FSM advance inside the closure eliminates the stale-snapshot pattern. Structural correctness verified by reading both the old pattern and the new implementation.

2. **MEDIUM (brittle mtime test):** Closed. Test uses `std::fs::read` byte-equality; no sleep, no clock dependency. Test renamed accurately.

3. **LOW (dead gag dep):** Closed. `cargo tree -i gag` returns no matches; all five silencer lines removed; `Cargo.lock` regenerated.

**Out-of-scope items confirmed untouched:** `send/mod.rs:514` `let _ = advance_terminal(...)` is present and unchanged. No 260425-ho8 commits in that file's log.

**Test count:** 396/396 passed (391 baseline + 5 new `try_update` integration tests). Clippy clean.

---

_Verified: 2026-04-25_
_Verifier: Claude (gsd-verifier)_

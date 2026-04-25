---
phase: quick-260425-lg7
plan: "01"
subsystem: famp-taskdir
tags: [rustdoc, doc-correctness, try_update, adversarial-review]
dependency_graph:
  requires: []
  provides: [tightened-try_update-closure-err-guarantee]
  affects: [crates/famp-taskdir/src/store.rs]
tech_stack:
  added: []
  patterns: []
key_files:
  modified:
    - crates/famp-taskdir/src/store.rs
decisions:
  - "Restate closure-Err bullet in function-local terms (no call to write_atomic_file) rather than file-state terms (byte-identical); auditable against the function body line-by-line"
  - "Add explicit cross-reference to # NOT guaranteed section in the new bullet so future reviewers understand why byte-state framing was deliberately avoided"
metrics:
  duration: "~4 minutes"
  completed: "2026-04-25"
  tasks_completed: 1
  files_modified: 1
---

# Phase quick-260425-lg7 Plan 01: Tighten try_update Closure-Err Guarantee Bullet

**One-liner:** Replaced overreaching "byte-identical / on-disk file is" language in `try_update`'s `# Guaranteed` section with auditable function-local fact: no call to `write_atomic_file` on the closure-Err path, with cross-reference to `# NOT guaranteed`.

## What Was Done

Single bullet in `crates/famp-taskdir/src/store.rs` (lines 138-141 before edit) was reworded to close the MEDIUM finding from round-3 adversarial review on `try_update`.

**Before:**
```rust
/// - **Closure errors prevent the disk write**: if the closure returns
///   `Err(E)`, NO call to [`write_atomic_file`] occurs. The on-disk file
///   is byte-identical to its pre-call state. The error is surfaced to
///   the caller as [`TryUpdateError::Closure`].
```

**After:**
```rust
/// - **Closure errors skip the write step**: if the closure returns
///   `Err(E)`, `try_update` performs no call to [`write_atomic_file`] and
///   returns immediately. The error is surfaced to the caller as
///   [`TryUpdateError::Closure`]. (What the on-disk file's bytes are at
///   that point is explicitly out of scope — see `# NOT guaranteed` below,
///   which explains that this method takes no file lock.)
```

## Verification Results

| Check | Result |
|-------|--------|
| `cargo nextest run --workspace` | 396/396 passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Clean |
| `cargo doc -p famp-taskdir --no-deps` | Clean |
| `grep -E "byte-identical\|on-disk file is"` | No matches |
| `git diff --stat` | 1 file, 1 bullet changed |

## Deviations from Plan

None — plan executed exactly as written. Single bullet replaced, no other lines touched.

## Self-Check: PASSED

- File modified: `crates/famp-taskdir/src/store.rs` — confirmed present
- Commit `cf29196` — confirmed in git log
- Banned phrases absent from file — confirmed by grep (exit 1 = no matches)
- 396/396 tests pass — no regression

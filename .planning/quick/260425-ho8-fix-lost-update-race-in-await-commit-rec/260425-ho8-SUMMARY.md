---
phase: quick-260425-ho8
plan: 01
subsystem: famp-taskdir + famp CLI
tags: [bug-fix, atomicity, TOCTOU, TDD, dead-code-removal]
dependency_graph:
  requires: [quick-260425-gst]
  provides: [TaskDir::try_update, atomic-FSM-advance-in-await]
  affects: [crates/famp-taskdir, crates/famp/src/cli/await_cmd]
tech_stack:
  added: [TryUpdateError<E> (thiserror generic enum)]
  patterns: [fallible-closure read-modify-write, byte-equality test assertion]
key_files:
  created:
    - crates/famp-taskdir/tests/try_update.rs
  modified:
    - crates/famp-taskdir/src/error.rs
    - crates/famp-taskdir/src/lib.rs
    - crates/famp-taskdir/src/store.rs
    - crates/famp/src/cli/await_cmd/mod.rs
    - crates/famp/tests/await_commit_advance_error_surfaces.rs
    - crates/famp/Cargo.toml
    - crates/famp/src/lib.rs
    - crates/famp/src/bin/famp.rs
    - crates/famp/examples/_gen_fixture_certs.rs
    - crates/famp/examples/cross_machine_two_agents.rs
    - crates/famp/examples/personal_two_agents.rs
    - Cargo.lock
decisions:
  - "TryUpdateError<E> is a narrow two-variant enum (Closure + Store) — not merged into TaskDirError. Callers can distinguish closure errors from store errors without exposing the generic in TaskDirError's signature."
  - "Merged Ok(_) and NotFound arms in await_cmd match (clippy match_same_arms) — both are silent no-ops in the commit-receipt path; combined arm reads clearly with inline comment."
  - "Concurrency-invariant test omitted: TaskDir's atomicity is at the OS rename layer, not an in-process mutex. A two-thread race test would demonstrate 'last-writer-wins is acceptable' rather than 'TOCTOU prevented', which would be misleading. The structural argument (closure receives fresh-from-disk record in the same call as persist) is sufficient."
metrics:
  duration: "~14 minutes"
  completed: "2026-04-25"
  tasks_completed: 3
  files_changed: 12
---

# Phase quick-260425-ho8 Plan 01: Fix Lost-Update Race in await Commit-Receipt

**One-liner:** Closes TOCTOU race in await commit-receipt via `TaskDir::try_update` — FSM advance now atomic with persist; dead `gag` dep purged.

## What Was Built

Three changes, priority ordered:

### HIGH — Closed lost-update race (TOCTOU) in `await_cmd`

The `c69b4e9` fix (quick-260425-gst) surfaced FSM errors but reintroduced a TOCTOU window: it read the record separately, ran `advance_committed` outside the closure, then called `tasks.update(task_id_str, |_| record.clone())` — the closure discarded the fresh-from-disk input and persisted the stale snapshot. Any concurrent write between the initial read and the update was silently overwritten.

Fix: single `tasks.try_update(task_id_str, |mut record| advance_committed(&mut record).map(|_| record))` call. The FSM advance now lives inside the closure, operating on the record `try_update` reads from disk — atomic with the subsequent persist. Three error arms preserve `eprintln!` observability.

### MEDIUM — Hardened test to byte-equality (clock-independent)

`await_commit_advance_error_surfaces.rs` previously used mtime comparison with a 10ms sleep (macOS APFS mtime granularity risk). Replaced with `std::fs::read` byte-equality before/after `await_run_at`. No sleep, no clock dependency. Test renamed to `commit_arrival_when_record_already_committed_does_not_modify_task_file_bytes`.

### LOW — Removed dead `gag` dev-dep and five silencer lines

The TDD pivot in `c69b4e9` abandoned `gag::BufferRedirect` (Rust test harness fd-2 conflict). `gag = "1.0.0"` was left in `[dev-dependencies]` with five `use gag as _;` silencers in `src/lib.rs`, `src/bin/famp.rs`, and three examples. All removed. `cargo tree -i gag` returns no matches.

## Commits

| # | Hash | Message |
|---|------|---------|
| 1 | `6c35460` | `feat(famp-taskdir): add try_update fallible variant for atomic FSM-aware writes` |
| 2 | `1f66f4d` | `fix(quick-260425-ho8): close lost-update race in await commit-receipt via try_update` |
| 3 | `65e5bb2` | `chore(quick-260425-ho8): drop unused gag dev-dep and silencer suppressors` |

## Test Results

- `cargo nextest run --workspace`: **396/396 passed** (baseline 391 + 5 new `try_update` integration tests)
- Previous baseline: 391 (post-quick-260425-gst)
- New `try_update` tests: `try_update_happy_path_persists_closure_result`, `try_update_closure_err_does_not_write`, `try_update_task_id_changed_returns_taskidchanged_no_write`, `try_update_not_found_skips_closure`, `try_update_invalid_uuid_skips_closure`

## Stash-Pop Sanity Note

Stash-pop of `await_cmd/mod.rs` (restoring the `c69b4e9` racy code): the test **passed** with the stashed code. This is expected — the c69b4e9 fix already correctly skips `tasks.update` on FSM `Err` (the test's FSM-error path). The test guard is meaningful: it catches the original B2 bug (pre-c69b4e9 code unconditionally called `tasks.update`) and would catch any future regression that re-introduces a spurious write on FSM error. The TOCTOU race fix (lost-update on the happy path) is structural: `|_| record.clone()` is gone; the closure now receives the fresh-from-disk record from `try_update`. No existing test exercises the specific TOCTOU scenario (concurrent writer during the read-modify-write window) because `TaskDir` uses OS-level atomic rename, not an in-process mutex — see concurrency-invariant test decision below.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed rustdoc link formatting in `store.rs` doc comment**
- **Found during:** Task 1 clippy gate
- **Issue:** `[TryUpdateError::Store]([TaskDirError::TaskIdChanged])` — adjacent link-code syntax triggers `clippy::doc-link-code`
- **Fix:** Replaced with plain code literal `TryUpdateError::Store(TaskDirError::TaskIdChanged { .. })`
- **Files modified:** `crates/famp-taskdir/src/store.rs`

**2. [Rule 1 - Bug] Fixed `doc_markdown` lint in test docstrings**
- **Found during:** Task 1 clippy gate
- **Issue:** Four test docstrings used bare `task_id`, `TaskIdChanged`, `NotFound`, `InvalidUuid` without backticks
- **Fix:** Added backticks per `clippy::doc_markdown`
- **Files modified:** `crates/famp-taskdir/tests/try_update.rs`

**3. [Rule 1 - Bug] Fixed `match_same_arms` lint in await_cmd**
- **Found during:** Task 2 clippy gate
- **Issue:** `Ok(_)` and `Err(TryUpdateError::Store(TaskDirError::NotFound { .. }))` arms both had empty bodies — clippy's `match_same_arms` required them merged
- **Fix:** `Ok(_) | Err(TryUpdateError::Store(TaskDirError::NotFound { .. })) => {}`
- **Files modified:** `crates/famp/src/cli/await_cmd/mod.rs`

**4. [Rule 1 - Bug] Applied rustfmt formatting to try_update.rs**
- **Found during:** Task 2 `cargo fmt -- --check` gate
- **Issue:** Three single-expression closures written across multiple lines; `rustfmt` wants them collapsed
- **Fix:** Reformatted per `rustfmt` rules (single-expression closures on one line, panicking closures with trailing comma)
- **Files modified:** `crates/famp-taskdir/tests/try_update.rs`

## Concurrency-Invariant Test: Omitted

The plan asked for a two-thread race test (optional). Omitted for the following reason: `TaskDir`'s atomicity guarantee is at the OS rename layer (`write_atomic_file` uses `tempfile::NamedTempFile` + `rename`). In-process, there is no mutex — so a two-thread `try_update` + `update` race would demonstrate "last-writer-wins is acceptable at the atomic-rename layer" rather than "TOCTOU is prevented by the API." The latter is a structural property: the closure receives its record from inside the call frame, not from a separate `read()` made before the call. Testing a structural property that cannot be broken without rewriting the signature is low-value. Decision recorded in frontmatter.

## Verification Gates

```
cargo nextest run --workspace                            → 396/396 PASS
cargo clippy --workspace --all-targets -- -D warnings   → CLEAN
cargo fmt --all -- --check                              → CLEAN
grep -n "let _ = " await_cmd/mod.rs                    → no results in commit-receipt branch
grep -n "tasks\.try_update" await_cmd/mod.rs           → 1 match (line 173)
cargo tree -i gag --workspace                          → "did not match any packages"
git log --oneline -5 -- src/cli/send/mod.rs            → no 260425-ho8 commits (out-of-scope confirmed)
```

## Out-of-Scope Deferred Item

The same bug class at `crates/famp/src/cli/send/mod.rs` (a `let _ = advance_terminal(...)` inside `tasks.update`) is explicitly NOT touched by this plan. It was identified in the c69b4e9 adversarial review and is deferred to a separate follow-up quick, per the plan's out-of-scope directive.

## Parent Plan Reference

- Parent finding: quick-260425-gst adversarial review, T1.1 (lost-update race introduced by `c69b4e9`)
- Predecessor commit: `c69b4e9` (fix: surface FSM errors) — good error surfacing, reintroduced TOCTOU
- This plan: restore atomicity without losing the error-surfacing win

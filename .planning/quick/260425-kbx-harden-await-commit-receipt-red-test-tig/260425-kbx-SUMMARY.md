---
phase: quick-260425-kbx
plan: 01
subsystem: famp-taskdir / famp-cli-tests
tags: [testing, rustdoc, try_update, sentinel, RED-test, discrimination]
dependency_graph:
  requires: [quick-260425-ho8]
  provides: [discriminating-RED-test-for-try_update-closure-err, honest-try_update-rustdoc]
  affects: [crates/famp/tests/await_commit_advance_error_surfaces.rs, crates/famp-taskdir/src/store.rs]
tech_stack:
  added: []
  patterns: [sentinel-comment discrimination, TOML comment survival as no-write proof]
key_files:
  created: []
  modified:
    - crates/famp/tests/await_commit_advance_error_surfaces.rs
    - crates/famp-taskdir/src/store.rs
decisions:
  - Path A (sentinel-based RED guard) chosen over Path B (delete integration test):
      the integration test exercises full wiring beyond the try_update API boundary
      (envelope parsing, find_match shaping, commit-class match arm, eprintln paths);
      the sentinel approach is a strict improvement at negligible cost.
  - Module-level const SENTINEL to satisfy clippy::items_after_statements.
  - Private-item intra-doc link [Self::path_for] changed to code-formatted
      `Self::path_for` to eliminate a pre-existing rustdoc warning.
metrics:
  duration: ~15 minutes
  completed: "2026-04-25T18:53:39Z"
  tasks_completed: 2
  files_changed: 2
---

# Phase quick-260425-kbx Plan 01: Harden await commit-receipt RED test + tighten try_update rustdoc

**One-liner:** Sentinel-based TOML comment discrimination replaces toothless byte-equality assertion; explicit `# NOT guaranteed` section eliminates concurrency overstatement in `try_update` rustdoc.

## Parent Context

Follow-up to quick-260425-ho8 (close lost-update race in await commit-receipt via try_update, commits `6c35460 / 1f66f4d / 65e5bb2`). Addresses round-2 adversarial-review findings:

- **MEDIUM:** RED test did not discriminate the bug class it claimed to test.
- **LOW:** `try_update` rustdoc overstated concurrency guarantees.

## Commits

| Task | Commit | Message |
|------|--------|---------|
| 1 — Sentinel RED test | `004ea87` | `test(quick-260425-kbx): harden await commit-receipt RED test with sentinel discriminator` |
| 2 — Rustdoc tighten | `36d6b72` | `docs(famp-taskdir): tighten try_update rustdoc to disclaim cross-writer atomicity` |

## Path A vs Path B Decision

Path A (add sentinel) was chosen over Path B (delete integration test) for a single clear reason: the integration test in `await_commit_advance_error_surfaces.rs` exercises wiring that the 5 unit tests in `famp-taskdir/tests/try_update.rs` do NOT cover — specifically, envelope parsing through `find_match`, the commit-class match arm dispatch in `await_cmd::run_at`, and the two `eprintln!` error-surface paths. The API-boundary proof (unit tests) and the wiring proof (integration test) are complementary, not redundant. Deleting the integration test removes wiring coverage even though the API contract holds at the `famp-taskdir` boundary. Adding a sentinel is structurally trivial (`OpenOptions::append` + one `String::contains` assertion) and turns a decorative test into a strict discrimination test. Path A is a strict improvement at low cost.

## Stash-Pop Sanity Outcome

**Setup:** applied the PRE-c69b4e9 buggy shape to `await_cmd/mod.rs` — replaced the `try_update` block with:

```rust
if let Ok(mut record) = tasks.read(task_id_str) {
    let _ = advance_committed(&mut record);
    let _ = tasks.update(task_id_str, |_| record.clone());
}
```

Under this shape, `advance_committed` returns `Err(IllegalTransition)` (record already COMMITTED) but the error is swallowed. `tasks.update` is then called unconditionally with the unmodified record. `toml::to_string(&record)` emits clean TOML without the sentinel comment. File is rewritten. Sentinel is clobbered.

**FAIL output (buggy shape):**

```
thread 'commit_arrival_when_record_already_committed_does_not_rewrite_task_file' panicked at crates/famp/tests/await_commit_advance_error_surfaces.rs:185:5:
sentinel was clobbered: a write occurred during await commit-receipt handling when the FSM advance returned Err. Bytes pre/post:
---PRE---
task_id = "019dc5f7-f458-76f1-b927-3e1eacb2393c"
state = "COMMITTED"
peer = "self"
opened_at = "2026-04-25T00:00:00Z"
terminal = false

# TEST_SENTINEL_DO_NOT_REWRITE

---POST---
task_id = "019dc5f7-f458-76f1-b927-3e1eacb2393c"
state = "COMMITTED"
peer = "self"
opened_at = "2026-04-25T00:00:00Z"
terminal = false

--- (quick-260425-kbx — RED guard for try_update closure-Err contract)
```

**After restore (post-ho8 `try_update` wiring):** `1 test run: 1 passed, 0 skipped`

The sentinel SURVIVED — no write occurred — because `try_update` closure returned `Err(IllegalTransition)` and the persist was skipped.

## Workspace Gate Results

- `cargo nextest run --workspace`: **396/396 passed, 2 skipped** — count unchanged (sentinel hardening modifies one existing test, does not add or remove tests).
- `cargo clippy --workspace --all-targets -- -D warnings`: **clean**
- `cargo doc -p famp-taskdir --no-deps`: **renders without warnings** (fixed pre-existing private-item link warning on `Self::path_for` as a side-effect of the docstring rewrite).
- `cargo fmt --all -- --check`: **clean** — no drive-by reformatting.

## Verification Checks

```
grep -n "TEST_SENTINEL_DO_NOT_REWRITE" crates/famp/tests/await_commit_advance_error_surfaces.rs
# → 3 matches: const declaration (line 82), pre-await assert (line 130), post-await assert (line 190)

grep -n "NOT guaranteed" crates/famp-taskdir/src/store.rs
# → 1 match (line 148, in try_update rustdoc)

grep -n "Atomic with respect to the read step\|No TOCTOU window between" crates/famp-taskdir/src/store.rs
# → 0 matches (misleading phrases removed)

git log --oneline -1 -- crates/famp/src/cli/await_cmd/mod.rs crates/famp/src/cli/send/mod.rs
# → most recent commit is 1f66f4d (fix quick-260425-ho8) — no 260425-kbx commits
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing correctness] Moved SENTINEL const to module level**
- **Found during:** Task 1 clippy run
- **Issue:** `clippy::items_after_statements` triggered because `const SENTINEL` was declared inside the test function body after executable statements. This is a pedantic clippy rule (`-D clippy::pedantic` is in workspace config).
- **Fix:** Moved `const SENTINEL` to module level with a brief comment explaining the placement, per the lint's intent.
- **Files modified:** `crates/famp/tests/await_commit_advance_error_surfaces.rs`
- **Commit:** `004ea87`

**2. [Rule 1 - Bug] Fixed pre-existing rustdoc private-item link warning**
- **Found during:** Task 2 `cargo doc` run
- **Issue:** The original docstring had `[`Self::path_for`]` as an intra-doc link, but `path_for` is a private method. This generated `warning: public documentation for 'try_update' links to private item`. The warning was pre-existing in the original docstring; the docstring rewrite was the right time to fix it.
- **Fix:** Changed to code-formatted `` `Self::path_for` `` (backtick, no bracket-link).
- **Files modified:** `crates/famp-taskdir/src/store.rs`
- **Commit:** `36d6b72`

## Out of Scope (confirmed untouched)

- `crates/famp/src/cli/send/mod.rs:514` — same bug class, separate quick task.
- `crates/famp/src/cli/await_cmd/mod.rs` — structural fix is correct; only temporarily modified during stash-pop sanity then fully restored.
- `crates/famp-taskdir/tests/try_update.rs` — the 5 unit tests are the API-boundary proof; not modified.
- File locking / CAS for `try_update` — separate design discussion per task brief.

## Self-Check: PASSED

- `crates/famp/tests/await_commit_advance_error_surfaces.rs` — exists and contains `TEST_SENTINEL_DO_NOT_REWRITE` (3 matches).
- `crates/famp-taskdir/src/store.rs` — exists and contains `NOT guaranteed` (1 match in try_update rustdoc).
- Commit `004ea87` — exists on main (`git log --oneline -1` confirms).
- Commit `36d6b72` — exists on main (`git log --oneline -1` confirms).
- `await_cmd/mod.rs` and `send/mod.rs` — no 260425-kbx commits in their log.

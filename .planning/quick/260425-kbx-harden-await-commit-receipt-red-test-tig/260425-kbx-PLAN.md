---
phase: quick-260425-kbx
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/famp/tests/await_commit_advance_error_surfaces.rs
  - crates/famp-taskdir/src/store.rs
autonomous: true
requirements:
  - QUICK-260425-KBX-T1.1
  - QUICK-260425-KBX-T1.2
must_haves:
  truths:
    - "When the closure passed to `try_update` returns `Err`, the underlying file bytes are byte-identical to their pre-call state — provable via a sentinel that ONLY survives if no write occurs (re-serialized identical TOML bytes would CLOBBER the sentinel)."
    - "The await commit-receipt RED test in `await_commit_advance_error_surfaces.rs` discriminates between (a) the OLD bug class (closure returns unmodified record, `tasks.update` re-serializes byte-identical TOML, sentinel clobbered) and (b) the post-fix behavior (closure returns `Err`, no write, sentinel survives) — proven via stash-pop sanity."
    - "The `try_update` rustdoc states only what is actually guaranteed: closure receives a fresh-from-disk `TaskRecord`, closure errors prevent the disk write. It explicitly disclaims atomicity against concurrent external writers (no file locking, no CAS)."
    - "Full workspace passes `cargo nextest run --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo doc -p famp-taskdir` (no warnings)."
    - "Stash-pop sanity proves the new test FAILS with `await_cmd/mod.rs` reverted to the c69b4e9 racy pattern, and PASSES with the current (post-ho8) `try_update` wiring."
  artifacts:
    - path: "crates/famp/tests/await_commit_advance_error_surfaces.rs"
      provides: "Discriminating RED test that uses an out-of-band sentinel (trailing comment line on the task TOML) to distinguish 'closure returned Err → no write' from 'closure returned unchanged record → re-serialized identical bytes'"
      contains: "TEST_SENTINEL_DO_NOT_REWRITE"
    - path: "crates/famp-taskdir/src/store.rs"
      provides: "Tightened try_update rustdoc — guaranteed properties + explicit non-guarantees (no file locking, no CAS, not safe vs concurrent external writers)"
      contains: "NOT guaranteed"
  key_links:
    - from: "crates/famp/tests/await_commit_advance_error_surfaces.rs"
      to: "crates/famp/src/cli/await_cmd/mod.rs"
      via: "Drives the IllegalTransition branch end-to-end; sentinel survival proves try_update closure-Err path executed with NO disk write"
      pattern: "TEST_SENTINEL_DO_NOT_REWRITE"
    - from: "crates/famp-taskdir/src/store.rs::try_update"
      to: "rustdoc readers (cargo doc, IDE hover)"
      via: "Honest precision about what try_update DOES and DOES NOT guarantee re: concurrent external writers"
      pattern: "NOT guaranteed"
---

<objective>
Two follow-ups from the adversarial review of quick-260425-ho8 (round 2):

1. **MEDIUM — RED test does not discriminate the bug it claims to test.** The current test in `crates/famp/tests/await_commit_advance_error_surfaces.rs` snapshots file bytes before/after `await_run_at` and asserts equality. Under the OLD (pre-c69b4e9) bug, `advance_committed` would return `Err` but the closure passed to `tasks.update` returned the unmodified record; `tasks.update` then re-serialized the same `TaskRecord` to TOML — and the resulting bytes would be byte-identical to the original. **The test would PASS under the old buggy code.** The structural fix is correct (proven by the 5 unit tests in `crates/famp-taskdir/tests/try_update.rs` which assert "closure-Err → no write" at the API boundary), but the integration test as written has zero discriminating power.

2. **LOW — `try_update` rustdoc overstates concurrency guarantees.** The current docstring (`crates/famp-taskdir/src/store.rs:123-145`) uses language like "Atomic with respect to the read step" and "No TOCTOU window between the read and the persist." This is technically true *within a single `try_update` call*, but a casual reader (or future executor) could infer atomicity against concurrent external writers — which `try_update` does NOT provide. There is no file lock, no CAS; a concurrent writer between the closure-internal `self.read(task_id)` (line 155) and `write_atomic_file` (line 172) WILL be silently overwritten. The fix is honest precision, no behavior change.

**Path A vs Path B decision (per task brief):** Path A — sentinel-based RED guard. Justification:

> The integration test in `await_commit_advance_error_surfaces.rs` exercises the FULL wiring: envelope injection into `inbox.jsonl` → `find_match` parse → commit-class branch dispatch in `await_cmd::run_at` → `try_update` invocation → closure-Err path → no-write contract. The 5 unit tests in `try_update.rs` test the `try_update` API in isolation; they do NOT cover the wiring (envelope parsing, find_match shaping, the commit-class match arm, the error-arm `eprintln!` paths). Deleting the integration test (Path B) removes wiring coverage even though the API contract holds. Adding a sentinel (Path A) is structurally trivial — `setup_home` already gives a writable dir, and the test already manually seeds the TOML via `tasks.create()`; appending a sentinel comment line via `OpenOptions::append` is one block. The sentinel survives only if no write occurs (TOML re-serialization would not preserve a non-TOML trailing comment, and even if it did, serde_toml's round-trip would emit a sorted/reformatted body that would not byte-match the sentinel-bearing original). Path A is a strict improvement over the current toothless byte-equality assertion at low cost.

Out of scope (DO NOT touch):
- `crates/famp/src/cli/send/mod.rs:514` (same bug class — separate quick).
- `crates/famp-taskdir/src/store.rs::try_update` body (only docstring tightens; behavior unchanged).
- `crates/famp/src/cli/await_cmd/mod.rs` (the structural fix is correct; do not modify).
- The 5 unit tests in `crates/famp-taskdir/tests/try_update.rs` (they ARE the API-boundary proof).
- File locking / CAS for `try_update` (separate design discussion).
- `cargo fmt --all` on unrelated files (surgical scope per CLAUDE.md).

Output:
- A discriminating RED test that demonstrably FAILS under the c69b4e9 racy pattern and PASSES under the current `try_update` wiring (stash-pop sanity captured in SUMMARY).
- Tightened `try_update` rustdoc that distinguishes guaranteed contracts from non-guarantees (concurrent-external-writer behavior).
- Surgical: every changed line traces to one of the two findings.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@CLAUDE.md
@.planning/quick/260425-ho8-fix-lost-update-race-in-await-commit-rec/260425-ho8-PLAN.md
@.planning/quick/260425-ho8-fix-lost-update-race-in-await-commit-rec/260425-ho8-SUMMARY.md
@.planning/quick/260425-ho8-fix-lost-update-race-in-await-commit-rec/260425-ho8-VERIFICATION.md

@crates/famp/tests/await_commit_advance_error_surfaces.rs
@crates/famp-taskdir/src/store.rs
@crates/famp-taskdir/tests/try_update.rs

<interfaces>
<!-- Key APIs the executor will use. Extracted from codebase — no exploration needed. -->

From crates/famp-taskdir/src/store.rs (current — try_update CURRENT docstring + signature):
```rust
impl TaskDir {
    /// Read → fallible-mutate → atomic write. Returns the persisted record.
    ///
    /// This is the fallible sibling of [`Self::update`] for callers that need
    /// to run a `Result`-returning mutation (e.g. an FSM advance that can
    /// return `Err`) atomically inside the same read+write critical section.
    ///
    /// Contracts:
    ///
    /// - **Atomic with respect to the read step**: the closure receives the
    ///   on-disk record and the persisted bytes derive from the closure's
    ///   returned record — NOT a stale snapshot from a separate [`Self::read`]
    ///   call made before this method. No TOCTOU window between the read and
    ///   the persist.
    /// - **On `Err` from the closure, NO disk write occurs**; the original
    ///   file is byte-identical to its pre-call state.
    /// - **`task_id` must be stable**: if the closure returns `Ok(record)`
    ///   whose `task_id` differs from the input, the call returns
    ///   `TryUpdateError::Store(TaskDirError::TaskIdChanged { .. })` with no
    ///   write — same invariant as [`Self::update`].
    ///
    /// Reuses [`Self::read`], [`Self::path_for`], and [`write_atomic_file`]
    /// — same atomicity/fsync/locking semantics as [`Self::update`]; no
    /// duplicated code.
    pub fn try_update<E, F>(&self, task_id: &str, mutate: F) -> Result<TaskRecord, TryUpdateError<E>>
    where F: FnOnce(TaskRecord) -> Result<TaskRecord, E>;
}
```

From crates/famp/tests/await_commit_advance_error_surfaces.rs (lines 66-153 — CURRENT structure):
```rust
#[tokio::test(flavor = "current_thread")]
async fn commit_arrival_when_record_already_committed_does_not_modify_task_file_bytes() {
    let tmp = setup_home();
    let home = tmp.path();

    // 1. Create a task record already in COMMITTED state.
    let task_id = uuid::Uuid::now_v7().to_string();
    let record = TaskRecord::new_committed(...);
    let tasks_dir = home.join("tasks");
    let tasks = TaskDir::open(&tasks_dir).unwrap();
    tasks.create(&record).expect("create task record");

    // 2. Snapshot the task file bytes BEFORE calling await_run_at.
    let task_file = tasks_dir.join(format!("{task_id}.toml"));
    let bytes_before = std::fs::read(&task_file).expect("read task file before await");

    // 3. Inject a synthetic commit-class envelope into inbox.jsonl ...
    // 4. Run await_run_at.
    // 5. Assert byte equality + state still COMMITTED.
}
```

Key insight for sentinel design: TOML's serde round-trip serializer (`toml::to_string(&record)`) produces deterministic bytes derived from the `TaskRecord` struct fields. A trailing comment line like `# TEST_SENTINEL_DO_NOT_REWRITE` is NOT part of the `TaskRecord` deserialized form — it lives in the file but never round-trips through `serde_toml`. Therefore:
- If `try_update` performs ANY write (whether the closure returns Ok or Err but the bug-path swallows that and writes anyway), the file is rewritten via `write_atomic_file(path, toml::to_string(&record).as_bytes())`. The sentinel comment is GONE.
- If `try_update` performs NO write (post-fix closure-Err path), the file is untouched. The sentinel SURVIVES.

This makes the sentinel a discriminating proof — strictly stronger than byte-equality on its own (which fails to discriminate when re-serialization yields identical bytes).

From CLAUDE.md (project) — Conventions:
- "surgical changes: every changed line must trace directly to the user's request"
- "data validation: silent fallthrough on bad values is a bug"
- "code comments: verify comments match actual implementation"

From CLAUDE.md (global) — Anti-patterns:
- "decorative tests": test that exists for optics but does not discriminate the bug class
- "stale comments mislead AI review pipelines and propagate errors"
</interfaces>

</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Harden RED test with out-of-band sentinel + stash-pop sanity</name>
  <files>crates/famp/tests/await_commit_advance_error_surfaces.rs</files>
  <behavior>
    Replace the toothless byte-equality assertion with a sentinel-based discriminating RED guard.

    **The discrimination property the test must establish:**

    | Code path | Behavior | Sentinel survives? | Test result |
    |---|---|---|---|
    | OLD bug (pre-c69b4e9) | Closure returns unmodified record; `tasks.update` re-serializes identical TOML; sentinel comment is NOT part of the serialized struct → file rewritten WITHOUT sentinel | NO | FAIL |
    | Post-c69b4e9 (current `await_cmd` wiring) | `try_update` closure returns `Err(IllegalTransition)`; NO disk write occurs | YES | PASS |
    | Hypothetical regression (any future code that swallows the FSM Err and writes anyway) | File rewritten via TOML serialization; sentinel gone | NO | FAIL |

    **Test structure (replace lines 66-153 — keep the surrounding silencer block + docstring intact, but UPDATE the docstring text per step 4):**

    ```rust
    #[tokio::test(flavor = "current_thread")]
    async fn commit_arrival_when_record_already_committed_does_not_rewrite_task_file() {
        let tmp = setup_home();
        let home = tmp.path();

        // 1. Create a task record already in COMMITTED state.
        let task_id = uuid::Uuid::now_v7().to_string();
        let record = TaskRecord::new_committed(
            task_id.clone(),
            "self".to_string(),
            "2026-04-25T00:00:00Z".to_string(),
        );
        let tasks_dir = home.join("tasks");
        let tasks = TaskDir::open(&tasks_dir).unwrap();
        tasks.create(&record).expect("create task record");

        // 2. Inject an out-of-band sentinel into the task file. This sentinel
        //    is a trailing TOML COMMENT line — it lives in the file bytes but
        //    is NOT part of the TaskRecord struct, so any future serde_toml
        //    re-serialization will omit it. Survival of this sentinel after
        //    `await_run_at` runs is a discriminating proof of "no write".
        //
        //    Why a comment line specifically: TOML allows trailing comments;
        //    `toml::from_str` parses TaskRecord from the body just fine; the
        //    sentinel does NOT affect deserialization. But `toml::to_string(&record)`
        //    has no knowledge of comments and emits clean serialized output
        //    without it. So:
        //      - No write → sentinel survives (PASS).
        //      - Any write (whether benign re-serialize or buggy spurious
        //        write) → sentinel clobbered (FAIL).
        let task_file = tasks_dir.join(format!("{task_id}.toml"));
        const SENTINEL: &str = "\n# TEST_SENTINEL_DO_NOT_REWRITE\n";
        {
            use std::io::Write as _;
            let mut f = std::fs::OpenOptions::new()
                .append(true)
                .open(&task_file)
                .expect("open task file for sentinel append");
            f.write_all(SENTINEL.as_bytes())
                .expect("append sentinel to task file");
        }

        // Sanity: confirm the sentinel is present pre-await, and the record
        // still parses (sentinel is a valid TOML comment).
        let pre = std::fs::read_to_string(&task_file).expect("read pre-await");
        assert!(
            pre.contains("TEST_SENTINEL_DO_NOT_REWRITE"),
            "sentinel must be present BEFORE await_run_at runs (test setup integrity check)"
        );
        let _parse_check: TaskRecord =
            toml::from_str(&pre).expect("sentinel must not break TOML parsing");

        // 3. Inject a synthetic commit-class envelope into inbox.jsonl so
        //    find_match picks it up and the commit-receipt branch fires.
        let inbox_path = home.join("inbox.jsonl");
        let envelope_id = uuid::Uuid::now_v7().to_string();
        let line = serde_json::json!({
            "famp": "0.5.1",
            "id": envelope_id,
            "class": "commit",
            "from": "agent:localhost/self",
            "to": "agent:localhost/self",
            "causality": { "ref": task_id },
            "body": {}
        });
        {
            use std::io::Write as _;
            let mut inbox_file = std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&inbox_path)
                .expect("open inbox.jsonl for append");
            writeln!(inbox_file, "{}", serde_json::to_string(&line).unwrap())
                .expect("write inbox line");
        }

        // 4. Run await_run_at. The commit envelope triggers the FSM branch;
        //    advance_committed returns Err(IllegalTransition) because the
        //    record is already COMMITTED. Under the post-ho8 `try_update`
        //    wiring, no disk write occurs.
        let mut out: Vec<u8> = Vec::new();
        await_run_at(
            home,
            AwaitArgs {
                timeout: "2s".to_string(),
                task: Some(task_id.clone()),
            },
            &mut out,
        )
        .await
        .expect("await_run_at should succeed (not crash on FSM error)");

        // 5. Assertions — sentinel survival is the discriminating proof.

        // 5a. await_run_at returned Ok above — loop continues on FSM error.

        // 5b. The sentinel must still be present. If a write occurred (whether
        //     the OLD bug-class re-serialization or any future spurious write),
        //     the sentinel comment would be gone because serde_toml does not
        //     preserve TOML comments.
        let post = std::fs::read_to_string(&task_file).expect("read post-await");
        assert!(
            post.contains("TEST_SENTINEL_DO_NOT_REWRITE"),
            "sentinel was clobbered: a write occurred during await commit-receipt \
             handling when the FSM advance returned Err. Bytes pre/post:\n\
             ---PRE---\n{pre}\n---POST---\n{post}\n--- \
             (quick-260425-kbx — RED guard for try_update closure-Err contract)"
        );

        // 5c. On-disk state must be COMMITTED (record must still parse + value unchanged).
        let rec = TaskDir::open(&tasks_dir).unwrap().read(&task_id).unwrap();
        assert_eq!(
            rec.state, "COMMITTED",
            "state must be unchanged on FSM error (no double-write, no corruption)"
        );
    }
    ```

    Test cases:
    - This single test replaces the existing `commit_arrival_when_record_already_committed_does_not_modify_task_file_bytes`. Renamed to `commit_arrival_when_record_already_committed_does_not_rewrite_task_file` (the new name is more accurate: we are now proving "no rewrite" via sentinel survival, not "byte equality").
    - Existing happy-path test in `conversation_auto_commit::auto_commit_round_trip` MUST continue to PASS unchanged (the sentinel hardening only affects this one test file).
  </behavior>
  <action>
    1. **Edit `crates/famp/tests/await_commit_advance_error_surfaces.rs`**:

       a. **Update the file-level docstring (lines 1-54)**: change the "Observable: byte equality (not mtime)" section to describe the sentinel approach. Keep the History section intact (it's accurate). Replace the "Observable" section with:

       ```
       //! ## Observable: sentinel survival (not just byte equality)
       //!
       //! The pre-260425-kbx version of this test snapshotted file bytes before
       //! and after `await_run_at` and asserted byte equality. That assertion
       //! was insufficient to discriminate the bug it claims to test: under the
       //! pre-c69b4e9 bug, `tasks.update` was called with `|_| record.clone()`
       //! after `advance_committed` returned `Err` — but the cloned record was
       //! UNMODIFIED, so `toml::to_string(&record)` produced byte-identical
       //! output to the original on-disk TOML. Byte equality would PASS under
       //! the old buggy code.
       //!
       //! The fix (quick-260425-kbx): inject a TRAILING TOML COMMENT into the
       //! task file out-of-band before invoking `await_run_at`. TOML comments
       //! are valid input (deserialization unaffected) but `toml::to_string`
       //! does NOT preserve them on round-trip. Therefore:
       //!
       //! - **No write occurred**: sentinel comment SURVIVES → test PASSES.
       //! - **Any write occurred** (benign re-serialize OR buggy spurious
       //!   write): sentinel comment is CLOBBERED → test FAILS.
       //!
       //! This is a strict discrimination test: it FAILS under the pre-c69b4e9
       //! racy/buggy code path even when bytes would otherwise be identical.
       ```

       b. **Replace the test function body (lines 66-153)** with the sentinel-based version specified in `<behavior>` above.

       c. **Rename the test** to `commit_arrival_when_record_already_committed_does_not_rewrite_task_file`.

       d. **Imports and silencers**: leave the silencer block at the bottom (lines 156-187) intact. The `std::io::Write` import at line 60 is still needed for the inbox writeln (use `use std::io::Write as _;` if the new closures need it; the inline `use std::io::Write as _;` blocks shown above are local — fine either way, prefer one top-level `use std::io::Write as _;` to keep diff minimal). Verify no new external crates are required (sentinel approach uses only `std::fs`, `std::io`, already-imported `serde_json`, `toml`, `uuid`, `famp_taskdir`).

    2. **Verify (TDD discipline — sentinel test must fail RED before fix wiring is verified)**:

       a. **Initial green check** (current `await_cmd/mod.rs` is the post-ho8 fix, sentinel test should PASS):
       ```
       cargo nextest run -p famp --test await_commit_advance_error_surfaces
       ```
       MUST PASS. If it fails, the test setup is wrong (likely sentinel parsing or file race) — fix before proceeding.

       b. **Stash-pop sanity (REQUIRED)**: revert `crates/famp/src/cli/await_cmd/mod.rs` to the c69b4e9 racy pattern temporarily and confirm the test FAILS:

       ```
       # Save current await_cmd state
       git stash push -m "kbx-stash-pop-sanity" -- crates/famp/src/cli/await_cmd/mod.rs

       # Apply the c69b4e9 racy pattern manually:
       # In the commit-class branch (~line 168-198), replace the try_update
       # block with:
       #
       #   if let Ok(mut record) = tasks.read(task_id_str) {
       #       match advance_committed(&mut record) {
       #           Ok(_) => {
       #               if let Err(e) = tasks.update(task_id_str, |_| record.clone()) {
       #                   eprintln!("famp await: failed to persist commit-advance for task {task_id_str}: {e}");
       #               }
       #           }
       #           Err(e) => {
       #               eprintln!("famp await: advance_committed failed for task {task_id_str}: {e}");
       #           }
       #       }
       #   }
       #
       # NOTE: this revert is the "Ok arm calls update unconditionally" shape.
       # Under this code path, when the record is already COMMITTED,
       # advance_committed returns Err — `tasks.update` is NOT called, so the
       # sentinel SURVIVES. Test would still PASS. That tells us the c69b4e9
       # shape (which already had the Err arm correct) does not exercise the
       # "spurious write under FSM Err" failure mode for the COMMITTED-record
       # scenario.
       #
       # To prove discrimination, revert to the PRE-c69b4e9 (genuinely buggy)
       # shape: replace the entire commit-class branch with:
       #
       #   if let Ok(mut record) = tasks.read(task_id_str) {
       #       let _ = advance_committed(&mut record);
       #       let _ = tasks.update(task_id_str, |_| record.clone());
       #   }
       #
       # Under this shape, advance_committed returns Err but is swallowed;
       # tasks.update is then called UNCONDITIONALLY with the unmodified record;
       # toml::to_string emits clean TOML; sentinel is CLOBBERED → test FAILS.

       # Edit await_cmd/mod.rs manually to apply the PRE-c69b4e9 buggy shape above.
       cargo nextest run -p famp --test await_commit_advance_error_surfaces
       # MUST FAIL with sentinel-clobber message.

       # Restore: discard the manual edits, then pop the stash.
       git checkout -- crates/famp/src/cli/await_cmd/mod.rs
       git stash pop
       cargo nextest run -p famp --test await_commit_advance_error_surfaces
       # MUST PASS again.
       ```

       Capture the FAIL output (assertion message + bytes-pre/bytes-post) and the subsequent PASS output. Both go in the SUMMARY.

       **If the executor finds the manual stash-pop edit awkward** (Rust file edits during a stash flow), an alternative is acceptable: copy `await_cmd/mod.rs` to a backup path, apply the buggy shape, run the test, restore from backup, run the test again. The mechanism doesn't matter — the discrimination proof matters.

       c. **Workspace gate**:
       ```
       cargo nextest run --workspace
       cargo clippy --workspace --all-targets -- -D warnings
       cargo fmt --all -- --check
       ```
       All green. Format check should report no diff (this plan only touches files it explicitly lists).

    3. **Commit**:
       ```
       git add crates/famp/tests/await_commit_advance_error_surfaces.rs
       git commit -m "test(quick-260425-kbx): harden await commit-receipt RED test with sentinel discriminator

       The pre-kbx version asserted bytes_before == bytes_after after
       running the await commit-receipt path against an already-COMMITTED
       record. Under the pre-c69b4e9 bug, advance_committed returned Err
       but the closure passed to tasks.update returned the unmodified
       record; tasks.update then re-serialized byte-identical TOML. The
       byte-equality assertion would PASS under the old buggy code — the
       test had zero discriminating power.

       This commit injects a TRAILING TOML COMMENT into the task file
       out-of-band before invoking await_run_at. TOML comments are valid
       input but toml::to_string does not preserve them on round-trip.
       Therefore:
         - No write occurred → sentinel survives → test PASSES.
         - Any write (benign re-serialize or buggy spurious write) →
           sentinel clobbered → test FAILS.

       Stash-pop sanity confirmed: under the pre-c69b4e9 buggy shape
       (let _ = advance_committed; let _ = tasks.update), the test FAILS
       with sentinel-clobber message. Under the current post-ho8 shape
       (try_update with FSM advance inside the closure), the test PASSES.

       Test renamed:
         commit_arrival_when_record_already_committed_does_not_modify_task_file_bytes
         → commit_arrival_when_record_already_committed_does_not_rewrite_task_file"
       ```

    DO NOT:
    - Touch `crates/famp/src/cli/await_cmd/mod.rs` permanently (only the temporary stash-pop revert during sanity check).
    - Delete the integration test (Path B was rejected — see Objective).
    - Modify `crates/famp-taskdir/tests/try_update.rs` (those 5 unit tests are the API-boundary proof and stay untouched).
    - Add a `sha2` dep for hashing (sentinel survival check is a `String::contains`; no hash needed).
    - Run `cargo fmt --all` on unrelated files.
  </action>
  <verify>
    <automated>cargo nextest run -p famp --test await_commit_advance_error_surfaces && cargo nextest run --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check</automated>
  </verify>
  <done>
    - `crates/famp/tests/await_commit_advance_error_surfaces.rs` test renamed to `commit_arrival_when_record_already_committed_does_not_rewrite_task_file`.
    - Test injects a `\n# TEST_SENTINEL_DO_NOT_REWRITE\n` trailing comment via `OpenOptions::append` immediately after `tasks.create(&record)`.
    - Test asserts the sentinel SURVIVES post-`await_run_at` via `String::contains`.
    - Test asserts the seeded `TaskRecord` still parses from the sentinel-bearing TOML pre-await (setup integrity guard).
    - Test asserts on-disk state is still `COMMITTED` post-await.
    - File-level docstring updated to describe sentinel approach + reference quick-260425-kbx.
    - Stash-pop sanity (manual revert of `await_cmd/mod.rs` to pre-c69b4e9 buggy shape) demonstrably FAILS the test with sentinel-clobber message; restoration PASSES it. Both outputs captured for SUMMARY.
    - `cargo nextest run --workspace` passes (test count: 396 baseline, no change — same one test, just hardened).
    - `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - `cargo fmt --all -- --check` clean (no drive-by reformatting).
    - Single commit `test(quick-260425-kbx): harden await commit-receipt RED test with sentinel discriminator` exists on `main`.
  </done>
</task>

<task type="auto">
  <name>Task 2: Tighten try_update rustdoc to disclaim concurrent-writer guarantees</name>
  <files>crates/famp-taskdir/src/store.rs</files>
  <action>
    The current `try_update` rustdoc (lines 123-145) leans on phrases like "Atomic with respect to the read step" and "No TOCTOU window between the read and the persist." Both phrases are technically true *within a single `try_update` invocation*, but a casual reader can infer that `try_update` provides atomicity against concurrent EXTERNAL writers. It does not. There is no file lock, no compare-and-swap; a concurrent writer between the closure-internal `self.read(task_id)` (line 155) and `write_atomic_file` (line 172) WILL be silently overwritten — same TOCTOU class as the original `await_cmd` bug, just with a smaller window.

    Tighten the docstring to be honest. Behavior-preserving (this is a doc-only change).

    1. **Edit `crates/famp-taskdir/src/store.rs`** lines 123-145. Replace the current docstring block on `try_update` with the following. Preserve the function signature and body unchanged below the docstring.

       ```rust
       /// Read → fallible-mutate → atomic write. Returns the persisted record.
       ///
       /// This is the fallible sibling of [`Self::update`] for callers that need
       /// to run a `Result`-returning mutation (e.g. an FSM advance that can
       /// return `Err`) in the same call as the persist, so the closure operates
       /// on a fresh-from-disk record rather than a caller-cached snapshot.
       ///
       /// # Guaranteed
       ///
       /// - **Closure receives a fresh-from-disk record**: the closure's input
       ///   `TaskRecord` comes from this method's own internal [`Self::read`]
       ///   call, NOT from a separate `read()` made by the caller before
       ///   invoking `try_update`. This eliminates the in-process stale-snapshot
       ///   pattern (caller does `read` → mutate → `update(|_| cached.clone())`,
       ///   discarding the closure's input).
       /// - **Closure errors prevent the disk write**: if the closure returns
       ///   `Err(E)`, NO call to [`write_atomic_file`] occurs. The on-disk file
       ///   is byte-identical to its pre-call state. The error is surfaced to
       ///   the caller as [`TryUpdateError::Closure`].
       /// - **`task_id` stability**: if the closure returns `Ok(record)` whose
       ///   `task_id` differs from the input, the call returns
       ///   [`TryUpdateError::Store`] wrapping
       ///   [`TaskDirError::TaskIdChanged`] with no write — same invariant as
       ///   [`Self::update`].
       ///
       /// # NOT guaranteed
       ///
       /// - **No protection against concurrent external writers**: this method
       ///   does NOT take a file lock and does NOT use compare-and-swap. A
       ///   concurrent writer (another process, or another thread holding a
       ///   different `TaskDir` handle) that modifies the same file between
       ///   this method's internal `read` and its `write_atomic_file` will be
       ///   silently overwritten. `try_update` closes the *in-process*
       ///   stale-snapshot anti-pattern; it does not provide cross-writer
       ///   linearizability.
       /// - **No retry on conflict**: there is no detect-and-retry loop. If
       ///   that semantic is needed in the future, add it as a separate API
       ///   (e.g. `update_with_retry` or an explicit OS-level lock).
       ///
       /// # Implementation
       ///
       /// Reuses [`Self::read`], [`Self::path_for`], and [`write_atomic_file`]
       /// — same single-call atomicity (rename-into-place) as [`Self::update`];
       /// no duplicated code.
       ```

       Specifically:
       - Drop the phrase "Atomic with respect to the read step" — it is ambiguous and overstates.
       - Drop the phrase "No TOCTOU window between the read and the persist" — false in the cross-writer sense.
       - Add explicit `# Guaranteed` and `# NOT guaranteed` sections so the contract is unmistakable.

    2. **Verify**:
       ```
       cargo doc -p famp-taskdir --no-deps                 # MUST render without warnings
       cargo clippy -p famp-taskdir --all-targets -- -D warnings
       cargo nextest run -p famp-taskdir                   # all 5 try_update tests still pass (no behavior change)
       cargo nextest run --workspace                       # full workspace still green
       cargo fmt --all -- --check                          # clean
       ```

       Pay attention to `cargo doc`: rustdoc intra-doc links can be brittle. The links `[`Self::update`]`, `[`Self::read`]`, `[`Self::path_for`]`, `[`write_atomic_file`]`, `[`TryUpdateError::Closure`]`, `[`TryUpdateError::Store`]`, `[`TaskDirError::TaskIdChanged`]` should all resolve. If any fail, fix by either qualifying the path or falling back to plain code formatting.

    3. **Commit**:
       ```
       git add crates/famp-taskdir/src/store.rs
       git commit -m "docs(famp-taskdir): tighten try_update rustdoc to disclaim cross-writer atomicity

       The previous docstring used phrases like 'Atomic with respect to the
       read step' and 'No TOCTOU window between the read and the persist' —
       technically true within a single try_update invocation, but easy to
       misread as protection against concurrent external writers. There is
       no file lock and no CAS; a concurrent writer between the internal
       read (store.rs:155) and write_atomic_file (store.rs:172) WILL be
       silently overwritten. try_update closes the in-process stale-snapshot
       anti-pattern that bit await_cmd in c69b4e9; it does not provide
       cross-writer linearizability.

       Replaced with explicit '# Guaranteed' and '# NOT guaranteed' sections
       per CLAUDE.md (code comments must match actual implementation;
       avoid overstated authority language until protocol-validated).

       Behavior unchanged. cargo doc -p famp-taskdir renders clean."
       ```

    DO NOT:
    - Modify the `try_update` body, signature, or return type.
    - Modify the `update` method or its docstring (out of scope).
    - Touch any other rustdoc in the file.
    - Add file locking or CAS implementation (separate design discussion).
  </action>
  <verify>
    <automated>cargo doc -p famp-taskdir --no-deps && cargo clippy -p famp-taskdir --all-targets -- -D warnings && cargo nextest run --workspace</automated>
  </verify>
  <done>
    - `crates/famp-taskdir/src/store.rs` `try_update` docstring (lines ~123-145) replaced with structure containing explicit `# Guaranteed` and `# NOT guaranteed` sections.
    - "Atomic with respect to the read step" and "No TOCTOU window between the read and the persist" phrases are GONE.
    - New docstring explicitly states: no file lock, no CAS, concurrent external writers are NOT prevented.
    - `cargo doc -p famp-taskdir --no-deps` renders without warnings.
    - `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - `cargo nextest run --workspace` green (no behavior change).
    - Single commit `docs(famp-taskdir): tighten try_update rustdoc to disclaim cross-writer atomicity` exists on `main`.
  </done>
</task>

</tasks>

<verification>
End-to-end gate (run after both tasks land):

1. `cargo nextest run --workspace` — green. Test count unchanged from kbx baseline (396); the sentinel hardening modifies one existing test, does not add or remove tests.

2. `cargo clippy --workspace --all-targets -- -D warnings` — clean.

3. `cargo doc -p famp-taskdir --no-deps` — renders without warnings (intra-doc links resolve).

4. `cargo fmt --all -- --check` — clean. CRITICAL: only files this plan modifies were touched by formatting.

5. `grep -n "TEST_SENTINEL_DO_NOT_REWRITE" crates/famp/tests/await_commit_advance_error_surfaces.rs` — returns at least 3 matches (constant declaration, pre-await assertion, post-await assertion).

6. `grep -n "NOT guaranteed" crates/famp-taskdir/src/store.rs` — returns at least 1 match in the `try_update` rustdoc.

7. `grep -n "Atomic with respect to the read step\|No TOCTOU window between" crates/famp-taskdir/src/store.rs` — returns NO matches (the misleading phrases are gone).

8. **Stash-pop sanity** (already executed during Task 1, but document the outcome — re-run if needed):
   - With `await_cmd/mod.rs` reverted to pre-c69b4e9 buggy shape (`let _ = advance_committed; let _ = tasks.update`), `cargo nextest run -p famp --test await_commit_advance_error_surfaces` MUST FAIL with the sentinel-clobber assertion message.
   - With `await_cmd/mod.rs` restored to the current post-ho8 `try_update` wiring, the same command MUST PASS.

9. **Out-of-scope verification** — confirm `crates/famp/src/cli/send/mod.rs` and `crates/famp/src/cli/await_cmd/mod.rs` are NOT permanently modified by this plan:
   ```
   git log --oneline -5 -- crates/famp/src/cli/await_cmd/mod.rs crates/famp/src/cli/send/mod.rs
   # Should show no commits from this plan's quick (260425-kbx).
   ```
</verification>

<success_criteria>
- The await commit-receipt RED test in `await_commit_advance_error_surfaces.rs` is now discriminating: it FAILS under the pre-c69b4e9 buggy shape (proven via stash-pop sanity) and PASSES under the current post-ho8 `try_update` wiring. The sentinel-comment approach is structurally sound: TOML comments are valid input but `toml::to_string` does not preserve them, so any spurious re-serialization clobbers the sentinel.
- The `try_update` rustdoc no longer overstates concurrency guarantees. Explicit `# Guaranteed` and `# NOT guaranteed` sections distinguish in-process stale-snapshot prevention (the actual win) from cross-writer linearizability (NOT provided). Honest precision per CLAUDE.md.
- Zero collateral changes: `await_cmd/mod.rs` and `send/mod.rs` are NOT modified by this plan; the 5 try_update unit tests in `famp-taskdir/tests/try_update.rs` are NOT modified; no drive-by `cargo fmt`; no new dependencies.
- Two commits on `main` (one per task), each with conventional-commit format and a body explaining what/why/impact per CLAUDE.md commit conventions.
- All workspace gates green: `cargo nextest run --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo doc -p famp-taskdir --no-deps`, `cargo fmt --all -- --check`.
</success_criteria>

<output>
After completion, create `.planning/quick/260425-kbx-harden-await-commit-receipt-red-test-tig/260425-kbx-SUMMARY.md` capturing:
- Two commit SHAs (one per task) with one-line summaries.
- Path A vs Path B decision rationale (one paragraph): why the sentinel-based RED guard was chosen over deletion of the integration test.
- Stash-pop sanity outcome: the exact failure message captured when `await_cmd/mod.rs` was reverted to the pre-c69b4e9 buggy shape (`let _ = advance_committed; let _ = tasks.update`), and confirmation the test re-greens after restoring the post-ho8 wiring.
- Confirmation that `cargo nextest run --workspace` is green (test count: 396, unchanged from kbx baseline since the sentinel hardening modifies one existing test rather than adding new ones).
- Confirmation that `cargo doc -p famp-taskdir --no-deps` renders without warnings.
- Pointer back to the c69b4e9 adversarial-review findings (round 2) and the parent quick (260425-ho8).
- Note that `crates/famp/src/cli/send/mod.rs` (same bug class at line 514) and any concurrent-writer protection in `try_update` (file locking / CAS) remain explicitly out of scope per the task brief.
</output>

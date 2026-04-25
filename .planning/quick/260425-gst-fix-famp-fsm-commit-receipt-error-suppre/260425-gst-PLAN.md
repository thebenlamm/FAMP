---
phase: quick-260425-gst
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/famp/src/cli/await_cmd/mod.rs
  - crates/famp/tests/await_commit_advance_error_surfaces.rs
autonomous: true
requirements:
  - QUICK-260425-GST-T1.1
must_haves:
  truths:
    - "When advance_committed() returns Err during commit-receipt handling, the error is surfaced (logged via eprintln!) instead of being silently swallowed."
    - "When tasks.update() fails to persist the COMMITTED transition, the error is surfaced (logged) instead of being silently swallowed."
    - "The await loop does NOT crash on a commit-receipt FSM error — it continues so the user can recover; the error is observable in stderr."
    - "The originator's task TOML transitions REQUESTED → COMMITTED on a successful commit-receipt (existing happy path in conversation_auto_commit.rs still passes)."
  artifacts:
    - path: "crates/famp/src/cli/await_cmd/mod.rs"
      provides: "Commit-receipt FSM advance with surfaced errors"
      contains: "eprintln!"
    - path: "crates/famp/tests/await_commit_advance_error_surfaces.rs"
      provides: "TDD test proving FSM-error path no longer silent"
      contains: "advance_committed"
  key_links:
    - from: "crates/famp/src/cli/await_cmd/mod.rs"
      to: "crates/famp/src/cli/send/fsm_glue.rs::advance_committed"
      via: "direct call; Err propagated to caller scope (not let _ =)"
      pattern: "advance_committed\\("
    - from: "crates/famp/tests/await_commit_advance_error_surfaces.rs"
      to: "crates/famp/src/cli/await_cmd/mod.rs::run_at"
      via: "calls await_run_at on a synthetic state where local task is already COMMITTED but a second commit envelope arrives — IllegalTransition path"
      pattern: "await_run_at"
---

<objective>
Fix the silent error suppression in `crates/famp/src/cli/await_cmd/mod.rs:163-172` (bug B2 from the 2026-04-25 pressure test). Two layers of `let _ = ...` discard errors from `advance_committed()` and `tasks.update()`. When `advance_committed()` returns `Err` (e.g., IllegalTransition because the local record isn't in REQUESTED), the originator's task TOML is never updated and stays stale forever, with zero observability.

Purpose: restore observability of FSM transition failures during commit-receipt handling without crashing the await loop. Surgical fix; no protocol or FSM-engine changes.

Output:
- A failing TDD test (today) that becomes passing after the fix, locking the "errors must surface" contract.
- Restructured await loop branch that logs both classes of error to stderr but continues the loop.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@CLAUDE.md
@~/.claude/plans/ok-now-analyze-and-toasty-waffle.md

@crates/famp/src/cli/await_cmd/mod.rs
@crates/famp/src/cli/send/fsm_glue.rs
@crates/famp/tests/conversation_auto_commit.rs
@crates/famp/tests/common/conversation_harness.rs

<interfaces>
<!-- Key APIs the executor will use. Extracted from codebase — no exploration needed. -->

From crates/famp/src/cli/send/fsm_glue.rs:
```rust
/// Advance a task record from REQUESTED → COMMITTED on receiving a
/// `MessageClass::Commit` envelope. Returns the new `TaskState`.
///
/// Precondition: `record.state == "REQUESTED"`. An FSM in any other state
/// will return `TaskFsmError::IllegalTransition` mapped to `CliError::Envelope`.
pub fn advance_committed(record: &mut TaskRecord) -> Result<TaskState, CliError>;
```

From crates/famp-taskdir/src/store.rs:
```rust
pub fn read(&self, task_id: &str) -> Result<TaskRecord, TaskDirError>;

// NOTE: closure signature is FnOnce(TaskRecord) -> TaskRecord, NOT Result.
// You CANNOT propagate an FSM error through the closure return.
// Required pattern: do FSM advance OUTSIDE the closure, only call update() on success.
pub fn update<F>(&self, task_id: &str, mutate: F) -> Result<TaskRecord, TaskDirError>
where F: FnOnce(TaskRecord) -> TaskRecord;
```

From crates/famp/src/cli/await_cmd/mod.rs (current buggy code at lines 163-172):
```rust
if class == "commit" && !task_id_str.is_empty() {
    let tasks_dir = paths::tasks_dir(home);
    if let Ok(tasks) = TaskDir::open(&tasks_dir) {
        if tasks.read(task_id_str).is_ok() {
            let _ = tasks.update(task_id_str, |mut r| {
                let _ = advance_committed(&mut r);   // <-- swallows FSM Err
                r
            });                                       // <-- swallows update Err
        }
    }
}
```

From crates/famp/tests/common/conversation_harness.rs (available helpers):
```rust
pub fn setup_home() -> tempfile::TempDir;
// plus: add_self_peer, await_once, deliver, new_task, read_task, stop_listener, update_peer_endpoint
```

From famp_taskdir::TaskRecord (relevant fields):
```rust
pub struct TaskRecord {
    pub task_id: String,
    pub state: String,         // "REQUESTED" | "COMMITTED" | "COMPLETED" | "FAILED" | "CANCELLED"
    pub terminal: bool,
    // ... other fields
}
```
</interfaces>

</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Write failing test that proves FSM errors are silently swallowed</name>
  <files>crates/famp/tests/await_commit_advance_error_surfaces.rs</files>
  <behavior>
    A new integration test that exercises the IllegalTransition path inside `await_cmd::run_at`'s commit-receipt handling and asserts the error becomes observable.

    Strategy (no daemon needed — drives the buggy branch directly):
    1. Create a temp `FAMP_HOME` via `setup_home()`.
    2. Manually construct a `TaskRecord` already in `state = "COMMITTED"` (i.e., NOT `REQUESTED`) and write it to the taskdir using `TaskDir::open(...).create(record)` so the on-disk state is already past the legal commit-arrival point.
    3. Manually append a synthetic commit-class JSON line to `inbox.jsonl` with `task_id` matching the record above, plus the minimum fields `find_match` and the FSM branch read: `class`, `task_id`, `from`, `body`, `offset` — model the shape after how `conversation_auto_commit.rs` consumes await output (an object with `class`, `task_id`, `from`, `body`).
    4. Call `await_run_at(home, AwaitArgs { timeout: "2s", task: Some(task_id) }, &mut buf)`.
       - The inbox entry IS matched by `find_match`, so `await` returns Ok and prints the line.
       - Internally, `advance_committed()` MUST return `Err(IllegalTransition)` because the record is already COMMITTED.
    5. Capture stderr during the `await_run_at` call (via `gag` is overkill — instead, simply use a Rust pattern: spawn a thread that reads `os_pipe` redirecting stderr; OR, simpler, refactor: assert the on-disk state is unchanged AND assert no panic).

    SIMPLER ASSERTION SHAPE (do this, avoids stderr-capture complexity):
    - Pre-fix (today, FAILING): the test asserts BOTH that `await_run_at` returns Ok AND that on disk the record state is still "COMMITTED" (unchanged) AND that some new observable signal exists. Since today's code silently succeeds and there is NO signal at all, the test must fail on the "signal exists" assertion.
    - Post-fix (after Task 2): the same assertion passes because Task 2 introduces the signal (eprintln! to stderr OR — preferred — a side-channel observable in test).

    RECOMMENDED SIGNAL CHOICE: the fix in Task 2 will use `eprintln!`. The test captures stderr by redirecting via `std::os::unix::io::AsRawFd` + `dup2` to a `tempfile::tempfile()`, runs `await_run_at`, restores stderr, and reads the captured bytes. Assert the captured stderr contains the substring `advance_committed` AND the task_id.

    Test cases:
    - Test 1 (`commit_arrival_when_record_already_committed_logs_error_and_continues`):
      Pre-state COMMITTED + arriving commit envelope → `await_run_at` returns Ok (prints the matched line, doesn't crash) AND captured stderr contains an FSM-error log line referencing the task_id. On-disk state remains "COMMITTED" (unchanged, no double-write).
  </behavior>
  <action>
    Create `crates/famp/tests/await_commit_advance_error_surfaces.rs`:

    1. Mirror the file headers, `#![cfg(unix)]`, `#![allow(...)]`, `mod common;` declaration, and trailing `use ... as _;` silencer block from `conversation_auto_commit.rs`. Reuse the same dependencies (the silencer list is required to keep `unused_crate_dependencies` quiet).

    2. Use `common::conversation_harness::setup_home` for the temp `FAMP_HOME`.

    3. Create the on-disk task record directly via `famp_taskdir::TaskDir::open(home.join("tasks"))` then `.create(TaskRecord { task_id, state: "COMMITTED", terminal: false, ... })`. Set every required field on `TaskRecord` — read the struct definition from `crates/famp-taskdir/src/record.rs` (or wherever `TaskRecord` is defined) and populate minimally valid values. Use a freshly-generated UUIDv7 task_id (`uuid::Uuid::now_v7().to_string()`).

    4. Append one synthetic JSON line to `home.join("inbox.jsonl")` with:
       ```json
       {"class":"commit","task_id":"<task_id>","from":"agent:localhost/self","body":{}}
       ```
       Use `tokio::fs::write` or `std::fs::OpenOptions::new().append(true).create(true).open(...)`. Add a trailing `\n`. (No signature verification on the file — we're testing await, not listen.)

    5. Capture stderr around `await_run_at`:
       ```rust
       use std::os::unix::io::{AsRawFd, FromRawFd};
       let stderr_fd = std::io::stderr().as_raw_fd();
       let saved = unsafe { libc::dup(stderr_fd) };
       let mut tmp = tempfile::tempfile().unwrap();
       unsafe { libc::dup2(tmp.as_raw_fd(), stderr_fd); }
       // ... run await_run_at ...
       unsafe { libc::dup2(saved, stderr_fd); libc::close(saved); }
       use std::io::{Read, Seek, SeekFrom};
       tmp.seek(SeekFrom::Start(0)).unwrap();
       let mut captured = String::new();
       tmp.read_to_string(&mut captured).unwrap();
       ```
       (Add `libc` to `[dev-dependencies]` of `crates/famp/Cargo.toml` if not already present — it's `unix`-only so this is acceptable for a `cfg(unix)` test.)

    6. Call `await_run_at(home, AwaitArgs { timeout: "2s".into(), task: Some(task_id.clone()) }, &mut buf).await.expect("await ok")`.

    7. Assertions:
       ```rust
       assert!(captured.contains(&task_id),
               "stderr must reference task_id; got: {captured}");
       assert!(captured.contains("advance_committed") || captured.contains("commit-advance"),
               "stderr must reference the failing operation; got: {captured}");
       let rec = TaskDir::open(home.join("tasks")).unwrap().read(&task_id).unwrap();
       assert_eq!(rec.state, "COMMITTED",
                  "state must be unchanged on FSM error (no double-write)");
       ```

    8. Run the test and CONFIRM IT FAILS today:
       `cargo nextest run -p famp --test await_commit_advance_error_surfaces`
       Expected pre-fix failure: `captured` is empty (today's code emits nothing), so the `contains(&task_id)` assertion fires. This proves the test is meaningful.

    9. Commit the failing test:
       `git add crates/famp/tests/await_commit_advance_error_surfaces.rs crates/famp/Cargo.toml`
       `git commit -m "test(quick-260425-gst): add failing test for await commit-advance error surfacing"`
       (Use `--no-verify`-free path — pre-commit hooks should pass since this is a new test file with valid Rust; if `clippy -D warnings` fires on the test, fix the lint, do NOT bypass.)

    REFERENCES (do not reinvent):
    - `crates/famp/tests/conversation_auto_commit.rs` for harness/silencer patterns.
    - `crates/famp/tests/common/conversation_harness.rs` for `setup_home`.
    - Existing dev-dependencies in `crates/famp/Cargo.toml` — likely already include `tempfile`, `tokio`, `serde_json`, `uuid`. Add `libc` if missing.

    DO NOT:
    - Spin up a daemon (we're testing one branch of `await_cmd::run_at`, not the round-trip).
    - Reuse `auto_commit_round_trip` — that test exercises the happy path; we need the IllegalTransition path.
    - Refactor anything in `await_cmd/mod.rs` in this task — Task 2 owns that.
  </action>
  <verify>
    <automated>cargo nextest run -p famp --test await_commit_advance_error_surfaces 2>&1 | grep -E "FAILED|test result" | head -5</automated>
  </verify>
  <done>
    - File `crates/famp/tests/await_commit_advance_error_surfaces.rs` exists and compiles (`cargo check -p famp --tests` clean).
    - Running `cargo nextest run -p famp --test await_commit_advance_error_surfaces` produces 1 FAILED test (the assertion that captured stderr contains the task_id fails because today's code emits nothing).
    - Failure message clearly indicates "stderr must reference task_id" — confirms the test exercises the right code path.
    - Test committed with message `test(quick-260425-gst): add failing test for await commit-advance error surfacing`.
  </done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: Replace `let _ = ...` swallowing with explicit error logging</name>
  <files>crates/famp/src/cli/await_cmd/mod.rs</files>
  <behavior>
    The buggy block at lines 163-172 must be restructured so:
    - If `advance_committed()` returns `Err`, log it via `eprintln!` with both the task_id and the underlying error (Display).
    - If `tasks.update()` returns `Err`, log it via `eprintln!` with both the task_id and the underlying error.
    - The await loop continues normally in both error cases (does NOT propagate the error up — we still want to print the inbox line and return Ok, so the consumer sees the commit envelope arrived).
    - Happy path is preserved exactly: `advance_committed` succeeds → `tasks.update` succeeds → record on disk transitions REQUESTED → COMMITTED.

    Constraint from `TaskDir::update` API: closure signature is `FnOnce(TaskRecord) -> TaskRecord` (NOT `Result`). Therefore the FSM advance MUST happen OUTSIDE the closure. Pattern:
    1. Read the record.
    2. Call `advance_committed(&mut record)` — if `Err`, log and skip the update entirely.
    3. If Ok, call `tasks.update(task_id, |_| record.clone())` to persist; if that fails, log.

    Test cases (must pass after this task):
    - Test 1 (`await_commit_advance_error_surfaces::commit_arrival_when_record_already_committed_logs_error_and_continues`): captured stderr contains task_id and "advance_committed" substring; on-disk state unchanged.
    - Test 2 (existing `conversation_auto_commit::auto_commit_round_trip`): still passes — happy path REQUESTED → COMMITTED works.
  </behavior>
  <action>
    Edit `crates/famp/src/cli/await_cmd/mod.rs` lines 163-172. Replace the current branch:

    ```rust
    if class == "commit" && !task_id_str.is_empty() {
        let tasks_dir = paths::tasks_dir(home);
        if let Ok(tasks) = TaskDir::open(&tasks_dir) {
            if tasks.read(task_id_str).is_ok() {
                let _ = tasks.update(task_id_str, |mut r| {
                    let _ = advance_committed(&mut r);
                    r
                });
            }
        }
    }
    ```

    With:

    ```rust
    if class == "commit" && !task_id_str.is_empty() {
        let tasks_dir = paths::tasks_dir(home);
        match TaskDir::open(&tasks_dir) {
            Ok(tasks) => match tasks.read(task_id_str) {
                Ok(mut record) => {
                    // Run the FSM advance OUTSIDE the update closure so we
                    // can observe the result. TaskDir::update's closure is
                    // FnOnce(TaskRecord) -> TaskRecord with no Result, so
                    // an in-closure error has nowhere to go.
                    match advance_committed(&mut record) {
                        Ok(_) => {
                            if let Err(e) = tasks.update(task_id_str, |_| record.clone()) {
                                eprintln!(
                                    "famp await: failed to persist commit-advance for task {task_id_str}: {e}"
                                );
                            }
                        }
                        Err(e) => {
                            eprintln!(
                                "famp await: advance_committed failed for task {task_id_str}: {e}"
                            );
                        }
                    }
                }
                Err(_) => {
                    // No matching local record — not our task; nothing to advance.
                    // (Matches prior behavior: the old `if tasks.read(...).is_ok()`
                    // also silently skipped this case, which is correct — a commit
                    // envelope for someone else's task is not an error here.)
                }
            },
            Err(e) => {
                eprintln!(
                    "famp await: failed to open task dir while handling commit for {task_id_str}: {e}"
                );
            }
        }
    }
    ```

    Notes for the executor:
    - `record.clone()` requires `TaskRecord: Clone`. Verify by checking `crates/famp-taskdir/src/record.rs` (or wherever `TaskRecord` is defined). If `Clone` is missing, ADD `#[derive(Clone)]` — it's a pure data struct and Clone is appropriate. (If Clone is undesirable for some reason, the alternative is to wrap `record` in `Option` and `take()` it inside the closure: `let mut slot = Some(record); tasks.update(task_id_str, |_| slot.take().unwrap())`. Prefer the Clone path if it works.)
    - Do NOT change the surrounding `find_match` / cursor advance / writeln logic — that path is correct and load-bearing.
    - Do NOT touch any other `let _ =` patterns in this file or other files. Per task brief: out of scope.
    - Do NOT change `advance_committed`'s signature in `fsm_glue.rs` — reuse only.

    Verification steps after the edit:

    1. `cargo check -p famp` — clean.
    2. `cargo nextest run -p famp --test await_commit_advance_error_surfaces` — Test 1 now PASSES.
    3. `cargo nextest run -p famp --test conversation_auto_commit` — happy path still PASSES.
    4. `cargo nextest run --workspace` — full workspace green.
    5. `cargo clippy --workspace --all-targets -- -D warnings` — clean.

    Sanity check (proves the test is meaningful per CLAUDE.md adversarial-review pattern):
    6. `git stash` (stash the await_cmd fix only, keep the test in place):
       `git stash push -m "verify-test-meaningful" crates/famp/src/cli/await_cmd/mod.rs`
       Re-run `cargo nextest run -p famp --test await_commit_advance_error_surfaces` → MUST FAIL.
       Then `git stash pop` to restore the fix. Re-run → MUST PASS.

    Commit:
    `git add crates/famp/src/cli/await_cmd/mod.rs`
    `git commit -m "fix(quick-260425-gst): surface FSM errors in await commit-receipt handling"`
    Multi-line body explaining: bug B2 from 2026-04-25 pressure test, evidence task 019dc45c, root cause (two `let _ =` swallowing errors), fix (FSM advance outside update closure with explicit eprintln! on each failure path), test reference.
  </action>
  <verify>
    <automated>cargo nextest run -p famp --test await_commit_advance_error_surfaces --test conversation_auto_commit && cargo clippy --workspace --all-targets -- -D warnings</automated>
  </verify>
  <done>
    - `crates/famp/src/cli/await_cmd/mod.rs` lines 163-172 no longer contain `let _ =`.
    - `cargo nextest run --workspace` is fully green (no regressions).
    - `cargo clippy --workspace --all-targets -- -D warnings` is clean.
    - Test 1 from Task 1 now passes (was failing pre-fix).
    - Stash-pop sanity check confirms the test fails when the fix is removed (proves test integrity, per CLAUDE.md).
    - Commit `fix(quick-260425-gst): surface FSM errors in await commit-receipt handling` exists on `main`.
  </done>
</task>

</tasks>

<verification>
End-to-end gate (run in order):

1. `cargo nextest run --workspace` — every test green, including the new `await_commit_advance_error_surfaces` test and the unchanged `conversation_auto_commit::auto_commit_round_trip`.
2. `cargo clippy --workspace --all-targets -- -D warnings` — clean.
3. `cargo fmt --all -- --check` — clean.
4. Manual stash-pop sanity (Task 2 covers this): removing the fix from `await_cmd/mod.rs` causes the new test to fail. Confirms the test is not a tautology.
5. `grep -n "let _ =" crates/famp/src/cli/await_cmd/mod.rs` returns NO results in the lines surrounding the commit-class branch (formerly 163-172). Other `let _ =` in unrelated parts of the file are explicitly out of scope.
</verification>

<success_criteria>
- Bug B2 from `~/.claude/plans/ok-now-analyze-and-toasty-waffle.md` section T1.1 is closed.
- An originator who hits an FSM error during commit-receipt handling now sees a clear stderr line naming the task_id and the underlying error — instead of a stale REQUESTED record with zero diagnostic signal.
- The await loop continues on FSM error (does not crash, does not hang) — preserves the user-facing await contract.
- TDD discipline: a meaningful failing test was committed before the fix; the same test passes after.
- Zero collateral changes — no drive-by refactors of other files or other `let _ =` patterns. Surgical, per CLAUDE.md.
- Sections T1.2 (MCP body schema) and T1.3 (redeploy script) from the parent fix plan remain untouched in this quick — they are explicit follow-up quicks.
</success_criteria>

<output>
After completion, create `.planning/quick/260425-gst-fix-famp-fsm-commit-receipt-error-suppre/260425-gst-SUMMARY.md` capturing:
- Task IDs of the two commits.
- Confirmation that `cargo nextest run --workspace` is green.
- Confirmation that the stash-pop sanity check passed.
- Pointer back to parent fix plan section T1.1.
- Note that T1.2 and T1.3 are deferred to follow-up quicks.
</output>

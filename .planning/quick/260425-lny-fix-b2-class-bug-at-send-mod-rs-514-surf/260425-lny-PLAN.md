---
phase: quick-260425-lny
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/famp/src/cli/send/mod.rs
  - crates/famp/tests/send_terminal_advance_error_surfaces.rs
autonomous: true
requirements:
  - QUICK-260425-LNY  # Fix B2-class FSM error suppression at send/mod.rs:514

must_haves:
  truths:
    - "When advance_terminal returns Err inside SendMode::DeliverTerminal's persist path, the error is surfaced to stderr (not swallowed)."
    - "When advance_terminal returns Err inside SendMode::DeliverTerminal's persist path, NO disk write occurs to the task TOML file (no spurious rewrite)."
    - "The :514 fix is structurally identical (eprintln format + match-arm shape) to await_cmd's commit-receipt branch (post-ho8)."
    - "Existing send_terminal_blocks_resend.rs happy-path test still passes (no regression on the COMMITTED → COMPLETED happy path)."
  artifacts:
    - path: "crates/famp/src/cli/send/mod.rs"
      provides: "Fixed SendMode::DeliverTerminal branch in persist_post_send using try_update + match arms."
      contains: "tasks.try_update"
    - path: "crates/famp/tests/send_terminal_advance_error_surfaces.rs"
      provides: "RED-then-GREEN sentinel test proving no spurious write on advance_terminal Err."
      contains: "TEST_SENTINEL_DO_NOT_REWRITE"
  key_links:
    - from: "crates/famp/src/cli/send/mod.rs SendMode::DeliverTerminal arm"
      to: "famp_taskdir::TaskDir::try_update + TryUpdateError variants"
      via: "match on TryUpdateError<CliError>"
      pattern: "tasks\\.try_update.*advance_terminal"
    - from: "crates/famp/src/cli/send/mod.rs error arms"
      to: "stderr via eprintln!"
      via: "eprintln! mirroring await_cmd::run_at post-ho8 commit-receipt branch"
      pattern: "eprintln!\\(\"famp send:"
---

<objective>
Fix B2-class bug at `crates/famp/src/cli/send/mod.rs:~514` — `let _ = fsm_glue::advance_terminal(&mut r);` inside the `tasks.update(...)` closure of the `SendMode::DeliverTerminal` arm of `persist_post_send`. Currently:

1. **Errors are swallowed** — if `advance_terminal` returns `Err(CliError::Envelope(IllegalTransition))` (e.g., because the on-disk record is in REQUESTED instead of COMMITTED), the error is silently discarded by `let _ =`.
2. **Spurious writes occur** — even on Err, the closure returns the (unmodified) record `r` and `tasks.update(...)` proceeds to re-serialize and atomically rewrite the file, producing a no-op-but-mutating disk write with no diagnostic.

This is the same anti-pattern that bug B2 (quick-260425-gst) fixed in `await_cmd/mod.rs`'s commit-receipt branch. That fix evolved through quick-260425-ho8 (lost-update race fix → `try_update`) and quick-260425-kbx (sentinel-discriminator RED test). This plan ports the post-ho8 / post-kbx pattern verbatim to the send-side terminal-advance branch.

**Surgical scope:**
- ONE bug site: `crates/famp/src/cli/send/mod.rs` SendMode::DeliverTerminal arm of `persist_post_send` (the `tasks.update(task_id, |mut r| { ... let _ = fsm_glue::advance_terminal(&mut r); r })` block at ~line 510-516).
- ONE new test file (sentinel-discriminator, mirrors `await_commit_advance_error_surfaces.rs`).
- ZERO other edits — no `try_update` docstring touches, no `await_cmd` touches, no drive-by audits, no fmt of unrelated files, no addressing the pre-existing `CliError::Envelope` masking IllegalTransition display issue.

Purpose: Close the same B2-class FSM error suppression on the send-side that ho8/kbx closed on the await-side. Consistency = future maintainability.

Output:
- `crates/famp/src/cli/send/mod.rs` — DeliverTerminal branch rewritten to use `tasks.try_update` with explicit `match` over `TryUpdateError` variants, mirroring `await_cmd/mod.rs:173-198` verbatim.
- `crates/famp/tests/send_terminal_advance_error_surfaces.rs` — new sentinel-discriminator integration test proving no spurious write on FSM Err.
- Stash-pop sanity capture in SUMMARY: revert the fix, observe RED; restore, observe GREEN.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
@~/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@CLAUDE.md
@crates/famp/src/cli/send/mod.rs
@crates/famp/src/cli/await_cmd/mod.rs
@crates/famp/tests/await_commit_advance_error_surfaces.rs
@crates/famp-taskdir/src/store.rs
@crates/famp/src/cli/send/fsm_glue.rs
@crates/famp/tests/send_terminal_blocks_resend.rs

<interfaces>
<!-- Key types and contracts the executor needs. Use these directly — no codebase exploration. -->

From `crates/famp-taskdir/src/store.rs`:
```rust
pub fn try_update<E, F>(
    &self,
    task_id: &str,
    mutate: F,
) -> Result<TaskRecord, TryUpdateError<E>>
where
    F: FnOnce(TaskRecord) -> Result<TaskRecord, E>;
```
Closure-Err → NO write to disk. Closure-Ok → atomic write-then-rename. `read` failure → `TryUpdateError::Store(NotFound|...)`.

From `crates/famp-taskdir/src/error.rs` (re-exported at crate root):
```rust
pub enum TryUpdateError<E> {
    Closure(E),
    Store(TaskDirError),
}
pub enum TaskDirError {
    NotFound { task_id: String },
    AlreadyExists { task_id: String },
    InvalidUuid { value: String },
    TaskIdChanged { original: String, next: String },
    TomlParse { path: PathBuf, source: toml::de::Error },
    TomlSerialize { task_id: String, source: toml::ser::Error },
    Io { path: PathBuf, source: std::io::Error },
}
```

From `crates/famp/src/cli/send/fsm_glue.rs`:
```rust
pub fn advance_terminal(record: &mut TaskRecord) -> Result<TaskState, CliError>;
// COMMITTED → COMPLETED via Deliver+Completed.
// Returns CliError::Envelope(IllegalTransition) if state != "COMMITTED".
```

From `crates/famp/src/cli/await_cmd/mod.rs:173-198` — **THE PATTERN TO MIRROR VERBATIM**:
```rust
match tasks.try_update(task_id_str, |mut record| {
    advance_committed(&mut record).map(|_| record)
}) {
    Ok(_) | Err(TryUpdateError::Store(TaskDirError::NotFound { .. })) => {}
    Err(TryUpdateError::Closure(e)) => {
        eprintln!(
            "famp await: advance_committed failed for task {task_id_str}: {e}"
        );
    }
    Err(TryUpdateError::Store(e)) => {
        eprintln!(
            "famp await: failed to persist commit-advance for task {task_id_str}: {e}"
        );
    }
}
```

From `crates/famp/src/cli/send/mod.rs:510-532` — **THE BUG SITE** (current shape):
```rust
SendMode::DeliverTerminal => {
    // Try to update existing record; if not found, create one.
    match tasks.update(task_id, |mut r| {
        r.last_send_at = Some(now_s.clone());
        let _ = fsm_glue::advance_terminal(&mut r);  // <-- BUG: error swallowed
        r                                              // <-- BUG: spurious write on Err
    }) {
        Ok(_) => {}
        Err(famp_taskdir::TaskDirError::NotFound { .. }) => {
            // Create and immediately mark terminal for this received task.
            let mut rec = TaskRecord::new_committed(
                task_id.to_string(),
                alias.to_string(),
                now_s.clone(),
            );
            rec.last_send_at = Some(now_s);
            // COMMITTED → COMPLETED is a valid FSM transition.
            fsm_glue::advance_terminal(&mut rec)?;
            tasks.create(&rec)?;
        }
        Err(e) => return Err(CliError::TaskDir(e)),
    }
}
```

The NotFound arm (the create-on-demand "I'm the responder" path) is correct and must be preserved unchanged. Only the `tasks.update(...)` block changes.

From `crates/famp/src/cli/await_cmd/mod.rs:52`:
```rust
use famp_taskdir::{TaskDir, TaskDirError, TryUpdateError};
```
The send-side will need the same import additions for `TaskDirError` and `TryUpdateError` (currently the `use famp_taskdir::...` set may already cover `TaskDirError` — check the existing imports at the top of `send/mod.rs` and only add what's missing).

From `crates/famp/tests/send_terminal_blocks_resend.rs` — **THE TEST HARNESS PATTERN TO MIRROR** (in-process listener via `run_on_listener`, send via `send_run_at`, await via `await_cmd::run_at`).
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: RED — sentinel-discriminator test that send-terminal advance_terminal Err does not rewrite the task TOML</name>
  <files>crates/famp/tests/send_terminal_advance_error_surfaces.rs</files>
  <behavior>
    Test fails BEFORE the Task 2 fix (sentinel clobbered by spurious `tasks.update` write); passes AFTER the fix (sentinel survives because `try_update` skips write on closure Err).

    Test scenario:
    1. Spin up an in-process listener (mirror `send_terminal_blocks_resend.rs:40-62` — `TcpListener::bind("127.0.0.1:0")`, `run_on_listener` task, ready-loop, `run_add_at` peer add).
    2. Send a `--new-task` via `send_run_at` to materialize a task record on disk in REQUESTED state. (Do NOT consume the auto-commit reply via `await_cmd::run_at` — leave the record in REQUESTED.)
    3. Pull `task_id` from `tasks.list()`.
    4. Inject a TOML-comment SENTINEL into the task file out-of-band:
       - Const `SENTINEL: &str = "\n# TEST_SENTINEL_DO_NOT_REWRITE\n";`
       - `OpenOptions::new().append(true).open(&task_file)` (NO `.create(true)` — file must already exist).
       - Pre-assert sentinel present + record still parses (TOML comments are valid).
    5. Call `send_run_at` with `terminal: true, task: Some(task_id), new_task: None`. The send path:
       - Pre-check at `send/mod.rs:130-144`: reads existing record, sees `terminal == false`, proceeds.
       - Builds + POSTs the deliver envelope (200 OK from the listener).
       - Hits `persist_post_send` → `SendMode::DeliverTerminal` arm at :510 → calls `advance_terminal(&mut r)` on a record in REQUESTED.
       - `advance_terminal` returns `Err(CliError::Envelope(IllegalTransition))` because the FSM rejects REQUESTED → COMPLETED via Deliver (post-Phase-4 the shortcut was removed; see `send_terminal_blocks_resend.rs:91-93` comment).
       - Pre-fix: `let _ =` swallows; `tasks.update` rewrites file (sentinel clobbered). Post-fix: `try_update` skips write (sentinel survives).
    6. Assertions:
       - `send_run_at` may return Ok (the wire-side already succeeded — we deliberately do NOT assert on the send return value's Ok/Err; the bug is about the *side effect*, not the return). If it does return Err, capture and print but do not panic — proceed to sentinel check.
       - **Primary assertion (sentinel survival):** `read_to_string(task_file).contains("TEST_SENTINEL_DO_NOT_REWRITE")` MUST be true. Failure message must include both pre and post bytes for diagnosis (mirror `await_commit_advance_error_surfaces.rs:188-195`).
       - State unchanged: `tasks.read(&task_id).state == "REQUESTED"`.
       - Inbox line count: 1 request + 1 commit-reply (from the listener auto-reply to step 2) + 1 deliver = 3 lines (as in `send_terminal_blocks_resend.rs:131-137`). Acceptable to skip this assertion if it proves brittle — sentinel survival is the load-bearing assertion.
    7. Tear down: `shutdown_tx.send(())`, `tokio::time::timeout` on `server_task`.

    Test attribute: `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` (matches `send_terminal_blocks_resend.rs:31` — required because `run_on_listener` uses tokio runtime services).

    Header attrs (mirror `await_commit_advance_error_surfaces.rs:68-69`):
    ```rust
    #![cfg(unix)]
    #![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]
    ```

    Module-level `mod common;` import (uses `common::init_home_in_process` from the existing test harness).

    `pubkey_b64` helper: copy verbatim from `send_terminal_blocks_resend.rs:24-29`.

    Bootstrap call: `famp::cli::send::client::allow_tofu_bootstrap_for_tests();` (mirror `send_terminal_blocks_resend.rs:34`).

    Silencer block at end (`use axum as _; ...`): mirror `await_commit_advance_error_surfaces.rs:206-237` and `send_terminal_blocks_resend.rs:167-192`. Include only the crates the test actually pulls transitively. Easiest: copy from `send_terminal_blocks_resend.rs` and add any missing ones (e.g., `tempfile as _;`, `humantime as _;`) the compiler complains about.

    Module-level docstring: explain that this test mirrors `await_commit_advance_error_surfaces.rs` for the send-side terminal-advance branch, references quick-260425-ho8 / kbx for the pattern lineage, and explicitly states why a TOML-comment sentinel is the discriminating proof (toml::to_string drops comments → any write clobbers sentinel).
  </behavior>
  <action>
    Create `crates/famp/tests/send_terminal_advance_error_surfaces.rs` per the behavior block above. **Do not yet write the fix in Task 2** — this task only adds the test, runs it, and confirms RED.

    1. Read `crates/famp/tests/await_commit_advance_error_surfaces.rs` and `crates/famp/tests/send_terminal_blocks_resend.rs` side-by-side. The new file is structurally a hybrid: harness setup from `send_terminal_blocks_resend.rs`, sentinel discriminator + assertion shape from `await_commit_advance_error_surfaces.rs`.

    2. Write the test file. Skeleton:
    ```rust
    #![cfg(unix)]
    #![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

    //! Sentinel-discriminator test for send-side terminal-advance error surfacing.
    //! Mirrors await_commit_advance_error_surfaces.rs (quick-260425-kbx) for the
    //! :514 site in send/mod.rs's SendMode::DeliverTerminal persist path.
    //! See quick-260425-lny task brief for full lineage.

    mod common;

    use std::io::Write as _;
    use std::net::SocketAddr;
    use std::time::Duration;

    use famp::cli::peer::add::run_add_at;
    use famp::cli::send::{run_at as send_run_at, SendArgs};
    use famp_taskdir::TaskDir;

    use common::init_home_in_process;

    const SENTINEL: &str = "\n# TEST_SENTINEL_DO_NOT_REWRITE\n";

    fn pubkey_b64(home: &std::path::Path) -> String {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine as _;
        let bytes = std::fs::read(home.join("pub.ed25519")).unwrap();
        URL_SAFE_NO_PAD.encode(bytes)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[allow(clippy::too_many_lines)]
    async fn terminal_send_when_record_in_requested_does_not_rewrite_task_file() {
        // ... (see behavior block — full body)
    }

    // Silencers (copied from send_terminal_blocks_resend.rs, extend as compiler demands).
    use axum as _;
    // ... etc.
    ```

    3. Run: `cargo nextest run -p famp --test send_terminal_advance_error_surfaces`
       Expected: TEST FAILS with "sentinel was clobbered" message.

    4. **CAPTURE THE RED OUTPUT** verbatim into a temp note (will be embedded in SUMMARY by Task 2). The output should show the assertion message including pre/post byte diff.

    5. If the test passes BEFORE the fix is applied, the test does NOT actually exercise :514 — STOP and re-examine. Likely culprits:
       - The send pre-check at :130-144 noticed something and short-circuited (e.g., the listener auto-commit raced and pushed the record to COMMITTED before our send fired — fix by NOT awaiting the commit reply, or by manually overwriting the on-disk record back to REQUESTED + terminal=false right before the terminal send).
       - The POST failed (e.g., listener not ready) — `persist_post_send` is never called; the bug never fires. Add a 2xx assertion or check listener readiness more robustly.
       - The `advance_terminal` call did NOT return Err (e.g., FSM allows REQUESTED → COMPLETED via Deliver in the current code) — verify by adding a one-shot `dbg!` of the call result; if true, force the precondition by writing the on-disk state to "FAILED" with terminal=false out-of-band (then advance_terminal will definitely error from FAILED via Deliver).

    6. Once RED is confirmed, do NOT commit yet — Task 2 will commit RED + GREEN as separate atoms.
  </action>
  <verify>
    <automated>cd /Users/benlamm/Workspace/FAMP &amp;&amp; cargo nextest run -p famp --test send_terminal_advance_error_surfaces 2>&amp;1 | tee /tmp/lny-red.log; grep -q "sentinel was clobbered\|FAILED" /tmp/lny-red.log</automated>
  </verify>
  <done>
    File `crates/famp/tests/send_terminal_advance_error_surfaces.rs` exists, compiles cleanly (no clippy warnings under the test's allow-list), and FAILS when run with a "sentinel was clobbered" or equivalent diagnostic message proving the bug at :514. RED output captured for inclusion in SUMMARY.
  </done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: GREEN — replace tasks.update with tasks.try_update + explicit match arms at send/mod.rs:510-516, mirroring await_cmd post-ho8 verbatim; commit RED then GREEN as separate atoms</name>
  <files>crates/famp/src/cli/send/mod.rs</files>
  <behavior>
    The SendMode::DeliverTerminal arm of `persist_post_send` (currently `crates/famp/src/cli/send/mod.rs:510-532`) is rewritten so that:

    - `last_send_at` continues to be set on the record (this is a legitimate mutation that was already happening).
    - `advance_terminal` runs INSIDE the `try_update` closure, so the FSM step operates on the fresh-from-disk record `try_update` reads (no in-process stale snapshot, mirroring the ho8 fix on the await side).
    - On `advance_terminal` Ok, the closure returns `Ok(record)` and `try_update` performs the atomic write (mutated `last_send_at` + new state are both persisted in one write — matches the existing happy-path semantic).
    - On `advance_terminal` Err, the closure returns `Err(e)` and `try_update` performs NO write. The error surfaces via `eprintln!` with the same format as `await_cmd/mod.rs:181-189`.
    - The existing `NotFound` create-on-demand arm (the "we're the responder" path at :518-529) is preserved BYTE-FOR-BYTE — only the `match tasks.update(...) { ... }` head and the Ok/Err arms wrapping the closure change.
    - The `Err(famp_taskdir::TaskDirError::NotFound { .. })` arm becomes `Err(TryUpdateError::Store(famp_taskdir::TaskDirError::NotFound { .. }))` — same body inside.
    - The catch-all `Err(e) => return Err(CliError::TaskDir(e))` arm is split: `Err(TryUpdateError::Store(e)) => return Err(CliError::TaskDir(e))` AND `Err(TryUpdateError::Closure(e))` is handled (eprintln + continue OR return — see "decision" below).

    **Decision required during implementation: closure-Err → return or continue?**
    Read `await_cmd/mod.rs:181-189` carefully: in await, the closure-Err case `eprintln!`s and CONTINUES (the await path then writes the structured JSON line + advances the cursor + returns Ok — the FSM error is logged but does not fail the await call). For send, the analogous decision is: after the wire POST has succeeded (200 OK), if the local-side FSM advance fails, should `send_run_at` still return Ok?

    **Recommendation (mirror await semantics):** YES — return Ok. The wire side succeeded; the on-disk FSM error is a local diagnostic, not a wire failure. eprintln, then fall through to the TOFU capture block + return Ok. This matches await's "log and proceed" behavior verbatim.

    However, if Ben's task brief eprintln template (`"famp send: ...: {e}"`) implies a different choice, defer to the brief. The brief says "mirror the await_cmd post-ho8 pattern" → that pattern is log-and-continue. Implement log-and-continue.

    **eprintln messages (mirror await format verbatim, substituting "send" for "await" and the operation name):**
    - Closure error: `"famp send: advance_terminal failed for task {task_id}: {e}"`
    - Store (non-NotFound) error: `"famp send: failed to persist terminal-advance for task {task_id}: {e}"`

    Imports: add `TryUpdateError` to the `use famp_taskdir::...` line at the top of `send/mod.rs` (likely `use famp_taskdir::{TaskDir, TaskDirError, ...};` already exists — extend it; if `TaskDirError` is currently fully-qualified at use sites, leave the use sites alone and only add `TryUpdateError`).
  </behavior>
  <action>
    1. Apply the fix. Replace the SendMode::DeliverTerminal block at `crates/famp/src/cli/send/mod.rs:510-532` with:

    ```rust
    SendMode::DeliverTerminal => {
        // Try to update existing record; if not found, create one.
        //
        // Mirrors await_cmd/mod.rs's commit-receipt branch (post quick-260425-ho8):
        // try_update runs the FSM advance INSIDE the closure on a fresh-from-disk
        // record, persists only on Ok, and surfaces Err via eprintln. This closes
        // the B2-class "let _ = advance(...); let _ = update(...)" anti-pattern
        // that swallowed errors AND produced spurious writes on FSM Err.
        // (quick-260425-lny.)
        match tasks.try_update(task_id, |mut r| {
            r.last_send_at = Some(now_s.clone());
            fsm_glue::advance_terminal(&mut r).map(|_| r)
        }) {
            Ok(_) => {}
            Err(TryUpdateError::Store(famp_taskdir::TaskDirError::NotFound { .. })) => {
                // Create and immediately mark terminal for this received task.
                let mut rec = TaskRecord::new_committed(
                    task_id.to_string(),
                    alias.to_string(),
                    now_s.clone(),
                );
                rec.last_send_at = Some(now_s);
                // COMMITTED → COMPLETED is a valid FSM transition.
                fsm_glue::advance_terminal(&mut rec)?;
                tasks.create(&rec)?;
            }
            Err(TryUpdateError::Closure(e)) => {
                eprintln!(
                    "famp send: advance_terminal failed for task {task_id}: {e}"
                );
            }
            Err(TryUpdateError::Store(e)) => {
                eprintln!(
                    "famp send: failed to persist terminal-advance for task {task_id}: {e}"
                );
            }
        }
    }
    ```

    2. Add `TryUpdateError` to the imports at the top of `send/mod.rs`. Check the current `use famp_taskdir::...` line (search for it). Likely change:
    ```rust
    // Before (example — actual content may differ):
    use famp_taskdir::{TaskDir, TaskRecord};
    // After:
    use famp_taskdir::{TaskDir, TaskRecord, TryUpdateError};
    ```
    Do NOT touch other imports. Do NOT reorder. Do NOT alphabetize anything that isn't already alphabetized.

    3. Run `cargo build -p famp` — confirm clean compile.

    4. Run the new test from Task 1: `cargo nextest run -p famp --test send_terminal_advance_error_surfaces` — expect GREEN (sentinel survives).

    5. Run the existing send happy-path regression: `cargo nextest run -p famp --test send_terminal_blocks_resend` — expect GREEN (the COMMITTED → COMPLETED happy path still writes the new state correctly).

    6. Run full workspace gates:
       - `cargo nextest run --workspace` — expect 100% green.
       - `cargo clippy --workspace --all-targets -- -D warnings` — expect zero warnings.

    7. **Stash-pop sanity check** (REQUIRED per task brief). Do this as a verification step, NOT a permanent change:
       a. `git stash push -m "lny-sanity" crates/famp/src/cli/send/mod.rs` — temporarily revert just the source fix.
       b. `cargo nextest run -p famp --test send_terminal_advance_error_surfaces 2>&1 | tee /tmp/lny-red-sanity.log` — confirm test FAILS with sentinel-clobber message.
       c. `git stash pop` — restore the fix.
       d. `cargo nextest run -p famp --test send_terminal_advance_error_surfaces 2>&1 | tee /tmp/lny-green-sanity.log` — confirm test PASSES.
       e. Capture both logs into the SUMMARY (the RED output and the GREEN output, tagged "stash-pop sanity").

    8. Commit as TWO atoms (mirror the gst/ho8/kbx pattern):
       - **Atom 1 (RED):** `git add crates/famp/tests/send_terminal_advance_error_surfaces.rs` and commit with conventional `test(quick-260425-lny):` subject explaining the sentinel-discriminator and that the test fails against current code.
       - **Atom 2 (GREEN):** `git add crates/famp/src/cli/send/mod.rs` and commit with `fix(quick-260425-lny):` subject explaining the B2-class fix mirrors await_cmd post-ho8.
       - To produce these as two commits cleanly: stage and commit the test file FIRST (the test is failing on disk at this moment, but `git commit` doesn't run tests; the RED commit is the historical record of "this test was failing here"). Then stage the source fix and commit GREEN. Pre-commit hooks will run on each commit — if a hook runs the failing test on the RED commit, fall back to a single combined commit and explain in SUMMARY.
       - DO NOT use `--no-verify`. If a pre-commit hook blocks the RED commit, combine into one `fix(quick-260425-lny): ...` commit and document the deviation in SUMMARY (the brief allows this — TDD discipline matters more than commit granularity).

    9. Confirm both files committed: `git log --oneline -n 3`.

    Surgical-changes audit: every changed line MUST trace directly to the :514 fix or the new test. Specifically forbidden:
    - DO NOT modify `try_update` or its docstring (lg7 just landed).
    - DO NOT touch `await_cmd/mod.rs` (already fixed in ho8).
    - DO NOT cargo fmt unrelated files.
    - DO NOT address the pre-existing `CliError::Envelope` Display issue masking IllegalTransition string content.
    - DO NOT audit other `let _ =` patterns in the codebase.
    - DO NOT improve adjacent comments, formatting, or other arms of the match (NewTask, DeliverNonTerminal) — leave them byte-for-byte identical.
  </action>
  <verify>
    <automated>cd /Users/benlamm/Workspace/FAMP &amp;&amp; cargo nextest run -p famp --test send_terminal_advance_error_surfaces &amp;&amp; cargo nextest run -p famp --test send_terminal_blocks_resend &amp;&amp; cargo nextest run --workspace &amp;&amp; cargo clippy --workspace --all-targets -- -D warnings</automated>
  </verify>
  <done>
    - `crates/famp/src/cli/send/mod.rs` SendMode::DeliverTerminal arm uses `tasks.try_update` with explicit match arms over `TryUpdateError::Closure(_)` and `TryUpdateError::Store(_)`, mirroring `await_cmd/mod.rs:173-198` verbatim modulo the operation name and `last_send_at` mutation.
    - `crates/famp/tests/send_terminal_advance_error_surfaces.rs` PASSES.
    - `crates/famp/tests/send_terminal_blocks_resend.rs` (existing happy-path regression) PASSES.
    - `cargo nextest run --workspace` 100% green.
    - `cargo clippy --workspace --all-targets -- -D warnings` zero warnings.
    - Stash-pop sanity check captured in SUMMARY (RED log under stash + GREEN log post-restore).
    - Two atomic commits (RED test + GREEN fix), OR one combined commit with deviation documented in SUMMARY if the RED commit cannot pass pre-commit hooks.
    - Side-by-side diff between `send/mod.rs` DeliverTerminal arm and `await_cmd/mod.rs` commit-receipt branch shows structural identity (eyeball check, captured in SUMMARY as "structural mirror confirmed").
  </done>
</task>

</tasks>

<verification>
- `cargo nextest run -p famp --test send_terminal_advance_error_surfaces` → green (PASS post-fix).
- `cargo nextest run -p famp --test send_terminal_blocks_resend` → green (no regression on existing happy path).
- `cargo nextest run --workspace` → 100% green.
- `cargo clippy --workspace --all-targets -- -D warnings` → zero warnings.
- Stash-pop sanity: revert source fix → test FAILS with sentinel-clobber message; restore → test PASSES. Both outputs captured in SUMMARY.
- Structural mirror: `send/mod.rs` DeliverTerminal try_update block is byte-for-byte equivalent in shape to `await_cmd/mod.rs:173-198` (only the operation name, `last_send_at` mutation, and NotFound create-on-demand body differ — these are intentional and pre-existing).
- Surgical scope: `git diff --stat HEAD~2 HEAD` shows changes only in `crates/famp/src/cli/send/mod.rs` and `crates/famp/tests/send_terminal_advance_error_surfaces.rs` (or HEAD~1 if combined-commit fallback was used).
</verification>

<success_criteria>
1. The `let _ = fsm_glue::advance_terminal(&mut r);` anti-pattern at `send/mod.rs:~514` is gone — replaced by `try_update` with explicit match arms.
2. When `advance_terminal` returns Err inside the SendMode::DeliverTerminal persist path, the error is surfaced to stderr (via `eprintln!`), AND no spurious write occurs to the task TOML file.
3. A new sentinel-discriminator test in `crates/famp/tests/send_terminal_advance_error_surfaces.rs` proves both behaviors; it FAILS on pre-fix code and PASSES on post-fix code.
4. Existing send happy-path test `send_terminal_blocks_resend.rs` continues to pass.
5. Workspace `cargo nextest` and `cargo clippy --all-targets -- -D warnings` are clean.
6. The fix code is structurally indistinguishable from `await_cmd/mod.rs`'s post-ho8 commit-receipt branch — same shape, same arm structure, same eprintln template.
7. Surgical scope respected: only the two listed files are modified; no drive-by edits to `try_update`, `await_cmd`, unrelated formatting, or pre-existing `CliError::Envelope` Display issue.
</success_criteria>

<output>
After completion, create `.planning/quick/260425-lny-fix-b2-class-bug-at-send-mod-rs-514-surf/260425-lny-SUMMARY.md` containing:

- One-paragraph "what + why" (B2-class fix at :514, mirrors ho8 pattern).
- Diff summary (which files, line counts).
- Stash-pop sanity capture (RED log + GREEN log, both verbatim from `/tmp/lny-red-sanity.log` and `/tmp/lny-green-sanity.log`).
- Side-by-side excerpt: the new send/mod.rs DeliverTerminal block alongside await_cmd/mod.rs:173-198, demonstrating structural identity.
- Commit hashes (one or two atoms — note any deviation from RED-then-GREEN if pre-commit hook forced a combined commit).
- Workspace test count (e.g., "397/397 green") and clippy clean confirmation.
- Explicit out-of-scope reaffirmation: no try_update changes, no await_cmd changes, no fmt drive-bys, no addressing CliError::Envelope IllegalTransition Display masking.
- Update `.planning/STATE.md` Quick Tasks Completed table with row `260425-lny | Fix B2-class FSM error suppression at send/mod.rs:514 | 2026-04-25 | <hashes> | | <dir>`.
- Update `.planning/STATE.md` Recent Activity bullet for 2026-04-25.
</output>

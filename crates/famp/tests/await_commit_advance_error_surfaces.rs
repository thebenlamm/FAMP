//! TDD test for the lost-update race + error-suppression in
//! `await_cmd::run_at`'s commit-receipt handling.
//!
//! ## History
//!
//! **Bug B2 (quick-260425-gst, commit c69b4e9):** two `let _ =` discarded
//! errors from `advance_committed()` and `tasks.update()`. The c69b4e9 fix
//! replaced them with explicit `match` + `eprintln!`, surfacing errors.
//! However, it reintroduced a TOCTOU window: the FSM advance moved OUTSIDE
//! the `TaskDir::update` closure, and the closure ignored the fresh-from-disk
//! record (`|_| record.clone()`). Any concurrent write between the initial
//! `tasks.read(...)` and the subsequent `tasks.update(...)` would be silently
//! overwritten.
//!
//! **Lost-update race fix (quick-260425-ho8):** replaced the racy
//! read → advance → `update(|_| clone)` pattern with a single
//! `tasks.try_update(task_id, |mut r| advance_committed(&mut r).map(|_| r))`
//! call. The FSM advance now lives INSIDE the closure, operating on the
//! fresh record that `try_update` reads from disk — atomic with the persist.
//! On closure `Err` (e.g. `IllegalTransition`), `try_update` performs NO disk
//! write. Errors continue to surface via `eprintln!`.
//!
//! ## What this test proves
//!
//! The test exercises the `IllegalTransition` path: the on-disk record is
//! already `COMMITTED`, so a second arriving commit envelope triggers
//! `advance_committed → Err(IllegalTransition)`.
//!
//! - **Pre-fix (bug B2):** the unconditional `tasks.update(...)` call rewrites
//!   the TOML even though the state didn't change, mutating the file bytes.
//! - **Post-c69b4e9:** the `Err` arm skips `tasks.update`, but the stale
//!   snapshot pattern (`|_| record.clone()`) is still present and would
//!   overwrite a concurrent write on the `Ok` arm.
//! - **Post-ho8:** `try_update` skips the persist on closure `Err` — the
//!   file bytes are byte-identical to their pre-call state. Additionally,
//!   the closure receives the FRESH record from disk on the `Ok` arm, closing
//!   the TOCTOU window.
//!
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
//!
//! ## Why no daemon?
//!
//! We drive `await_cmd::run_at` directly — no listener needed. The commit
//! envelope is injected into `inbox.jsonl` manually. This isolates exactly
//! the buggy branch without requiring round-trip network traffic.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

mod common;

use std::io::Write as _;

use common::conversation_harness::setup_home;
use famp::cli::await_cmd::{run_at as await_run_at, AwaitArgs};
use famp_taskdir::{TaskDir, TaskRecord};

// Sentinel string for `commit_arrival_when_record_already_committed_does_not_rewrite_task_file`.
// Must be a module-level const to satisfy clippy::items_after_statements.
// See test body for explanation of why a TOML comment sentinel discriminates the bug.
const SENTINEL: &str = "\n# TEST_SENTINEL_DO_NOT_REWRITE\n";

#[tokio::test(flavor = "current_thread")]
async fn commit_arrival_when_record_already_committed_does_not_rewrite_task_file() {
    let tmp = setup_home();
    let home = tmp.path();

    // 1. Create a task record already in COMMITTED state. This means any
    //    arriving commit envelope triggers IllegalTransition inside
    //    advance_committed() — the exact error path we want to exercise.
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
    {
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
    //
    //    The raw envelope format (see poll::find_match docs): for non-request
    //    classes, task_id is extracted from `causality["ref"]`. The shaped
    //    output from find_match then carries `task_id` as a top-level field,
    //    which is what await_cmd/mod.rs reads for the FSM branch.
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
        let mut inbox_file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&inbox_path)
            .expect("open inbox.jsonl for append");
        writeln!(inbox_file, "{}", serde_json::to_string(&line).unwrap())
            .expect("write inbox line");
    }

    // 4. Run await_run_at. The commit envelope triggers the FSM branch;
    //    advance_committed returns Err(IllegalTransition) because the record
    //    is already COMMITTED. Under the post-ho8 `try_update` wiring, no
    //    disk write occurs.
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

// Silencers — required to keep `unused_crate_dependencies` quiet.
use axum as _;
use base64 as _;
use clap as _;
use ed25519_dalek as _;
use famp_canonical as _;
use famp_core as _;
use famp_crypto as _;
use famp_envelope as _;
use famp_fsm as _;
use famp_inbox as _;
use famp_keyring as _;
use famp_taskdir as _;
use famp_transport as _;
use famp_transport_http as _;
use hex as _;
use humantime as _;
use rand as _;
use rcgen as _;
use reqwest as _;
use rustls as _;
use serde as _;
use serde_json as _;
use sha2 as _;
use tempfile as _;
use thiserror as _;
use time as _;
use tokio as _;
use toml as _;
use tower as _;
use tower_http as _;
use url as _;
use uuid as _;

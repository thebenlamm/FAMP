//! TDD test for bug B2 (2026-04-25 pressure test): silent error suppression in
//! `await_cmd::run_at`'s commit-receipt handling.
//!
//! Pre-fix: two `let _ =` discard errors from `advance_committed()` and
//! `tasks.update()`. This test exercises the `IllegalTransition` path
//! (record already COMMITTED, another commit envelope arrives).
//!
//! ## What changed between pre-fix and post-fix?
//!
//! Pre-fix buggy code (lines 163-172):
//! ```rust
//! if tasks.read(task_id_str).is_ok() {
//!     let _ = tasks.update(task_id_str, |mut r| {
//!         let _ = advance_committed(&mut r);  // Err ignored; r unchanged
//!         r
//!     });  // update IS called unconditionally — rewrites the TOML even on error
//! }
//! ```
//!
//! The unconditional `tasks.update(...)` call writes back the record (even
//! though its state didn't change), changing the file's mtime.
//!
//! Post-fix: `tasks.update` is only called when `advance_committed` returns
//! `Ok`. When it returns `Err` (`IllegalTransition`), we skip the update entirely
//! and log an `eprintln!` instead. The file is NOT rewritten; mtime is unchanged.
//!
//! ## Observable: mtime
//!
//! The test records the task file's mtime before calling `await_run_at`, then
//! checks it after:
//!
//! - Pre-fix → mtime HAS changed (spurious `tasks.update` call) → assertion FAILS (RED).
//! - Post-fix → mtime unchanged (update correctly skipped) → assertion PASSES (GREEN).
//!
//! This mtime assertion is more reliable than stderr capture (which is tricky
//! with the Rust test harness's own stderr redirection) and directly proves
//! the invariant we care about: no spurious disk writes on FSM error.
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
use std::time::Duration;

use common::conversation_harness::setup_home;
use famp::cli::await_cmd::{run_at as await_run_at, AwaitArgs};
use famp_taskdir::{TaskDir, TaskRecord};

#[tokio::test(flavor = "current_thread")]
async fn commit_arrival_when_record_already_committed_does_not_spuriously_update_task_file() {
    let tmp = setup_home();
    let home = tmp.path();

    // 1. Create a task record already in COMMITTED state. This means any
    //    arriving commit envelope triggers IllegalTransition inside
    //    advance_committed() — the exact error path we want to surface.
    let task_id = uuid::Uuid::now_v7().to_string();
    let record = TaskRecord::new_committed(
        task_id.clone(),
        "self".to_string(),
        "2026-04-25T00:00:00Z".to_string(),
    );
    let tasks_dir = home.join("tasks");
    let tasks = TaskDir::open(&tasks_dir).unwrap();
    tasks.create(&record).expect("create task record");

    // Small sleep so mtime-based checks are reliable (filesystem mtime
    // resolution is typically 1s on macOS HFS+ / APFS, 1ns on ext4).
    // We only need to distinguish "file was rewritten" from "file unchanged",
    // so 10ms is more than enough for a monotonic mtime check.
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Record the task file's mtime BEFORE calling await_run_at.
    let task_file = tasks_dir.join(format!("{task_id}.toml"));
    let mtime_before = std::fs::metadata(&task_file)
        .expect("task file exists")
        .modified()
        .expect("mtime available");

    // 2. Inject a synthetic commit-class envelope into inbox.jsonl so
    //    find_match will pick it up and the commit-receipt branch fires.
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
    let mut inbox_file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&inbox_path)
        .expect("open inbox.jsonl for append");
    writeln!(inbox_file, "{}", serde_json::to_string(&line).unwrap()).expect("write inbox line");
    drop(inbox_file);

    // 3. Run await_run_at. The commit envelope triggers the FSM branch;
    //    advance_committed returns Err(IllegalTransition) because the record
    //    is already COMMITTED.
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

    // 4. Assertions.
    //
    // a) await_run_at returned Ok above — loop continues on FSM error, does not crash.

    // b) The task file's mtime must be UNCHANGED. If mtime changed, it means
    //    the buggy `tasks.update` was called spuriously even though the FSM
    //    transition failed. Pre-fix: mtime HAS changed → FAILS (RED).
    //    Post-fix: update is skipped on FSM error → mtime unchanged → PASSES.
    let mtime_after = std::fs::metadata(&task_file)
        .expect("task file still exists after await")
        .modified()
        .expect("mtime available");

    assert_eq!(
        mtime_before, mtime_after,
        "task file must NOT be rewritten on FSM error: mtime changed, \
         indicating advance_committed's Err was swallowed and tasks.update \
         was called spuriously (bug B2)"
    );

    // c) On-disk state must be COMMITTED (unchanged — no double-write, no corruption).
    let rec = TaskDir::open(&tasks_dir).unwrap().read(&task_id).unwrap();
    assert_eq!(
        rec.state, "COMMITTED",
        "state must be unchanged on FSM error (no double-write)"
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
use gag as _;
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

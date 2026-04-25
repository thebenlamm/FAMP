//! TDD test for bug B2 (2026-04-25 pressure test): silent error suppression in
//! `await_cmd::run_at`'s commit-receipt handling.
//!
//! Pre-fix: two `let _ =` discard errors from `advance_committed()` and
//! `tasks.update()`. This test exercises the `IllegalTransition` path
//! (record already COMMITTED, another commit envelope arrives) and asserts
//! that stderr contains an error log referencing the task_id. Pre-fix, the
//! error is silently swallowed so stderr is empty and the assertion fails.
//!
//! Post-fix (Task 2): explicit `eprintln!` on both error paths makes the
//! test pass.
//!
//! ## Why no daemon?
//!
//! We drive `await_cmd::run_at` directly — no listener needed. The commit
//! envelope is injected into `inbox.jsonl` manually. This isolates exactly
//! the buggy branch without requiring round-trip network traffic.
//!
//! ## Stderr capture
//!
//! `gag::BufferRedirect::stderr()` provides a safe, Unix-only stderr
//! redirect. The `#![cfg(unix)]` gate matches `gag`'s platform support.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unused_crate_dependencies
)]

mod common;

use std::io::{Read, Write as _};

use common::conversation_harness::setup_home;
use famp::cli::await_cmd::{run_at as await_run_at, AwaitArgs};
use famp_taskdir::{TaskDir, TaskRecord};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn commit_arrival_when_record_already_committed_logs_error_and_continues() {
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
    let tasks = TaskDir::open(home.join("tasks")).unwrap();
    tasks.create(&record).expect("create task record");

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
    writeln!(inbox_file, "{}", serde_json::to_string(&line).unwrap())
        .expect("write inbox line");
    drop(inbox_file);

    // 3. Capture stderr during await_run_at so we can assert on it.
    //    `gag::BufferRedirect::stderr()` is safe and Unix-only (matching
    //    this file's `#![cfg(unix)]`). The redirect is active for the
    //    duration of the await call, then released so we can read it.
    let mut stderr_buf = gag::BufferRedirect::stderr().expect("gag stderr redirect");

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

    // Release the redirect and read what was captured.
    let mut captured = String::new();
    stderr_buf
        .read_to_string(&mut captured)
        .expect("read captured stderr");
    drop(stderr_buf);

    // 4. Assertions.
    //
    // a) await_run_at returned Ok above (loop continues on FSM error — doesn't crash).
    //
    // b) Captured stderr must reference the task_id — proves the error log fired.
    //    Pre-fix: captured is empty, this assertion FAILS (RED).
    //    Post-fix: captured contains the eprintln! output, assertion PASSES.
    assert!(
        captured.contains(&task_id),
        "stderr must reference task_id; got: {captured:?}"
    );

    // c) Stderr must mention the failing operation so the user knows what went wrong.
    assert!(
        captured.contains("advance_committed") || captured.contains("commit-advance"),
        "stderr must reference the failing operation; got: {captured:?}"
    );

    // d) On-disk state must be unchanged (COMMITTED, not double-written or corrupted).
    let rec = TaskDir::open(home.join("tasks"))
        .unwrap()
        .read(&task_id)
        .unwrap();
    assert_eq!(
        rec.state, "COMMITTED",
        "state must be unchanged on FSM error (no double-write)"
    );
}

// Silencers — required to keep `unused_crate_dependencies` quiet.
use gag as _;
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

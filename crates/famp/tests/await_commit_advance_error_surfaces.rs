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
//! ## Observable: byte equality (not mtime)
//!
//! The test snapshots the raw file bytes BEFORE calling `await_run_at` and
//! asserts byte-equality AFTER. This is:
//! - **Immune to filesystem mtime granularity** (macOS APFS mtime is 1 ns
//!   but the test can still race in CI with 1-second HFS+ images; byte
//!   comparison has no clock dependency at all).
//! - **Directly proves both failure modes** of the pre-fix code: a spurious
//!   write (B2 path) changes bytes; a lost-update write also changes bytes.
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

#[tokio::test(flavor = "current_thread")]
async fn commit_arrival_when_record_already_committed_does_not_modify_task_file_bytes() {
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

    // 2. Snapshot the task file bytes BEFORE calling await_run_at.
    //    Byte equality is clock-independent — no sleep needed.
    let task_file = tasks_dir.join(format!("{task_id}.toml"));
    let bytes_before = std::fs::read(&task_file).expect("read task file before await");

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
    let mut inbox_file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&inbox_path)
        .expect("open inbox.jsonl for append");
    writeln!(inbox_file, "{}", serde_json::to_string(&line).unwrap()).expect("write inbox line");
    drop(inbox_file);

    // 4. Run await_run_at. The commit envelope triggers the FSM branch;
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

    // 5. Assertions.
    //
    // a) await_run_at returned Ok above — loop continues on FSM error, does not crash.

    // b) The task file bytes must be byte-identical. If bytes changed, a spurious
    //    write occurred: either the old B2 bug (unconditional update on FSM Err)
    //    or the lost-update race (stale snapshot overwrite). Both failure modes
    //    mutate the file; byte equality catches both.
    //    Pre-fix: bytes CHANGE → assertion FAILS (RED).
    //    Post-fix: try_update skips persist on closure Err → bytes UNCHANGED → PASSES.
    let bytes_after = std::fs::read(&task_file).expect("read task file after await");
    assert_eq!(
        bytes_before, bytes_after,
        "task file bytes must NOT change on FSM error: spurious write detected \
         (lost-update race or swallowed error) — quick-260425-ho8"
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

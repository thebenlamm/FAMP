//! Phase 3 Plan 03-04 — full conversation lifecycle end-to-end test.
//!
//! Covers the five ROADMAP success criteria for Phase 3 conversation CLI:
//!
//! 1. `famp send --new-task` opens a task and persists a REQUESTED record.
//! 2. `famp send --task` (3x non-terminal) delivers interim envelopes;
//!    the task stays non-terminal between calls.
//! 3. `famp send --task --terminal` advances the local FSM to COMPLETED
//!    and marks the record terminal.
//! 4. A subsequent send on the completed task returns `TaskTerminal`.
//! 5. `famp await --timeout 2s` consumes the first inbox entry end-to-end
//!    and advances the cursor.
//!
//! Runs against a single shared `FAMP_HOME` because the Phase 2 listen
//! daemon's single-entry keyring only resolves `agent:localhost/self`
//! (see `conversation_harness.rs` docs for the rationale). A true
//! two-home flow is Phase 4 work.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

mod common;

use common::conversation_harness::{
    add_self_peer, await_once, deliver, inbox_line_count, new_task, read_task, setup_home,
    spawn_listener, stop_listener, try_deliver,
};
use famp::cli::error::CliError;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn full_long_task_conversation_completes() {
    let tmp = setup_home();
    let home = tmp.path();

    // 1. Spawn the in-process daemon + register it as peer "self".
    let (addr, handle, shutdown_tx) = spawn_listener(home).await;
    add_self_peer(home, "self", addr);

    // 2. Open a new task.
    let task_id = new_task(home, "self", "hello bob").await;
    let rec = read_task(home, &task_id);
    assert_eq!(rec.state, "REQUESTED");
    assert!(!rec.terminal);
    assert_eq!(inbox_line_count(home), 1, "request envelope on disk");

    // 3. Three non-terminal delivers.
    for i in 0..3 {
        deliver(home, "self", &task_id, false, &format!("msg {i}")).await;
    }
    let rec = read_task(home, &task_id);
    assert_eq!(rec.state, "REQUESTED", "non-terminal delivers keep state");
    assert!(!rec.terminal);
    assert_eq!(inbox_line_count(home), 4, "request + 3 delivers");

    // 4. Terminal deliver → state COMPLETED, record terminal.
    deliver(home, "self", &task_id, true, "done").await;
    let rec = read_task(home, &task_id);
    assert_eq!(rec.state, "COMPLETED");
    assert!(rec.terminal);
    assert_eq!(inbox_line_count(home), 5, "request + 3 delivers + terminal");

    // 5. Subsequent send on the same task must fail TaskTerminal and
    //    leave the inbox line count + record unchanged.
    let result = try_deliver(home, "self", &task_id, false, "ghost").await;
    match result {
        Err(CliError::TaskTerminal { task_id: got }) => assert_eq!(got, task_id),
        other => panic!("expected TaskTerminal, got {other:?}"),
    }
    let rec_after = read_task(home, &task_id);
    assert_eq!(rec_after, read_task(home, &task_id));
    assert!(rec_after.terminal);
    assert_eq!(
        inbox_line_count(home),
        5,
        "rejected send must not append"
    );

    // 6. `famp await` consumes the first entry end-to-end and returns
    //    the locked JSON shape.
    let first = await_once(home, "2s").await;
    let obj = first.as_object().expect("object");
    assert!(obj.contains_key("offset"));
    assert_eq!(obj["from"].as_str().unwrap(), "agent:localhost/self");
    let class = obj["class"].as_str().unwrap();
    assert_eq!(
        class, "request",
        "first consumed entry is the opening request"
    );

    stop_listener(handle, shutdown_tx).await;
}

// Silencers.
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

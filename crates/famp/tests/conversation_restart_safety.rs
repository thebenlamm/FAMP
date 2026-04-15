//! Phase 3 Plan 03-04 — CONV-04 restart safety.
//!
//! Scenario: open a task, send a non-terminal deliver, stop the listen
//! daemon, restart it (on a fresh ephemeral port), point the peer entry
//! at the new port, and send another deliver on the SAME task id. The
//! second send must succeed against the reloaded record and the
//! daemon's inbox must contain all three envelopes.
//!
//! Shared-home constraint applies (see `conversation_harness.rs`).

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

mod common;

use common::conversation_harness::{
    add_self_peer, deliver, inbox_line_count, new_task, read_task, setup_home, spawn_listener,
    stop_listener, update_peer_endpoint,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn task_record_survives_listener_restart() {
    let tmp = setup_home();
    let home = tmp.path();

    // 1. Daemon v1, peer, new task, one deliver.
    let (addr1, handle1, shutdown1) = spawn_listener(home).await;
    add_self_peer(home, "self", addr1);
    let task_id = new_task(home, "self", "before restart").await;
    deliver(home, "self", &task_id, false, "msg 1").await;
    // Phase 4: request triggers auto-commit reply, so inbox has request + commit + deliver = 3.
    assert_eq!(inbox_line_count(home), 3);
    let rec_before = read_task(home, &task_id);
    assert_eq!(rec_before.state, "REQUESTED");
    assert!(!rec_before.terminal);

    // 2. Stop the daemon.
    stop_listener(handle1, shutdown1).await;

    // 3. Re-spawn the daemon on a fresh ephemeral port + repoint the peer.
    let (addr2, handle2, shutdown2) = spawn_listener(home).await;
    update_peer_endpoint(home, "self", addr2);

    // 4. Record is still readable and identical on disk.
    let rec_after = read_task(home, &task_id);
    assert_eq!(rec_after.task_id, task_id);
    assert_eq!(rec_after.state, "REQUESTED");
    assert_eq!(rec_after, rec_before);

    // 5. Send another deliver on the SAME task id: the --task path must
    //    find the persisted record and accept the send.
    deliver(home, "self", &task_id, false, "after restart").await;
    // Phase 4: inbox = request + commit-reply + first-deliver + second-deliver = 4.
    assert_eq!(
        inbox_line_count(home),
        4,
        "inbox grows on the restarted daemon"
    );

    // 6. last_send_at should have moved forward (or at least been set).
    let rec_final = read_task(home, &task_id);
    assert_eq!(rec_final.state, "REQUESTED");
    assert!(rec_final.last_send_at.is_some());
    assert!(!rec_final.terminal);

    stop_listener(handle2, shutdown2).await;
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

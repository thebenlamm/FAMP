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
//! ## Phase 4 update (Plan 04-01)
//!
//! With the FSM shortcut (`__with_state_for_testing`) removed, `advance_terminal`
//! now requires the record to be in COMMITTED state. The natural flow is:
//!
//! 1. `famp send --new-task` → REQUESTED
//! 2. `famp await --task <id>` → receives the auto-commit reply → COMMITTED
//! 3. Three non-terminal delivers (record stays COMMITTED)
//! 4. `famp send --task --terminal` → COMPLETED
//!
//! Runs against a single shared `FAMP_HOME` (from == to == agent:localhost/self)
//! since the daemon's self-entry in the keyring handles both inbound and
//! outbound on the same host.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

mod common;

use std::net::SocketAddr;

use common::conversation_harness::{
    add_self_peer, await_once, deliver, inbox_line_count, new_task, read_task, setup_home,
    stop_listener, try_deliver, update_peer_endpoint,
};
use common::wait_for_tls_listener_ready;
use famp::cli::await_cmd::{run_at as await_run_at, AwaitArgs};
use famp::cli::error::CliError;
use tokio::sync::oneshot;

/// Spawn daemon on a pre-bound listener (so peers.toml can be written first).
async fn spawn_daemon_pre_bound(
    home: &std::path::Path,
    listener: std::net::TcpListener,
) -> (SocketAddr, tokio::task::JoinHandle<()>, oneshot::Sender<()>) {
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel::<()>();
    let home_owned = home.to_path_buf();
    let handle = tokio::spawn(async move {
        famp::cli::listen::run_on_listener(&home_owned, listener, async move {
            let _ = rx.await;
        })
        .await
        .expect("run_on_listener");
    });
    wait_for_tls_listener_ready().await;
    (addr, handle, tx)
}

#[ignore = "Phase 02 Plan 02-04: rewired send to bus path; v0.8 HTTPS shape; \
revisit / migrate in Phase 4 federation gateway"]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn full_long_task_conversation_completes() {
    let tmp = setup_home();
    let home = tmp.path();

    // Bind the listener BEFORE registering the peer so the port is known.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();

    // Register self as peer BEFORE daemon starts (auto-commit needs peers.toml).
    add_self_peer(home, "self", addr);

    // 1. Spawn daemon.
    let (addr, handle, shutdown_tx) = spawn_daemon_pre_bound(home, listener).await;
    // Update endpoint in case addr changed (ephemeral port may differ).
    update_peer_endpoint(home, "self", addr);

    // 2. Open a new task.
    let task_id = new_task(home, "self", "hello bob").await;
    let rec = read_task(home, &task_id);
    assert_eq!(rec.state, "REQUESTED");
    assert!(!rec.terminal);
    // The auto-commit reply may have already arrived by the time we check,
    // so the inbox may hold 1 (request only) or 2 (request + commit).
    assert!(
        inbox_line_count(home) >= 1,
        "at least the request envelope is on disk"
    );

    // 3. Await the auto-commit reply → record advances to COMMITTED.
    let mut buf: Vec<u8> = Vec::new();
    await_run_at(
        home,
        AwaitArgs {
            timeout: "5s".to_string(),
            task: Some(task_id.clone()),
        },
        &mut buf,
    )
    .await
    .expect("await commit reply");
    let rec = read_task(home, &task_id);
    assert_eq!(rec.state, "COMMITTED", "after commit reply: COMMITTED");
    assert_eq!(inbox_line_count(home), 2, "request + commit reply");

    // 4. Three non-terminal delivers (record stays COMMITTED).
    for i in 0..3 {
        deliver(home, "self", &task_id, false, &format!("msg {i}")).await;
    }
    let rec = read_task(home, &task_id);
    assert_eq!(
        rec.state, "COMMITTED",
        "non-terminal delivers keep COMMITTED"
    );
    assert!(!rec.terminal);
    assert_eq!(inbox_line_count(home), 5, "request + commit + 3 delivers");

    // 5. Terminal deliver → state COMPLETED, record terminal.
    deliver(home, "self", &task_id, true, "done").await;
    let rec = read_task(home, &task_id);
    assert_eq!(rec.state, "COMPLETED");
    assert!(rec.terminal);
    assert_eq!(
        inbox_line_count(home),
        6,
        "request + commit + 3 delivers + terminal"
    );

    // 6. Subsequent send on the same task must fail TaskTerminal.
    let result = try_deliver(home, "self", &task_id, false, "ghost").await;
    match result {
        Err(CliError::TaskTerminal { task_id: got }) => assert_eq!(got, task_id),
        other => panic!("expected TaskTerminal, got {other:?}"),
    }
    let rec_after = read_task(home, &task_id);
    assert_eq!(rec_after, read_task(home, &task_id));
    assert!(rec_after.terminal);
    assert_eq!(inbox_line_count(home), 6, "rejected send must not append");

    // 7. `famp await` (no task filter) consumes the next inbox entry and
    //    returns the locked JSON shape. The cursor is now past the commit
    //    reply (step 3 consumed one entry); the next unread entry is one
    //    of the delivers.
    let next = await_once(home, "2s").await;
    let obj = next.as_object().expect("object");
    assert!(obj.contains_key("offset"));
    assert_eq!(obj["from"].as_str().unwrap(), "agent:localhost/self");
    let class = obj["class"].as_str().unwrap();
    // Next unread is the first non-terminal deliver.
    assert_eq!(
        class, "deliver",
        "next consumed entry after commit is a deliver"
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

//! Phase 4 Plan 04-01 Task 2 — auto-commit round-trip test.
//!
//! Verifies that when a daemon receives a signed `request` envelope, it
//! automatically sends a signed `commit` reply to the originator. The
//! originator's `famp await` call detects the commit reply and advances the
//! local task record from REQUESTED → COMMITTED. A subsequent `famp send
//! --terminal` then drives COMMITTED → COMPLETED via the real FSM.
//!
//! ## Cursor positioning
//!
//! The inbox accumulates ALL inbound envelopes (request sent by us, commit
//! reply from daemon). The originator's cursor starts at 0. To await the
//! COMMIT reply specifically, we first consume the request envelope (which
//! we already know about — we sent it) via a no-filter await, then await
//! with `--task <id>` to receive the commit-class envelope.
//!
//! Uses the single-home flow (from == to == agent:localhost/self) because the
//! daemon's self-entry in the keyring handles both inbound and outbound in
//! the local loopback case. The auto-commit handler re-reads peers.toml to
//! find the reply endpoint, so the peer must be registered before the daemon
//! starts.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

mod common;

use std::time::Duration;

use common::conversation_harness::{
    add_self_peer, await_once, deliver, new_task, read_task, setup_home, stop_listener,
    update_peer_endpoint,
};
use famp::cli::await_cmd::{run_at as await_run_at, AwaitArgs};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn auto_commit_round_trip() {
    let tmp = setup_home();
    let home = tmp.path();

    // Bind the listener before registering the peer so the port is known.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr: std::net::SocketAddr = listener.local_addr().unwrap();

    // Register self as a peer pointing back to the daemon's own addr BEFORE
    // starting the daemon — auto-commit re-reads peers.toml on each dispatch.
    add_self_peer(home, "self", addr);

    // Start the daemon in-process.
    let (addr, handle, shutdown_tx) = spawn_listener_at(home, listener).await;
    // Update endpoint in case addr changed (shouldn't for ephemeral port but
    // ensures consistency).
    update_peer_endpoint(home, "self", addr);

    // 1. Send --new-task; local record should be REQUESTED.
    let task_id = new_task(home, "self", "phase4 auto-commit test").await;
    let rec = read_task(home, &task_id);
    assert_eq!(rec.state, "REQUESTED", "initial state is REQUESTED");
    assert!(!rec.terminal);

    // 2. The inbox now contains the request envelope we just sent. Consume
    //    it via a no-filter await (we already know we sent it). This advances
    //    the cursor past the request so the next await sees only new entries.
    let first = await_once(home, "2s").await;
    assert_eq!(
        first["class"].as_str().unwrap_or(""),
        "request",
        "first inbox entry is our outgoing request"
    );

    // 3. Now await with --task <id> with 5s timeout — the daemon auto-commits
    //    upon receiving our request, so a commit reply should arrive shortly.
    //    `advance_committed` in await_cmd/mod.rs fires when class == "commit".
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
    .expect("await timed out waiting for commit reply");

    let text = String::from_utf8(buf).unwrap();
    let line = text
        .lines()
        .next()
        .expect("await printed at least one line");
    let val: serde_json::Value = serde_json::from_str(line).expect("await output is JSON");
    let class = val["class"].as_str().unwrap_or("");
    assert_eq!(class, "commit", "second entry must be the commit reply");

    // 4. Re-read task record — should now be COMMITTED (advance_committed ran).
    let rec = read_task(home, &task_id);
    assert_eq!(
        rec.state, "COMMITTED",
        "record must be COMMITTED after commit reply"
    );
    assert!(!rec.terminal, "COMMITTED is non-terminal");

    // 5. Send --task <id> --terminal. Real FSM walks COMMITTED → COMPLETED.
    deliver(home, "self", &task_id, true, "done").await;
    let rec = read_task(home, &task_id);
    assert_eq!(
        rec.state, "COMPLETED",
        "terminal deliver must produce COMPLETED"
    );
    assert!(rec.terminal, "COMPLETED is terminal");

    // 6. Structural check: __with_state_for_testing must not appear in src/.
    let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "grep -r '__with_state_for_testing' {}",
            src_dir.display()
        ))
        .output()
        .expect("grep command");
    assert!(
        output.stdout.is_empty(),
        "__with_state_for_testing must not appear in src/: {}",
        String::from_utf8_lossy(&output.stdout)
    );

    stop_listener(handle, shutdown_tx).await;
}

/// Spawn the daemon on a pre-bound listener (so we can register the peer
/// endpoint before starting). Returns (actual addr, handle, `shutdown_tx`).
async fn spawn_listener_at(
    home: &std::path::Path,
    listener: std::net::TcpListener,
) -> (
    std::net::SocketAddr,
    tokio::task::JoinHandle<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let home_owned = home.to_path_buf();
    let handle = tokio::spawn(async move {
        famp::cli::listen::run_on_listener(&home_owned, listener, async move {
            let _ = rx.await;
        })
        .await
        .expect("run_on_listener");
    });
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "daemon bind timed out"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    (addr, handle, tx)
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

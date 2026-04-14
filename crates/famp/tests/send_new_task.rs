//! Phase 3 Plan 03-02 Task 2 — `famp send --new-task` E2E integration.
//!
//! Spins up `famp::cli::listen::run_on_listener` in-process on an ephemeral
//! port, peer-adds the daemon as alias "self" with principal
//! `agent:localhost/self`, and runs `famp send --new-task --to self`. Asserts
//! the task record is created in REQUESTED state with a valid `UUIDv7` id and
//! the daemon's inbox contains exactly one request envelope line.

#![cfg(unix)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unused_crate_dependencies
)]

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use famp::cli::peer::add::run_add_at;
use famp::cli::send::{run_at as send_run_at, SendArgs};
use famp_taskdir::TaskDir;

use common::init_home_in_process;

fn pubkey_b64(home: &std::path::Path) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    let bytes = std::fs::read(home.join("pub.ed25519")).unwrap();
    URL_SAFE_NO_PAD.encode(bytes)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_new_task_creates_record_and_hits_daemon() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home_in_process(&home);

    // Bind an ephemeral port and start the daemon in-process.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let shutdown_signal = async move {
        let _ = shutdown_rx.await;
    };
    let home_for_task = home.clone();
    let server_task = tokio::spawn(async move {
        famp::cli::listen::run_on_listener(&home_for_task, listener, shutdown_signal)
            .await
            .expect("run_on_listener");
    });

    // Wait until the TCP port is accepting.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline, "daemon bind timed out");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // Register the daemon as peer "self" with the daemon's own pubkey.
    run_add_at(
        &home,
        "self".to_string(),
        format!("https://{addr}"),
        pubkey_b64(&home),
        Some("agent:localhost/self".to_string()),
    )
    .expect("peer add");

    // Send a new task.
    let args = SendArgs {
        to: "self".to_string(),
        new_task: Some("hello phase 3".to_string()),
        task: None,
        terminal: false,
        body: None,
    };
    send_run_at(&home, args).await.expect("famp send");

    // Exactly one task record should exist, state REQUESTED, peer=self.
    let tasks = TaskDir::open(home.join("tasks")).unwrap();
    let records = tasks.list().unwrap();
    assert_eq!(records.len(), 1, "exactly one task record expected");
    let rec = &records[0];
    assert_eq!(rec.state, "REQUESTED");
    assert_eq!(rec.peer, "self");
    assert!(!rec.terminal);
    assert!(rec.last_send_at.is_some());
    // UUIDv7 hyphenated form is 36 chars.
    assert_eq!(rec.task_id.len(), 36);

    // Daemon inbox should have exactly one line (the request envelope).
    let lines = famp_inbox::read::read_all(home.join("inbox.jsonl")).unwrap();
    assert_eq!(lines.len(), 1, "expected one inbox line");
    let class = lines[0]
        .get("class")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    assert_eq!(class, "request");

    // Shutdown.
    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(Duration::from_secs(2), server_task).await;
}

// Silencers (minimum set that keeps unused_crate_dependencies quiet in this
// specific test binary).
use axum as _;
use clap as _;
use ed25519_dalek as _;
use famp_canonical as _;
use famp_core as _;
use famp_crypto as _;
use famp_envelope as _;
use famp_fsm as _;
use famp_keyring as _;
use famp_transport as _;
use famp_transport_http as _;
use hex as _;
use rand as _;
use rcgen as _;
use rustls as _;
use serde as _;
use sha2 as _;
use thiserror as _;
use time as _;
use toml as _;
use tower as _;
use tower_http as _;
use url as _;
use uuid as _;

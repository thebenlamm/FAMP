//! Phase 3 Plan 03-03 Task 2 — `famp await` blocks until inbox receives.
//!
//! Spawns `famp::cli::listen::run_on_listener` in-process on an ephemeral
//! port. Concurrently launches `famp::cli::await_cmd::run_at` on the same
//! `FAMP_HOME`. After a short delay POSTs a signed envelope to the daemon.
//! Asserts that `await` unblocks, prints one structured JSON line whose
//! keys match the locked schema (`offset`, `task_id`, `from`, `class`,
//! `body`), and advances the cursor past exactly one entry.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use famp::cli::await_cmd::{run_at as await_run_at, AwaitArgs};
use famp_inbox::InboxCursor;

use common::listen_harness::{
    build_signed_ack_bytes, build_trusting_reqwest_client, post_bytes, self_principal,
};
use common::init_home_in_process;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn await_blocks_until_message_arrives() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home_in_process(&home);

    // Bind ephemeral port + spawn in-process daemon.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let shutdown_signal = async move {
        let _ = shutdown_rx.await;
    };
    let home_srv = home.clone();
    let server_task = tokio::spawn(async move {
        famp::cli::listen::run_on_listener(&home_srv, listener, shutdown_signal)
            .await
            .expect("run_on_listener");
    });

    // Wait for daemon bind.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline, "daemon bind timed out");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // Spawn the await task FIRST — it should be blocked polling.
    let home_await = home.clone();
    let await_task = tokio::spawn(async move {
        let mut buf: Vec<u8> = Vec::new();
        let args = AwaitArgs {
            timeout: "5s".to_string(),
            task: None,
        };
        let res = await_run_at(&home_await, args, &mut buf).await;
        (res, buf)
    });

    // Give await one poll cycle to establish "nothing yet" state.
    tokio::time::sleep(Duration::from_millis(400)).await;

    // POST a signed envelope so the listen daemon appends one line.
    let client = build_trusting_reqwest_client(&home);
    let bytes = build_signed_ack_bytes(&home);
    let principal = self_principal();
    let resp = post_bytes(&client, addr, &principal, bytes)
        .await
        .expect("post bytes");
    assert!(resp.status().is_success(), "POST {:?}", resp.status());

    // await must unblock within another ~500ms (one more poll cycle + write).
    let (res, buf) = tokio::time::timeout(Duration::from_secs(3), await_task)
        .await
        .expect("await task timeout")
        .expect("await task panic");
    res.expect("await returned err");

    // Exactly one JSON line on stdout.
    let text = String::from_utf8(buf).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 1, "expected one await output line, got: {text}");

    // Parse and lock the JSON shape.
    let value: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let obj = value.as_object().expect("object");
    assert!(obj.contains_key("offset"), "missing offset");
    assert!(obj.contains_key("task_id"), "missing task_id");
    assert!(obj.contains_key("from"), "missing from");
    assert!(obj.contains_key("class"), "missing class");
    assert!(obj.contains_key("body"), "missing body");
    assert!(obj["offset"].as_u64().is_some(), "offset not u64");
    assert_eq!(obj["from"].as_str().unwrap(), "agent:localhost/self");
    // The ack envelope's class is "ack".
    assert_eq!(obj["class"].as_str().unwrap(), "ack");

    // Cursor should equal the printed offset (advanced past one entry).
    let cursor = InboxCursor::at(home.join("inbox.cursor"));
    let cur = cursor.read().await.unwrap();
    assert_eq!(cur, obj["offset"].as_u64().unwrap());

    // Shutdown.
    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(Duration::from_secs(2), server_task).await;
}

// Silencers (match the set used by the Phase 2/3 integration tests).
use axum as _;
use base64 as _;
use clap as _;
use ed25519_dalek as _;
use famp_canonical as _;
use famp_core as _;
use famp_crypto as _;
use famp_envelope as _;
use famp_fsm as _;
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
use sha2 as _;
use thiserror as _;
use time as _;
use toml as _;
use tower as _;
use tower_http as _;
use url as _;
use uuid as _;

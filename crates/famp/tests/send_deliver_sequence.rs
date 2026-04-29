//! Phase 3 Plan 03-02 Task 2 — `famp send --task` multi-deliver sequence.
//!
//! After a new task, sends three non-terminal deliver envelopes and asserts:
//! - the task record stays in REQUESTED (Phase 3 does not step the FSM on
//!   non-terminal deliver — see `fsm_glue` module docs for the Phase 4 plan)
//! - `record.terminal` stays false
//! - `last_send_at` is updated on each call
//! - the daemon inbox now contains exactly four lines (1 request + 3 deliver)
//!
//! Phase 02 Plan 02-04: gated off — v0.8 HTTPS shape incompatible with
//! v0.9 bus path. See `send_more_coming_requires_new_task.rs` header.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use famp::cli::peer::add::run_add_at;
use famp::cli::send::{run_at as send_run_at, SendArgs};
use famp_taskdir::TaskDir;

use common::{init_home_in_process, wait_for_tls_listener_ready};

fn pubkey_b64(home: &std::path::Path) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    let bytes = std::fs::read(home.join("pub.ed25519")).unwrap();
    URL_SAFE_NO_PAD.encode(bytes)
}

#[ignore = "Phase 02 Plan 02-04: rewired send to bus path; v0.8 HTTPS shape; \
revisit / migrate in Phase 4 federation gateway"]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_deliver_sequence_keeps_record_non_terminal() {
    famp::cli::send::client::allow_tofu_bootstrap_for_tests();

    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home_in_process(&home);

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

    wait_for_tls_listener_ready().await;

    run_add_at(
        &home,
        "self".to_string(),
        format!("https://{addr}"),
        pubkey_b64(&home),
        Some("agent:localhost/self".to_string()),
    )
    .expect("peer add");

    // 1. Open task.
    send_run_at(
        &home,
        SendArgs {
            to: Some("self".to_string()),
            channel: None,
            new_task: Some("open task".to_string()),
            task: None,
            terminal: false,
            body: None,
            more_coming: false,
            act_as: None,
        },
    )
    .await
    .expect("new task send");

    let tasks = TaskDir::open(home.join("tasks")).unwrap();
    let task_id = {
        let records = tasks.list().unwrap();
        assert_eq!(records.len(), 1);
        records[0].task_id.clone()
    };
    let first_send_at = tasks
        .read(&task_id)
        .unwrap()
        .last_send_at
        .expect("last_send_at after first send");

    // 2. Three non-terminal delivers.
    for i in 1..=3 {
        // Small sleep so RFC-3339-second timestamps differ across sends.
        tokio::time::sleep(Duration::from_millis(1100)).await;
        send_run_at(
            &home,
            SendArgs {
                to: Some("self".to_string()),
                channel: None,
                new_task: None,
                task: Some(task_id.clone()),
                terminal: false,
                body: Some(format!("interim {i}")),
                more_coming: false,
                act_as: None,
            },
        )
        .await
        .unwrap_or_else(|e| panic!("deliver {i} failed: {e}"));
    }

    // 3. Record should still be non-terminal, last_send_at should have moved.
    let rec = tasks.read(&task_id).unwrap();
    assert_eq!(rec.state, "REQUESTED");
    assert!(!rec.terminal);
    assert_ne!(rec.last_send_at.as_deref(), Some(first_send_at.as_str()));

    // 4. Inbox should have 5 lines: 1 request + 1 auto-commit reply + 3 delivers.
    // Phase 4: the daemon auto-commits on every inbound request, so the commit
    // reply envelope is stored in the inbox alongside the request and delivers.
    let lines = famp_inbox::read::read_all(home.join("inbox.jsonl")).unwrap();
    assert_eq!(
        lines.len(),
        5,
        "expected 1 request + 1 commit reply + 3 delivers"
    );

    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(Duration::from_secs(2), server_task).await;
}

// Silencers.
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
use serde_json as _;
use sha2 as _;
use thiserror as _;
use time as _;
use toml as _;
use tower as _;
use tower_http as _;
use url as _;
use uuid as _;

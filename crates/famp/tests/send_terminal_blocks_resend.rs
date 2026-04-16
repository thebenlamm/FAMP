//! Phase 3 Plan 03-02 Task 2 — terminal deliver locks further sends.
//!
//! 1. Open a task via `--new-task`.
//! 2. Send `--terminal`: expect FSM advance to COMPLETED, `record.terminal = true`.
//! 3. Attempt another send on the same task: expect `CliError::TaskTerminal`.
//! 4. Assert inbox line count did not grow beyond step 2 and the record on
//!    disk is unchanged after the rejected send.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use famp::cli::error::CliError;
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
#[allow(clippy::too_many_lines)]
async fn terminal_send_locks_resend() {
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

    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline, "bind timed out");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    run_add_at(
        &home,
        "self".to_string(),
        format!("https://{addr}"),
        pubkey_b64(&home),
        Some("agent:localhost/self".to_string()),
    )
    .expect("peer add");

    // Open task.
    send_run_at(
        &home,
        SendArgs {
            to: "self".to_string(),
            new_task: Some("terminal test".to_string()),
            task: None,
            terminal: false,
            body: None,
        },
    )
    .await
    .expect("new task");

    let tasks = TaskDir::open(home.join("tasks")).unwrap();
    let task_id = tasks.list().unwrap()[0].task_id.clone();

    // Phase 4: await the auto-commit reply to advance record to COMMITTED.
    // The daemon fires a commit reply on every inbound request; we must
    // consume it via `famp await --task <id>` before sending a terminal deliver
    // (advance_terminal requires COMMITTED state since the FSM shortcut was removed).
    {
        let mut buf = Vec::<u8>::new();
        famp::cli::await_cmd::run_at(
            &home,
            famp::cli::await_cmd::AwaitArgs {
                timeout: "5s".to_string(),
                task: Some(task_id.clone()),
            },
            &mut buf,
        )
        .await
        .expect("await commit reply before terminal send");
    }
    let rec_committed = tasks.read(&task_id).unwrap();
    assert_eq!(
        rec_committed.state, "COMMITTED",
        "must be COMMITTED before terminal send"
    );

    // Terminal deliver.
    send_run_at(
        &home,
        SendArgs {
            to: "self".to_string(),
            new_task: None,
            task: Some(task_id.clone()),
            terminal: true,
            body: Some("done".to_string()),
        },
    )
    .await
    .expect("terminal send");

    let rec_after_terminal = tasks.read(&task_id).unwrap();
    assert_eq!(rec_after_terminal.state, "COMPLETED");
    assert!(rec_after_terminal.terminal);

    let lines_after_terminal = famp_inbox::read::read_all(home.join("inbox.jsonl")).unwrap();
    // Phase 4: request + commit-reply + terminal deliver = 3 lines.
    assert_eq!(
        lines_after_terminal.len(),
        3,
        "request + commit reply + terminal deliver"
    );

    // Subsequent send must fail with TaskTerminal.
    let err = send_run_at(
        &home,
        SendArgs {
            to: "self".to_string(),
            new_task: None,
            task: Some(task_id.clone()),
            terminal: false,
            body: Some("should fail".to_string()),
        },
    )
    .await
    .expect_err("expected TaskTerminal");
    match err {
        CliError::TaskTerminal { task_id: got } => assert_eq!(got, task_id),
        other => panic!("expected TaskTerminal, got {other:?}"),
    }

    // Record unchanged, inbox line count unchanged.
    let rec_after_reject = tasks.read(&task_id).unwrap();
    assert_eq!(rec_after_reject, rec_after_terminal);
    let lines_after_reject = famp_inbox::read::read_all(home.join("inbox.jsonl")).unwrap();
    assert_eq!(lines_after_reject.len(), 3);

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

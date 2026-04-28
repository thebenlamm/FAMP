//! Quick task 260424-7z5 — regression test for `famp_send` `new_task` body loss.
//!
//! Beta feedback (2026-04-24) reported that `famp send --new-task "<title>"
//! --body "<prose>"` silently dropped the prose on the wire: receivers saw
//! `body.scope == {}` and had no way to recover the task content.
//!
//! This test locks in the fix: with a body present, the signed request
//! envelope on the peer's `inbox.jsonl` MUST have
//! `body.scope.instructions == <prose>` (exact match) and
//! `body.natural_language_summary == <title>` (title path unchanged).
//!
//! On the pre-fix build this test MUST fail with a clear `scope.instructions
//! did not match …` assertion diff.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use famp::cli::peer::add::run_add_at;
use famp::cli::send::{run_at as send_run_at, SendArgs};

use common::{init_home_in_process, wait_for_tls_listener_ready};

fn pubkey_b64(home: &std::path::Path) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    let bytes = std::fs::read(home.join("pub.ed25519")).unwrap();
    URL_SAFE_NO_PAD.encode(bytes)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_new_task_body_lands_in_scope_instructions() {
    // Existing tests rely on first-contact TOFU pinning, which is opt-in.
    // The production env var equivalent is FAMP_TOFU_BOOTSTRAP=1.
    famp::cli::send::client::allow_tofu_bootstrap_for_tests();

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

    wait_for_tls_listener_ready().await;

    // Register the daemon as peer "self" with the daemon's own pubkey.
    run_add_at(
        &home,
        "self".to_string(),
        format!("https://{addr}"),
        pubkey_b64(&home),
        Some("agent:localhost/self".to_string()),
    )
    .expect("peer add");

    // Send a new task WITH a body — this is the behaviour under test.
    let title = "verify scope.instructions round-trip";
    let body = "This prose is the real task content. It MUST reach the peer intact.";
    let args = SendArgs {
        to: "self".to_string(),
        new_task: Some(title.to_string()),
        task: None,
        terminal: false,
        body: Some(body.to_string()),
        more_coming: false,
    };
    send_run_at(&home, args).await.expect("famp send");

    // Give the daemon time to persist the inbound envelope.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let lines = famp_inbox::read::read_all(home.join("inbox.jsonl")).unwrap();
    let request = lines
        .iter()
        .find(|l| l.get("class").and_then(|c| c.as_str()) == Some("request"))
        .expect("no request line in inbox");

    let scope = request.pointer("/body/scope").expect("body.scope missing");
    assert_eq!(
        scope.pointer("/instructions").and_then(|v| v.as_str()),
        Some(body),
        "scope.instructions did not match the sent body; actual scope = {scope}"
    );
    let nls = request
        .pointer("/body/natural_language_summary")
        .and_then(|v| v.as_str());
    assert_eq!(nls, Some(title), "natural_language_summary regressed");

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
use famp_taskdir as _;
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

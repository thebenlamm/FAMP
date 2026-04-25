//! Regression test for the TOFU bootstrap hardening.
//!
//! By default, `famp send` MUST refuse a first-contact connection (no pinned
//! `tls_fingerprint_sha256` for the alias). The pre-fix behaviour silently
//! pinned whatever leaf the network returned, which made a one-time on-path
//! attacker able to permanently hijack the alias.
//!
//! This test deliberately does NOT call
//! `allow_tofu_bootstrap_for_tests()` and does NOT set
//! `FAMP_TOFU_BOOTSTRAP=1` — and lives in its own test binary so other
//! tests' process-wide opt-in cannot leak in.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use famp::cli::peer::add::run_add_at;
use famp::cli::send::{run_at as send_run_at, SendArgs};

use common::init_home_in_process;

fn pubkey_b64(home: &std::path::Path) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    let bytes = std::fs::read(home.join("pub.ed25519")).unwrap();
    URL_SAFE_NO_PAD.encode(bytes)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn first_contact_without_pin_or_opt_in_is_refused() {
    // Ensure the env opt-in is NOT set for this test binary.
    // (set_var would race; this test process should not have it set anyway.)
    assert!(
        std::env::var("FAMP_TOFU_BOOTSTRAP").is_err(),
        "test must run without FAMP_TOFU_BOOTSTRAP set"
    );
    // Tripwire: this test binary must NOT contain any test that calls
    // `allow_tofu_bootstrap_for_tests()` (e.g. via `setup_home()`), or
    // parallel execution would silently flip the atomic and regress this
    // assertion to a false pass. If a future dev adds such a test here,
    // this assertion fires loudly instead of letting the security check
    // become a no-op.
    assert!(
        !famp::cli::send::client::ALLOW_TOFU_BOOTSTRAP_FOR_TESTS
            .load(std::sync::atomic::Ordering::SeqCst),
        "test binary must not have any test that opts into TOFU bootstrap"
    );

    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    init_home_in_process(&home);

    // Spin up the daemon so a real TLS leaf is presented.
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

    // Wait for daemon bind.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline, "bind timeout");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    // Register peer with NO tls_fingerprint_sha256 — first-contact territory.
    run_add_at(
        &home,
        "self".to_string(),
        format!("https://{addr}"),
        pubkey_b64(&home),
        Some("agent:localhost/self".to_string()),
    )
    .expect("peer add");

    let res = send_run_at(
        &home,
        SendArgs {
            to: "self".to_string(),
            new_task: Some("must be refused".to_string()),
            task: None,
            terminal: false,
            body: None,
            more_coming: false,
        },
    )
    .await;
    let err = res.expect_err("first contact without opt-in must fail closed");
    let kind = err.mcp_error_kind();
    assert_eq!(
        kind, "tofu_bootstrap_refused",
        "expected typed TofuBootstrapRefused, got {kind}: {err}"
    );

    // Tear down.
    let _ = shutdown_tx.send(());
    let _ = tokio::time::timeout(Duration::from_secs(2), server_task).await;
}

// Silencers
use axum as _;
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

//! Phase 4 Plan 04-01 Task 1 — multi-entry keyring tests.
//!
//! Verifies that a `famp listen` daemon built from a `peers.toml` with N
//! registered peers:
//!   A. Accepts signed envelopes from a registered peer principal.
//!   B. Accepts signed envelopes from self (backward compat).
//!   C. Rejects signed envelopes from an unknown principal with non-2xx.
//!
//! Two distinct homes are used for Tests A and C to isolate key material:
//!   - `daemon_home` — the home running the daemon (has TLS cert, signing key)
//!   - `sender_home` — a separate initialized home representing a peer
//!
//! Test B uses a single home (from == to == agent:localhost/self) mirroring
//! the Phase 3 convention; this confirms backward compat is preserved after
//! the multi-peer keyring migration.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use common::conversation_harness::{pubkey_b64, setup_home};
use common::listen_harness::{
    build_trusting_reqwest_client, init_home_in_process, load_self_signing_key, post_bytes,
    self_principal, wait_for_tls_listener_ready,
};
use famp::cli::peer::add::run_add_at;
use famp_core::{AuthorityScope, MessageId, Principal};
use famp_envelope::{
    body::{request::RequestBody, Bounds},
    SignedEnvelope, Timestamp, UnsignedEnvelope,
};
use tokio::sync::oneshot;

/// Spawn the `famp listen` daemon in-process against `daemon_home`.
/// Returns (bound addr, join handle, shutdown sender).
async fn spawn_daemon_in_process(
    daemon_home: &std::path::Path,
) -> (SocketAddr, tokio::task::JoinHandle<()>, oneshot::Sender<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel::<()>();
    let home_owned = daemon_home.to_path_buf();
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

async fn stop_daemon(handle: tokio::task::JoinHandle<()>, tx: oneshot::Sender<()>) {
    let _ = tx.send(());
    let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
}

/// Build a signed `request` envelope from `sender_home` using `from_principal`
/// and `to_principal`.
fn build_signed_request_bytes(
    sender_home: &std::path::Path,
    from_principal: &Principal,
    to_principal: &Principal,
) -> Vec<u8> {
    let sk = load_self_signing_key(sender_home);
    let id = MessageId::new_v7();
    let ts = Timestamp("2026-04-15T00:00:00Z".to_string());
    let body = RequestBody {
        scope: serde_json::Value::Object(serde_json::Map::new()),
        bounds: Bounds {
            hop_limit: Some(16),
            recursion_depth: Some(4),
            deadline: None,
            budget: None,
            policy_domain: None,
            authority_scope: None,
            max_artifact_size: None,
            confidence_floor: None,
        },
        natural_language_summary: Some("test request".to_string()),
    };
    let unsigned: UnsignedEnvelope<RequestBody> = UnsignedEnvelope::new(
        id,
        from_principal.clone(),
        to_principal.clone(),
        AuthorityScope::Advisory,
        ts,
        body,
    );
    let signed: SignedEnvelope<RequestBody> = unsigned.sign(&sk).expect("sign request");
    signed.encode().expect("encode")
}

// ---------------------------------------------------------------------------
// Test A: envelope from a peer registered in daemon's peers.toml is accepted.
// ---------------------------------------------------------------------------
            surface that Phase 04 removes per ROADMAP.md (`famp setup/init/listen/peer add`, \
            old `famp send`). Held at #[ignore] until Phase 04 either migrates this to \
            the `famp-transport-http` library API (alongside `e2e_two_daemons`) or deletes \
            it with the v0.8 CLI surface."]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepts_envelope_from_registered_peer() {
    // Setup daemon home D and sender home S.
    let daemon_tmp = setup_home();
    let daemon_home = daemon_tmp.path();

    let sender_tmp = tempfile::TempDir::new().unwrap();
    let sender_home = sender_tmp.path();
    init_home_in_process(sender_home);

    // Register S's pubkey in D's peers.toml with principal agent:localhost/sender.
    let sender_pk = pubkey_b64(sender_home);
    let sender_principal = "agent:localhost/sender";
    // We need an endpoint to register; the actual value doesn't matter for keyring building,
    // but peer add validates it as a valid HTTPS URL.
    run_add_at(
        daemon_home,
        "sender".to_string(),
        "https://127.0.0.1:9999".to_string(),
        sender_pk,
        Some(sender_principal.to_string()),
    )
    .expect("peer add sender into daemon home");

    // Start the daemon using daemon_home's keyring (should include the sender entry).
    let (addr, handle, shutdown_tx) = spawn_daemon_in_process(daemon_home).await;

    // POST a signed request from sender_home using sender's key and principal.
    let from_p: Principal = sender_principal.parse().unwrap();
    let to_p: Principal = "agent:localhost/self".parse().unwrap();
    let bytes = build_signed_request_bytes(sender_home, &from_p, &to_p);
    let client = build_trusting_reqwest_client(daemon_home);
    let resp = post_bytes(&client, addr, &to_p, bytes).await.expect("post");

    let status = resp.status();
    stop_daemon(handle, shutdown_tx).await;

    assert_eq!(
        status.as_u16(),
        200,
        "daemon must accept signed envelope from registered peer (got {status})"
    );
}

// ---------------------------------------------------------------------------
// Test B: envelope where from == to == self still accepted (backward compat).
// ---------------------------------------------------------------------------
            surface that Phase 04 removes per ROADMAP.md (`famp setup/init/listen/peer add`, \
            old `famp send`). Held at #[ignore] until Phase 04 either migrates this to \
            the `famp-transport-http` library API (alongside `e2e_two_daemons`) or deletes \
            it with the v0.8 CLI surface."]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepts_envelope_from_self() {
    let tmp = setup_home();
    let home = tmp.path();

    let (addr, handle, shutdown_tx) = spawn_daemon_in_process(home).await;

    // self_principal() = agent:localhost/self, signing key from home.
    let me = self_principal();
    let bytes = {
        use famp_envelope::body::{AckBody, AckDisposition};
        let id = MessageId::new_v7();
        let ts = Timestamp("2026-04-15T00:00:00Z".to_string());
        let body = AckBody {
            disposition: AckDisposition::Accepted,
            reason: None,
        };
        let unsigned: UnsignedEnvelope<AckBody> = UnsignedEnvelope::new(
            id,
            me.clone(),
            me.clone(),
            AuthorityScope::Advisory,
            ts,
            body,
        );
        let signed = unsigned.sign(&load_self_signing_key(home)).expect("sign");
        signed.encode().expect("encode")
    };
    let client = build_trusting_reqwest_client(home);
    let resp = post_bytes(&client, addr, &me, bytes).await.expect("post");
    let status = resp.status();
    stop_daemon(handle, shutdown_tx).await;

    assert_eq!(
        status.as_u16(),
        200,
        "daemon must accept self-signed envelope (backward compat, got {status})"
    );
}

// ---------------------------------------------------------------------------
// Test C: envelope from an unknown principal is rejected (non-2xx).
// ---------------------------------------------------------------------------
            surface that Phase 04 removes per ROADMAP.md (`famp setup/init/listen/peer add`, \
            old `famp send`). Held at #[ignore] until Phase 04 either migrates this to \
            the `famp-transport-http` library API (alongside `e2e_two_daemons`) or deletes \
            it with the v0.8 CLI surface."]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejects_envelope_from_unknown_principal() {
    // Daemon home D — no peers registered (clean).
    let daemon_tmp = setup_home();
    let daemon_home = daemon_tmp.path();

    // Sender home S with its own key; NOT registered in D's peers.toml.
    let ghost_tmp = tempfile::TempDir::new().unwrap();
    let ghost_home = ghost_tmp.path();
    init_home_in_process(ghost_home);

    let (addr, handle, shutdown_tx) = spawn_daemon_in_process(daemon_home).await;

    // Build a request signed by ghost's key but claiming principal agent:localhost/ghost.
    let ghost_p: Principal = "agent:localhost/ghost".parse().unwrap();
    let to_p: Principal = "agent:localhost/self".parse().unwrap();
    let bytes = build_signed_request_bytes(ghost_home, &ghost_p, &to_p);

    let client = build_trusting_reqwest_client(daemon_home);
    let resp = post_bytes(&client, addr, &to_p, bytes).await.expect("post");
    let status = resp.status();
    stop_daemon(handle, shutdown_tx).await;

    assert!(
        !status.is_success(),
        "daemon must reject envelope from unregistered principal (got {status})"
    );
}

// Silencers.
use axum as _;
use base64 as _;
use clap as _;
use ed25519_dalek as _;
use famp_canonical as _;
use famp_core as _;
use famp_crypto as _;
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

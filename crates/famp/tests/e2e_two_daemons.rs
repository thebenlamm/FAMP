//! Phase 4 plumb-line-2 insurance: target famp-transport-http library API
//! directly to keep the federation HTTPS path exercised in `just ci`.
//!
//! After Phase 4 the federation CLI verbs (`famp init/setup/listen/peer`) are
//! deleted; this test is the SOLE consumer of `famp-transport-http` + `famp-keyring`
//! from the active test surface. See ARCHITECTURE.md and
//! `crates/famp/tests/_deferred_v1/README.md` (created in Plan 04-02) for the
//! v1.0 reactivation context. Same tokio runtime + two `tokio::spawn` listener
//! tasks per D-10; fixture certs reused per D-11; conversation shape (request ->
//! commit -> deliver -> ack) unchanged per D-12.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::similar_names,
    clippy::significant_drop_tightening,
    clippy::doc_markdown,
    unused_crate_dependencies
)]

// Pull the cycle_driver via #[path] so this test binary and the example
// consume the SAME driver implementation.
#[path = "common/cycle_driver.rs"]
mod cycle_driver;

use std::{
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
};

use famp_core::Principal;
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_keyring::Keyring;
use famp_transport_http::{build_router, tls, tls_server, HttpTransport};
use url::Url;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("cross_machine")
}

async fn wait_for_tls_listener_ready() {
    tokio::task::yield_now().await;
    tokio::time::sleep(std::time::Duration::from_millis(75)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn e2e_two_daemons_happy_path() {
    // --- Keys + keyrings ---
    let alice = Principal::from_str("agent:local/alice").unwrap();
    let bob = Principal::from_str("agent:local/bob").unwrap();

    let alice_sk = FampSigningKey::from_bytes([1u8; 32]);
    let bob_sk = FampSigningKey::from_bytes([2u8; 32]);
    let alice_vk: TrustedVerifyingKey = alice_sk.verifying_key();
    let bob_vk: TrustedVerifyingKey = bob_sk.verifying_key();

    let alice_keyring = Arc::new(
        Keyring::new()
            .with_peer(bob.clone(), bob_vk.clone())
            .unwrap()
            .with_peer(alice.clone(), alice_vk.clone())
            .unwrap(),
    );
    let bob_keyring = Arc::new(
        Keyring::new()
            .with_peer(alice.clone(), alice_vk.clone())
            .unwrap()
            .with_peer(bob.clone(), bob_vk.clone())
            .unwrap(),
    );

    // --- Fixture certs from disk (D-B7) ---
    let dir = fixture_dir();
    let alice_cert = tls::load_pem_cert(&dir.join("alice.crt")).unwrap();
    let alice_key = tls::load_pem_key(&dir.join("alice.key")).unwrap();
    let bob_cert = tls::load_pem_cert(&dir.join("bob.crt")).unwrap();
    let bob_key = tls::load_pem_key(&dir.join("bob.key")).unwrap();
    let alice_server_cfg = tls::build_server_config(alice_cert, alice_key).unwrap();
    let bob_server_cfg = tls::build_server_config(bob_cert, bob_key).unwrap();

    // --- Bind listeners first (read local_addr before spawn) ---
    let bob_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    bob_listener.set_nonblocking(true).unwrap();
    let bob_addr = bob_listener.local_addr().unwrap();
    let alice_listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    alice_listener.set_nonblocking(true).unwrap();
    let alice_addr = alice_listener.local_addr().unwrap();

    // --- HttpTransports (each trusts the peer's fixture cert) ---
    let alice_trust = dir.join("bob.crt");
    let bob_trust = dir.join("alice.crt");

    let alice_transport = HttpTransport::new_client_only(Some(&alice_trust)).unwrap();
    alice_transport.register(alice.clone()).await;
    alice_transport
        .add_peer(
            bob.clone(),
            Url::parse(&format!("https://localhost:{}/", bob_addr.port())).unwrap(),
        )
        .await;

    let bob_transport = HttpTransport::new_client_only(Some(&bob_trust)).unwrap();
    bob_transport.register(bob.clone()).await;
    bob_transport
        .add_peer(
            alice.clone(),
            Url::parse(&format!("https://localhost:{}/", alice_addr.port())).unwrap(),
        )
        .await;

    // --- Spawn the two axum-rustls servers ---
    let bob_router = build_router(bob_keyring.clone(), bob_transport.inboxes());
    let alice_router = build_router(alice_keyring.clone(), alice_transport.inboxes());

    let bob_handle =
        tls_server::serve_std_listener(bob_listener, bob_router, Arc::new(bob_server_cfg));
    let alice_handle =
        tls_server::serve_std_listener(alice_listener, alice_router, Arc::new(alice_server_cfg));
    bob_transport.attach_server(bob_handle).await;
    alice_transport.attach_server(alice_handle).await;

    wait_for_tls_listener_ready().await;

    // --- Drive the cycle via the shared helper ---
    let trace_alice: cycle_driver::Trace = Arc::new(Mutex::new(Vec::new()));
    let trace_bob: cycle_driver::Trace = Arc::new(Mutex::new(Vec::new()));

    let bob_fut = cycle_driver::drive_bob(
        &bob_transport,
        &bob_keyring,
        &bob,
        &alice,
        &bob_sk,
        &trace_bob,
    );
    let alice_fut = cycle_driver::drive_alice(
        &alice_transport,
        &alice_keyring,
        &alice,
        &bob,
        &alice_sk,
        &trace_alice,
    );

    let (bob_res, alice_res) = tokio::join!(bob_fut, alice_fut);
    bob_res.expect("bob driver");
    alice_res.expect("alice driver");

    // Trace sanity — alice's trace must see Commit, Deliver, Ack lines.
    let alice_trace = trace_alice.lock().unwrap();
    assert!(
        alice_trace.iter().any(|l| l.contains("Commit")),
        "alice trace missing Commit: {alice_trace:?}"
    );
    assert!(
        alice_trace.iter().any(|l| l.contains("Deliver")),
        "alice trace missing Deliver: {alice_trace:?}"
    );
    assert!(
        alice_trace.iter().any(|l| l.contains("Ack")),
        "alice trace missing Ack: {alice_trace:?}"
    );
}

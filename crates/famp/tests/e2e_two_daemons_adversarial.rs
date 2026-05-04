//! Phase 4 plumb-line-2 sentinel (D-09): the FampSigVerifyLayer middleware
//! rejects an unsigned envelope BEFORE the inbox handler closure is entered.
//! Cheapest possible sentinel; full adversarial matrix lives in
//! `tests/adversarial/http.rs` per D-13 (not duplicated here).

#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use famp_core::Principal;
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_keyring::Keyring;
use famp_transport::TransportMessage;
use famp_transport_http::{build_router, InboxRegistry};
use std::{
    collections::HashMap,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::{mpsc, Mutex};

fn bob() -> Principal {
    Principal::from_str("agent:localhost/bob").unwrap()
}

fn build_bob_keyring() -> Arc<Keyring> {
    let bob_sk = FampSigningKey::from_bytes([2u8; 32]);
    let bob_vk: TrustedVerifyingKey = bob_sk.verifying_key();
    Arc::new(Keyring::new().with_peer(bob(), bob_vk).unwrap())
}

struct HttpRig {
    base_url: String,
    inbox_rx: mpsc::Receiver<TransportMessage>,
    sentinel: Arc<AtomicBool>,
    _server: tokio::task::JoinHandle<()>,
}

async fn build_rig() -> HttpRig {
    let inboxes: Arc<InboxRegistry> = Arc::new(Mutex::new(HashMap::new()));
    let (tx, inbox_rx) = mpsc::channel::<TransportMessage>(8);
    inboxes.lock().await.insert(bob(), tx);

    let keyring = build_bob_keyring();
    let router = build_router(keyring, inboxes.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    HttpRig {
        base_url: format!("http://{addr}"),
        inbox_rx,
        sentinel: Arc::new(AtomicBool::new(false)),
        _server: server,
    }
}

#[tokio::test]
async fn e2e_two_daemons_rejects_unsigned() {
    let mut rig = build_rig().await;

    // Hand-craft an unsigned envelope (no `signature` field). Use a minimal
    // JSON body the server will parse far enough to feed the FampSigVerifyLayer.
    // The middleware decodes the envelope shape first, then checks signature;
    // a missing signature short-circuits before route dispatch.
    let unsigned_envelope = serde_json::json!({
        "version": "0.5.2",
        "class": "request",
        "from": "agent:localhost/alice",
        "to": "agent:localhost/bob",
        "id": "01HZZZZZZZZZZZZZZZZZZZZZZZ",
        "ts": "2026-05-03T00:00:00Z",
        "body": { "task": "test", "scope": [], "instructions": "x" }
    });

    let client = reqwest::Client::builder().build().unwrap();
    let resp = client
        .post(format!(
            "{}/famp/v0.5.1/inbox/agent%3Alocalhost%2Fbob",
            rig.base_url
        ))
        .header("content-type", "application/json")
        .body(unsigned_envelope.to_string())
        .send()
        .await
        .unwrap();

    assert!(
        !resp.status().is_success(),
        "FampSigVerifyLayer must reject unsigned envelope; got status {}",
        resp.status()
    );

    // Sentinel proof: the inbox handler's sole side-effect is pushing onto the
    // mpsc channel. If try_recv returns Ok, the handler ran and the middleware
    // failed to short-circuit.
    match rig.inbox_rx.try_recv() {
        Err(mpsc::error::TryRecvError::Empty) => {
            // Expected: middleware short-circuited, handler never ran.
        }
        Ok(_msg) => {
            rig.sentinel.store(true, Ordering::SeqCst);
            panic!(
                "Phase 4 D-09 sentinel: handler closure entered on unsigned envelope; \
                 FampSigVerifyLayer middleware FAILED to short-circuit"
            );
        }
        Err(mpsc::error::TryRecvError::Disconnected) => {
            panic!("inbox channel disconnected unexpectedly");
        }
    }
    assert!(
        !rig.sentinel.load(Ordering::SeqCst),
        "sentinel must remain false: handler must not have entered"
    );
}

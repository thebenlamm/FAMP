//! HttpTransport adversarial rows + sentinel proof of TRANS-09 SC#2.
//!
//! D-D2: raw reqwest POST as the injection surface (NO test-util feature).
//! D-D5: an mpsc receiver acts as the sentinel — the inbox handler's only
//!       visible effect is pushing a `TransportMessage` onto the channel, so
//!       if the handler was entered, `try_recv` on the receiver returns
//!       `Ok(_)`. We assert it returns `Empty` on every adversarial case,
//!       proving the middleware stopped the request before handler dispatch.
//! D-D6 HTTP column: the server response status+slug must match the D-C6
//!       mapping per case.
//!
//! The test rig mounts `famp_transport_http::build_router` on plain HTTP
//! (127.0.0.1:ephemeral, no TLS). TLS adds nothing to adversarial-byte
//! rejection — the middleware runs identically regardless. The happy-path
//! HTTPS cycle is covered by Plan 04-04's `http_happy_path_same_process`.
//!
//! Note on the sentinel seam: the originally-proposed `route_layer(from_fn)`
//! approach in the plan draft is observationally equivalent to the mpsc
//! try_recv approach — both prove the handler closure never ran. We use the
//! mpsc path because it is strictly black-box (no axum internals), survives
//! any future middleware reshuffle, and matches D-D5 verbatim ("sentinel ==
//! handler-observable side-effect").

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use super::fixtures::{alice, bob, case_bytes, ALICE_SECRET};
use super::harness::{assert_expected_error, Case};
use famp::runtime::RuntimeError;
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_envelope::EnvelopeDecodeError;
use famp_keyring::Keyring;
use famp_transport::TransportMessage;
use famp_transport_http::{build_router, InboxRegistry};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::{mpsc, Mutex};

struct HttpRig {
    base_url: String,
    /// The inbox receiver for `bob`. If the handler closure was entered, a
    /// message would be queued here — adversarial rows assert it stays empty.
    inbox_rx: mpsc::Receiver<TransportMessage>,
    /// Secondary sentinel kept for symmetry with D-D5 wording: set to `true`
    /// only if the inbox_rx yields a message (i.e. the handler fired).
    sentinel: Arc<AtomicBool>,
    server: tokio::task::JoinHandle<()>,
}

fn build_bob_keyring() -> Arc<Keyring> {
    let alice_sk = FampSigningKey::from_bytes(ALICE_SECRET);
    let alice_vk: TrustedVerifyingKey = alice_sk.verifying_key();
    Arc::new(Keyring::new().with_peer(alice(), alice_vk).unwrap())
}

async fn build_rig() -> HttpRig {
    // Pre-register bob so the failure mode cannot be unknown_recipient — we
    // want bad_envelope / signature_invalid / canonical_divergence to surface.
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
    // Tiny settle so server is ready to accept.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    HttpRig {
        base_url: format!("http://{addr}"),
        inbox_rx,
        sentinel: Arc::new(AtomicBool::new(false)),
        server,
    }
}

async fn inject(rig: &HttpRig, bytes: Vec<u8>) -> (u16, Option<String>) {
    let url = format!("{}/famp/v0.5.1/inbox/{}", rig.base_url, bob());
    let resp = reqwest::Client::new()
        .post(&url)
        .header("content-type", "application/famp+json")
        .body(bytes)
        .send()
        .await
        .expect("reqwest send");
    let status = resp.status().as_u16();
    let body = resp.text().await.unwrap_or_default();
    let slug = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.as_str().map(String::from)));
    (status, slug)
}

fn project_to_runtime_error(status: u16, slug: Option<&str>) -> RuntimeError {
    match (status, slug.unwrap_or("")) {
        (400, "bad_envelope") => RuntimeError::Decode(EnvelopeDecodeError::MissingSignature),
        (401, "signature_invalid") => RuntimeError::Decode(EnvelopeDecodeError::SignatureInvalid),
        (400, "canonical_divergence") => RuntimeError::CanonicalDivergence,
        _ => {
            panic!("HTTP adversarial returned status={status} slug={slug:?} (not in D-D6 mapping)")
        }
    }
}

async fn run_http_case(case: Case) {
    let mut rig = build_rig().await;
    let bytes = case_bytes(case);
    let (status, slug) = inject(&rig, bytes).await;
    let projected = project_to_runtime_error(status, slug.as_deref());
    assert_expected_error(case, &projected);

    // D-D5 sentinel proof: the inbox handler's sole side-effect is pushing a
    // TransportMessage onto this channel. If we can observe a message, the
    // handler ran. try_recv must return Empty on every adversarial row.
    match rig.inbox_rx.try_recv() {
        Err(mpsc::error::TryRecvError::Empty) => {
            // Expected: middleware short-circuited, handler never ran.
        }
        Ok(msg) => {
            rig.sentinel.store(true, Ordering::SeqCst);
            panic!(
                "TRANS-09 SC#2: handler closure entered on adversarial case {case:?}; \
                 observed TransportMessage sender={} recipient={}",
                msg.sender, msg.recipient
            );
        }
        Err(mpsc::error::TryRecvError::Disconnected) => {
            panic!("inbox channel disconnected unexpectedly for case {case:?}");
        }
    }
    assert!(
        !rig.sentinel.load(Ordering::SeqCst),
        "TRANS-09 SC#2: handler closure entered on adversarial case {case:?}"
    );

    rig.server.abort();
}

#[tokio::test]
async fn http_unsigned() {
    run_http_case(Case::Unsigned).await;
}

#[tokio::test]
async fn http_wrong_key() {
    run_http_case(Case::WrongKey).await;
}

#[tokio::test]
async fn http_canonical_divergence() {
    run_http_case(Case::CanonicalDivergence).await;
}

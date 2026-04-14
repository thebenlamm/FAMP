//! TRANS-09 SC#2 — middleware rejects 4 adversarial case shapes BEFORE the
//! handler closure is invoked. Mechanism (D-D5): `Arc<AtomicBool>` sentinel
//! wired into a custom handler that flips to `true` if entered.
//!
//! Four cases:
//!   1. unsigned (non-empty keyring, alice pinned) → 400 bad_envelope
//!   2. body > 1 MB                                → 413 body_too_large
//!   3. unknown sender (empty keyring)             → 401 unknown_sender
//!   4. wrong-key signature                        → 401 signature_invalid
//!
//! Case 1 uses a NON-EMPTY keyring so the "unsigned" path is distinct from
//! the "unknown sender" path — checker W-1 fix.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::doc_markdown,
    unused_crate_dependencies
)]

use std::str::FromStr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use axum::{
    body::{Body, Bytes},
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use famp_canonical::{canonicalize, from_slice_strict};
use famp_core::{AuthorityScope, MessageId, Principal};
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_envelope::body::{AckBody, AckDisposition, Bounds, Budget, RequestBody};
use famp_envelope::{SignedEnvelope, Timestamp, UnsignedEnvelope};
use famp_keyring::Keyring;
use famp_transport_http::FampSigVerifyLayer;
use tower::{ServiceBuilder, ServiceExt};
use tower_http::limit::RequestBodyLimitLayer;

const ONE_MIB: usize = 1_048_576;
const ALICE_SECRET: [u8; 32] = [1u8; 32];
const WRONG_SECRET: [u8; 32] = [99u8; 32];

fn ts() -> Timestamp {
    Timestamp("2026-04-13T00:00:00Z".to_string())
}

fn two_key_bounds() -> Bounds {
    Bounds {
        deadline: Some("2026-05-01T00:00:00Z".to_string()),
        budget: Some(Budget {
            amount: "100".to_string(),
            unit: "usd".to_string(),
        }),
        hop_limit: None,
        policy_domain: None,
        authority_scope: None,
        max_artifact_size: None,
        confidence_floor: None,
        recursion_depth: None,
    }
}

fn fixed_msg_id() -> MessageId {
    "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a6b".parse().unwrap()
}

fn canonical_bytes<B: famp_envelope::BodySchema>(signed: &SignedEnvelope<B>) -> Vec<u8> {
    let encoded = signed.encode().unwrap();
    let value: serde_json::Value = from_slice_strict(&encoded).unwrap();
    canonicalize(&value).unwrap()
}

/// Alice pinned to her REAL key in a fresh keyring.
fn alice_pinned_keyring() -> Arc<Keyring> {
    let alice = Principal::from_str("agent:local/alice").unwrap();
    let alice_vk: TrustedVerifyingKey = FampSigningKey::from_bytes(ALICE_SECRET).verifying_key();
    let kr = Keyring::new().with_peer(alice, alice_vk).unwrap();
    Arc::new(kr)
}

fn empty_keyring() -> Arc<Keyring> {
    Arc::new(Keyring::new())
}

fn sentinel_router(keyring: Arc<Keyring>, sentinel: Arc<AtomicBool>) -> Router {
    Router::new()
        .route(
            "/famp/v0.5.1/inbox/{principal}",
            post(move |_body: Bytes| {
                let s = sentinel.clone();
                async move {
                    s.store(true, Ordering::SeqCst);
                    StatusCode::ACCEPTED
                }
            }),
        )
        .layer(
            ServiceBuilder::new()
                .layer(RequestBodyLimitLayer::new(ONE_MIB))
                .map_request(|req: Request<_>| req.map(Body::new))
                .layer(FampSigVerifyLayer::new(keyring)),
        )
}

/// CONF-05 shape: a valid signed Ack envelope with the `signature` field
/// stripped, then re-canonicalized. Alice is pinned in the keyring so this
/// hits `MissingSignature`, NOT `UnknownSender`.
fn unsigned_bytes() -> Vec<u8> {
    let alice_sk = FampSigningKey::from_bytes(ALICE_SECRET);
    let alice = Principal::from_str("agent:local/alice").unwrap();
    let bob = Principal::from_str("agent:local/bob").unwrap();
    let body = AckBody {
        disposition: AckDisposition::Accepted,
        reason: None,
    };
    let signed = UnsignedEnvelope::<AckBody>::new(
        fixed_msg_id(),
        alice,
        bob,
        AuthorityScope::Advisory,
        ts(),
        body,
    )
    .sign(&alice_sk)
    .unwrap();
    let encoded = signed.encode().unwrap();
    let mut value: serde_json::Value = from_slice_strict(&encoded).unwrap();
    value.as_object_mut().unwrap().remove("signature");
    canonicalize(&value).unwrap()
}

/// CONF-06 shape: signed with WRONG_SECRET, but from=alice (pinned to
/// ALICE_SECRET's pubkey). Verify-against-alice fails.
fn wrong_key_bytes() -> Vec<u8> {
    let wrong_sk = FampSigningKey::from_bytes(WRONG_SECRET);
    let alice = Principal::from_str("agent:local/alice").unwrap();
    let bob = Principal::from_str("agent:local/bob").unwrap();
    let body = RequestBody {
        scope: serde_json::json!({"task": "translate"}),
        bounds: two_key_bounds(),
        natural_language_summary: None,
    };
    let signed = UnsignedEnvelope::<RequestBody>::new(
        fixed_msg_id(),
        alice,
        bob,
        AuthorityScope::Advisory,
        ts(),
        body,
    )
    .sign(&wrong_sk)
    .unwrap();
    canonical_bytes(&signed)
}

async fn oneshot(app: Router, uri: &str, body: Vec<u8>) -> axum::response::Response {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .body(Body::from(body))
        .unwrap();
    app.oneshot(req).await.unwrap()
}

#[tokio::test]
async fn unsigned_request_does_not_enter_handler() {
    // W-1 fix: non-empty keyring so this path is *distinct* from the unknown-sender test.
    let sentinel = Arc::new(AtomicBool::new(false));
    let app = sentinel_router(alice_pinned_keyring(), sentinel.clone());
    let resp = oneshot(
        app,
        "/famp/v0.5.1/inbox/agent:local/bob",
        unsigned_bytes(),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "unsigned-with-pinned-sender must be bad_envelope, not unknown_sender"
    );
    assert!(
        !sentinel.load(Ordering::SeqCst),
        "TRANS-09 SC#2: handler must NOT be entered"
    );
}

#[tokio::test]
async fn body_over_1mb_does_not_enter_handler() {
    let sentinel = Arc::new(AtomicBool::new(false));
    let app = sentinel_router(empty_keyring(), sentinel.clone());
    let big = vec![b'x'; ONE_MIB + 1];
    let resp = oneshot(app, "/famp/v0.5.1/inbox/agent:local/bob", big).await;
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert!(!sentinel.load(Ordering::SeqCst));
}

#[tokio::test]
async fn unknown_sender_does_not_enter_handler() {
    // Empty keyring + an envelope claiming `from=agent:local/alice` → UnknownSender.
    let sentinel = Arc::new(AtomicBool::new(false));
    let app = sentinel_router(empty_keyring(), sentinel.clone());
    let resp = oneshot(
        app,
        "/famp/v0.5.1/inbox/agent:local/bob",
        unsigned_bytes(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(!sentinel.load(Ordering::SeqCst));
}

#[tokio::test]
async fn wrong_key_does_not_enter_handler() {
    // Alice pinned to her REAL key; we post bytes signed by WRONG_SECRET
    // claiming from=alice → SignatureInvalid.
    let sentinel = Arc::new(AtomicBool::new(false));
    let app = sentinel_router(alice_pinned_keyring(), sentinel.clone());
    let resp = oneshot(
        app,
        "/famp/v0.5.1/inbox/agent:local/bob",
        wrong_key_bytes(),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(!sentinel.load(Ordering::SeqCst));
}

//! Inbound HTTP listener: wire -> local bus (Phase 9 D-04/D-05).
//!
//! A gateway-owned axum router (NOT `famp_transport_http::build_router`,
//! whose own sig-verify middleware layer would introduce a second trust
//! source — 09-RESEARCH.md §3 Pitfall 1) that extracts the raw request
//! body, verifies it with [`crate::verify::verify_inbound_any`] against
//! the gateway's own peers keyring, and on success delivers the verified
//! envelope onto the local bus via the backed *sender* stand-in's
//! [`crate::ProxiedPrincipal::send_recv`] (D-05). A rejected envelope
//! produces zero bus writes and surfaces as an HTTP 4xx.
//!
//! `verify_inbound_any` is the ONLY signature check on this path — the
//! handler never mounts the transport's own sig-verify middleware and
//! never calls `build_router` (D-04). The registry lock is held only for
//! the single delivery `send_recv` call, mirroring 09-02 egress's
//! shared-connection contract so neither direction starves the other on
//! the same backed principal's UDS connection.

use std::str::FromStr;
use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::post,
    Router,
};
use famp::Principal;
use famp_bus::{BusMessage, BusReply, Target};
use famp_envelope::AnySignedEnvelope;
use famp_keyring::Keyring;
use famp_transport_http::INBOX_ROUTE;
use serde_json::Value;
use tokio::sync::Mutex;
use tower::ServiceBuilder;

use crate::error::RejectReason;
use crate::registry::GatewayRegistry;
use crate::verify::verify_inbound_any;

/// Body-size DoS mitigation (09-RESEARCH.md Security V/DoS; TRANS-07 §18).
/// Kept identical to `famp-transport-http::server`'s cap even though the
/// transport's own `build_router` is never called here.
const ONE_MIB: usize = 1_048_576;

/// Shared state for the gateway-owned inbox router.
#[derive(Clone)]
struct GatewayIngressState {
    registry: Arc<Mutex<GatewayRegistry>>,
    keyring: Arc<Keyring>,
}

/// Build the gateway-owned inbound router.
///
/// Reuses only [`famp_transport_http::INBOX_ROUTE`] (the route string)
/// and a body-limit layer from the preserved transport crate — NOT
/// `famp_transport_http::build_router`, which unconditionally mounts its
/// own sig-verify middleware against the *transport's* keyring (a
/// second, forbidden trust source per D-04). [`inbox_handler`] is the
/// sole verification site, calling [`verify_inbound_any`] against the
/// gateway's own pinned peers keyring.
pub fn build_gateway_router(
    registry: Arc<Mutex<GatewayRegistry>>,
    keyring: Arc<Keyring>,
) -> Router {
    let state = GatewayIngressState { registry, keyring };
    Router::new()
        .route(INBOX_ROUTE, post(inbox_handler))
        .with_state(state)
        .layer(ServiceBuilder::new().layer(tower_http::limit::RequestBodyLimitLayer::new(ONE_MIB)))
}

/// `envelope_sender` mirrors the 6-arm match in
/// `famp-transport-http/src/server.rs` (that fn is crate-private there,
/// so this is a deliberate, documented duplication — if the adapter
/// shape changes, update this too).
fn envelope_sender(env: &AnySignedEnvelope) -> &Principal {
    match env {
        AnySignedEnvelope::Request(e) => e.from_principal(),
        AnySignedEnvelope::Commit(e) => e.from_principal(),
        AnySignedEnvelope::Deliver(e) => e.from_principal(),
        AnySignedEnvelope::Ack(e) => e.from_principal(),
        AnySignedEnvelope::Control(e) => e.from_principal(),
        AnySignedEnvelope::AuditLog(e) => e.from_principal(),
    }
}

/// Rejections this ingress handler can produce. Deliberately distinct
/// from `famp-transport-http::MiddlewareError` (a different trust
/// boundary) even though the shapes rhyme — D-08's two-reason split
/// (`InvalidSignature` vs `UnpinnedKey`) must stay operator-visible at
/// the HTTP layer, not collapsed into one flat reject.
#[derive(Debug, thiserror::Error)]
enum IngressError {
    #[error("bad principal in inbox path")]
    BadPrincipal,
    #[error("invalid or missing signature")]
    InvalidSignature,
    #[error("sender principal '{principal}' has no pinned key")]
    UnpinnedKey { principal: Principal },
    #[error("verified sender is not backed by this gateway")]
    SenderNotBacked,
    #[error("internal error")]
    Internal,
}

impl IntoResponse for IngressError {
    fn into_response(self) -> Response {
        let (code, slug) = match &self {
            Self::BadPrincipal => (StatusCode::BAD_REQUEST, "bad_principal"),
            Self::InvalidSignature => (StatusCode::BAD_REQUEST, "invalid_signature"),
            Self::UnpinnedKey { .. } => (StatusCode::FORBIDDEN, "unpinned_key"),
            Self::SenderNotBacked => (StatusCode::BAD_GATEWAY, "sender_not_backed"),
            Self::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };
        let body = serde_json::json!({ "error": slug, "detail": self.to_string() });
        (code, Json(body)).into_response()
    }
}

/// `POST /famp/v0.5.1/inbox/{principal}` handler.
///
/// No `Extension<Arc<AnySignedEnvelope>>` parameter — nothing upstream
/// pre-verifies (that is the entire point of not mounting the
/// transport's own sig-verify middleware). [`verify_inbound_any`] runs
/// FIRST and is the only signature check on this path; any reject
/// returns before the registry is ever touched (zero bus writes, D-08).
async fn inbox_handler(
    Path(principal_str): Path<String>,
    State(state): State<GatewayIngressState>,
    body: Bytes,
) -> impl IntoResponse {
    let Ok(recipient) = Principal::from_str(&principal_str) else {
        return IngressError::BadPrincipal.into_response();
    };

    let envelope = match verify_inbound_any(&body, &state.keyring) {
        Ok(e) => e,
        Err(RejectReason::InvalidSignature) => {
            return IngressError::InvalidSignature.into_response()
        }
        Err(RejectReason::UnpinnedKey { principal }) => {
            return IngressError::UnpinnedKey { principal }.into_response()
        }
    };

    let sender = envelope_sender(&envelope).clone();

    // Content-transparent delivery: re-parse the already-verified,
    // already-strict-parsed bytes into a plain `Value` for `Send` — no
    // typed reconstruction, task_id/class/body stay byte-exact
    // (09-RESEARCH.md §5 Pitfall 3). This can only fail if the bytes
    // that just verified were somehow not valid JSON, which
    // `verify_inbound_any`'s `AnySignedEnvelope::decode` already ruled
    // out via `from_slice_strict` internally.
    let value: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("famp-gateway: ingress: verified bytes failed to re-parse as JSON: {e}");
            return IngressError::Internal.into_response();
        }
    };

    // Registry lock held ONLY for this single delivery `send_recv` —
    // acquired after verification succeeds, dropped immediately after,
    // mirroring 09-02 egress's short-hold shared-connection contract so
    // ingress and egress never starve each other on the same backed
    // principal's connection.
    let reply = {
        let mut guard = state.registry.lock().await;
        let Some(principal) = guard.get_mut(sender.name()) else {
            drop(guard);
            return IngressError::SenderNotBacked.into_response();
        };
        let reply = principal
            .send_recv(BusMessage::Send {
                to: Target::Agent {
                    name: recipient.name().to_string(),
                },
                envelope: value,
            })
            .await;
        drop(guard);
        reply
    };

    match reply {
        Ok(BusReply::SendOk { .. }) => StatusCode::ACCEPTED.into_response(),
        Ok(other) => {
            eprintln!("famp-gateway: ingress: unexpected Send reply: {other:?}");
            IngressError::Internal.into_response()
        }
        Err(e) => {
            eprintln!("famp-gateway: ingress: delivery failed: {e}");
            IngressError::Internal.into_response()
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use famp::{AuthorityScope, FampSigningKey, MessageId, Timestamp, UnsignedEnvelope};
    use famp_envelope::body::ack::{AckBody, AckDisposition};
    use famp_keyring::Keyring;
    use tower::ServiceExt;

    fn ack_bytes(sk: &FampSigningKey, from: &Principal, to: &Principal) -> Vec<u8> {
        let id: MessageId = "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a71".parse().unwrap();
        let ts = Timestamp("2026-07-27T00:00:00Z".to_string());
        let body = AckBody {
            disposition: AckDisposition::Accepted,
            reason: None,
        };
        let unsigned = UnsignedEnvelope::<AckBody>::new(
            id,
            from.clone(),
            to.clone(),
            AuthorityScope::Advisory,
            ts,
            body,
        );
        unsigned.sign(sk).unwrap().encode().unwrap()
    }

    fn router_with_keyring(keyring: Keyring) -> (Router, Arc<Mutex<GatewayRegistry>>) {
        let registry = Arc::new(Mutex::new(GatewayRegistry::default()));
        (
            build_gateway_router(registry.clone(), Arc::new(keyring)),
            registry,
        )
    }

    /// Percent-encode a `Principal`'s `to_string()` into a single URL path
    /// segment — mirrors exactly what `HttpTransport::send` does
    /// (`crates/famp-transport-http/src/transport.rs`'s
    /// `segs.push(&msg.recipient.to_string())`), so the axum `{principal}`
    /// route matches on one segment even though `Principal::to_string()`
    /// itself contains `:` and `/`.
    fn inbox_uri(recipient: &Principal) -> String {
        let mut url = url::Url::parse("http://gateway.test/").unwrap();
        {
            let mut segs = url.path_segments_mut().unwrap();
            segs.pop_if_empty();
            segs.extend(["famp", "v0.5.1", "inbox"]);
            segs.push(&recipient.to_string());
        }
        url.path().to_string()
    }

    async fn post_inbox_raw(router: Router, uri: &str, body: Vec<u8>) -> Response {
        let req = axum::http::Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/famp+json")
            .body(axum::body::Body::from(body))
            .unwrap();
        router.oneshot(req).await.unwrap()
    }

    async fn post_inbox(router: Router, recipient: &Principal, body: Vec<u8>) -> Response {
        post_inbox_raw(router, &inbox_uri(recipient), body).await
    }

    #[tokio::test]
    async fn invalid_signature_and_unpinned_key_map_to_distinct_4xx_with_no_registry_mutation() {
        let sk = FampSigningKey::from_bytes([30u8; 32]);
        let from: Principal = "agent:hosta.test/oscar".parse().unwrap();
        let to: Principal = "agent:hostb.test/peggy".parse().unwrap();
        let bytes = ack_bytes(&sk, &from, &to);

        // --- invalid signature: pinned to the WRONG key ---
        let wrong_sk = FampSigningKey::from_bytes([31u8; 32]);
        let mut keyring_wrong = Keyring::new();
        keyring_wrong
            .pin_tofu(from.clone(), wrong_sk.verifying_key())
            .unwrap();
        let (router, registry) = router_with_keyring(keyring_wrong);
        let resp = post_inbox(router, &to, bytes.clone()).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"], "invalid_signature");
        assert_eq!(
            registry.lock().await.names().count(),
            0,
            "reject path must perform zero registry mutation"
        );

        // --- unpinned key: sender absent from an empty keyring ---
        let (router2, registry2) = router_with_keyring(Keyring::new());
        let resp2 = post_inbox(router2, &to, bytes).await;
        assert_eq!(resp2.status(), StatusCode::FORBIDDEN);
        let body2 = to_bytes(resp2.into_body(), usize::MAX).await.unwrap();
        let v2: Value = serde_json::from_slice(&body2).unwrap();
        assert_eq!(v2["error"], "unpinned_key");
        assert_eq!(
            registry2.lock().await.names().count(),
            0,
            "reject path must perform zero registry mutation"
        );

        // Two distinct status codes, per D-08.
        assert_ne!(StatusCode::BAD_REQUEST, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn bad_principal_in_path_is_rejected_before_verification() {
        let (router, _registry) = router_with_keyring(Keyring::new());
        let resp =
            post_inbox_raw(router, "/famp/v0.5.1/inbox/not-a-principal", b"x".to_vec()).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}

//! axum 0.8 router + inbox handler for `famp-transport-http`.
//!
//! D-A1: ONE listener per process, path-multiplexed via `POST /famp/v0.5.1/inbox/{principal}`.
//! D-C1: tower layer order (outer -> inner): `RequestBodyLimitLayer` -> `FampSigVerifyLayer` -> handler.
//! D-C3: handler reads the `Arc<AnySignedEnvelope>` stashed by the middleware
//!       and populates `TransportMessage.sender` from `envelope_sender(&env)`.

use std::{collections::HashMap, str::FromStr, sync::Arc};

use axum::{
    body::{Body, Bytes},
    extract::{Extension, Path, State},
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use famp_core::Principal;
use famp_envelope::AnySignedEnvelope;
use famp_keyring::Keyring;
use famp_transport::TransportMessage;
use tokio::sync::{mpsc, Mutex};
use tower::ServiceBuilder;
use tower_http::limit::RequestBodyLimitLayer;

use crate::{error::MiddlewareError, middleware::FampSigVerifyLayer};

const ONE_MIB: usize = 1_048_576;
pub const INBOX_ROUTE: &str = "/famp/v0.5.1/inbox/{principal}";

pub type InboxRegistry = Mutex<HashMap<Principal, mpsc::Sender<TransportMessage>>>;

#[derive(Clone)]
pub struct ServerState {
    pub inboxes: Arc<InboxRegistry>,
}

pub fn build_router(keyring: Arc<Keyring>, inboxes: Arc<InboxRegistry>) -> Router {
    let state = ServerState { inboxes };
    Router::new()
        .route(INBOX_ROUTE, post(inbox_handler))
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                // OUTER — runs first (Pitfall 2). 1 MB cap per spec §18 (TRANS-07).
                .layer(RequestBodyLimitLayer::new(ONE_MIB))
                // Re-unify body type after RequestBodyLimitLayer wraps it in
                // Limited<Body>, so the next layer (FampSigVerify) can keep
                // its `Service<Request<Body>>` bound.
                .map_request(|req: Request<_>| req.map(Body::new))
                // INNER — runs second. Decode + verify; rejects before route dispatch.
                .layer(FampSigVerifyLayer::new(keyring)),
        )
}

/// `envelope_sender` 6-arm match inlined here because `famp-transport-http`
/// cannot depend on `crates/famp::runtime::adapter`. Mirrors the shape in
/// `crates/famp/src/runtime/adapter.rs` — if the adapter changes, update this.
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

async fn inbox_handler(
    Path(principal_str): Path<String>,
    State(state): State<ServerState>,
    Extension(envelope): Extension<Arc<AnySignedEnvelope>>,
    body: Bytes,
) -> Result<StatusCode, MiddlewareError> {
    let recipient =
        Principal::from_str(&principal_str).map_err(|_| MiddlewareError::BadPrincipal)?;

    // D-C3: sender comes from the stashed decoded envelope, NEVER from
    // `recipient.clone()`. Setting sender = recipient would break the runtime
    // cross-check and CONF-04 happy path.
    let sender = envelope_sender(&envelope).clone();

    let inboxes_guard = state.inboxes.lock().await;
    let tx = inboxes_guard
        .get(&recipient)
        .ok_or(MiddlewareError::UnknownRecipient)?
        .clone();
    drop(inboxes_guard);

    let recipient_for_log = recipient.clone();
    let sender_for_log = sender.clone();
    tx.send(TransportMessage {
        sender,
        recipient,
        bytes: body.to_vec(),
    })
    .await
    .map_err(|e| {
        // LOW-04: surface diagnostics on inbox send failure. The only
        // failure mode for mpsc::Sender::send on a bounded channel is
        // channel-closed (receiver dropped), which collapses to a 500
        // without context. Tracing is not yet wired in Phase 4, so log
        // via eprintln! per the review guidance; upgrade to
        // tracing::error! when the tracing layer lands.
        eprintln!(
            "famp-transport-http: inbox send failed (sender={sender_for_log}, \
             recipient={recipient_for_log}): {e}"
        );
        MiddlewareError::Internal
    })?;

    Ok(StatusCode::ACCEPTED)
}

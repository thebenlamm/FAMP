//! Custom axum router for `famp listen`.
//!
//! REUSES `famp_transport_http::FampSigVerifyLayer` unmodified (D-C1 middleware
//! order is byte-identical to `famp_transport_http::server::build_router`: outer
//! `RequestBodyLimitLayer(1 MiB)` → `map_request(|r| r.map(Body::new))` → inner
//! `FampSigVerifyLayer`). The ONLY difference from the upstream router is the
//! handler: instead of fanning out to an in-memory `InboxRegistry` and returning
//! 202 ACCEPTED, this handler calls `inbox.append(&body).await` and returns
//! **200 OK** — stricter than 202 because the 200 is a durability receipt
//! (fsync happened before we return).
//!
//! ## Phase 4 auto-commit wiring
//!
//! After the inbox append (durability receipt is 200 — MUST happen first),
//! the handler inspects the stashed `Extension<Arc<AnySignedEnvelope>>`. If
//! `class == Request`, it spawns a detached tokio task calling
//! `auto_commit::spawn_reply`. The 200 response semantics are preserved: the
//! client always sees 200 before any outbound commit POST begins.
//!
//! `AutoCommitCtx` is injected via router state alongside the inbox.

use std::sync::Arc;

use axum::{
    body::{Body, Bytes},
    extract::{Extension, State},
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use famp_envelope::{AnySignedEnvelope, MessageClass};
use famp_keyring::Keyring;
use famp_transport_http::FampSigVerifyLayer;
use tower::ServiceBuilder;
use tower_http::limit::RequestBodyLimitLayer;

use super::auto_commit::{spawn_reply, AutoCommitCtx};

const ONE_MIB: usize = 1_048_576;
const INBOX_ROUTE: &str = "/famp/v0.5.1/inbox/{principal}";

/// Combined router state — inbox + auto-commit context.
#[derive(Clone)]
struct ListenState {
    inbox: Arc<famp_inbox::Inbox>,
    auto_commit_ctx: Arc<AutoCommitCtx>,
}

/// Build the axum router for `famp listen`.
///
/// Phase 4: takes an `AutoCommitCtx` in addition to the inbox so the handler
/// can dispatch commit replies on inbound request envelopes.
pub fn build_listen_router(
    keyring: Arc<Keyring>,
    inbox: Arc<famp_inbox::Inbox>,
    auto_commit_ctx: Arc<AutoCommitCtx>,
) -> Router {
    let state = ListenState {
        inbox,
        auto_commit_ctx,
    };
    Router::new()
        .route(INBOX_ROUTE, post(inbox_append_handler))
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                // OUTER — 1 MiB body cap per TRANS-07. Runs first.
                .layer(RequestBodyLimitLayer::new(ONE_MIB))
                // Re-unify body type after RequestBodyLimitLayer wraps it in
                // Limited<Body>, so the next layer (FampSigVerify) keeps its
                // `Service<Request<Body>>` bound.
                .map_request(|req: Request<_>| req.map(Body::new))
                // INNER — sig verification. Rejects before the handler runs.
                .layer(FampSigVerifyLayer::new(keyring)),
        )
}

/// Handler: append the raw (already-verified) body bytes to the inbox,
/// return 200 OK on durable commit, THEN spawn an auto-commit reply if
/// the envelope class is Request.
///
/// Ordering guarantee: inbox append (fsync → 200) happens BEFORE the
/// auto-commit task is spawned. The 200 durability receipt semantics
/// (Plan 02-02 contract) are fully preserved.
async fn inbox_append_handler(
    State(state): State<ListenState>,
    Extension(envelope): Extension<Arc<AnySignedEnvelope>>,
    body: Bytes,
) -> Result<StatusCode, StatusCode> {
    // Step 1: durable append — fsync before 200.
    match state.inbox.append(&body).await {
        Ok(()) => {}
        Err(e) => {
            eprintln!("famp listen: inbox append failed: {e}");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    // Step 2: auto-commit dispatch (fire-and-forget, non-blocking).
    // Only for Request-class envelopes; other classes are stored but not replied to.
    if envelope.class() == MessageClass::Request {
        spawn_reply(state.auto_commit_ctx.clone(), envelope);
    }

    Ok(StatusCode::OK)
}

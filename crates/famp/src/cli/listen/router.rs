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
//! The `Extension<Arc<AnySignedEnvelope>>` extractor is pulled even though we
//! do not use its contents: it is a compile-time proof that the upstream
//! sig-verify layer actually ran (the layer stashes the decoded envelope into
//! request extensions in Step 5 of middleware.rs). If anyone ever removes the
//! layer, this extractor will panic at runtime and the listen router will 500
//! every request — a loud failure, which is what we want.

use std::sync::Arc;

use axum::{
    body::{Body, Bytes},
    extract::{Extension, State},
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use famp_envelope::AnySignedEnvelope;
use famp_keyring::Keyring;
use famp_transport_http::FampSigVerifyLayer;
use tower::ServiceBuilder;
use tower_http::limit::RequestBodyLimitLayer;

const ONE_MIB: usize = 1_048_576;
const INBOX_ROUTE: &str = "/famp/v0.5.1/inbox/{principal}";

/// Build the axum router for `famp listen`.
///
/// State: `Arc<famp_inbox::Inbox>` injected via `with_state`. The inbox is
/// shared across all handler invocations — concurrent appends serialize on
/// the inbox's internal `tokio::sync::Mutex<File>` (Plan 02-01 contract).
pub fn build_listen_router(
    keyring: Arc<Keyring>,
    inbox: Arc<famp_inbox::Inbox>,
) -> Router {
    Router::new()
        .route(INBOX_ROUTE, post(inbox_append_handler))
        .with_state(inbox)
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

/// Handler: append the raw (already-verified) body bytes to the inbox and
/// return 200 OK on durable commit.
///
/// We append `body` (the raw wire bytes) rather than re-encoding the decoded
/// envelope. This preserves byte-exactness (P3 of famp-inbox's pitfall notes)
/// — the bytes signed on the wire are the bytes on disk, no canonicalization
/// round-trip drift possible.
async fn inbox_append_handler(
    State(inbox): State<Arc<famp_inbox::Inbox>>,
    Extension(_envelope): Extension<Arc<AnySignedEnvelope>>,
    body: Bytes,
) -> Result<StatusCode, StatusCode> {
    match inbox.append(&body).await {
        Ok(()) => Ok(StatusCode::OK),
        Err(e) => {
            // Tracing is deferred to Phase 4 per CONTEXT §Deferred — until
            // then, log errors with `eprintln!` so operators can diagnose
            // inbox failures from the daemon's stderr stream.
            eprintln!("famp listen: inbox append failed: {e}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

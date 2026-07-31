//! The relay's HTTP surface: an unauthenticated enqueue route (this
//! task) and an authenticated fetch route (Task 3).
//!
//! Mounted on `famp_transport_http::INBOX_ROUTE` VERBATIM (Task 1) so a
//! sending gateway pointed at this relay with `--peer <domain>=<relay-url>`
//! needs ZERO egress change, and on [`crate::fetch_auth::RELAY_FETCH_ROUTE`]
//! (Task 3).

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use famp_crypto::TrustedVerifyingKey;
use time::OffsetDateTime;
use tokio::sync::Mutex;
use tower::ServiceBuilder;

use crate::fetch_auth::FetchAuthState;
use crate::queue::{EnqueueOutcome, RelayQueues, RELAY_MAX_BODY_BYTES};

/// Shared relay state — one instance per process, cloned (cheaply, via
/// `Arc`) into every request.
#[derive(Clone)]
pub struct RelayState {
    queues: Arc<Mutex<RelayQueues>>,
    /// Task 2/3's signed-fetch replay-cache + rate-limiter state. Held
    /// here (not read by this task's own POST route) so both routes
    /// share one process-lifetime instance rather than two independently
    /// constructed ones.
    auth: Arc<Mutex<FetchAuthState>>,
    /// D-27: the ONE source of truth for which domains this relay
    /// serves and which public keys may drain each domain's queue —
    /// both routes consult it, and it is behind a shared IMMUTABLE
    /// handle (`Arc`, no interior mutability) so no request path can add
    /// a domain or a key at runtime. Given D-25, a first-come
    /// registration would let an attacker who claims a victim's domain
    /// READ their plaintext, not merely deny them service.
    domains: Arc<HashMap<String, Vec<TrustedVerifyingKey>>>,
    /// This relay's own normalized public URL (Task 3: every fetch
    /// signature is bound to this value as its audience).
    audience: Arc<str>,
}

impl RelayState {
    #[must_use]
    pub fn new(domains: HashMap<String, Vec<TrustedVerifyingKey>>, audience: Arc<str>) -> Self {
        Self {
            queues: Arc::new(Mutex::new(RelayQueues::new())),
            auth: Arc::new(Mutex::new(FetchAuthState::new())),
            domains: Arc::new(domains),
            audience,
        }
    }

    /// Shared handle for `main.rs`'s background sweep task.
    #[must_use]
    pub fn queues(&self) -> Arc<Mutex<RelayQueues>> {
        Arc::clone(&self.queues)
    }

    /// Shared handle for `main.rs`'s background sweep task (Task 3 wires
    /// its first real reader).
    #[must_use]
    pub fn auth(&self) -> Arc<Mutex<FetchAuthState>> {
        Arc::clone(&self.auth)
    }

    #[must_use]
    pub fn audience(&self) -> Arc<str> {
        Arc::clone(&self.audience)
    }
}

/// Extract the destination DOMAIN from a principal path segment
/// (`agent:<authority>/<name>`), without depending on `famp-core`'s
/// `Principal` type — this crate has no dependency on `famp-core`/`famp`
/// (see `lib.rs`'s module doc: only `famp-crypto` and
/// `famp-transport-http`). Parses only as far as the authority: strips
/// the `agent:` scheme, then everything up to the first `/` is the
/// domain. Returns `None` on anything else (missing scheme, missing
/// separator, empty domain), which the caller maps to a 400 — the
/// destination comes from the path rather than the body precisely so no
/// body parsing is ever needed on this route, and per D-25 this
/// opacity is about what this CODE does, not about what the operator can
/// see.
fn extract_domain(principal: &str) -> Option<&str> {
    let rest = principal.strip_prefix("agent:")?;
    let slash = rest.find('/')?;
    let domain = &rest[..slash];
    if domain.is_empty() {
        None
    } else {
        Some(domain)
    }
}

/// `POST famp_transport_http::INBOX_ROUTE` handler.
///
/// Deliberately UNAUTHENTICATED at the relay layer: a sending gateway's
/// egress needs zero code change to reach the relay
/// (`--peer <domain>=<relay-url>` already works), and the envelope
/// inside is signed end to end regardless — verifying it here would
/// duplicate, not add, trust, and adding authorization here was not
/// asked for and would reintroduce the shared-secret problem D-26
/// rejects.
///
/// Known, accepted residual (T-17-37): because this route is open,
/// anyone who knows this relay's URL and a served domain can flood a
/// queue to `RELAY_QUEUE_MAX_PER_DOMAIN` and evict legitimate entries —
/// and the evicted sender already received a 202 from an earlier POST.
/// This is named, not papered over; see `crate::queue::RelayQueues::enqueue`.
///
/// Never turns the body into any structured value: it is extracted as
/// raw `Bytes` and stored as-is. No extractor that would parse it is
/// ever used on this handler.
async fn enqueue_handler(
    Path(principal): Path<String>,
    State(state): State<RelayState>,
    body: Bytes,
) -> Response {
    let Some(domain) = extract_domain(&principal) else {
        eprintln!("famp-relay: enqueue: rejected — path is not a parseable principal");
        return StatusCode::BAD_REQUEST.into_response();
    };
    if !state.domains.contains_key(domain) {
        eprintln!("famp-relay: enqueue: rejected — domain '{domain}' not served by this relay");
        return StatusCode::NOT_FOUND.into_response();
    }

    let now = OffsetDateTime::now_utc();
    let mut queues = state.queues.lock().await;
    let outcome = queues.enqueue(domain, principal.clone(), body.to_vec(), now);
    let depth = queues.depth(domain);
    drop(queues);

    if outcome == EnqueueOutcome::QueuedAfterDroppingOldest {
        eprintln!(
            "famp-relay: enqueue: domain={domain} depth={depth} dropped oldest entry (queue at cap)"
        );
    } else {
        eprintln!("famp-relay: enqueue: domain={domain} depth={depth}");
    }

    StatusCode::ACCEPTED.into_response()
}

/// Build the relay's axum router. Task 1 wires the POST enqueue route
/// only; Task 3 adds the GET fetch route on
/// [`crate::fetch_auth::RELAY_FETCH_ROUTE`].
pub fn build_relay_router(state: RelayState) -> Router {
    Router::new()
        .route(famp_transport_http::INBOX_ROUTE, post(enqueue_handler))
        .with_state(state)
        .layer(
            ServiceBuilder::new().layer(tower_http::limit::RequestBodyLimitLayer::new(
                RELAY_MAX_BODY_BYTES,
            )),
        )
}

// A pure-string-parsing unit test for `extract_domain` — kept inline
// (unlike the route tests, which live in `tests/relay_routes.rs`)
// because it does no body parsing at all and so does not weaken the
// crate's opacity gate (`grep -c 'serde_json::from' src/*.rs` == 0).
#[cfg(test)]
mod tests {
    use super::extract_domain;

    #[test]
    fn extract_domain_parses_authority_only() {
        assert_eq!(extract_domain("agent:hosta.test/alice"), Some("hosta.test"));
        assert_eq!(extract_domain("not-a-principal"), None);
        assert_eq!(extract_domain("agent:no-slash-here"), None);
        assert_eq!(extract_domain("agent:/alice"), None);
    }
}

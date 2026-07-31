//! The relay's HTTP surface: an unauthenticated enqueue route (Task 1)
//! and an authenticated fetch route (this task).
//!
//! Mounted on `famp_transport_http::INBOX_ROUTE` VERBATIM (Task 1) so a
//! sending gateway pointed at this relay with `--peer <domain>=<relay-url>`
//! needs ZERO egress change, and on [`crate::fetch_auth::RELAY_FETCH_ROUTE`]
//! (this task).

use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use famp_crypto::TrustedVerifyingKey;
use serde_json::json;
use time::OffsetDateTime;
use tokio::sync::Mutex;
use tower::ServiceBuilder;

use crate::fetch_auth::{
    verify_fetch_auth, FetchAuthReject, FetchAuthState, PresentedFetchAuth, RELAY_AUTH_AUD_HEADER,
    RELAY_AUTH_NONCE_HEADER, RELAY_AUTH_SIG_HEADER, RELAY_AUTH_TS_HEADER, RELAY_FETCH_ROUTE,
};
use crate::queue::{EnqueueOutcome, RelayQueues, RELAY_FETCH_MAX_BATCH, RELAY_MAX_BODY_BYTES};

/// Shared relay state — one instance per process, cloned (cheaply, via
/// `Arc`) into every request.
#[derive(Clone)]
pub struct RelayState {
    queues: Arc<Mutex<RelayQueues>>,
    /// The signed-fetch replay-cache + rate-limiter state the fetch
    /// route reads. Shared here (not owned by the fetch route itself)
    /// so a background sweep task can reclaim expired entries even for
    /// a domain nobody fetches.
    auth: Arc<Mutex<FetchAuthState>>,
    /// D-27: the ONE source of truth for which domains this relay
    /// serves and which public keys may drain each domain's queue —
    /// both routes consult it, and it is behind a shared IMMUTABLE
    /// handle (`Arc`, no interior mutability) so no request path can add
    /// a domain or a key at runtime. Given D-25, a first-come
    /// registration would let an attacker who claims a victim's domain
    /// READ their plaintext, not merely deny them service.
    domains: Arc<HashMap<String, Vec<TrustedVerifyingKey>>>,
    /// This relay's own normalized public URL — every fetch signature is
    /// bound to this value as its audience (`normalize_audience`, called
    /// once at startup in `main.rs`).
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

    /// Shared handle for `main.rs`'s background sweep task.
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
/// only; this task adds the GET fetch route on
/// [`crate::fetch_auth::RELAY_FETCH_ROUTE`]. Both routes share the same
/// body-limit layer.
pub fn build_relay_router(state: RelayState) -> Router {
    Router::new()
        .route(famp_transport_http::INBOX_ROUTE, post(enqueue_handler))
        .route(RELAY_FETCH_ROUTE, get(fetch_handler))
        .with_state(state)
        .layer(
            ServiceBuilder::new().layer(tower_http::limit::RequestBodyLimitLayer::new(
                RELAY_MAX_BODY_BYTES,
            )),
        )
}

/// `GET famp_relay::fetch_auth::RELAY_FETCH_ROUTE` handler.
///
/// The order below is a contract; each step's position is load-bearing,
/// not incidental:
///
/// 1. Look up the path domain in `state.domains`; unknown returns 404
///    with slug `unknown_domain`, before any allocation and before any
///    cryptography. An unconfigured domain is the cheapest possible
///    reject and it is also the D-27 enforcement point: nothing here may
///    create an entry.
/// 2. Extract the four authorization headers; any missing one returns
///    400 with slug `missing_fetch_auth`.
/// 3. Take the [`FetchAuthState`] lock and call `admit` for this domain;
///    over the limit returns 429 with slug `fetch_rate_limited`. This
///    runs before verification so a flood cannot force unbounded
///    Ed25519 work, and it runs after the header check so a malformed
///    request is rejected for THAT reason rather than consuming a
///    domain's rate budget.
/// 4. Call [`verify_fetch_auth`] with the path domain, `state.audience`,
///    and the configured key list; each reject variant maps to its own
///    status and slug via [`fetch_reject_response`].
/// 5. Only after verification succeeds, call `record_nonce`; a replay
///    returns 403 with slug `replayed_fetch`. The cache write sits
///    behind proof of authorization precisely because this route is
///    reachable anonymously — see [`FetchAuthState::record_nonce`]'s doc
///    comment for the full divergence-from-the-gateway rationale.
/// 6. Drain up to [`RELAY_FETCH_MAX_BATCH`] and return a JSON object
///    with a single `envelopes` array whose elements each carry a `to`
///    string (the recipient principal verbatim from the original POST
///    path) and a `body` string (the opaque bytes, base64-encoded with
///    the strict unpadded URL-safe engine — specifically so this box
///    never has to interpret them as structured data).
async fn fetch_handler(
    Path(domain): Path<String>,
    State(state): State<RelayState>,
    headers: HeaderMap,
) -> Response {
    let Some(keys) = state.domains.get(&domain) else {
        eprintln!("famp-relay: fetch: rejected — unknown domain '{domain}'");
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "unknown_domain" })),
        )
            .into_response();
    };

    match ingest_fetch(&domain, keys, &headers, &state).await {
        Ok(response) => response,
        Err(reject) => {
            eprintln!("famp-relay: fetch: rejected — domain={domain} reject={reject:?}");
            fetch_reject_response(&reject)
        }
    }
}

/// The header-extraction-through-drain body of [`fetch_handler`],
/// factored out so the handler above stays a thin, orderable checklist.
async fn ingest_fetch(
    domain: &str,
    keys: &[TrustedVerifyingKey],
    headers: &HeaderMap,
    state: &RelayState,
) -> Result<Response, FetchAuthReject> {
    let aud = extract_header(headers, RELAY_AUTH_AUD_HEADER)?;
    let ts = extract_header(headers, RELAY_AUTH_TS_HEADER)?;
    let nonce = extract_header(headers, RELAY_AUTH_NONCE_HEADER)?;
    let signature_b64url = extract_header(headers, RELAY_AUTH_SIG_HEADER)?;

    let now = OffsetDateTime::now_utc();

    let mut auth = state.auth.lock().await;
    auth.admit(domain, now)?;
    drop(auth);

    let presented = PresentedFetchAuth {
        aud,
        ts,
        nonce,
        signature_b64url,
    };
    verify_fetch_auth(&presented, domain, &state.audience, keys, now)?;

    let mut auth = state.auth.lock().await;
    auth.record_nonce(domain, nonce, now)?;
    drop(auth);

    let mut queues = state.queues.lock().await;
    let drained = queues.drain(domain, now, RELAY_FETCH_MAX_BATCH);
    drop(queues);
    eprintln!(
        "famp-relay: fetch: domain={domain} drained={}",
        drained.len()
    );

    let envelopes: Vec<serde_json::Value> = drained
        .into_iter()
        .map(|entry| {
            json!({
                "to": entry.recipient,
                "body": URL_SAFE_NO_PAD.encode(entry.bytes),
            })
        })
        .collect();

    // QUAR-05 quarantine-surfaces allowlist justification (T-14-15
    // scope, this is a NEW `envelopes-field` record, not a false
    // positive to suppress): this JSON object is a machine-to-machine
    // WIRE response, consumed only by a peer gateway's fetch client —
    // it is never rendered to a human. The `body` values inside are
    // base64-opaque bytes this relay never parses (D-25/lib.rs), and
    // the actual human-facing quarantine boundary sits downstream, at
    // the RECEIVING gateway's own local-bus ingestion (Phase 14's
    // additive `famp-bus` `Register`-frame provenance tag), which this
    // relay's fetch response arrives strictly BEFORE. Nothing here
    // bypasses that boundary; it does not exist on this wire hop at
    // all.
    Ok(Json(json!({ "envelopes": envelopes })).into_response())
}

/// Extract a single header's value as `&str`. A missing header, or one
/// whose bytes are not valid header-string content, maps to
/// [`FetchAuthReject::MissingHeader`] — there is nothing usable to
/// present to [`verify_fetch_auth`] either way.
fn extract_header<'a>(
    headers: &'a HeaderMap,
    name: &'static str,
) -> Result<&'a str, FetchAuthReject> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .ok_or(FetchAuthReject::MissingHeader { header: name })
}

/// Map a [`FetchAuthReject`] onto its distinct status + JSON slug. Every
/// variant gets its own status and slug — none collapses into a shared
/// generic reject (T-17-35/T-17-36's mitigation depends on this being
/// diagnosable).
fn fetch_reject_response(reject: &FetchAuthReject) -> Response {
    let (status, slug) = match reject {
        FetchAuthReject::MissingHeader { .. } => (StatusCode::BAD_REQUEST, "missing_fetch_auth"),
        FetchAuthReject::HeaderTooLong { .. } => (StatusCode::BAD_REQUEST, "fetch_header_too_long"),
        FetchAuthReject::AudienceMismatch { .. } => {
            (StatusCode::BAD_REQUEST, "fetch_audience_mismatch")
        }
        FetchAuthReject::MalformedTimestamp => {
            (StatusCode::BAD_REQUEST, "fetch_malformed_timestamp")
        }
        FetchAuthReject::StaleFetch => (StatusCode::UNAUTHORIZED, "stale_fetch"),
        FetchAuthReject::MalformedSignature => {
            (StatusCode::BAD_REQUEST, "fetch_malformed_signature")
        }
        FetchAuthReject::BadSignature { .. } => (StatusCode::FORBIDDEN, "fetch_bad_signature"),
        FetchAuthReject::ReplayedNonce => (StatusCode::FORBIDDEN, "replayed_fetch"),
        FetchAuthReject::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "fetch_rate_limited"),
    };
    let body = json!({ "error": slug, "detail": reject.to_string() });
    (status, Json(body)).into_response()
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

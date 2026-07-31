//! Inbound HTTP listener: wire -> local bus (Phase 9 D-04/D-05; reordered
//! Phase 17 D-09/INGR-05).
//!
//! A gateway-owned axum router (NOT `famp_transport_http::build_router`,
//! whose own sig-verify middleware layer would introduce a second trust
//! source — 09-RESEARCH.md §3 Pitfall 1) that extracts the raw request
//! body, runs it through [`crate::ingress_guard`]'s cheap pre-verify gate
//! chain, verifies it with [`crate::verify::verify_inbound_any`] against
//! the gateway's own peers keyring, and on success delivers the verified
//! envelope onto the local bus via the backed *sender* stand-in's
//! [`crate::ProxiedPrincipal::send_recv`] (D-05). A rejected envelope
//! produces zero bus writes and surfaces as an HTTP 4xx.
//!
//! `verify_inbound_any` remains the ONLY signature check on this path —
//! the handler never mounts the transport's own sig-verify middleware and
//! never calls `build_router` (D-04) — but as of Phase 17 (INGR-05) it no
//! longer runs first: [`ingest_inbound`] runs the cheap guard chain
//! ([`crate::ingress_guard::run_cheap_gates`]) BEFORE `verify_inbound_any`,
//! and `verify_inbound_any` itself still runs BEFORE any registry
//! mutation. A cheap-to-send request can therefore never force an
//! expensive Ed25519 verify (D-09) — see
//! `cheap_checks_reject_before_signature_verify_ever_runs` in this
//! module's test suite for the pinned proof. The registry lock is held
//! only for the single delivery `send_recv` call, mirroring 09-02
//! egress's shared-connection contract so neither direction starves the
//! other on the same backed principal's UDS connection.

use std::net::SocketAddr;
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
use crate::ingress_guard::{self, GuardInput, GuardReject, IngressGuard};
use crate::registry::GatewayRegistry;
use crate::verify::verify_inbound_any;

/// Body-size DoS mitigation (09-RESEARCH.md Security V/DoS; TRANS-07 §18).
/// Kept identical to `famp-transport-http::server`'s cap even though the
/// transport's own `build_router` is never called here.
const ONE_MIB: usize = 1_048_576;

/// Shared state for the gateway-owned inbox router.
///
/// `pub(crate)` (not private): Task 1 of Phase 17 plan 01 extracts
/// [`ingest_inbound`] as `pub(crate)` so plan 05's relay-fetch loop can
/// call it directly, and that function's signature names this type —
/// callers in other modules of this crate must be able to name it too.
/// Never widened to `pub`: this stays an internal wiring detail of
/// `famp-gateway`, not part of any published API.
#[derive(Clone)]
pub(crate) struct GatewayIngressState {
    registry: Arc<Mutex<GatewayRegistry>>,
    keyring: Arc<Keyring>,
    /// T-11-23/T-11-24: this host's own-domain federation authority
    /// (plan 02's single source, resolved ONCE in `main.rs` and reused —
    /// see `main.rs::resolve_own_domain_or_exit`/`own_domain`). `Some(d)`
    /// means ingress rejects any verified envelope whose `to.authority()`
    /// != `d`; `None` means that check is skipped (T-11-29, mirroring
    /// 11-07's enforced-when-configured egress posture).
    own_domain: Option<Arc<str>>,
    /// Task 1 (INGR-01/INGR-05): process-lifetime pre-verify guard state,
    /// shared across every inbound request this router serves.
    /// `started_at` (see [`IngressGuard`]) anchors plan 02's
    /// restart-reopened replay-window documentation (INGR-03). The mutex
    /// is held only for the single [`crate::ingress_guard::run_cheap_gates`]
    /// call per request, mirroring the registry lock's short-hold
    /// contract below.
    guard: Arc<Mutex<IngressGuard>>,
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
    own_domain: Option<Arc<str>>,
) -> Router {
    let state = GatewayIngressState {
        registry,
        keyring,
        own_domain,
        guard: Arc::new(Mutex::new(IngressGuard::new())),
    };
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

/// F-1/T-11-23: the verified envelope's OWN `to` principal — read from the
/// signed content, never the URL path. Same 6-arm duplication rationale as
/// `envelope_sender` above.
fn envelope_recipient(env: &AnySignedEnvelope) -> &Principal {
    match env {
        AnySignedEnvelope::Request(e) => e.to_principal(),
        AnySignedEnvelope::Commit(e) => e.to_principal(),
        AnySignedEnvelope::Deliver(e) => e.to_principal(),
        AnySignedEnvelope::Ack(e) => e.to_principal(),
        AnySignedEnvelope::Control(e) => e.to_principal(),
        AnySignedEnvelope::AuditLog(e) => e.to_principal(),
    }
}

/// F-1/T-11-26: `SignedEnvelope::federation_format_ok` has no dispatch
/// helper on `AnySignedEnvelope` itself — same 6-arm duplication rationale
/// as `envelope_sender` above.
fn envelope_federation_format_ok(env: &AnySignedEnvelope) -> bool {
    match env {
        AnySignedEnvelope::Request(e) => e.federation_format_ok(),
        AnySignedEnvelope::Commit(e) => e.federation_format_ok(),
        AnySignedEnvelope::Deliver(e) => e.federation_format_ok(),
        AnySignedEnvelope::Ack(e) => e.federation_format_ok(),
        AnySignedEnvelope::Control(e) => e.federation_format_ok(),
        AnySignedEnvelope::AuditLog(e) => e.federation_format_ok(),
    }
}

/// Field names `sign_federation_fields` (09-02 egress) adds to a plain
/// drained local-bus `Value` on its way out: the federation wrapper
/// (`from_domain`/`to_domain`/`sender_key_id`/`nonce`/`expiry`,
/// `capability`/`approval` reserved v2.0+ real-estate per
/// `WireEnvelope<B>`) plus the Ed25519 `signature` itself. None of these
/// are local-bus-legal (BUS-11) — remove them all before any local
/// `Send`.
const RELAY_WRAPPER_FIELDS: [&str; 8] = [
    "signature",
    "from_domain",
    "to_domain",
    "sender_key_id",
    "nonce",
    "expiry",
    "capability",
    "approval",
];

/// Strip the outer federation/signature wrapper fields before an
/// ingress-verified envelope is delivered onto the local bus. See the
/// call site's comment for the BUS-11 rationale. A no-op on any field
/// that is absent — most inbound envelopes will not carry the two
/// reserved-but-unused `capability`/`approval` keys at all.
fn strip_relay_fields(value: &mut Value) {
    if let Some(obj) = value.as_object_mut() {
        for key in RELAY_WRAPPER_FIELDS {
            obj.remove(key);
        }
    }
}

/// Rejections this ingress handler can produce. Deliberately distinct
/// from `famp-transport-http::MiddlewareError` (a different trust
/// boundary) even though the shapes rhyme — D-08's two-reason split
/// (`InvalidSignature` vs `UnpinnedKey`) must stay operator-visible at
/// the HTTP layer, not collapsed into one flat reject.
#[derive(Debug, thiserror::Error)]
pub(crate) enum IngressError {
    #[error("bad principal in inbox path")]
    BadPrincipal,
    #[error("invalid or missing signature")]
    InvalidSignature,
    #[error("sender principal '{principal}' has no pinned key")]
    UnpinnedKey { principal: Principal },
    /// REVK-01: the sender's pinned key verified cryptographically, but
    /// its validity window has closed as of this gateway's own wall
    /// clock — enforced regardless of whether any revocation record was
    /// ever received.
    #[error("sender principal '{principal}' key expired at {valid_until}")]
    ExpiredKey {
        principal: Principal,
        valid_until: String,
    },
    /// REVK-02: the sender's pinned key has been explicitly revoked.
    #[error("sender principal '{principal}' key revoked")]
    RevokedKey { principal: Principal },
    #[error("verified sender is not backed by this gateway")]
    SenderNotBacked,
    /// T-11-23: the envelope's signed `to` does not match the URL-path
    /// recipient — the sender addressed someone else and the path must
    /// not override the signature. Distinct from `ForeignDomain` (below)
    /// so an operator can tell "misrouted to the wrong mailbox on this
    /// gateway" apart from "this gateway doesn't own that domain at all".
    #[error("envelope 'to' ({to}) does not match the requested recipient ({recipient})")]
    MisaddressedRecipient { to: Principal, recipient: Principal },
    /// T-11-24: this gateway is not authoritative for `to`'s domain. Only
    /// produced when own-domain IS configured (T-11-29: unset skips this
    /// check with a one-line warning, mirroring 11-07's egress posture).
    #[error("this gateway is not authoritative for domain '{got}' (configured own-domain: '{expected}')")]
    ForeignDomain { expected: String, got: String },
    /// T-11-26: `federation_format_ok()` returned false — malformed
    /// inbound `nonce`/`expiry`.
    #[error("envelope failed federation format validation (nonce/expiry)")]
    MalformedFederationFields,
    /// Task 1 (INGR-01/INGR-05, D-05/D-09): the pre-verify raw peek of
    /// `from`/`to`/`ts` failed — malformed JSON, a duplicate object key,
    /// or a missing/non-string `from`/`to`/`ts` field. Produced BEFORE
    /// `verify_inbound_any` ever runs; never implies anything about
    /// whether a valid signature is present.
    #[error(
        "envelope failed pre-verify shape check (malformed JSON, duplicate key, or missing/invalid from/to/ts)"
    )]
    BadEnvelopeShape,
    /// Task 1 (INGR-01/INGR-05, D-05/D-09): the envelope's `ts` differs
    /// from this gateway's own wall clock by more than `skew_secs`
    /// seconds, in either direction. Produced BEFORE `verify_inbound_any`
    /// runs — a stale-but-validly-signed envelope is rejected here,
    /// never reaching signature verification. Carries only the two
    /// timestamps and the window size — never key bytes or signature
    /// material.
    #[error(
        "envelope timestamp {ts} is outside this gateway's {skew_secs}s clock-skew window of its own now ({now})"
    )]
    StaleTimestamp {
        ts: String,
        now: String,
        skew_secs: i64,
    },
    #[error("internal error")]
    Internal,
}

impl IntoResponse for IngressError {
    fn into_response(self) -> Response {
        let (code, slug) = match &self {
            Self::BadPrincipal => (StatusCode::BAD_REQUEST, "bad_principal"),
            Self::InvalidSignature => (StatusCode::BAD_REQUEST, "invalid_signature"),
            Self::UnpinnedKey { .. } => (StatusCode::FORBIDDEN, "unpinned_key"),
            Self::ExpiredKey { .. } => (StatusCode::FORBIDDEN, "expired_key"),
            Self::RevokedKey { .. } => (StatusCode::FORBIDDEN, "revoked_key"),
            Self::SenderNotBacked => (StatusCode::BAD_GATEWAY, "sender_not_backed"),
            Self::MisaddressedRecipient { .. } => {
                (StatusCode::BAD_REQUEST, "misaddressed_recipient")
            }
            Self::ForeignDomain { .. } => (StatusCode::FORBIDDEN, "foreign_domain"),
            Self::MalformedFederationFields => {
                (StatusCode::BAD_REQUEST, "malformed_federation_fields")
            }
            Self::BadEnvelopeShape => (StatusCode::BAD_REQUEST, "bad_envelope_shape"),
            Self::StaleTimestamp { .. } => (StatusCode::BAD_REQUEST, "stale_timestamp"),
            Self::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };
        let body = serde_json::json!({ "error": slug, "detail": self.to_string() });
        (code, Json(body)).into_response()
    }
}

/// Map a [`RejectReason`] onto its ingress-layer [`IngressError`]. Kept as
/// a standalone function so [`inbox_handler`] itself stays under the
/// workspace's `too_many_lines` lint budget — the two error enums are
/// deliberately distinct (see `IngressError`'s module doc) so this is a
/// pure translation, not a collapse.
fn ingress_error_for_reject(reason: RejectReason) -> IngressError {
    match reason {
        RejectReason::InvalidSignature => IngressError::InvalidSignature,
        RejectReason::UnpinnedKey { principal } => IngressError::UnpinnedKey { principal },
        RejectReason::ExpiredKey {
            principal,
            valid_until,
        } => IngressError::ExpiredKey {
            principal,
            valid_until,
        },
        RejectReason::RevokedKey { principal, .. } => IngressError::RevokedKey { principal },
    }
}

/// Map a [`GuardReject`] onto its ingress-layer [`IngressError`]. Kept as
/// a standalone function, mirroring [`ingress_error_for_reject`], so
/// [`ingest_inbound`] stays under the workspace `too_many_lines` lint
/// budget — the two reject families (pre-verify guard vs. post-verify
/// trust) stay visibly distinct call sites rather than a collapsed
/// generic reject. Pure translation, no logging: callers log the
/// concrete reject values at the call site (matching the existing
/// `MisaddressedRecipient`/`ForeignDomain` convention).
fn ingress_error_for_guard(reject: GuardReject) -> IngressError {
    match reject {
        GuardReject::BadEnvelopeShape => IngressError::BadEnvelopeShape,
        GuardReject::StaleTimestamp { ts, now, skew_secs } => {
            IngressError::StaleTimestamp { ts, now, skew_secs }
        }
    }
}

/// `POST /famp/v0.5.1/inbox/{principal}` handler.
///
/// Thin per Task 1's extraction: parses the path principal, then delegates
/// the entire ingest to [`ingest_inbound`] (THE single ingest core — plan
/// 05's relay-fetch loop calls this same function, so no ingest path can
/// bypass the guard chain). No `Extension<Arc<AnySignedEnvelope>>`
/// parameter — nothing upstream pre-verifies (that is the entire point of
/// not mounting the transport's own sig-verify middleware).
async fn inbox_handler(
    Path(principal_str): Path<String>,
    State(state): State<GatewayIngressState>,
    body: Bytes,
) -> impl IntoResponse {
    let Ok(recipient) = Principal::from_str(&principal_str) else {
        return IngressError::BadPrincipal.into_response();
    };

    match ingest_inbound(&recipient, &body, &state).await {
        Ok(()) => StatusCode::ACCEPTED.into_response(),
        Err(e) => e.into_response(),
    }
}

/// THE single ingest core (Task 1, INGR-01/INGR-05): wire bytes in,
/// verified-and-delivered (or a distinct reject) out. `inbox_handler`
/// above is the only caller today; plan 05's relay-fetch loop is
/// documented (17-01-PLAN.md) to become the second, so no ingest path can
/// bypass this function's guard chain.
///
/// Order (D-09, pinned by `cheap_checks_reject_before_signature_verify_ever_runs`):
/// (1) peek unverified fields, (2) run the cheap pre-verify guard chain
/// ([`crate::ingress_guard::run_cheap_gates`]), (3) ONLY THEN
/// `verify_inbound_any` (the expensive Ed25519 verify), (4) the existing
/// audience/format checks, (5) delivery. The registry lock for
/// `sender_is_backed` (step 1's forward-shaped context) is taken and
/// dropped immediately — no mutation, matching the module's short-hold
/// contract.
pub(crate) async fn ingest_inbound(
    recipient: &Principal,
    body: &[u8],
    state: &GatewayIngressState,
) -> Result<(), IngressError> {
    let now = crate::clock::now_canonical_utc();

    let peeked = ingress_guard::peek_guard_fields(body).map_err(|reject| {
        eprintln!(
            "famp-gateway: ingress: rejected — pre-verify peek failed (malformed envelope shape)"
        );
        ingress_error_for_guard(reject)
    })?;

    // Short-hold: acquire, read, drop — no mutation, mirroring the
    // delivery lock's contract below.
    let sender_is_backed = {
        let registry = state.registry.lock().await;
        registry.get(peeked.from.name()).is_some()
    };

    let guard_input = GuardInput {
        peeked: &peeked,
        now: now.as_str(),
        own_domain: state.own_domain.as_deref(),
        sender_is_backed,
    };
    {
        let mut guard = state.guard.lock().await;
        ingress_guard::run_cheap_gates(&guard_input, &mut guard).map_err(|reject| {
            if let GuardReject::StaleTimestamp {
                ts,
                now: guard_now,
                skew_secs,
            } = &reject
            {
                eprintln!(
                    "famp-gateway: ingress: rejected — stale timestamp ts={ts} now={guard_now} \
                     skew_secs={skew_secs}"
                );
            }
            ingress_error_for_guard(reject)
        })?;
    }

    // EXPENSIVE: only reached once every cheap gate above has passed
    // (D-09/INGR-05).
    let envelope =
        verify_inbound_any(body, &state.keyring, &now).map_err(ingress_error_for_reject)?;

    check_audience_and_format(&envelope, recipient, state.own_domain.as_ref())?;

    let sender = envelope_sender(&envelope).clone();
    deliver(&sender, recipient, body, &state.registry).await
}

/// F-1 (T-11-23/T-11-24/T-11-26): the gateway is authoritative ONLY for
/// its own domain and the mailbox the sender actually signed for. Each
/// check gets its own distinct 4xx + log line (operator-facing security
/// boundary; a single generic 400 is not enough to debug a misrouted
/// federation). Extracted from `inbox_handler` (Task 1) so
/// [`ingest_inbound`] stays under the workspace `too_many_lines` budget —
/// behaviorally identical to the pre-Phase-17 inline checks, just now
/// running AFTER `verify_inbound_any` (D-09 only reorders the *cheap*
/// checks ahead of the *expensive* verify; these three checks are
/// themselves cheap but need the verified envelope's own `to`, so they
/// stay here, after verify, exactly as before).
fn check_audience_and_format(
    envelope: &AnySignedEnvelope,
    recipient: &Principal,
    own_domain: Option<&Arc<str>>,
) -> Result<(), IngressError> {
    let envelope_to = envelope_recipient(envelope).clone();
    if envelope_to != *recipient {
        eprintln!(
            "famp-gateway: ingress: rejected — envelope 'to' ({envelope_to}) != URL-path \
             recipient ({recipient})"
        );
        return Err(IngressError::MisaddressedRecipient {
            to: envelope_to,
            recipient: recipient.clone(),
        });
    }
    if let Some(own_domain) = own_domain {
        let got = envelope_to.authority();
        if got != own_domain.as_ref() {
            eprintln!(
                "famp-gateway: ingress: rejected — envelope 'to' domain '{got}' != this \
                 gateway's own-domain '{own_domain}'"
            );
            return Err(IngressError::ForeignDomain {
                expected: own_domain.to_string(),
                got: got.to_string(),
            });
        }
    } else {
        eprintln!(
            "famp-gateway: ingress: own-domain unset; skipping to-authority check for '{envelope_to}' \
             (T-11-29 residual — accepted posture until own-domain is configured)"
        );
    }
    if !envelope_federation_format_ok(envelope) {
        let sender = envelope_sender(envelope);
        eprintln!("famp-gateway: ingress: rejected — federation_format_ok() failed for envelope from {sender}");
        return Err(IngressError::MalformedFederationFields);
    }
    Ok(())
}

/// Content-transparent delivery of an already-verified envelope onto the
/// local bus. Extracted from `inbox_handler` (Task 1) for the same
/// `too_many_lines` reason as [`check_audience_and_format`] — behavior is
/// byte-identical to the pre-Phase-17 inline delivery block.
async fn deliver(
    sender: &Principal,
    recipient: &Principal,
    body: &[u8],
    registry: &Arc<Mutex<GatewayRegistry>>,
) -> Result<(), IngressError> {
    // Content-transparent delivery: re-parse the already-verified,
    // already-strict-parsed bytes into a plain `Value` for `Send` — no
    // typed reconstruction, task_id/class/body stay byte-exact
    // (09-RESEARCH.md §5 Pitfall 3). This can only fail if the bytes
    // that just verified were somehow not valid JSON, which
    // `verify_inbound_any`'s `AnySignedEnvelope::decode` already ruled
    // out via `from_slice_strict` internally.
    let mut value: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("famp-gateway: ingress: verified bytes failed to re-parse as JSON: {e}");
            return Err(IngressError::Internal);
        }
    };
    // BUS-11 fix: the verified bytes still carry the outer federation
    // wrapper (`signature`, `from_domain`, `to_domain`, `sender_key_id`,
    // `nonce`, `expiry`) egress's `sign_federation_fields` (09-02) added
    // on the way out. Content-transparency (D-03) covers `task_id` /
    // `class` / `body` — never touched, above or below — but these
    // relay-internal wrapper fields are NOT part of that content and
    // MUST be stripped before the onward local `Send`: `BusEnvelope::
    // decode` hard-rejects any local-bus line carrying a `signature`
    // key (BUS-11, famp-envelope/src/bus.rs), so forwarding them
    // verbatim would silently make every cross-host-relayed envelope
    // permanently undecodable by `famp inbox`/`famp inspect` on the
    // receiving side once it lands on the local bus.
    strip_relay_fields(&mut value);

    // Registry lock held ONLY for this single delivery `send_recv` —
    // acquired after verification succeeds, dropped immediately after,
    // mirroring 09-02 egress's short-hold shared-connection contract so
    // ingress and egress never starve each other on the same backed
    // principal's connection.
    let reply = {
        let mut guard = registry.lock().await;
        let Some(principal) = guard.get_mut(sender.name()) else {
            drop(guard);
            return Err(IngressError::SenderNotBacked);
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
        Ok(BusReply::SendOk { .. }) => Ok(()),
        Ok(other) => {
            eprintln!(
                "famp-gateway: ingress: unexpected Send reply: {}",
                other.variant_name()
            );
            Err(IngressError::Internal)
        }
        Err(e) => {
            eprintln!("famp-gateway: ingress: delivery failed: {e}");
            Err(IngressError::Internal)
        }
    }
}

/// Bind a rustls-terminated inbox listener on `listen_addr` and serve the
/// gateway-owned router (see [`build_gateway_router`]) until the listener
/// task exits.
///
/// TLS here is channel encryption only, not a peer-authorization boundary
/// (D-08, 09-RESEARCH.md §7 Pattern B): [`inbox_handler`]'s
/// `verify_inbound_any` call remains the sole trust decision. This function
/// deliberately adds no cert-based peer authorization on top of rustls'
/// standard server handshake.
///
/// Binds a std `TcpListener` first (so the caller could read `local_addr()`
/// for ephemeral-port scenarios, mirroring the deferred v1 e2e fixture
/// pattern), sets it non-blocking, then hands it to
/// [`famp_transport_http::tls_server::serve_std_listener`]. Intended to be
/// raced inside `main.rs`'s `tokio::select!` alongside the egress drain loop
/// and shutdown signal — this future does not resolve until the underlying
/// server task exits (bind error, accept-loop error, or panic).
pub async fn run_ingress(
    listen_addr: SocketAddr,
    tls_cert_path: &std::path::Path,
    tls_key_path: &std::path::Path,
    registry: Arc<Mutex<GatewayRegistry>>,
    keyring: Arc<Keyring>,
    own_domain: Option<Arc<str>>,
) -> std::io::Result<()> {
    let cert =
        famp_transport_http::tls::load_pem_cert(tls_cert_path).map_err(std::io::Error::other)?;
    let key =
        famp_transport_http::tls::load_pem_key(tls_key_path).map_err(std::io::Error::other)?;
    let server_config =
        famp_transport_http::tls::build_server_config(cert, key).map_err(std::io::Error::other)?;

    let listener = std::net::TcpListener::bind(listen_addr)?;
    listener.set_nonblocking(true)?;

    let router = build_gateway_router(registry, keyring, own_domain);
    let handle = famp_transport_http::tls_server::serve_std_listener(
        listener,
        router,
        Arc::new(server_config),
    );

    match handle.await {
        Ok(serve_result) => serve_result,
        Err(join_err) => Err(std::io::Error::other(join_err)),
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

    /// Task 1 (INGR-01) deviation: prior to this plan, every helper in
    /// this module hardcoded a literal past `ts` ("2026-07-27T00:00:00Z")
    /// because nothing enforced freshness. Now that `ingest_inbound` runs
    /// [`crate::ingress_guard::freshness_check`] before signature
    /// verification, a stale hardcoded `ts` would make every pre-existing
    /// reject-path test in this module fail with `stale_timestamp`
    /// instead of the reject reason it's actually testing. `ack_bytes`
    /// therefore stamps a live `ts` via `crate::clock::now_canonical_utc()`
    /// on every call; tests that need a deliberately stale envelope use
    /// [`ack_bytes_with_ts`] directly.
    fn ack_bytes(sk: &FampSigningKey, from: &Principal, to: &Principal) -> Vec<u8> {
        ack_bytes_with_ts(sk, from, to, Timestamp(crate::clock::now_canonical_utc()))
    }

    fn ack_bytes_with_ts(
        sk: &FampSigningKey,
        from: &Principal,
        to: &Principal,
        ts: Timestamp,
    ) -> Vec<u8> {
        let id: MessageId = "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a71".parse().unwrap();
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
        router_with_keyring_and_domain(keyring, None)
    }

    fn router_with_keyring_and_domain(
        keyring: Keyring,
        own_domain: Option<&str>,
    ) -> (Router, Arc<Mutex<GatewayRegistry>>) {
        let registry = Arc::new(Mutex::new(GatewayRegistry::default()));
        (
            build_gateway_router(
                registry.clone(),
                Arc::new(keyring),
                own_domain.map(Arc::from),
            ),
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

    /// Task 1 (INGR-01/INGR-05, D-05/D-09): an otherwise-valid,
    /// correctly-signed envelope (the sender's key is pinned to the SAME
    /// key that signed — verification WOULD succeed if it were ever
    /// reached) with a one-hour-old `ts` is rejected `stale_timestamp`
    /// with zero registry mutation. Proves the freshness gate is real,
    /// not a side effect of signature failure.
    #[tokio::test]
    async fn stale_timestamp_rejected_before_verify_with_no_registry_mutation() {
        let sk = FampSigningKey::from_bytes([40u8; 32]);
        let from: Principal = "agent:hosta.test/oscar".parse().unwrap();
        let to: Principal = "agent:hostb.test/peggy".parse().unwrap();

        let one_hour_ago = time::OffsetDateTime::now_utc() - time::Duration::hours(1);
        let stale_ts = crate::clock::strip_subseconds(
            &one_hour_ago
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap(),
        );
        let bytes = ack_bytes_with_ts(&sk, &from, &to, Timestamp(stale_ts));

        let mut keyring = Keyring::new();
        keyring.pin_tofu(from.clone(), sk.verifying_key()).unwrap();
        let (router, registry) = router_with_keyring(keyring);

        let resp = post_inbox(router, &to, bytes).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"], "stale_timestamp");
        assert_eq!(
            registry.lock().await.names().count(),
            0,
            "reject path must perform zero registry mutation"
        );
    }

    /// Task 1 (INGR-01/INGR-02 boundary probe): exactly
    /// `CLOCK_SKEW_WINDOW_SECS` from `now`, in either direction, must NOT
    /// be classified `stale_timestamp` (inclusive bound); one second
    /// beyond, in either direction, MUST be. The in-window cases assert
    /// the response is anything OTHER than `stale_timestamp` rather than
    /// asserting full delivery success, because full delivery requires a
    /// live broker connection this unit test does not spin up — the
    /// sender is deliberately left unbacked, so an in-window envelope
    /// still gets rejected downstream (`sender_not_backed`), just not by
    /// the freshness gate. That is sufficient to prove the freshness gate
    /// itself passed the boundary case.
    #[tokio::test]
    async fn freshness_boundary_is_inclusive_in_both_directions() {
        let sk = FampSigningKey::from_bytes([41u8; 32]);
        let from: Principal = "agent:hosta.test/oscar".parse().unwrap();
        let to: Principal = "agent:hostb.test/peggy".parse().unwrap();
        let skew = crate::ingress_guard::CLOCK_SKEW_WINDOW_SECS;

        let mk_ts = |delta_secs: i64| -> Timestamp {
            let instant = time::OffsetDateTime::now_utc() + time::Duration::seconds(delta_secs);
            Timestamp(crate::clock::strip_subseconds(
                &instant
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap(),
            ))
        };

        for delta in [-skew, skew] {
            let bytes = ack_bytes_with_ts(&sk, &from, &to, mk_ts(delta));
            let mut keyring = Keyring::new();
            keyring.pin_tofu(from.clone(), sk.verifying_key()).unwrap();
            let (router, registry) = router_with_keyring(keyring);
            let resp = post_inbox(router, &to, bytes).await;
            let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
            let v: Value = serde_json::from_slice(&body).unwrap();
            assert_ne!(
                v["error"], "stale_timestamp",
                "delta {delta}s is within the inclusive skew window and must pass the freshness gate"
            );
            assert_eq!(
                registry.lock().await.names().count(),
                0,
                "no registry mutation expected (sender deliberately left unbacked)"
            );
        }

        for delta in [-(skew + 1), skew + 1] {
            let bytes = ack_bytes_with_ts(&sk, &from, &to, mk_ts(delta));
            let mut keyring = Keyring::new();
            keyring.pin_tofu(from.clone(), sk.verifying_key()).unwrap();
            let (router, registry) = router_with_keyring(keyring);
            let resp = post_inbox(router, &to, bytes).await;
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
            let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
            let v: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(
                v["error"], "stale_timestamp",
                "delta {delta}s is one second beyond the inclusive skew window"
            );
            assert_eq!(registry.lock().await.names().count(), 0);
        }
    }

    #[tokio::test]
    async fn bad_principal_in_path_is_rejected_before_verification() {
        let (router, _registry) = router_with_keyring(Keyring::new());
        let resp =
            post_inbox_raw(router, "/famp/v0.5.1/inbox/not-a-principal", b"x".to_vec()).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    /// BUS-11 regression lock: `strip_relay_fields` removes every field
    /// `sign_federation_fields` (egress.rs) adds — `signature` plus the
    /// full federation wrapper — while leaving `task_id`/`class`/`body`
    /// (content-transparency, D-03) byte-identical. A local-bus line
    /// still carrying `signature` is permanently undecodable
    /// (`BusEnvelope::decode` -> `EnvelopeDecodeError::UnexpectedSignature`,
    /// famp-envelope/src/bus.rs), so this is the exact shape ingress
    /// MUST hand to `BusMessage::Send`.
    #[test]
    fn strip_relay_fields_removes_wrapper_keeps_content() {
        let mut value = serde_json::json!({
            "famp": "0.5.2",
            "id": "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a71",
            "from": "agent:hosta.test/alice",
            "to": "agent:hostb.test/bob",
            "scope": "standalone",
            "class": "request",
            "authority": "advisory",
            "ts": "2026-07-27T00:00:00Z",
            "from_domain": "hosta.test",
            "to_domain": "hostb.test",
            "sender_key_id": "abc123",
            "nonce": "def456",
            "expiry": "2026-07-27T00:05:00Z",
            "signature": "some-signature-b64url",
            "body": {"scope": {}, "bounds": {"deadline": "2026-12-31T00:00:00Z", "budget": {"amount": "10", "unit": "usd"}}},
        });
        let before_body = value["body"].clone();
        let before_id = value["id"].clone();
        let before_class = value["class"].clone();

        strip_relay_fields(&mut value);

        for key in RELAY_WRAPPER_FIELDS {
            assert!(
                value.get(key).is_none(),
                "relay wrapper field '{key}' must be stripped, got {value}"
            );
        }
        assert_eq!(value["body"], before_body, "content-transparency: body");
        assert_eq!(value["id"], before_id, "content-transparency: id");
        assert_eq!(value["class"], before_class, "content-transparency: class");

        // The stripped Value must now be local-bus-legal per BUS-11.
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(
            famp_envelope::AnyBusEnvelope::decode(&bytes).is_ok(),
            "post-strip Value must decode as a local BusEnvelope"
        );
    }

    /// Absence of the reserved `capability`/`approval` keys (never
    /// populated this phase) must not make `strip_relay_fields` panic or
    /// otherwise misbehave — `Map::remove` on a missing key is already a
    /// no-op, this locks that expectation explicitly.
    #[test]
    fn strip_relay_fields_is_a_noop_on_already_plain_value() {
        let mut value = serde_json::json!({
            "famp": "0.5.2",
            "id": "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a71",
            "from": "agent:hosta.test/alice",
            "to": "agent:hostb.test/bob",
            "class": "ack",
            "body": {"disposition": "accepted"},
        });
        let before = value.clone();
        strip_relay_fields(&mut value);
        assert_eq!(value, before, "no wrapper fields present -> no-op");
    }
}

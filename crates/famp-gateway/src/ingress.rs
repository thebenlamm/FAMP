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
    /// T-11-23/T-11-24/17-03: this host's own-domain federation authority
    /// (plan 02's single source, resolved ONCE in `main.rs` and reused —
    /// see `main.rs::resolve_own_domain_or_exit`/`own_domain`). Mandatory
    /// and startup-fatal as of 17-03/D-30: ingress unconditionally rejects
    /// any verified envelope whose `to.authority()` != `own_domain`.
    own_domain: Arc<str>,
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
    own_domain: Arc<str>,
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
    /// Task 1 (17-02, INGR-02/D-06): a nonce previously recorded for the
    /// same peeked sender, within `ingress_guard::REPLAY_CACHE_TTL_SECS`
    /// seconds, was submitted again. Produced BEFORE `verify_inbound_any`
    /// runs, keyed on the peeked (not yet cryptographically confirmed)
    /// sender — a correctly-signed replay maps here too, by design.
    /// Carries the nonce (a public routing token, never a secret) and
    /// the peeked sender name; never key bytes or signature material.
    #[error("nonce '{nonce}' from sender '{principal}' was already seen within the replay window")]
    ReplayedNonce { principal: String, nonce: String },
    /// Task 1 (17-02, INGR-02): the peeked `nonce` field is absent or
    /// empty. Produced BEFORE `verify_inbound_any` runs; the replay
    /// cache is never asked to key on nothing. Carries only the peeked
    /// sender name.
    #[error("sender '{principal}' submitted an envelope with an absent or empty nonce")]
    MissingNonce { principal: String },
    /// Task 1 (17-03, INGR-06/D-10): `principal` has exceeded `limit`
    /// admitted requests within the current `window_secs`-second fixed
    /// window. Produced BEFORE `verify_inbound_any` runs (INGR-05), and
    /// last among the cheap gates — see
    /// `ingress_guard::run_cheap_gates`'s ordering comment. Carries the
    /// principal, limit, and window length so an operator can read the
    /// applied policy directly off the reject.
    #[error("sender '{principal}' exceeded {limit} requests per {window_secs}s")]
    RateLimited {
        principal: String,
        limit: u32,
        window_secs: i64,
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
            Self::ReplayedNonce { .. } => (StatusCode::BAD_REQUEST, "replayed_nonce"),
            Self::MissingNonce { .. } => (StatusCode::BAD_REQUEST, "missing_nonce"),
            Self::RateLimited { .. } => (StatusCode::TOO_MANY_REQUESTS, "rate_limited"),
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
        GuardReject::ReplayedNonce { principal, nonce } => {
            IngressError::ReplayedNonce { principal, nonce }
        }
        GuardReject::MissingNonce { principal } => IngressError::MissingNonce { principal },
        // Task 1 (17-03, INGR-04/D-22): reuses the EXISTING
        // `IngressError::ForeignDomain` variant (Phase 11) rather than
        // adding a duplicate — this is now produced pre-verify by
        // `audience_check` instead of post-verify by
        // `check_audience_and_format`, but an operator grepping the slug
        // sees the same `foreign_domain` either way.
        GuardReject::ForeignDomain { expected, got } => {
            IngressError::ForeignDomain { expected, got }
        }
        // Task 1 (17-03, INGR-04/D-22): reuses the EXISTING
        // `IngressError::SenderNotBacked` variant (Phase 9) rather than
        // adding a duplicate. That variant carries no fields (the
        // pre-existing post-verify reject in `deliver()` never needed
        // one); `GuardReject::SenderNotBacked`'s `principal` field is
        // still logged at the `ingest_inbound` call site before this
        // translation drops it.
        GuardReject::SenderNotBacked { .. } => IngressError::SenderNotBacked,
        GuardReject::RateLimited {
            principal,
            limit,
            window_secs,
        } => IngressError::RateLimited {
            principal,
            limit,
            window_secs,
        },
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
        own_domain: &state.own_domain,
        sender_is_backed,
    };
    {
        let mut guard = state.guard.lock().await;
        ingress_guard::run_cheap_gates(&guard_input, &mut guard).map_err(|reject| {
            match &reject {
                GuardReject::StaleTimestamp {
                    ts,
                    now: guard_now,
                    skew_secs,
                } => {
                    eprintln!(
                        "famp-gateway: ingress: rejected — stale timestamp ts={ts} now={guard_now} \
                         skew_secs={skew_secs}"
                    );
                }
                GuardReject::ReplayedNonce { principal, nonce } => {
                    eprintln!(
                        "famp-gateway: ingress: rejected — replayed nonce '{nonce}' from sender '{principal}'"
                    );
                }
                GuardReject::MissingNonce { principal } => {
                    eprintln!(
                        "famp-gateway: ingress: rejected — sender '{principal}' submitted an envelope with no nonce"
                    );
                }
                GuardReject::ForeignDomain { expected, got } => {
                    eprintln!(
                        "famp-gateway: ingress: rejected — envelope 'to' domain '{got}' != this \
                         gateway's own-domain '{expected}' (pre-verify audience check)"
                    );
                }
                GuardReject::SenderNotBacked { principal } => {
                    eprintln!(
                        "famp-gateway: ingress: rejected — sender '{principal}' is not backed by \
                         this gateway (pre-verify audience check)"
                    );
                }
                GuardReject::RateLimited {
                    principal,
                    limit,
                    window_secs,
                } => {
                    eprintln!(
                        "famp-gateway: ingress: rejected — sender '{principal}' exceeded \
                         {limit} requests per {window_secs}s"
                    );
                }
                GuardReject::BadEnvelopeShape => {}
            }
            ingress_error_for_guard(reject)
        })?;
    }

    // EXPENSIVE: only reached once every cheap gate above has passed
    // (D-09/INGR-05).
    let envelope =
        verify_inbound_any(body, &state.keyring, &now).map_err(ingress_error_for_reject)?;

    check_audience_and_format(&envelope, recipient)?;

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
///
/// Task 1 (17-03, INGR-04/D-22): the own-domain-authority check that used
/// to live here (post-verify) is REMOVED — [`crate::ingress_guard::audience_check`]
/// now enforces the domain half pre-verify, ahead of `verify_inbound_any`
/// (INGR-05), so re-checking it here on the same verified `to` would be
/// pure duplication. `MisaddressedRecipient` and `federation_format_ok`
/// stay here unchanged: both need the VERIFIED envelope's own fields
/// (`to` and the federation `nonce`/`expiry` shape respectively), which
/// only exist after `verify_inbound_any` returns — moving either onto
/// unverified data would be a different and worse change, not a
/// reordering.
fn check_audience_and_format(
    envelope: &AnySignedEnvelope,
    recipient: &Principal,
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
        // Task 1 (17-03, INGR-04/D-22): `audience_check` already proved
        // this sender is backed, pre-verify (`GuardReject::SenderNotBacked`)
        // — a hostile "sender not backed" request never reaches this far
        // anymore. This `else` branch is kept purely as defense-in-depth:
        // reaching it now would mean the sender was backed when the guard
        // chain ran but was concurrently de-registered before this lock
        // was acquired, a benign race, not an attack surface.
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
    own_domain: Arc<str>,
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
    ///
    /// Task 2 (17-02, INGR-02) deviation: as of the replay gate,
    /// `ingest_inbound` also requires a non-empty peeked `nonce` before
    /// signature verification ever runs. `ack_bytes`/`ack_bytes_with_ts`
    /// therefore also stamp a fresh, unique `nonce` (`.with_nonce(...)`,
    /// the same federation-wrapper field `egress.rs::sign_federation_fields`
    /// adds on the way out) on every call, so a pre-existing reject-path
    /// test doesn't start failing `missing_nonce` instead of the reject
    /// reason it actually exercises. Tests that need to control the exact
    /// nonce value (the replay-precedence and double-POST cases) use
    /// [`ack_bytes_with_ts_and_nonce`] directly.
    fn ack_bytes(sk: &FampSigningKey, from: &Principal, to: &Principal) -> Vec<u8> {
        ack_bytes_with_ts(sk, from, to, Timestamp(crate::clock::now_canonical_utc()))
    }

    fn ack_bytes_with_ts(
        sk: &FampSigningKey,
        from: &Principal,
        to: &Principal,
        ts: Timestamp,
    ) -> Vec<u8> {
        ack_bytes_with_ts_and_nonce(sk, from, to, ts, &uuid::Uuid::now_v7().to_string())
    }

    fn ack_bytes_with_ts_and_nonce(
        sk: &FampSigningKey,
        from: &Principal,
        to: &Principal,
        ts: Timestamp,
        nonce: &str,
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
        )
        .with_nonce(nonce.to_string());
        unsigned.sign(sk).unwrap().encode().unwrap()
    }

    fn router_with_keyring(keyring: Keyring) -> (Router, Arc<Mutex<GatewayRegistry>>) {
        router_with_keyring_and_domain(keyring, "hostb.test")
    }

    fn router_with_keyring_and_domain(
        keyring: Keyring,
        own_domain: &str,
    ) -> (Router, Arc<Mutex<GatewayRegistry>>) {
        let registry = Arc::new(Mutex::new(GatewayRegistry::default()));
        (
            build_gateway_router(registry.clone(), Arc::new(keyring), Arc::from(own_domain)),
            registry,
        )
    }

    /// Task 1 (17-03, INGR-04) deviation: `audience_check` now requires
    /// `sender_is_backed` to be TRUE before ANY later cheap gate
    /// (freshness/replay/rate-limit) or signature verification ever runs.
    /// Every pre-17-03 test in this module that wanted to reach one of
    /// those later checks relied on an UNBACKED sender reaching as far as
    /// `sender_not_backed` (BAD_GATEWAY) — that terminal state is now
    /// reached FIRST, before freshness/replay/nonce/signature ever run,
    /// which would silently stop testing the exact property most of
    /// those tests exist to prove. `LiveBroker` stands up a real,
    /// in-process broker (`famp::cli::broker::run_on_listener` — a
    /// test-facing entry point taking a pre-bound `UnixListener`; no
    /// subprocess, no binary build, unlike
    /// `inbound_destination_validation.rs`'s heavier subprocess pattern)
    /// so `GatewayRegistry::back` can register a REAL backed sender,
    /// letting these tests reach the gate they actually test.
    struct LiveBroker {
        _tmp: tempfile::TempDir,
        _shutdown: tokio::sync::oneshot::Sender<()>,
        _broker: tokio::task::JoinHandle<()>,
        sock: std::path::PathBuf,
    }

    impl LiveBroker {
        async fn spawn() -> Self {
            let tmp = tempfile::TempDir::new().expect("tempdir");
            let sock = tmp.path().join("bus.sock");
            let listener =
                tokio::net::UnixListener::bind(&sock).expect("bind in-process broker socket");
            let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
            let shutdown_fut = async move {
                let _ = shutdown_rx.await;
            };
            let sock_for_broker = sock.clone();
            let bus_dir = tmp.path().to_path_buf();
            let broker = tokio::spawn(async move {
                let _ = famp::cli::broker::run_on_listener(
                    &sock_for_broker,
                    &bus_dir,
                    listener,
                    shutdown_fut,
                )
                .await;
            });
            // Give the broker a tick to enter its select loop (matches
            // famp/tests/broker_lifecycle.rs's own convention for this
            // same in-process entry point).
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            Self {
                _tmp: tmp,
                _shutdown: shutdown_tx,
                _broker: broker,
                sock,
            }
        }
    }

    /// Build a router with `from_name` REALLY backed (see [`LiveBroker`]'s
    /// doc comment) via a fresh, isolated in-process broker — mirroring
    /// `router_with_keyring`'s existing fresh-empty-registry-per-call
    /// convention, just with one real connection instead of zero.
    async fn router_with_backed_sender(
        keyring: Keyring,
        own_domain: &str,
        from_name: &str,
    ) -> (Router, Arc<Mutex<GatewayRegistry>>, LiveBroker) {
        let broker = LiveBroker::spawn().await;
        let mut registry = GatewayRegistry::default();
        registry
            .back(&broker.sock, from_name.to_owned())
            .await
            .expect("back sender on in-process broker");
        let registry = Arc::new(Mutex::new(registry));
        let router =
            build_gateway_router(registry.clone(), Arc::new(keyring), Arc::from(own_domain));
        (router, registry, broker)
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
        let (router, registry, _broker) =
            router_with_backed_sender(keyring_wrong, "hostb.test", "oscar").await;
        let resp = post_inbox(router, &to, bytes.clone()).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"], "invalid_signature");
        assert_eq!(
            registry.lock().await.names().count(),
            1,
            "reject path must perform zero registry mutation BEYOND the \
             harness's own backed sender (see LiveBroker's doc comment)"
        );

        // --- unpinned key: sender absent from an empty keyring ---
        let (router2, registry2, _broker2) =
            router_with_backed_sender(Keyring::new(), "hostb.test", "oscar").await;
        let resp2 = post_inbox(router2, &to, bytes).await;
        assert_eq!(resp2.status(), StatusCode::FORBIDDEN);
        let body2 = to_bytes(resp2.into_body(), usize::MAX).await.unwrap();
        let v2: Value = serde_json::from_slice(&body2).unwrap();
        assert_eq!(v2["error"], "unpinned_key");
        assert_eq!(
            registry2.lock().await.names().count(),
            1,
            "reject path must perform zero registry mutation BEYOND the \
             harness's own backed sender"
        );

        // Two distinct status codes, per D-08.
        assert_ne!(StatusCode::BAD_REQUEST, StatusCode::FORBIDDEN);
    }

    /// Task 1 (INGR-01/INGR-05, D-05/D-09): an otherwise-valid,
    /// correctly-signed envelope (the sender's key is pinned to the SAME
    /// key that signed — verification WOULD succeed if it were ever
    /// reached) with a one-hour-old `ts` is rejected `stale_timestamp`
    /// with zero registry mutation beyond the harness's own backed
    /// sender. Proves the freshness gate is real, not a side effect of
    /// signature failure.
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
        let (router, registry, _broker) =
            router_with_backed_sender(keyring, "hostb.test", "oscar").await;

        let resp = post_inbox(router, &to, bytes).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"], "stale_timestamp");
        assert_eq!(
            registry.lock().await.names().count(),
            1,
            "reject path must perform zero registry mutation BEYOND the \
             harness's own backed sender (see LiveBroker's doc comment)"
        );
    }

    /// Task 1 (INGR-01/INGR-02 boundary probe): exactly
    /// `CLOCK_SKEW_WINDOW_SECS` from `now`, in either direction, must NOT
    /// be classified `stale_timestamp` (inclusive bound); one second
    /// beyond, in either direction, MUST be.
    ///
    /// 17-03 deviation: the in-window cases used to assert only "not
    /// stale_timestamp" because full delivery needed a live broker this
    /// test didn't spin up. `audience_check` now REQUIRES a backed
    /// sender before any other cheap gate runs (see `LiveBroker`'s doc
    /// comment), so this test now stands one up regardless — which means
    /// the in-window cases can assert the STRONGER, more meaningful
    /// property: full delivery success (202 ACCEPTED, no JSON body).
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
            // Build the harness (spawns a real in-process broker + backs
            // the sender) BEFORE stamping `ts` -- broker startup latency
            // sits OUTSIDE the timestamp window this way, not inside it.
            // Stamping `ts` first (the original ordering) left an
            // unbounded gap between "timestamp computed" and "freshness
            // checked against the live wall clock" that widens under
            // system load, intermittently pushing an exactly-at-the-
            // boundary case past the window before the request is even
            // sent -- an observed flake, not merely a theoretical one.
            let mut keyring = Keyring::new();
            keyring.pin_tofu(from.clone(), sk.verifying_key()).unwrap();
            let (router, registry, _broker) =
                router_with_backed_sender(keyring, "hostb.test", "oscar").await;
            let bytes = ack_bytes_with_ts(&sk, &from, &to, mk_ts(delta));
            let resp = post_inbox(router, &to, bytes).await;
            assert_eq!(
                resp.status(),
                StatusCode::ACCEPTED,
                "delta {delta}s is within the inclusive skew window and, with a real backed \
                 sender and a correctly-pinned key, must fully deliver"
            );
            assert_eq!(
                registry.lock().await.names().count(),
                1,
                "no registry mutation expected BEYOND the harness's own backed sender"
            );
        }

        for delta in [-(skew + 1), skew + 1] {
            let mut keyring = Keyring::new();
            keyring.pin_tofu(from.clone(), sk.verifying_key()).unwrap();
            let (router, registry, _broker) =
                router_with_backed_sender(keyring, "hostb.test", "oscar").await;
            let bytes = ack_bytes_with_ts(&sk, &from, &to, mk_ts(delta));
            let resp = post_inbox(router, &to, bytes).await;
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
            let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
            let v: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(
                v["error"], "stale_timestamp",
                "delta {delta}s is one second beyond the inclusive skew window"
            );
            assert_eq!(registry.lock().await.names().count(), 1);
        }
    }

    /// Task 2 (INGR-05, D-09): pins the check order with a
    /// reorder-detector. One case per cheap gate that exists so far, each
    /// asserting the response slug is the CHEAP gate's slug rather than a
    /// signature-layer slug. If a later refactor moves
    /// `verify_inbound_any` back above `run_cheap_gates`, each of these
    /// cases starts returning a signature-layer slug (`invalid_signature`
    /// / `unpinned_key`) instead and this test fails — the exact
    /// falsification 17-01-SUMMARY.md reports was run once, deliberately,
    /// with `strip_relay_fields_removes_wrapper_keeps_content` as the
    /// named passing control, then reverted.
    ///
    /// Rationale in the requirement's own terms: a cheap-to-send request
    /// that forces expensive verification on every hit is itself the
    /// amplifier INGR-05's ordering exists to remove.
    ///
    /// MAINTENANCE: every plan adding a new cheap gate MUST append one
    /// case per gate to this same table, so the chain stays fully pinned
    /// rather than pinned only at its head. Case 4 (17-02) is the replay
    /// gate's entry; cases 5-6 (17-03) are the audience and rate-limit
    /// gates' entries — the chain is now fully pinned across all four
    /// cheap gates (audience, freshness, replay, rate limit).
    #[tokio::test]
    async fn cheap_checks_reject_before_signature_verify_ever_runs() {
        let sk = FampSigningKey::from_bytes([50u8; 32]);
        let from: Principal = "agent:hosta.test/oscar".parse().unwrap();
        let to: Principal = "agent:hostb.test/peggy".parse().unwrap();

        let one_hour_ago = time::OffsetDateTime::now_utc() - time::Duration::hours(1);
        let stale_ts = Timestamp(crate::clock::strip_subseconds(
            &one_hour_ago
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap(),
        ));
        let stale_bytes = ack_bytes_with_ts(&sk, &from, &to, stale_ts);

        // --- case 1: stale ts AND pinned to the WRONG key -> stale_timestamp, never invalid_signature ---
        // (17-03: `from` must be REALLY backed now, since `audience_check`
        // runs before `freshness_check` — see `LiveBroker`'s doc comment.)
        let wrong_sk = FampSigningKey::from_bytes([51u8; 32]);
        let mut keyring_wrong = Keyring::new();
        keyring_wrong
            .pin_tofu(from.clone(), wrong_sk.verifying_key())
            .unwrap();
        let (router, registry, _broker) =
            router_with_backed_sender(keyring_wrong, "hostb.test", "oscar").await;
        let resp = post_inbox(router, &to, stale_bytes.clone()).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            v["error"], "stale_timestamp",
            "wrong-pinned-key case must reject on the CHEAP gate, never invalid_signature"
        );
        assert_eq!(registry.lock().await.names().count(), 1);

        // --- case 2: stale ts AND sender absent from the keyring entirely -> stale_timestamp, never unpinned_key ---
        // (backing != keyring pinning: `from` is backed in the registry but
        // has no pinned key at all -- proves audience_check's backing half
        // is independent of the keyring.)
        let (router2, registry2, _broker2) =
            router_with_backed_sender(Keyring::new(), "hostb.test", "oscar").await;
        let resp2 = post_inbox(router2, &to, stale_bytes).await;
        assert_eq!(resp2.status(), StatusCode::BAD_REQUEST);
        let body2 = to_bytes(resp2.into_body(), usize::MAX).await.unwrap();
        let v2: Value = serde_json::from_slice(&body2).unwrap();
        assert_eq!(
            v2["error"], "stale_timestamp",
            "absent-from-keyring case must reject on the CHEAP gate, never unpinned_key"
        );
        assert_eq!(registry2.lock().await.names().count(), 1);

        // --- case 3: malformed JSON -> bad_envelope_shape, never invalid_signature ---
        // (no backing needed: `peek_guard_fields` fails before the
        // registry is even consulted.)
        let (router3, registry3) = router_with_keyring(Keyring::new());
        let resp3 = post_inbox(router3, &to, b"not json".to_vec()).await;
        assert_eq!(resp3.status(), StatusCode::BAD_REQUEST);
        let body3 = to_bytes(resp3.into_body(), usize::MAX).await.unwrap();
        let v3: Value = serde_json::from_slice(&body3).unwrap();
        assert_eq!(
            v3["error"], "bad_envelope_shape",
            "malformed JSON must reject on the CHEAP gate, never invalid_signature"
        );
        assert_eq!(registry3.lock().await.names().count(), 0);

        // --- case 4 (17-02, INGR-02): a replay that is ALSO signed with
        // the wrong key -> replayed_nonce, never invalid_signature. The
        // first POST records the nonce (replay runs before verify,
        // regardless of whether verify would ultimately succeed) and
        // fails on signature as expected; the SECOND, identical POST
        // must be rejected on the replay gate before signature
        // verification runs again.
        let wrong_sk2 = FampSigningKey::from_bytes([52u8; 32]);
        let mut keyring_wrong2 = Keyring::new();
        keyring_wrong2
            .pin_tofu(from.clone(), wrong_sk2.verifying_key())
            .unwrap();
        let (router4, registry4, _broker4) =
            router_with_backed_sender(keyring_wrong2, "hostb.test", "oscar").await;
        let fresh_bytes = ack_bytes(&sk, &from, &to);

        let first = post_inbox(router4.clone(), &to, fresh_bytes.clone()).await;
        let first_body = to_bytes(first.into_body(), usize::MAX).await.unwrap();
        let first_v: Value = serde_json::from_slice(&first_body).unwrap();
        assert_eq!(
            first_v["error"], "invalid_signature",
            "first sighting with the wrong pinned key must still fail signature verification"
        );

        let second = post_inbox(router4, &to, fresh_bytes).await;
        assert_eq!(second.status(), StatusCode::BAD_REQUEST);
        let second_body = to_bytes(second.into_body(), usize::MAX).await.unwrap();
        let second_v: Value = serde_json::from_slice(&second_body).unwrap();
        assert_eq!(
            second_v["error"], "replayed_nonce",
            "replayed AND wrong-key case must surface replayed_nonce, never invalid_signature"
        );
        assert_eq!(registry4.lock().await.names().count(), 1);

        assert_foreign_domain_case_rejects_before_signature_verify(&sk, &from).await;
        assert_rate_limit_case_rejects_before_signature_verify(&sk, &from, &to).await;
    }

    /// Case 5 (17-03, INGR-04) of
    /// `cheap_checks_reject_before_signature_verify_ever_runs`: a
    /// foreign-domain envelope, ALSO pinned to the WRONG key ->
    /// `foreign_domain`, never `invalid_signature`. Domain check runs
    /// before backing (and thus before freshness/replay/rate-limit/verify)
    /// so no live broker is needed here — `router_with_keyring_and_domain`
    /// (unbacked) is sufficient. Extracted to a standalone fn to keep the
    /// caller under the workspace `too_many_lines` budget.
    async fn assert_foreign_domain_case_rejects_before_signature_verify(
        sk: &FampSigningKey,
        from: &Principal,
    ) {
        let wrong_sk = FampSigningKey::from_bytes([53u8; 32]);
        let mut keyring_wrong = Keyring::new();
        keyring_wrong
            .pin_tofu(from.clone(), wrong_sk.verifying_key())
            .unwrap();
        let foreign_to: Principal = "agent:otherdomain.test/peggy".parse().unwrap();
        let (router, registry) = router_with_keyring_and_domain(keyring_wrong, "hostb.test");
        let bytes = ack_bytes(sk, from, &foreign_to);
        let resp = post_inbox(router, &foreign_to, bytes).await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            v["error"], "foreign_domain",
            "foreign-domain-AND-wrong-key case must reject on the CHEAP gate, never invalid_signature"
        );
        assert_eq!(registry.lock().await.names().count(), 0);
    }

    /// Case 6 (17-03, INGR-06) of
    /// `cheap_checks_reject_before_signature_verify_ever_runs`: a request
    /// past the rate limit, ALSO signed with the wrong key -> `rate_limited`,
    /// never `invalid_signature`. Exhausts the backed sender's budget with
    /// valid-shaped (but wrong-key-signed) requests first, then proves the
    /// very next one is rejected on the rate-limit gate before signature
    /// verification ever runs again. Extracted to a standalone fn for the
    /// same `too_many_lines` reason as the foreign-domain case above.
    async fn assert_rate_limit_case_rejects_before_signature_verify(
        sk: &FampSigningKey,
        from: &Principal,
        to: &Principal,
    ) {
        let wrong_sk = FampSigningKey::from_bytes([54u8; 32]);
        let mut keyring_wrong = Keyring::new();
        keyring_wrong
            .pin_tofu(from.clone(), wrong_sk.verifying_key())
            .unwrap();
        let (router, registry, _broker) =
            router_with_backed_sender(keyring_wrong, "hostb.test", "oscar").await;
        for i in 0..crate::ingress_guard::RATE_LIMIT_MAX_PER_WINDOW {
            let bytes = ack_bytes(sk, from, to);
            let resp = post_inbox(router.clone(), to, bytes).await;
            let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
            let v: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(
                v["error"], "invalid_signature",
                "request {i} within the rate limit must reach signature verification"
            );
        }
        let over_limit_bytes = ack_bytes(sk, from, to);
        let resp = post_inbox(router, to, over_limit_bytes).await;
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            v["error"], "rate_limited",
            "over-the-limit-AND-wrong-key case must reject on the CHEAP gate, never invalid_signature"
        );
        assert_eq!(registry.lock().await.names().count(), 1);
    }

    /// Task 2 (17-02, INGR-02): the same signed envelope POSTed twice is
    /// accepted past the replay gate the first time and rejected
    /// `replayed_nonce` the second time.
    ///
    /// 17-03 deviation: `audience_check` now requires a real backed
    /// sender before any other cheap gate runs (see `LiveBroker`'s doc
    /// comment). With a real backed sender and a correctly-pinned key,
    /// the first POST now fully DELIVERS (202 ACCEPTED, no JSON body)
    /// instead of stopping at `sender_not_backed` — asserted directly on
    /// the status code rather than parsed as JSON.
    #[tokio::test]
    async fn replayed_envelope_is_rejected_on_second_delivery_with_no_extra_registry_mutation() {
        let sk = FampSigningKey::from_bytes([60u8; 32]);
        let from: Principal = "agent:hosta.test/oscar".parse().unwrap();
        let to: Principal = "agent:hostb.test/peggy".parse().unwrap();
        let bytes = ack_bytes(&sk, &from, &to);

        let mut keyring = Keyring::new();
        keyring.pin_tofu(from.clone(), sk.verifying_key()).unwrap();
        let (router, registry, _broker) =
            router_with_backed_sender(keyring, "hostb.test", "oscar").await;

        let resp1 = post_inbox(router.clone(), &to, bytes.clone()).await;
        assert_eq!(
            resp1.status(),
            StatusCode::ACCEPTED,
            "first sighting of a nonce, with a real backed sender and a correctly-pinned key, \
             must pass the replay gate and fully deliver"
        );

        let resp2 = post_inbox(router, &to, bytes).await;
        assert_eq!(resp2.status(), StatusCode::BAD_REQUEST);
        let body2 = to_bytes(resp2.into_body(), usize::MAX).await.unwrap();
        let v2: Value = serde_json::from_slice(&body2).unwrap();
        assert_eq!(v2["error"], "replayed_nonce");

        assert_eq!(
            registry.lock().await.names().count(),
            1,
            "reject path must perform zero registry mutation BEYOND the \
             harness's own backed sender"
        );
    }

    /// Task 2 (17-02, INGR-02): an envelope with no `nonce` field at all
    /// is rejected `missing_nonce`, before signature verification runs.
    #[tokio::test]
    async fn envelope_with_no_nonce_is_rejected_missing_nonce() {
        let sk = FampSigningKey::from_bytes([61u8; 32]);
        let from: Principal = "agent:hosta.test/oscar".parse().unwrap();
        let to: Principal = "agent:hostb.test/peggy".parse().unwrap();

        let id: MessageId = "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a72".parse().unwrap();
        let body = AckBody {
            disposition: AckDisposition::Accepted,
            reason: None,
        };
        let unsigned = UnsignedEnvelope::<AckBody>::new(
            id,
            from.clone(),
            to.clone(),
            AuthorityScope::Advisory,
            Timestamp(crate::clock::now_canonical_utc()),
            body,
        );
        // Deliberately no `.with_nonce(...)`.
        let bytes = unsigned.sign(&sk).unwrap().encode().unwrap();

        let mut keyring = Keyring::new();
        keyring.pin_tofu(from.clone(), sk.verifying_key()).unwrap();
        let (router, registry, _broker) =
            router_with_backed_sender(keyring, "hostb.test", "oscar").await;

        let resp = post_inbox(router, &to, bytes).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"], "missing_nonce");
        assert_eq!(
            registry.lock().await.names().count(),
            1,
            "reject path must perform zero registry mutation BEYOND the \
             harness's own backed sender"
        );
    }

    #[tokio::test]
    async fn bad_principal_in_path_is_rejected_before_verification() {
        let (router, _registry) = router_with_keyring(Keyring::new());
        let resp =
            post_inbox_raw(router, "/famp/v0.5.1/inbox/not-a-principal", b"x".to_vec()).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    /// Task 2 (17-03, INGR-07) finding: the `ONE_MIB` cap already existed,
    /// applied via `tower_http::limit::RequestBodyLimitLayer` in
    /// [`build_gateway_router`] — INGR-07 needed a TEST, not a new
    /// implementation. Confirmed by reading
    /// `tower_http::limit::service::RequestBodyLimit::call` (crate source,
    /// version pinned in `Cargo.lock`, cited in 17-RESEARCH.md's Code
    /// Examples): when a request declares a `Content-Length` above the
    /// configured limit, the layer returns `413` IMMEDIATELY — before the
    /// inner service (this router, and therefore [`inbox_handler`]) is
    /// EVER called — the strongest available proof that no full buffer
    /// occurs. When no `Content-Length` is declared, the layer instead
    /// wraps the body in `http_body_util::Limited`, which rejects once
    /// actual bytes read exceed the limit WHILE STREAMING, not after full
    /// collection. Do not add a manual length check or a hand-rolled byte
    /// counter anywhere in this crate — a second cap would drift from the
    /// first.
    #[tokio::test]
    async fn oversized_declared_content_length_is_rejected_413_without_entering_the_handler() {
        let (router, registry) = router_with_keyring(Keyring::new());
        let declared_len = ONE_MIB + 1;
        let req = axum::http::Request::builder()
            .method("POST")
            .uri(inbox_uri(&"agent:hostb.test/peggy".parse().unwrap()))
            .header("content-type", "application/famp+json")
            .header(axum::http::header::CONTENT_LENGTH, declared_len.to_string())
            // The ACTUAL body is tiny -- proves the layer rejects on the
            // DECLARED length alone, never by reading this far.
            .body(axum::body::Body::from(b"tiny".to_vec()))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            registry.lock().await.names().count(),
            0,
            "the handler (and therefore any registry lookup) must never be entered"
        );
    }

    #[tokio::test]
    async fn oversized_streamed_body_is_rejected_413() {
        let (router, _registry) = router_with_keyring(Keyring::new());
        // No declared Content-Length: `Router::oneshot` never auto-populates
        // one from the body's own size hint (there is no wire-serialization
        // step), so this exercises the STREAMING limiter path.
        let oversized_body = vec![0u8; ONE_MIB + 1024];
        let req = axum::http::Request::builder()
            .method("POST")
            .uri(inbox_uri(&"agent:hostb.test/peggy".parse().unwrap()))
            .header("content-type", "application/famp+json")
            .body(axum::body::Body::from(oversized_body))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn exactly_one_mib_body_is_not_size_rejected() {
        let (router, _registry) = router_with_keyring(Keyring::new());
        let exact_body = vec![0u8; ONE_MIB];
        let req = axum::http::Request::builder()
            .method("POST")
            .uri(inbox_uri(&"agent:hostb.test/peggy".parse().unwrap()))
            .header("content-type", "application/famp+json")
            .body(axum::body::Body::from(exact_body))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_ne!(
            resp.status(),
            StatusCode::PAYLOAD_TOO_LARGE,
            "a body of EXACTLY ONE_MIB must not be rejected for size (it fails later, for \
             shape/signature reasons -- an all-zero body is not a valid envelope -- which is \
             the correct outcome for this test)"
        );
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

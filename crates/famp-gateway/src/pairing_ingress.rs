//! Cross-person pairing redemption route: `POST /famp/v1/pair/redeem`
//! (Phase 18, PAIR-01/06; the transport decision recorded at
//! `18-01-PLAN.md` Task 1, Option A).
//!
//! Deliberately NOT inside [`crate::verify::verify_inbound_any`]'s path
//! (`ingress.rs`) — this route is unauthenticated by design; entropy (the
//! five-word code) is the trust boundary, not Ed25519 pinning
//! (T-18-01/T-18-06). Its own `Router` and its own state type
//! ([`PairingIngressState`]) never touch [`crate::ingress::GatewayIngressState`],
//! [`crate::ingress_guard`], `inbox_handler`, or `ingest_inbound` — Task
//! 1's binding constraint 1.
//!
//! Gate order, cheap before expensive (mirrors `ingress_guard`'s INGR-05
//! discipline, T-18-06/T-18-07):
//! 1. deserialize the signed request and `parse_code` its code — reject
//!    `malformed_code` BEFORE any file touch.
//! 2. reject `own_domain_refused` if the presented principal's domain
//!    equals this gateway's own domain (T-18-07) — a redeemer cannot
//!    claim a principal inside the inviter's own authority.
//! 3. load the store — absent or zero `Pending` records ->
//!    `no_pending_invite`, HTTP 404 (T-18-06: the unauthenticated surface
//!    is live only while an invite is genuinely outstanding).
//! 4. compare the presented code's digest against every `Pending` record
//!    via `digests_equal` -> `code_mismatch` on no match, ALSO HTTP 404 —
//!    a distinct status code here would let a caller distinguish "no
//!    invite exists at all" from "wrong code for an existing invite", the
//!    exact oracle T-18-09 is scoped to avoid.
//! 5. verify the request signature against the presented `pubkey_b64url`
//!    -> `invalid_signature` (T-18-04, proof of possession).
//! 6. on match: `StoreLock`, re-load, mutate the matched record to
//!    `Redeemed`, `save_atomic`, and ONLY THEN build and sign the
//!    response. Persist before replying, never the reverse (T-18-03). On
//!    ANY rejection, mutate nothing.

use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::post,
    Router,
};
use famp::cli::peer::identity::load_or_generate;
use famp::pairing::invite::{InviteState, InviteStore, StoreLock};
use famp::pairing::wordlist::{code_digest, digest_from_hex, digests_equal, parse_code};
use famp::pairing::{RedemptionReject, RedemptionRequest, RedemptionResponse, Signed};
use famp::{FampSigningKey, Principal, TrustedVerifyingKey};
use famp_crypto::key_id;

/// `POST /famp/v1/pair/redeem`.
pub const PAIRING_REDEEM_ROUTE: &str = "/famp/v1/pair/redeem";

/// Shared state for the pairing router.
///
/// Its OWN type — never [`crate::ingress::GatewayIngressState`] (Task 1's
/// binding constraint 1): this route never reads the registry, the peers
/// keyring, or the ingress guard.
#[derive(Clone)]
pub struct PairingIngressState {
    store_path: Arc<PathBuf>,
    signing_key_path: Arc<PathBuf>,
    own_domain: Arc<str>,
}

impl PairingIngressState {
    #[must_use]
    pub const fn new(
        store_path: Arc<PathBuf>,
        signing_key_path: Arc<PathBuf>,
        own_domain: Arc<str>,
    ) -> Self {
        Self {
            store_path,
            signing_key_path,
            own_domain,
        }
    }
}

/// Build the pairing router — its OWN `Router`, mounted alongside (not
/// inside) [`crate::ingress::build_gateway_router`]'s state.
///
/// The caller (`ingress.rs`/`main.rs`) merges this with `Router::merge`
/// BEFORE the shared 1 MiB body-limit layer so this route inherits the
/// same cap (T-18-06).
pub fn build_pairing_router(state: PairingIngressState) -> Router {
    Router::new()
        .route(PAIRING_REDEEM_ROUTE, post(pairing_redeem_handler))
        .with_state(state)
}

/// `reason` slugs this route can reject with, and the HTTP status each
/// maps to. Kept as a standalone function (mirrors
/// `ingress.rs::ingress_error_for_reject`) so [`pairing_redeem_handler`]
/// stays a thin translation layer.
fn reject_status(reason: &str) -> StatusCode {
    match reason {
        "malformed_code" | "invalid_signature" => StatusCode::BAD_REQUEST,
        "own_domain_refused" => StatusCode::FORBIDDEN,
        "no_pending_invite" | "code_mismatch" => StatusCode::NOT_FOUND,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn pairing_redeem_handler(State(state): State<PairingIngressState>, body: Bytes) -> Response {
    match ingest_redemption(&body, &state).await {
        Ok(signed) => (StatusCode::OK, Json(signed)).into_response(),
        Err(reject) => {
            let status = reject_status(&reject.reason);
            (status, Json(reject)).into_response()
        }
    }
}

/// THE single ingest core — separately testable, mirroring
/// `ingress.rs::ingest_inbound`'s delegation shape.
pub async fn ingest_redemption(
    body: &[u8],
    state: &PairingIngressState,
) -> Result<Signed<RedemptionResponse>, RedemptionReject> {
    let now = crate::clock::now_canonical_utc();
    ingest_redemption_at(body, state, &now).await
}

fn reject(reason: &str) -> RedemptionReject {
    RedemptionReject {
        reason: reason.to_string(),
    }
}

/// `now`-injected core of [`ingest_redemption`] — [`ingest_redemption`]
/// is the sole production caller; tests call this directly with a pinned
/// instant (mirrors `ingress.rs::ingest_inbound_at`'s split, for the same
/// determinism reason).
///
/// Deliberately `async` even though this plan's gate chain (steps 1-6 in
/// this module's doc comment) performs no `.await` internally today: it
/// mirrors `ingest_inbound_at`'s async shape, and Plan 02's attempt-limit
/// / TTL persistence work is expected to add real awaited I/O to this
/// exact function rather than to a newly-async-ified sibling.
#[allow(clippy::unused_async)]
async fn ingest_redemption_at(
    body: &[u8],
    state: &PairingIngressState,
    now: &str,
) -> Result<Signed<RedemptionResponse>, RedemptionReject> {
    // (1) cheap: deserialize + parse_code BEFORE any file touch.
    let signed_request: Signed<RedemptionRequest> =
        serde_json::from_slice(body).map_err(|_| reject("malformed_code"))?;
    let request = &signed_request.statement;
    let code = parse_code(&request.code).map_err(|_| reject("malformed_code"))?;
    let presented_principal: Principal = request
        .principal
        .parse()
        .map_err(|_| reject("malformed_code"))?;
    let presented_vk = TrustedVerifyingKey::from_b64url(&request.pubkey_b64url)
        .map_err(|_| reject("malformed_code"))?;

    // (2) T-18-07: refuse a redeemer claiming a principal inside the
    // inviter's own authority.
    if presented_principal.authority() == &*state.own_domain {
        return Err(reject("own_domain_refused"));
    }

    // (3) load the store — absent or zero Pending -> 404.
    let store = InviteStore::load(&state.store_path).map_err(|_| reject("no_pending_invite"))?;
    let any_pending = store
        .invites
        .iter()
        .any(|r| matches!(r.state, InviteState::Pending));
    if !any_pending {
        return Err(reject("no_pending_invite"));
    }

    // (4) constant-time digest compare against every Pending record.
    let presented_digest = code_digest(&code);
    let matched_id = store
        .invites
        .iter()
        .filter(|r| matches!(r.state, InviteState::Pending))
        .find(|r| {
            digest_from_hex(&r.code_digest)
                .is_some_and(|stored| digests_equal(&presented_digest, &stored))
        })
        .map(|r| r.id.clone());
    let Some(invite_id) = matched_id else {
        return Err(reject("code_mismatch"));
    };

    // (5) verify the request signature — proof of possession (T-18-04).
    signed_request
        .verify(&presented_vk)
        .map_err(|_| reject("invalid_signature"))?;

    let redeemer_key_id = key_id(&presented_vk);

    // (6) persist BEFORE replying (T-18-03). Re-acquire the lock and
    // re-load rather than trusting the step-4 read, so a concurrent
    // redemption that won the race in between is not double-honored.
    let lock = StoreLock::acquire(&state.store_path).map_err(|_| reject("no_pending_invite"))?;
    let mut store =
        InviteStore::load(&state.store_path).map_err(|_| reject("no_pending_invite"))?;
    let Some(record) = store
        .invites
        .iter_mut()
        .find(|r| r.id == invite_id && matches!(r.state, InviteState::Pending))
    else {
        drop(lock);
        return Err(reject("code_mismatch"));
    };
    let inviter_principal = record.principal.clone();
    record.state = InviteState::Redeemed {
        by: request.principal.clone(),
        key_id: redeemer_key_id.clone(),
        pubkey_b64url: request.pubkey_b64url.clone(),
        at: now.to_string(),
    };
    store
        .save_atomic(&state.store_path)
        .map_err(|_| reject("internal_error"))?;
    drop(lock);

    // Build + sign the response with the INVITER's own gateway key —
    // never a per-invite key — loaded via the same idempotent
    // `load_or_generate` path `famp peer export`/`import` already use.
    let sk: FampSigningKey =
        load_or_generate(&state.signing_key_path).map_err(|_| reject("internal_error"))?;
    let response = RedemptionResponse {
        invite_id,
        inviter_principal,
        inviter_pubkey_b64url: sk.verifying_key().to_b64url(),
        redeemer_key_id,
        at: now.to_string(),
    };
    Signed::new(response, &sk).map_err(|_| reject("internal_error"))
}

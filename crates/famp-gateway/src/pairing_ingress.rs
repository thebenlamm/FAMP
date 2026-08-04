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
//! 3. load the store and call `InviteStore::decide` (Plan 02,
//!    `famp::pairing::invite`) — a pure, read-only classification against
//!    the gateway's own wall clock (T-18-11). `NoPendingInvite` and
//!    `WrongCode` both 404 (T-18-09: a distinct status code between them
//!    would let a caller distinguish "no invite exists at all" from
//!    "wrong code for an existing invite").
//! 4. `WrongCode`: acquire `StoreLock`, RE-LOAD, `burn_attempt`,
//!    `save_atomic`, drop the lock, THEN reject `code_mismatch` — the
//!    ONLY rejection path that mutates anything.
//! 5. `Expired`/`AlreadyRedeemed`/`AttemptsExhausted`/`NoPendingInvite`:
//!    reject immediately with zero filesystem writes; the lock is never
//!    acquired.
//! 6. `Accept`: verify the request signature against the presented
//!    `pubkey_b64url` FIRST (T-18-04, proof of possession, and the
//!    expensive check that must not run for any cheap rejection above).
//!    Then acquire `StoreLock`, RE-LOAD, and call `decide` a SECOND TIME
//!    under the lock — a racing request that already consumed the invite
//!    makes this second `decide` return `AlreadyRedeemed`, which is what
//!    turns two concurrent valid redemptions into exactly one success. On
//!    a still-`Accept` second decision: `consume`, `save_atomic`, drop
//!    the lock, and ONLY THEN build and sign the response. **This
//!    ordering — persist under the lock, THEN construct the HTTP
//!    response — is a load-bearing invariant** (T-18-03): a process kill
//!    between the two must leave the invite consumed, never available for
//!    replay. Pinned by
//!    `crates/famp-gateway/tests/pairing_ingress.rs`'s
//!    `replay_of_consumed_code_after_reload_is_rejected`, itself proven
//!    fail-first by a recorded manual falsification against the reverted
//!    ordering (18-02-SUMMARY.md).

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
use famp::pairing::invite::{InviteStore, RedemptionDecision, StoreLock};
use famp::pairing::wordlist::{code_digest, parse_code};
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
        "no_pending_invite" | "code_mismatch" | "expired" => StatusCode::NOT_FOUND,
        "already_redeemed" => StatusCode::CONFLICT,
        "attempts_exhausted" => StatusCode::TOO_MANY_REQUESTS,
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

    // (3) load the store and classify — pure, read-only, no lock held.
    let store = InviteStore::load(&state.store_path).map_err(|_| reject("no_pending_invite"))?;
    let presented_digest = code_digest(&code);
    let decision = store.decide(&presented_digest, now);

    match decision {
        // (5) cheap rejections: zero filesystem writes, lock never
        // acquired.
        RedemptionDecision::NoPendingInvite => Err(reject("no_pending_invite")),
        RedemptionDecision::Expired => Err(reject("expired")),
        RedemptionDecision::AlreadyRedeemed => Err(reject("already_redeemed")),
        RedemptionDecision::AttemptsExhausted => Err(reject("attempts_exhausted")),

        // (4) the ONLY rejection path that mutates: burn the attempt
        // budget under the lock, persist, THEN reject.
        RedemptionDecision::WrongCode => {
            burn_and_reject_wrong_code(state, now)?;
            Err(reject("code_mismatch"))
        }

        // (6) Accept: verify signature FIRST (expensive), then
        // lock+re-decide+consume+persist, THEN build the response.
        RedemptionDecision::Accept { id: first_pass_id } => {
            signed_request
                .verify(&presented_vk)
                .map_err(|_| reject("invalid_signature"))?;
            accept_and_consume(
                state,
                &signed_request.statement,
                &presented_vk,
                &presented_digest,
                &first_pass_id,
                now,
            )
        }
    }
}

/// The WrongCode mutation, isolated so [`ingest_redemption_at`] stays
/// under the workspace's function-length lint: acquire the lock, RE-LOAD,
/// `burn_attempt`, `save_atomic`, drop the lock. Never called for any
/// other [`RedemptionDecision`] — the caller is the sole match arm.
fn burn_and_reject_wrong_code(
    state: &PairingIngressState,
    now: &str,
) -> Result<(), RedemptionReject> {
    let lock = StoreLock::acquire(&state.store_path).map_err(|_| reject("no_pending_invite"))?;
    let mut store =
        InviteStore::load(&state.store_path).map_err(|_| reject("no_pending_invite"))?;
    store.burn_attempt(now);
    store
        .save_atomic(&state.store_path)
        .map_err(|_| reject("internal_error"))?;
    drop(lock);
    Ok(())
}

/// The Accept lock-and-consume sequence, isolated so
/// [`ingest_redemption_at`] stays under the workspace's function-length
/// lint. Re-decides UNDER the lock before consuming — see this module's
/// doc comment for why that ordering is load-bearing (T-18-03).
fn accept_and_consume(
    state: &PairingIngressState,
    request: &RedemptionRequest,
    presented_vk: &TrustedVerifyingKey,
    presented_digest: &[u8; 32],
    first_pass_id: &str,
    now: &str,
) -> Result<Signed<RedemptionResponse>, RedemptionReject> {
    let redeemer_key_id = key_id(presented_vk);

    let lock = StoreLock::acquire(&state.store_path).map_err(|_| reject("no_pending_invite"))?;
    let mut store =
        InviteStore::load(&state.store_path).map_err(|_| reject("no_pending_invite"))?;

    // Re-decide UNDER the lock: a racing request that already consumed
    // this invite in between makes this second decide return
    // AlreadyRedeemed, turning two concurrent valid redemptions into
    // exactly one success (T-18-03).
    let invite_id = match store.decide(presented_digest, now) {
        RedemptionDecision::Accept { id } => id,
        RedemptionDecision::AlreadyRedeemed => {
            drop(lock);
            return Err(reject("already_redeemed"));
        }
        RedemptionDecision::Expired => {
            drop(lock);
            return Err(reject("expired"));
        }
        RedemptionDecision::AttemptsExhausted => {
            drop(lock);
            return Err(reject("attempts_exhausted"));
        }
        RedemptionDecision::WrongCode | RedemptionDecision::NoPendingInvite => {
            drop(lock);
            return Err(reject("no_pending_invite"));
        }
    };
    debug_assert_eq!(
        invite_id, first_pass_id,
        "re-decide under the lock matched a different record than the unlocked pass"
    );

    let Some(inviter_principal) = store
        .invites
        .iter()
        .find(|r| r.id == invite_id)
        .map(|r| r.principal.clone())
    else {
        drop(lock);
        return Err(reject("no_pending_invite"));
    };

    store
        .consume(
            &invite_id,
            &request.principal,
            &redeemer_key_id,
            &request.pubkey_b64url,
            now,
        )
        .map_err(|_| reject("internal_error"))?;
    store
        .save_atomic(&state.store_path)
        .map_err(|_| reject("internal_error"))?;
    drop(lock);

    // Build + sign the response with the INVITER's own gateway key —
    // never a per-invite key — loaded via the same idempotent
    // `load_or_generate` path `famp peer export`/`import` already use.
    // Constructed strictly AFTER the successful persist above (T-18-03's
    // load-bearing ordering, see this module's doc comment).
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

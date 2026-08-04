//! Endpoint enforcement tests for `POST /famp/v1/pair/redeem` (Phase 18,
//! PAIR-02/PAIR-03, Plan 02): attempt budget, TTL, single-use, and the
//! persist-before-reply ordering.
//!
//! Complements `pairing_e2e.rs` (Plan 01's happy path + own-domain +
//! absent-store proofs) rather than duplicating it. Every invite fixture
//! is built via the REAL `InviteStore`/`InviteRecord`/`save_atomic` types
//! and written to a `tempfile::tempdir()` — never hand-assembled JSON —
//! and every `expires_at` is a literal string chosen to be either far in
//! the past or far in the future relative to any real wall clock this
//! suite could run under, so no test needs to inject a fake clock: only
//! the crate-internal, `pub(crate)`-scoped `ingest_redemption_at` is
//! clock-injectable (see `pairing_e2e.rs`'s own note on this), and this
//! external test file calls the public `ingest_redemption` wrapper only.

#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;
use std::sync::Arc;

use famp::cli::peer::identity::gateway_identity_path;
use famp::pairing::invite::{pairing_store_path, InviteRecord, InviteState, InviteStore};
use famp::pairing::wordlist::{code_digest, parse_code};
use famp::pairing::{RedemptionReject, RedemptionRequest, Signed};
use famp::FampSigningKey;
use famp_gateway::pairing_ingress::{ingest_redemption, PairingIngressState};
use tempfile::TempDir;

const INVITER_PRINCIPAL: &str = "agent:inviter.test/gateway";
const INVITER_DOMAIN: &str = "inviter.test";
const REDEEMER_PRINCIPAL: &str = "agent:redeemer.test/gateway";

/// Far enough in the past that any real wall clock this suite runs under
/// is already past it.
const FAR_PAST: &str = "2020-01-01T00:00:00Z";
/// Far enough in the future that any real wall clock this suite runs
/// under has not reached it yet.
const FAR_FUTURE: &str = "2099-01-01T00:00:00Z";

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(out, "{b:02x}").unwrap();
    }
    out
}

fn digest_hex(code_str: &str) -> String {
    let code = parse_code(code_str).unwrap();
    hex_encode(&code_digest(&code))
}

fn pairing_state_for(home: &Path, own_domain: &str) -> PairingIngressState {
    PairingIngressState::new(
        Arc::new(pairing_store_path(home)),
        Arc::new(gateway_identity_path(home)),
        Arc::from(own_domain),
    )
}

fn fixture_record(id: &str, code_str: &str, expires_at: &str, attempts: u32) -> InviteRecord {
    InviteRecord {
        id: id.to_string(),
        principal: INVITER_PRINCIPAL.to_string(),
        code_digest: digest_hex(code_str),
        created_at: "2026-08-03T00:00:00Z".to_string(),
        expires_at: expires_at.to_string(),
        attempts,
        state: InviteState::Pending,
    }
}

fn write_store(home: &Path, invites: Vec<InviteRecord>) {
    InviteStore { invites }
        .save_atomic(&pairing_store_path(home))
        .unwrap();
}

/// Signed request for `code_str`, with a fresh keypair. Returns the
/// serialized body and the signing key (for tests that need the
/// verifying key, e.g. none currently, but kept symmetric with
/// `pairing_e2e.rs`'s fixture shape).
fn signed_body(code_str: &str, principal: &str) -> (Vec<u8>, FampSigningKey) {
    let sk = FampSigningKey::generate();
    let vk = sk.verifying_key();
    let request = RedemptionRequest {
        code: code_str.to_string(),
        principal: principal.to_string(),
        pubkey_b64url: vk.to_b64url(),
        nonce: uuid::Uuid::now_v7().to_string(),
    };
    let signed = Signed::new(request, &sk).unwrap();
    (serde_json::to_vec(&signed).unwrap(), sk)
}

const CORRECT_CODE: &str = "abandon ability able about above";
const WRONG_CODE: &str = "absent absorb abstract absurd abuse";

/// The falsification control: the correct code, presented once against a
/// fresh `Pending` invite, succeeds — asserts ONLY that the call returns
/// `Ok`, deliberately NOT that the store was persisted (that is a
/// SEPARATE fact, checked by `redeeming_one_invite_leaves_sibling_invites_untouched`
/// and others). Paired with
/// `replay_of_consumed_code_after_reload_is_rejected` below — under the
/// reverted persist-before-reply ordering this test MUST stay GREEN
/// (the call still returns `Ok` even though nothing was persisted) while
/// the other goes RED (recorded in 18-02-SUMMARY.md). A version of this
/// test that also asserted persisted state would conflate the two facts
/// and defeat its own purpose as a control.
#[tokio::test]
async fn correct_code_first_use_succeeds() {
    let home = TempDir::new().unwrap();
    write_store(
        home.path(),
        vec![fixture_record("inv-1", CORRECT_CODE, FAR_FUTURE, 0)],
    );
    let state = pairing_state_for(home.path(), INVITER_DOMAIN);
    let (body, _sk) = signed_body(CORRECT_CODE, REDEEMER_PRINCIPAL);

    let result = ingest_redemption(&body, &state).await;
    assert!(result.is_ok(), "expected success, got {result:?}");
}

/// T-18-03's core proof: after a successful redemption, dropping every
/// in-memory handle and re-loading the store from disk, replaying the
/// IDENTICAL request is rejected `already_redeemed` — the persist
/// happened before the first call ever returned, so a process kill
/// between persist and reply cannot resurrect the invite.
#[tokio::test]
async fn replay_of_consumed_code_after_reload_is_rejected() {
    let home = TempDir::new().unwrap();
    write_store(
        home.path(),
        vec![fixture_record("inv-1", CORRECT_CODE, FAR_FUTURE, 0)],
    );
    let state = pairing_state_for(home.path(), INVITER_DOMAIN);
    let (body, _sk) = signed_body(CORRECT_CODE, REDEEMER_PRINCIPAL);

    let first = ingest_redemption(&body, &state).await;
    assert!(first.is_ok(), "first redemption must succeed: {first:?}");

    // Drop everything, re-load fresh from disk (simulates a restart
    // between the persist and any further activity), then replay the
    // identical request.
    drop(state);
    let reloaded_state = pairing_state_for(home.path(), INVITER_DOMAIN);
    let second = ingest_redemption(&body, &reloaded_state).await;
    let Err(RedemptionReject { reason }) = second else {
        panic!("expected already_redeemed on replay, got {second:?}");
    };
    assert_eq!(reason, "already_redeemed");
}

/// Attempts 1-5 with wrong codes each burn exactly one attempt, ending at
/// `MAX_ATTEMPTS` (5); attempt 6 is rejected `attempts_exhausted` and
/// leaves `attempts` at 5 — the counter does not run away.
#[tokio::test]
async fn wrong_code_burns_one_attempt_each_up_to_max_then_exhausted() {
    let home = TempDir::new().unwrap();
    write_store(
        home.path(),
        vec![fixture_record("inv-1", CORRECT_CODE, FAR_FUTURE, 0)],
    );
    let state = pairing_state_for(home.path(), INVITER_DOMAIN);
    let store_path = pairing_store_path(home.path());

    for attempt in 1..=5u32 {
        let (body, _sk) = signed_body(WRONG_CODE, REDEEMER_PRINCIPAL);
        let result = ingest_redemption(&body, &state).await;
        let Err(RedemptionReject { reason }) = result else {
            panic!("attempt {attempt}: expected a rejection, got {result:?}");
        };
        assert_eq!(reason, "code_mismatch", "attempt {attempt}");
        let store = InviteStore::load(&store_path).unwrap();
        assert_eq!(
            store.invites[0].attempts, attempt,
            "attempt {attempt}: attempts counter mismatch"
        );
    }

    // 6th wrong-code attempt: attempts_exhausted, counter frozen at 5.
    let (body, _sk) = signed_body(WRONG_CODE, REDEEMER_PRINCIPAL);
    let result = ingest_redemption(&body, &state).await;
    let Err(RedemptionReject { reason }) = result else {
        panic!("6th attempt: expected a rejection, got {result:?}");
    };
    assert_eq!(reason, "attempts_exhausted");
    let store = InviteStore::load(&store_path).unwrap();
    assert_eq!(
        store.invites[0].attempts, 5,
        "the counter must not run away past MAX_ATTEMPTS"
    );
}

/// T-18-12: the CORRECT code presented after `attempts` has reached
/// `MAX_ATTEMPTS` is ALSO rejected `attempts_exhausted`, and the record's
/// state stays `Pending` (never transitions to `Redeemed`).
#[tokio::test]
async fn correct_code_after_attempts_exhausted_is_still_refused() {
    let home = TempDir::new().unwrap();
    write_store(
        home.path(),
        vec![fixture_record("inv-1", CORRECT_CODE, FAR_FUTURE, 5)],
    );
    let state = pairing_state_for(home.path(), INVITER_DOMAIN);
    let (body, _sk) = signed_body(CORRECT_CODE, REDEEMER_PRINCIPAL);

    let result = ingest_redemption(&body, &state).await;
    let Err(RedemptionReject { reason }) = result else {
        panic!("expected attempts_exhausted, got {result:?}");
    };
    assert_eq!(reason, "attempts_exhausted");

    let store = InviteStore::load(&pairing_store_path(home.path())).unwrap();
    assert!(
        matches!(store.invites[0].state, InviteState::Pending),
        "the record must still be Pending, never Redeemed, after budget exhaustion"
    );
}

/// The concurrency proof: two `ingest_redemption` futures driven
/// concurrently against the SAME valid code produce exactly one `Accept`
/// and one `already_redeemed` rejection — the second re-checks record
/// state under `StoreLock` after acquiring it (never trusting the
/// pre-lock read).
#[tokio::test]
async fn concurrent_redemptions_of_same_code_yield_exactly_one_success() {
    let home = TempDir::new().unwrap();
    write_store(
        home.path(),
        vec![fixture_record("inv-1", CORRECT_CODE, FAR_FUTURE, 0)],
    );
    let state = pairing_state_for(home.path(), INVITER_DOMAIN);
    let (body_a, _sk_a) = signed_body(CORRECT_CODE, REDEEMER_PRINCIPAL);
    let (body_b, _sk_b) = signed_body(CORRECT_CODE, "agent:redeemer2.test/gateway");

    let (result_a, result_b) = tokio::join!(
        ingest_redemption(&body_a, &state),
        ingest_redemption(&body_b, &state)
    );

    let ok_count = usize::from(result_a.is_ok()) + usize::from(result_b.is_ok());
    assert_eq!(
        ok_count, 1,
        "expected exactly one Accept, got a={result_a:?} b={result_b:?}"
    );
    let rejections: Vec<&RedemptionReject> = [result_a.as_ref(), result_b.as_ref()]
        .into_iter()
        .filter_map(std::result::Result::err)
        .collect();
    assert_eq!(rejections.len(), 1);
    assert_eq!(rejections[0].reason, "already_redeemed");
}

/// Every rejection path except wrong-code leaves `pairing.json`
/// byte-identical — proven by before/after byte reads, not by an `Err`
/// return.
#[tokio::test]
async fn non_wrong_code_rejections_leave_store_byte_identical() {
    // expired
    {
        let home = TempDir::new().unwrap();
        write_store(
            home.path(),
            vec![fixture_record("inv-1", CORRECT_CODE, FAR_PAST, 0)],
        );
        let state = pairing_state_for(home.path(), INVITER_DOMAIN);
        let store_path = pairing_store_path(home.path());
        let before = std::fs::read(&store_path).unwrap();
        let (body, _sk) = signed_body(CORRECT_CODE, REDEEMER_PRINCIPAL);
        let result = ingest_redemption(&body, &state).await;
        let Err(RedemptionReject { reason }) = result else {
            panic!("expected expired, got {result:?}");
        };
        assert_eq!(reason, "expired");
        let after = std::fs::read(&store_path).unwrap();
        assert_eq!(before, after, "expired rejection must not mutate the store");
    }

    // already_redeemed
    {
        let home = TempDir::new().unwrap();
        let mut record = fixture_record("inv-1", CORRECT_CODE, FAR_FUTURE, 0);
        record.state = InviteState::Redeemed {
            by: REDEEMER_PRINCIPAL.to_string(),
            key_id: "key-1".to_string(),
            pubkey_b64url: "pk".to_string(),
            at: "2026-08-03T00:01:00Z".to_string(),
        };
        write_store(home.path(), vec![record]);
        let state = pairing_state_for(home.path(), INVITER_DOMAIN);
        let store_path = pairing_store_path(home.path());
        let before = std::fs::read(&store_path).unwrap();
        let (body, _sk) = signed_body(CORRECT_CODE, REDEEMER_PRINCIPAL);
        let result = ingest_redemption(&body, &state).await;
        let Err(RedemptionReject { reason }) = result else {
            panic!("expected already_redeemed, got {result:?}");
        };
        assert_eq!(reason, "already_redeemed");
        let after = std::fs::read(&store_path).unwrap();
        assert_eq!(
            before, after,
            "already_redeemed rejection must not mutate the store"
        );
    }

    // attempts_exhausted
    {
        let home = TempDir::new().unwrap();
        write_store(
            home.path(),
            vec![fixture_record("inv-1", CORRECT_CODE, FAR_FUTURE, 5)],
        );
        let state = pairing_state_for(home.path(), INVITER_DOMAIN);
        let store_path = pairing_store_path(home.path());
        let before = std::fs::read(&store_path).unwrap();
        let (body, _sk) = signed_body(CORRECT_CODE, REDEEMER_PRINCIPAL);
        let result = ingest_redemption(&body, &state).await;
        let Err(RedemptionReject { reason }) = result else {
            panic!("expected attempts_exhausted, got {result:?}");
        };
        assert_eq!(reason, "attempts_exhausted");
        let after = std::fs::read(&store_path).unwrap();
        assert_eq!(
            before, after,
            "attempts_exhausted rejection must not mutate the store"
        );
    }

    // no_pending_invite (empty store)
    {
        let home = TempDir::new().unwrap();
        write_store(home.path(), vec![]);
        let state = pairing_state_for(home.path(), INVITER_DOMAIN);
        let store_path = pairing_store_path(home.path());
        let before = std::fs::read(&store_path).unwrap();
        let (body, _sk) = signed_body(CORRECT_CODE, REDEEMER_PRINCIPAL);
        let result = ingest_redemption(&body, &state).await;
        let Err(RedemptionReject { reason }) = result else {
            panic!("expected no_pending_invite, got {result:?}");
        };
        assert_eq!(reason, "no_pending_invite");
        let after = std::fs::read(&store_path).unwrap();
        assert_eq!(
            before, after,
            "no_pending_invite rejection must not mutate the store"
        );
    }
}

/// Redeeming one of several outstanding invites transitions exactly that
/// record to `Redeemed` and leaves every sibling record untouched,
/// including its own `attempts` counter.
#[tokio::test]
async fn redeeming_one_invite_leaves_sibling_invites_untouched() {
    const OTHER_CODE: &str = "access accident account accuse achieve";
    let home = TempDir::new().unwrap();
    write_store(
        home.path(),
        vec![
            fixture_record("inv-1", CORRECT_CODE, FAR_FUTURE, 0),
            fixture_record("inv-2", OTHER_CODE, FAR_FUTURE, 2),
        ],
    );
    let state = pairing_state_for(home.path(), INVITER_DOMAIN);
    let (body, _sk) = signed_body(CORRECT_CODE, REDEEMER_PRINCIPAL);

    let result = ingest_redemption(&body, &state).await;
    assert!(result.is_ok(), "expected success, got {result:?}");

    let store = InviteStore::load(&pairing_store_path(home.path())).unwrap();
    assert!(matches!(
        store.invites[0].state,
        InviteState::Redeemed { .. }
    ));
    assert!(
        matches!(store.invites[1].state, InviteState::Pending),
        "sibling invite must stay Pending"
    );
    assert_eq!(
        store.invites[1].attempts, 2,
        "sibling invite's attempts counter must be untouched"
    );
}

/// A `Revoked` invite never matches any presented code, regardless of
/// digest equality — it is treated identically to "no invite outstanding".
#[tokio::test]
async fn revoked_invite_never_matches_its_own_code() {
    let home = TempDir::new().unwrap();
    let mut record = fixture_record("inv-1", CORRECT_CODE, FAR_FUTURE, 0);
    record.state = InviteState::Revoked {
        at: "2026-08-03T00:01:00Z".to_string(),
    };
    write_store(home.path(), vec![record]);
    let state = pairing_state_for(home.path(), INVITER_DOMAIN);
    let (body, _sk) = signed_body(CORRECT_CODE, REDEEMER_PRINCIPAL);

    let result = ingest_redemption(&body, &state).await;
    let Err(RedemptionReject { reason }) = result else {
        panic!("expected no_pending_invite for a revoked invite, got {result:?}");
    };
    assert_eq!(reason, "no_pending_invite");
}

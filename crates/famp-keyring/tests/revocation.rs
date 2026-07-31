//! Phase 15 Plan 04 (REVK-02/REVK-03): signed revocation statements and the
//! D15-B authorized-signer rule.
//!
//! Covers every case in the plan's `<behavior>` block: an authorized
//! self-signed revocation transitioning an entry to `Revoked`; the four
//! fail-closed cases (tampered statement, unauthorized signer, unknown
//! principal, unknown key_id) each proven via a byte-identical keyring
//! save, not merely an `Err`; non-canonical-timestamp rejection; monotonic
//! idempotence; the `to_blob`/`from_blob` round-trip and its
//! `deny_unknown_fields` gate; the local unilateral `Keyring::revoke` path;
//! the "only active key revoked leaves the principal unverifiable" outcome;
//! and the resurrection guard shared with `rotate_to`/`pin_tofu`/`retire`.
//!
//! Every `now`/`revoked_at` is a literal canonical string — no test reads a
//! clock.

#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use std::io::Write;
use std::str::FromStr;

use famp_core::Principal;
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_keyring::revocation::{authorized_signer_for, RevocationStatement, SignedRevocation};
use famp_keyring::{KeyLookupOutcome, KeyState, Keyring, KeyringError};

const NOW: &str = "2026-07-31T12:00:00Z";
const REVOKED_AT: &str = "2026-07-31T13:00:00Z";

fn alice() -> Principal {
    Principal::from_str("agent:local/alice").unwrap()
}

/// Fresh keyring with `alice` pinned to a fresh generated keypair. Returns
/// the keyring, the signing key, and the verifying key.
fn keyring_with_active_alice() -> (Keyring, FampSigningKey, TrustedVerifyingKey) {
    let sk = FampSigningKey::generate();
    let vk = sk.verifying_key();
    let mut k = Keyring::new();
    k.pin_tofu(alice(), vk.clone()).unwrap();
    (k, sk, vk)
}

fn statement_for(principal: &Principal, key_id: &str, revoked_at: &str) -> RevocationStatement {
    RevocationStatement {
        principal: principal.to_string(),
        revoked_key_id: key_id.to_string(),
        revoked_at: revoked_at.to_string(),
        reason: Some("compromised".to_string()),
    }
}

/// Save `k` to a fresh temp file and return its bytes — used to prove a
/// failed call left the keyring byte-identical (mirrors
/// `rotation.rs`'s convention).
fn snapshot(k: &Keyring) -> Vec<u8> {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    k.save_to_file(tmp.path()).unwrap();
    std::fs::read(tmp.path()).unwrap()
}

#[test]
fn apply_signed_revocation_by_authorized_key_transitions_entry_and_blocks_future_verify() {
    let (mut k, sk, vk) = keyring_with_active_alice();
    let key_id = famp_crypto::key_id(&vk);
    let statement = statement_for(&alice(), &key_id, REVOKED_AT);
    let signed = statement.sign(&sk).unwrap();

    k.apply_signed_revocation(&signed, NOW).unwrap();

    let entries = k.entries(&alice());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].state(), KeyState::Revoked);
    assert_eq!(entries[0].state_since(), Some(REVOKED_AT));

    match k.active_key(&alice(), "2099-01-01T00:00:00Z") {
        KeyLookupOutcome::Revoked { revoked_at } => assert_eq!(revoked_at, Some(REVOKED_AT)),
        other => panic!("expected Revoked, got {other:?}"),
    }
}

#[test]
fn revk02_tampered_statement_fails_closed_and_mutates_nothing() {
    let (mut k, sk, vk) = keyring_with_active_alice();
    let key_id = famp_crypto::key_id(&vk);
    let statement = statement_for(&alice(), &key_id, REVOKED_AT);
    let mut signed = statement.sign(&sk).unwrap();
    // Mutate the statement AFTER signing — the sig no longer matches.
    signed.statement.reason = Some("tampered".to_string());

    let before = snapshot(&k);
    let err = k.apply_signed_revocation(&signed, NOW).unwrap_err();
    assert!(
        matches!(err, KeyringError::RevocationSignatureInvalid { .. }),
        "expected RevocationSignatureInvalid, got {err:?}"
    );
    let after = snapshot(&k);
    assert_eq!(before, after, "a tampered statement must mutate nothing");
}

#[test]
fn revk02_unauthorized_signer_fails_closed_and_mutates_nothing() {
    // Alice's ONLY entry is already `revoked` (loaded from disk — there is
    // no other public API to construct this state). No `Active` entry
    // exists, and the target itself is excluded once already `Revoked`, so
    // `authorized_signer_for` returns an EMPTY candidate list for any
    // fresh attempt to revoke it again with a new signature.
    let sk = FampSigningKey::generate();
    let vk = sk.verifying_key();
    let key_id = famp_crypto::key_id(&vk);
    let content = format!(
        "agent:local/alice  {}  revoked  -  -  {NOW}\n",
        vk.to_b64url()
    );
    let tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.as_file().write_all(content.as_bytes()).unwrap();
    let mut k = Keyring::load_from_file(tmp.path()).unwrap();

    // Sign a NEW statement (a fresh signing attempt, not a replay of the
    // statement that produced this state) with the very key being
    // targeted — even self-signing cannot authorize this, since the
    // candidate set is empty once the target is already revoked and no
    // Active sibling exists.
    let statement = statement_for(&alice(), &key_id, "2026-08-01T00:00:00Z");
    let signed = statement.sign(&sk).unwrap();

    let before = snapshot(&k);
    let err = k.apply_signed_revocation(&signed, NOW).unwrap_err();
    assert!(
        matches!(err, KeyringError::RevocationSignerNotAuthorized { .. }),
        "expected RevocationSignerNotAuthorized, got {err:?}"
    );
    let after = snapshot(&k);
    assert_eq!(before, after, "an unauthorized signer must mutate nothing");
}

#[test]
fn revk02_unknown_principal_fails_closed_and_mutates_nothing() {
    let (mut k, _sk, _vk) = keyring_with_active_alice();
    let unrelated_sk = FampSigningKey::generate();
    let mallory: Principal = "agent:local/mallory".parse().unwrap();
    let statement = statement_for(&mallory, "deadbeefdeadbeef", REVOKED_AT);
    let signed = statement.sign(&unrelated_sk).unwrap();

    let before = snapshot(&k);
    let err = k.apply_signed_revocation(&signed, NOW).unwrap_err();
    assert!(
        matches!(err, KeyringError::NoSuchKeyEntry { .. }),
        "expected NoSuchKeyEntry, got {err:?}"
    );
    let after = snapshot(&k);
    assert_eq!(before, after, "an unknown principal must mutate nothing");
}

#[test]
fn revk02_unknown_key_id_fails_closed_and_mutates_nothing() {
    let (mut k, sk, _vk) = keyring_with_active_alice();
    let statement = statement_for(&alice(), "deadbeefdeadbeef", REVOKED_AT);
    let signed = statement.sign(&sk).unwrap();

    let before = snapshot(&k);
    let err = k.apply_signed_revocation(&signed, NOW).unwrap_err();
    assert!(
        matches!(err, KeyringError::NoSuchKeyEntry { .. }),
        "expected NoSuchKeyEntry, got {err:?}"
    );
    let after = snapshot(&k);
    assert_eq!(before, after, "an unknown key_id must mutate nothing");
}

#[test]
fn apply_signed_revocation_rejects_non_canonical_revoked_at() {
    let (mut k, sk, vk) = keyring_with_active_alice();
    let key_id = famp_crypto::key_id(&vk);
    let statement = statement_for(&alice(), &key_id, "2026-07-31T13:00:00.123Z");
    let signed = statement.sign(&sk).unwrap();

    let before = snapshot(&k);
    let err = k.apply_signed_revocation(&signed, NOW).unwrap_err();
    assert!(
        matches!(err, KeyringError::NonCanonicalTimestamp { .. }),
        "expected NonCanonicalTimestamp, got {err:?}"
    );
    let after = snapshot(&k);
    assert_eq!(
        before, after,
        "a non-canonical revoked_at must mutate nothing"
    );
}

/// Idempotent replay: the same statement, applied twice, is a no-op on the
/// second call. Constructed so the signer (Active key A) is NOT the target
/// being revoked (a distinct, already-`retired` key B) — after the first
/// call B is `Revoked` but A is still `Active`, so the SAME signature by A
/// re-verifies successfully on replay, and `Keyring::revoke`'s own
/// already-revoked short-circuit makes the second call a true no-op.
#[test]
fn apply_signed_revocation_applied_twice_is_idempotent() {
    let sk_a = FampSigningKey::generate();
    let vk_a = sk_a.verifying_key();
    let sk_b = FampSigningKey::generate();
    let vk_b = sk_b.verifying_key();
    let key_id_b = famp_crypto::key_id(&vk_b);

    let content = format!(
        "agent:local/alice  {}  active  -  -  -\nagent:local/alice  {}  retired  -  -  2026-07-01T00:00:00Z\n",
        vk_a.to_b64url(),
        vk_b.to_b64url(),
    );
    let tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.as_file().write_all(content.as_bytes()).unwrap();
    let mut k = Keyring::load_from_file(tmp.path()).unwrap();

    let statement = statement_for(&alice(), &key_id_b, REVOKED_AT);
    let signed = statement.sign(&sk_a).unwrap();

    k.apply_signed_revocation(&signed, NOW)
        .expect("first application must succeed");
    let after_first = snapshot(&k);

    k.apply_signed_revocation(&signed, NOW)
        .expect("second application of the same statement must be idempotent Ok");
    let after_second = snapshot(&k);

    assert_eq!(
        after_first, after_second,
        "applying the same statement twice must produce no further state change"
    );
}

#[test]
fn signed_revocation_blob_round_trips() {
    let (_, sk, vk) = keyring_with_active_alice();
    let key_id = famp_crypto::key_id(&vk);
    let statement = statement_for(&alice(), &key_id, REVOKED_AT);
    let signed = statement.sign(&sk).unwrap();

    let blob = signed.to_blob().unwrap();
    let parsed = SignedRevocation::from_blob(&blob).unwrap();

    assert_eq!(parsed, signed);
}

#[test]
fn from_blob_rejects_unknown_field() {
    let (_, sk, vk) = keyring_with_active_alice();
    let key_id = famp_crypto::key_id(&vk);
    let statement = statement_for(&alice(), &key_id, REVOKED_AT);
    let signed = statement.sign(&sk).unwrap();
    let blob = signed.to_blob().unwrap();

    let mut value: serde_json::Value = serde_json::from_str(&blob).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("bogus_field".to_string(), serde_json::json!("x"));
    let corrupted = serde_json::to_string(&value).unwrap();

    let err = SignedRevocation::from_blob(&corrupted).unwrap_err();
    assert!(
        matches!(err, KeyringError::RevocationBlobMalformed { .. }),
        "expected RevocationBlobMalformed, got {err:?}"
    );
}

#[test]
fn keyring_revoke_local_unilateral_needs_no_signature() {
    let (mut k, _sk, vk) = keyring_with_active_alice();
    let key_id = famp_crypto::key_id(&vk);

    k.revoke(&alice(), &key_id, REVOKED_AT).unwrap();

    let entries = k.entries(&alice());
    assert_eq!(entries[0].state(), KeyState::Revoked);
    assert_eq!(entries[0].state_since(), Some(REVOKED_AT));
}

#[test]
fn revoke_only_active_key_leaves_principal_unverifiable_with_no_fallback() {
    let (mut k, _sk, vk) = keyring_with_active_alice();
    let key_id = famp_crypto::key_id(&vk);

    k.revoke(&alice(), &key_id, REVOKED_AT).unwrap();

    match k.active_key(&alice(), "2099-01-01T00:00:00Z") {
        KeyLookupOutcome::Revoked { .. } => {}
        other => panic!("expected Revoked with no fallback, got {other:?}"),
    }
}

#[test]
fn revoke_is_idempotent_on_an_already_revoked_entry() {
    let (mut k, _sk, vk) = keyring_with_active_alice();
    let key_id = famp_crypto::key_id(&vk);

    k.revoke(&alice(), &key_id, REVOKED_AT).unwrap();
    let after_first = snapshot(&k);
    k.revoke(&alice(), &key_id, "2026-09-01T00:00:00Z")
        .expect("revoking an already-revoked entry must be Ok, not an error");
    let after_second = snapshot(&k);

    assert_eq!(
        after_first, after_second,
        "re-revoking an already-revoked entry must not change state_since"
    );
}

#[test]
fn revoked_entry_cannot_be_resurrected_via_rotate_pin_or_retire() {
    let (mut k, _sk, vk) = keyring_with_active_alice();
    let key_id = famp_crypto::key_id(&vk);
    k.revoke(&alice(), &key_id, REVOKED_AT).unwrap();

    let rotate_err = k
        .rotate_to(alice(), vk.clone(), "2026-08-01T00:00:00Z", None, true)
        .unwrap_err();
    assert!(matches!(rotate_err, KeyringError::KeyRevoked { .. }));

    let pin_err = k.pin_tofu(alice(), vk).unwrap_err();
    assert!(matches!(pin_err, KeyringError::KeyRevoked { .. }));

    let retire_err = k.retire(&alice(), &key_id).unwrap_err();
    assert!(matches!(retire_err, KeyringError::CannotRetire { .. }));
}

#[test]
fn self_revocation_of_sole_active_key_is_an_authorized_candidate() {
    let (k, _sk, vk) = keyring_with_active_alice();
    let key_id = famp_crypto::key_id(&vk);

    let candidates = authorized_signer_for(&k, &alice(), &key_id);
    assert_eq!(
        candidates.len(),
        1,
        "the sole active key must be its own authorized candidate (D15-B signer-self-allowed)"
    );
    assert_eq!(candidates[0].as_bytes(), vk.as_bytes());
}

#[test]
fn revoked_entry_is_never_an_authorized_signer_candidate() {
    let sk = FampSigningKey::generate();
    let vk = sk.verifying_key();
    let key_id = famp_crypto::key_id(&vk);
    let content = format!(
        "agent:local/alice  {}  revoked  -  -  {NOW}\n",
        vk.to_b64url()
    );
    let tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.as_file().write_all(content.as_bytes()).unwrap();
    let k = Keyring::load_from_file(tmp.path()).unwrap();

    let candidates = authorized_signer_for(&k, &alice(), &key_id);
    assert!(
        candidates.is_empty(),
        "a Revoked entry must never be an authorized signer candidate"
    );
}

//! Phase 15 Plan 03 (KEYR-02/KEYR-03): `Keyring::rotate_to` / `Keyring::retire`.
//!
//! Covers every case in the plan's `<behavior>` block: first-pin,
//! idempotent re-pin, an unconfirmed key change (with byte-identity proof
//! that a refused rotation mutates nothing), a confirmed rotation (the
//! previous key is retained as `retired`, never dropped), the
//! revoked-pubkey resurrection guard shared with `pin_tofu`, non-canonical
//! timestamp rejection, and `retire`'s active/revoked refusals plus a
//! round-trip through disk.

#![allow(clippy::unwrap_used, unused_crate_dependencies)]

use std::io::Write;
use std::str::FromStr;

use famp_core::Principal;
use famp_crypto::TrustedVerifyingKey;
use famp_keyring::{KeyEntry, KeyState, Keyring, KeyringError, RotationOutcome};

const KEY_A: &str = "iojj3XQJ8ZX9UtstPLpdcspnCb8dlBIb83SIAbQPb1w";
const KEY_B: &str = "gTl3Dqh9F19Wo1Rmw0x-zMuNipG07jeiXfYPW4_Js5Q";

const NOW: &str = "2026-07-31T12:00:00Z";
const LATER: &str = "2026-08-01T00:00:00Z";
const VALID_UNTIL: &str = "2027-01-15T00:00:00Z";

fn key(b64url: &str) -> TrustedVerifyingKey {
    TrustedVerifyingKey::from_b64url(b64url).unwrap()
}

fn alice() -> Principal {
    Principal::from_str("agent:local/alice").unwrap()
}

/// Build a single-line keyring file with a `revoked` entry for `alice`
/// pinned to `KEY_A`, and load it. Used to exercise the resurrection guard
/// — there is no public API to push a `Revoked` entry other than loading
/// one from disk in the D15-A on-disk shape.
fn revoked_alice_keyring() -> Keyring {
    let content = format!("agent:local/alice  {KEY_A}  revoked  -  -  {NOW}\n");
    let tmp = tempfile::NamedTempFile::new().unwrap();
    tmp.as_file().write_all(content.as_bytes()).unwrap();
    Keyring::load_from_file(tmp.path()).unwrap()
}

type EntryDescription = (
    String,
    KeyState,
    Option<String>,
    Option<String>,
    Option<String>,
);

fn describe(entries: &[KeyEntry]) -> Vec<EntryDescription> {
    let mut v: Vec<_> = entries
        .iter()
        .map(|e| {
            (
                e.key_id(),
                e.state(),
                e.valid_until().map(str::to_string),
                e.pinned_at().map(str::to_string),
                e.state_since().map(str::to_string),
            )
        })
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

#[test]
fn rotate_to_unknown_principal_is_first_pin() {
    let mut k = Keyring::new();
    let outcome = k.rotate_to(alice(), key(KEY_A), NOW, None, false).unwrap();
    assert_eq!(outcome, RotationOutcome::FirstPin);

    let entries = k.entries(&alice());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].state(), KeyState::Active);
    assert_eq!(entries[0].pinned_at(), Some(NOW));
}

#[test]
fn rotate_to_same_pubkey_already_pinned_is_idempotent() {
    let mut k = Keyring::new();
    k.pin_tofu(alice(), key(KEY_A)).unwrap();

    let outcome = k.rotate_to(alice(), key(KEY_A), NOW, None, false).unwrap();
    assert_eq!(outcome, RotationOutcome::AlreadyPinned);
    assert_eq!(
        k.entries(&alice()).len(),
        1,
        "idempotent re-pin must not mutate"
    );
}

#[test]
fn rotate_to_different_pubkey_unconfirmed_refuses_with_zero_mutation() {
    let mut k = Keyring::new();
    k.pin_tofu(alice(), key(KEY_A)).unwrap();

    let tmp = tempfile::NamedTempFile::new().unwrap();
    k.save_to_file(tmp.path()).unwrap();
    let before = std::fs::read(tmp.path()).unwrap();

    let err = k
        .rotate_to(alice(), key(KEY_B), NOW, None, false)
        .unwrap_err();
    match err {
        KeyringError::KeyChangeRequiresConfirmation {
            principal,
            previous_key_id,
            new_key_id,
        } => {
            assert_eq!(principal, alice());
            assert_eq!(previous_key_id, famp_crypto::key_id(&key(KEY_A)));
            assert_eq!(new_key_id, famp_crypto::key_id(&key(KEY_B)));
        }
        other => panic!("expected KeyChangeRequiresConfirmation, got {other:?}"),
    }

    k.save_to_file(tmp.path()).unwrap();
    let after = std::fs::read(tmp.path()).unwrap();
    assert_eq!(
        before, after,
        "a refused rotation must leave the keyring byte-identical"
    );
}

#[test]
fn rotate_to_different_pubkey_confirmed_retains_previous_as_retired() {
    let mut k = Keyring::new();
    k.pin_tofu(alice(), key(KEY_A)).unwrap();

    let outcome = k
        .rotate_to(alice(), key(KEY_B), NOW, Some(VALID_UNTIL), true)
        .unwrap();
    let (previous_key_id, new_key_id) = match outcome {
        RotationOutcome::Rotated {
            previous_key_id,
            new_key_id,
        } => (previous_key_id, new_key_id),
        other => panic!("expected Rotated, got {other:?}"),
    };
    assert_eq!(previous_key_id, famp_crypto::key_id(&key(KEY_A)));
    assert_eq!(new_key_id, famp_crypto::key_id(&key(KEY_B)));

    let entries = k.entries(&alice());
    assert_eq!(entries.len(), 2, "previous key must not be dropped");

    let active = entries
        .iter()
        .find(|e| e.state() == KeyState::Active)
        .unwrap();
    assert_eq!(active.key().as_bytes(), key(KEY_B).as_bytes());
    assert_eq!(active.pinned_at(), Some(NOW));
    assert_eq!(active.valid_until(), Some(VALID_UNTIL));

    let retired = entries
        .iter()
        .find(|e| e.state() == KeyState::Retired)
        .unwrap();
    assert_eq!(retired.key().as_bytes(), key(KEY_A).as_bytes());
    assert_eq!(retired.state_since(), Some(NOW));
}

#[test]
fn active_key_returns_new_key_after_confirmed_rotation_never_the_old_bytes() {
    let mut k = Keyring::new();
    k.pin_tofu(alice(), key(KEY_A)).unwrap();
    k.rotate_to(alice(), key(KEY_B), NOW, None, true).unwrap();

    for probe_now in [NOW, LATER] {
        match k.active_key(&alice(), probe_now) {
            famp_keyring::KeyLookupOutcome::Active(vk) => {
                assert_eq!(vk.as_bytes(), key(KEY_B).as_bytes());
            }
            other => panic!("probe_now={probe_now}: expected Active, got {other:?}"),
        }
    }
}

#[test]
fn rotate_to_a_revoked_pubkey_is_refused_regardless_of_confirmed() {
    for confirmed in [false, true] {
        let mut k = revoked_alice_keyring();
        let err = k
            .rotate_to(alice(), key(KEY_A), LATER, None, confirmed)
            .unwrap_err();
        assert!(
            matches!(err, KeyringError::KeyRevoked { .. }),
            "confirmed={confirmed}: expected KeyRevoked, got {err:?}"
        );
    }
}

#[test]
fn pin_tofu_refuses_a_revoked_pubkey_too() {
    let mut k = revoked_alice_keyring();
    let err = k.pin_tofu(alice(), key(KEY_A)).unwrap_err();
    assert!(matches!(err, KeyringError::KeyRevoked { .. }));
}

#[test]
fn rotate_to_with_non_canonical_now_refuses_and_mutates_nothing() {
    let mut k = Keyring::new();
    k.pin_tofu(alice(), key(KEY_A)).unwrap();
    let err = k
        .rotate_to(alice(), key(KEY_B), "2026-07-31T12:00:00.5Z", None, true)
        .unwrap_err();
    assert!(matches!(err, KeyringError::NonCanonicalTimestamp { .. }));
    assert_eq!(k.entries(&alice()).len(), 1);
}

#[test]
fn rotate_to_with_non_canonical_valid_until_refuses_and_mutates_nothing() {
    let mut k = Keyring::new();
    k.pin_tofu(alice(), key(KEY_A)).unwrap();
    let err = k
        .rotate_to(
            alice(),
            key(KEY_B),
            NOW,
            Some("2027-01-15T00:00:00+00:00"),
            true,
        )
        .unwrap_err();
    assert!(matches!(err, KeyringError::NonCanonicalTimestamp { .. }));
    assert_eq!(k.entries(&alice()).len(), 1);
}

#[test]
fn retire_removes_a_retired_entry() {
    let mut k = Keyring::new();
    k.pin_tofu(alice(), key(KEY_A)).unwrap();
    k.rotate_to(alice(), key(KEY_B), NOW, None, true).unwrap();
    assert_eq!(k.entries(&alice()).len(), 2);

    let retired_key_id = famp_crypto::key_id(&key(KEY_A));
    k.retire(&alice(), &retired_key_id).unwrap();

    let entries = k.entries(&alice());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key().as_bytes(), key(KEY_B).as_bytes());

    let tmp = tempfile::NamedTempFile::new().unwrap();
    k.save_to_file(tmp.path()).unwrap();
    let saved = std::fs::read_to_string(tmp.path()).unwrap();
    assert!(
        !saved.contains(KEY_A),
        "retired key material must no longer be emitted by save_to_file"
    );
}

#[test]
fn retire_on_active_entry_refuses() {
    let mut k = Keyring::new();
    k.pin_tofu(alice(), key(KEY_A)).unwrap();
    let key_id = famp_crypto::key_id(&key(KEY_A));

    let err = k.retire(&alice(), &key_id).unwrap_err();
    match err {
        KeyringError::CannotRetire { state, .. } => assert_eq!(state, "active"),
        other => panic!("expected CannotRetire, got {other:?}"),
    }
    assert_eq!(
        k.entries(&alice()).len(),
        1,
        "a refused retire must not mutate"
    );
}

#[test]
fn retire_on_revoked_entry_refuses() {
    let mut k = revoked_alice_keyring();
    let key_id = famp_crypto::key_id(&key(KEY_A));

    let err = k.retire(&alice(), &key_id).unwrap_err();
    match err {
        KeyringError::CannotRetire { state, .. } => assert_eq!(state, "revoked"),
        other => panic!("expected CannotRetire, got {other:?}"),
    }
}

#[test]
fn retire_with_unknown_key_id_refuses() {
    let mut k = Keyring::new();
    k.pin_tofu(alice(), key(KEY_A)).unwrap();
    let err = k.retire(&alice(), "not-a-real-key-id").unwrap_err();
    assert!(matches!(err, KeyringError::NoSuchKeyEntry { .. }));
}

#[test]
fn rotated_then_saved_keyring_reloads_to_the_identical_entry_set() {
    let mut k = Keyring::new();
    k.pin_tofu(alice(), key(KEY_A)).unwrap();
    k.rotate_to(alice(), key(KEY_B), NOW, Some(VALID_UNTIL), true)
        .unwrap();

    let tmp = tempfile::NamedTempFile::new().unwrap();
    k.save_to_file(tmp.path()).unwrap();
    let reloaded = Keyring::load_from_file(tmp.path()).unwrap();

    assert_eq!(
        describe(k.entries(&alice())),
        describe(reloaded.entries(&alice())),
        "a rotated-then-saved keyring must reload to the identical entry set"
    );
}

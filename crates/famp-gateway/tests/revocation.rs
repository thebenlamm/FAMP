//! End-to-end proof (15-02-PLAN.md Task 1): an otherwise-valid,
//! correctly-signed envelope from a principal whose pinned key's validity
//! window has closed is rejected as `ExpiredKey` at `famp-gateway` ingress
//! using the VERIFIER's own wall clock — never a field decoded from the
//! envelope (T-15-02).
//!
//! Every keyring is built by writing a literal file into a
//! `tempfile::tempdir()` and loading it with `Keyring::load_from_file` —
//! never by constructing `KeyEntry`s in memory — because the point of this
//! test is that the on-disk grammar and the trust decision agree. Every
//! `now` is a literal string; no test reads a clock.

#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp::{AuthorityScope, FampSigningKey, MessageId, Principal, Timestamp, UnsignedEnvelope};
use famp_envelope::body::ack::{AckBody, AckDisposition};
use famp_gateway::error::RejectReason;
use famp_gateway::verify::verify_inbound_any;
use famp_keyring::Keyring;
use std::io::Write as _;

/// Build correctly-signed bytes for a single `Ack` envelope from `sk` as
/// `from` to `to`. Content is irrelevant to this test — only the signature
/// and sender identity matter.
fn signed_bytes(sk: &FampSigningKey, from: &Principal, to: &Principal) -> Vec<u8> {
    let id: MessageId = "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5b00".parse().unwrap();
    let ts = Timestamp("2026-06-01T00:00:00Z".to_string());
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

/// Write `content` into a fresh tempdir and load it as a `Keyring`.
fn load_literal_keyring(content: &str) -> Keyring {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.keyring");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
    Keyring::load_from_file(&path).unwrap()
}

fn alice() -> Principal {
    "agent:local/alice".parse().unwrap()
}

fn bob() -> Principal {
    "agent:local/bob".parse().unwrap()
}

/// The falsification control (acceptance criteria): the SAME signed bytes,
/// verified at a `now` BEFORE the window closes, must verify `Ok` — proving
/// the later rejection is caused by the expiry window, not by an unrelated
/// parse failure. A run where both the expired and the not-yet-expired
/// case fail carries zero information.
#[test]
fn expired_pinned_key_rejected_but_same_bytes_verify_ok_before_expiry() {
    let sk = FampSigningKey::from_bytes([70u8; 32]);
    let vk = sk.verifying_key();
    let from = alice();
    let to = bob();
    let bytes = signed_bytes(&sk, &from, &to);

    let content = format!(
        "agent:local/alice  {}  active  2026-01-01T00:00:00Z  -  -\n",
        vk.to_b64url()
    );
    let keyring = load_literal_keyring(&content);

    // Past the window: must reject as ExpiredKey.
    let result = verify_inbound_any(&bytes, &keyring, "2026-06-01T00:00:00Z");
    match result {
        Err(RejectReason::ExpiredKey {
            principal,
            valid_until,
        }) => {
            assert_eq!(principal, from);
            assert_eq!(valid_until, "2026-01-01T00:00:00Z");
        }
        other => panic!("expected ExpiredKey, got {other:?}"),
    }

    // Falsification control: the identical bytes, at an earlier `now`
    // (before the window closes), must verify Ok.
    let control = verify_inbound_any(&bytes, &keyring, "2025-06-01T00:00:00Z");
    assert!(
        control.is_ok(),
        "same bytes at an earlier now must verify Ok, got {control:?}"
    );
}

/// A legacy 2-field line carries no expiry and must never become unusable
/// through age — `Active` at ANY `now`.
#[test]
fn legacy_two_field_entry_verifies_ok_at_any_now() {
    let sk = FampSigningKey::from_bytes([71u8; 32]);
    let vk = sk.verifying_key();
    let from = alice();
    let to = bob();
    let bytes = signed_bytes(&sk, &from, &to);

    let content = format!("agent:local/alice  {}\n", vk.to_b64url());
    let keyring = load_literal_keyring(&content);

    for now in ["1970-01-01T00:00:00Z", "2099-01-01T00:00:00Z"] {
        let result = verify_inbound_any(&bytes, &keyring, now);
        assert!(
            result.is_ok(),
            "legacy entry must verify Ok at now={now}, got {result:?}"
        );
    }
}

/// A `revoked` v1.1 entry rejects at any `now`, regardless of expiry.
#[test]
fn revoked_key_rejected_at_any_now() {
    let sk = FampSigningKey::from_bytes([72u8; 32]);
    let vk = sk.verifying_key();
    let from = alice();
    let to = bob();
    let bytes = signed_bytes(&sk, &from, &to);

    let content = format!(
        "agent:local/alice  {}  revoked  -  -  2026-02-01T00:00:00Z\n",
        vk.to_b64url()
    );
    let keyring = load_literal_keyring(&content);

    for now in ["2020-01-01T00:00:00Z", "2099-01-01T00:00:00Z"] {
        let result = verify_inbound_any(&bytes, &keyring, now);
        match result {
            Err(RejectReason::RevokedKey {
                principal,
                revoked_at,
            }) => {
                assert_eq!(principal, from);
                assert_eq!(revoked_at.as_deref(), Some("2026-02-01T00:00:00Z"));
            }
            other => panic!("now={now}: expected RevokedKey, got {other:?}"),
        }
    }
}

/// A `retired` entry with no active sibling fails closed — there is no
/// "most recent key wins" path.
#[test]
fn retired_key_with_no_active_sibling_fails_closed() {
    let sk = FampSigningKey::from_bytes([73u8; 32]);
    let vk = sk.verifying_key();
    let from = alice();
    let to = bob();
    let bytes = signed_bytes(&sk, &from, &to);

    let content = format!(
        "agent:local/alice  {}  retired  -  -  2026-02-01T00:00:00Z\n",
        vk.to_b64url()
    );
    let keyring = load_literal_keyring(&content);

    let result = verify_inbound_any(&bytes, &keyring, "2026-06-01T00:00:00Z");
    assert!(
        result.is_err(),
        "a retired-only principal must fail closed, got {result:?}"
    );
}

/// `active_key` (and therefore the gateway) must never accept a
/// non-canonical `now` (offset form or fractional-second form) —
/// `NonCanonicalNow` maps to a hard reject, never an accept.
#[test]
fn non_canonical_now_never_accepts() {
    let sk = FampSigningKey::from_bytes([74u8; 32]);
    let vk = sk.verifying_key();
    let from = alice();
    let to = bob();
    let bytes = signed_bytes(&sk, &from, &to);

    let content = format!("agent:local/alice  {}\n", vk.to_b64url());
    let keyring = load_literal_keyring(&content);

    for bad_now in ["2026-06-01T00:00:00+00:00", "2026-06-01T00:00:00.123Z", ""] {
        let result = verify_inbound_any(&bytes, &keyring, bad_now);
        assert!(
            result.is_err(),
            "non-canonical now '{bad_now}' must never accept, got {result:?}"
        );
    }
}

/// `clock::now_canonical_utc()` — proven end-to-end here too (unit-tested
/// directly in `clock.rs`): exactly 20 bytes, ends in `Z`.
#[test]
fn now_canonical_utc_shape() {
    let s = famp_gateway::now_canonical_utc();
    assert_eq!(s.len(), 20);
    assert!(s.ends_with('Z'));
}

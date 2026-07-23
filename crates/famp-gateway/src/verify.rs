//! Pure, transport-agnostic gateway ingress verification (WIRE-01, TRUST-02).
//!
//! `verify_inbound` takes `(bytes, &Keyring)` as *input* — data-as-input per
//! D-07, not synthetic wire routing — so it is unit-testable in-process here
//! and Phase 9's HTTP transport handler just feeds it the request body.
//!
//! Flow (the two-pass shape D-07 locks): [`famp_envelope::peek_sender`]
//! extracts the `from` principal WITHOUT verifying anything, then the
//! peeked principal is looked up in the pinned `Keyring` — only once the
//! verifying key is known does [`famp::SignedEnvelope::decode`] run
//! `verify_strict` over the canonical bytes. There is no raw
//! `ed25519_dalek::VerifyingKey` construction anywhere on this path; the
//! only crypto surface touched is `TrustedVerifyingKey` /
//! `SignedEnvelope::decode` (`famp-crypto`'s `verify_strict`-only contract).
//!
//! D-08: on EITHER reject path this function performs zero local-bus writes
//! and zero pinned/registry state mutation — it is a pure `Result`-returning
//! function. TRUST-02: an unpinned sender key is a hard reject with no
//! auto-pin, no fallback trust path.

use crate::error::RejectReason;
use famp::SignedEnvelope;
use famp_envelope::body::BodySchema;
use famp_envelope::peek_sender;
use famp_keyring::Keyring;

/// Verify inbound cross-host envelope bytes against the pinned keyring.
///
/// Two-pass flow (D-07): peek the sender principal from unverified bytes,
/// look it up in `keyring` (TRUST-02 hard-reject gate on `None` — no
/// auto-pin, no fallback), then `SignedEnvelope::decode` (which runs
/// `verify_strict` internally) against the pinned key. Returns the typed,
/// verified `SignedEnvelope<B>` on success, or one of two distinct
/// [`RejectReason`]s (D-08) on failure. Performs no I/O and mutates no
/// state on any path.
pub fn verify_inbound<B: BodySchema>(
    bytes: &[u8],
    keyring: &Keyring,
) -> Result<SignedEnvelope<B>, RejectReason> {
    let from = peek_sender(bytes).map_err(|_| RejectReason::InvalidSignature)?;
    let Some(vk) = keyring.get(&from) else {
        return Err(RejectReason::UnpinnedKey { principal: from });
    };
    SignedEnvelope::decode(bytes, vk).map_err(|_| RejectReason::InvalidSignature)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use famp::{AuthorityScope, FampSigningKey, MessageId, Principal, Timestamp};
    use famp_envelope::body::ack::{AckBody, AckDisposition};
    use famp_envelope::UnsignedEnvelope;

    fn signed_bytes(sk: &FampSigningKey, from: &Principal, to: &Principal) -> Vec<u8> {
        let id: MessageId = "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a6b".parse().unwrap();
        let ts = Timestamp("2026-07-23T00:00:00Z".to_string());
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
        let signed = unsigned.sign(sk).unwrap();
        signed.encode().unwrap()
    }

    fn strip_signature(bytes: &[u8]) -> Vec<u8> {
        let mut value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        value.as_object_mut().unwrap().remove("signature");
        serde_json::to_vec(&value).unwrap()
    }

    #[test]
    fn accepts_pinned_valid() {
        let sk = FampSigningKey::from_bytes([1u8; 32]);
        let vk = sk.verifying_key();
        let from: Principal = "agent:example.test/alice".parse().unwrap();
        let to: Principal = "agent:example.test/bob".parse().unwrap();
        let bytes = signed_bytes(&sk, &from, &to);

        let mut keyring = Keyring::new();
        keyring.pin_tofu(from.clone(), vk).unwrap();
        let len_before = keyring.len();

        let result = verify_inbound::<AckBody>(&bytes, &keyring);
        let envelope = result.expect("pinned-valid envelope must verify");
        assert_eq!(envelope.from_principal(), &from);
        assert_eq!(
            keyring.len(),
            len_before,
            "verify_inbound must not mutate the keyring"
        );
    }

    #[test]
    fn rejects_unsigned() {
        let sk = FampSigningKey::from_bytes([2u8; 32]);
        let vk = sk.verifying_key();
        let from: Principal = "agent:example.test/carol".parse().unwrap();
        let to: Principal = "agent:example.test/dave".parse().unwrap();
        let bytes = signed_bytes(&sk, &from, &to);
        let unsigned_bytes = strip_signature(&bytes);

        let mut keyring = Keyring::new();
        keyring.pin_tofu(from, vk).unwrap();
        let len_before = keyring.len();

        let result = verify_inbound::<AckBody>(&unsigned_bytes, &keyring);
        assert!(matches!(result, Err(RejectReason::InvalidSignature)));
        assert_eq!(
            keyring.len(),
            len_before,
            "reject path must not mutate the keyring"
        );
    }

    #[test]
    fn rejects_bad_signature() {
        let sk = FampSigningKey::from_bytes([3u8; 32]);
        let wrong_sk = FampSigningKey::from_bytes([4u8; 32]);
        let wrong_vk = wrong_sk.verifying_key();
        let from: Principal = "agent:example.test/erin".parse().unwrap();
        let to: Principal = "agent:example.test/frank".parse().unwrap();
        let bytes = signed_bytes(&sk, &from, &to);

        // Sender is pinned to a DIFFERENT key than the one that signed —
        // decode-verify must fail against the pinned (wrong) key.
        let mut keyring = Keyring::new();
        keyring.pin_tofu(from, wrong_vk).unwrap();
        let len_before = keyring.len();

        let result = verify_inbound::<AckBody>(&bytes, &keyring);
        assert!(matches!(result, Err(RejectReason::InvalidSignature)));
        assert_eq!(
            keyring.len(),
            len_before,
            "reject path must not mutate the keyring"
        );
    }

    #[test]
    fn rejects_unpinned_key() {
        let sk = FampSigningKey::from_bytes([5u8; 32]);
        let from: Principal = "agent:example.test/grace".parse().unwrap();
        let to: Principal = "agent:example.test/heidi".parse().unwrap();
        let bytes = signed_bytes(&sk, &from, &to);

        // Empty keyring: sender principal is absent entirely.
        let keyring = Keyring::new();
        let len_before = keyring.len();

        let result = verify_inbound::<AckBody>(&bytes, &keyring);
        match result {
            Err(RejectReason::UnpinnedKey { principal }) => assert_eq!(principal, from),
            other => panic!("expected UnpinnedKey{{ principal }}, got {other:?}"),
        }
        assert_eq!(
            keyring.len(),
            len_before,
            "reject path must not mutate the keyring"
        );
    }
}

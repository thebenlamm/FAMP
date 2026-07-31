//! Pure, transport-agnostic gateway ingress verification (WIRE-01, TRUST-02;
//! extended by REVK-01/REVK-02 in Phase 15).
//!
//! `verify_inbound` takes `(bytes, &Keyring, now)` as *input* — data-as-input
//! per D-07, not synthetic wire routing — so it is unit-testable in-process
//! here and Phase 9's HTTP transport handler just feeds it the request body
//! plus the gateway's own wall-clock reading.
//!
//! Flow (the two-pass shape D-07 locks, extended with a time-aware trust
//! query): [`famp_envelope::peek_sender`] extracts the `from` principal
//! WITHOUT verifying anything, then the peeked principal is looked up via
//! `keyring.active_key(from, now)` — only once a usable, non-expired,
//! non-revoked verifying key is confirmed does
//! [`famp::SignedEnvelope::decode`] run `verify_strict` over the canonical
//! bytes. There is no raw `ed25519_dalek::VerifyingKey` construction
//! anywhere on this path; the only crypto surface touched is
//! `TrustedVerifyingKey` / `SignedEnvelope::decode` (`famp-crypto`'s
//! `verify_strict`-only contract).
//!
//! T-15-02 (critical): the expiry and revocation decision is made
//! EXCLUSIVELY against the verifier-supplied `now` — no field decoded from
//! the envelope itself (`ts`, `expiry`, or anything else) ever participates
//! in this decision. `famp-envelope`'s `ts`/`expiry` fields are
//! sender-asserted and only format-validated; trusting them here would let
//! a compromised sender assert a pre-expiry `ts` forever.
//!
//! D-08: on EVERY reject path this function performs zero local-bus writes
//! and zero pinned/registry state mutation — it is a pure `Result`-returning
//! function. TRUST-02: an unpinned sender key is a hard reject with no
//! auto-pin, no fallback trust path.

use crate::error::RejectReason;
use famp::SignedEnvelope;
use famp_envelope::body::BodySchema;
use famp_envelope::peek_sender;
use famp_envelope::AnySignedEnvelope;
use famp_keyring::{KeyLookupOutcome, Keyring};

/// Map every non-`Active` [`KeyLookupOutcome`] to its [`RejectReason`].
/// `Retired` and `NonCanonicalNow` both fail closed as `UnpinnedKey` —
/// neither implies a usable pinned key exists. Both `verify_inbound` and
/// `verify_inbound_any` call the time-aware lookup directly (not through
/// a shared lookup wrapper) and pass the outcome here, so the trust query
/// itself stays visibly inline at both call sites.
fn reject_for_outcome(from: &famp::Principal, outcome: KeyLookupOutcome<'_>) -> RejectReason {
    match outcome {
        KeyLookupOutcome::Active(_) => unreachable!("Active is handled by the caller before this"),
        KeyLookupOutcome::Unpinned | KeyLookupOutcome::Retired { .. } => {
            RejectReason::UnpinnedKey {
                principal: from.clone(),
            }
        }
        KeyLookupOutcome::Expired { valid_until } => RejectReason::ExpiredKey {
            principal: from.clone(),
            valid_until: valid_until.to_string(),
        },
        KeyLookupOutcome::Revoked { revoked_at } => RejectReason::RevokedKey {
            principal: from.clone(),
            revoked_at: revoked_at.map(str::to_string),
        },
        KeyLookupOutcome::NonCanonicalNow => RejectReason::UnpinnedKey {
            principal: from.clone(),
        },
    }
}

/// Verify inbound cross-host envelope bytes against the pinned keyring at
/// verifier instant `now`.
///
/// Two-pass flow (D-07): peek the sender principal from unverified bytes,
/// resolve its usable key via the time-aware lookup (TRUST-02
/// hard-reject on `Unpinned`; REVK-01 hard-reject on `Expired`; REVK-02
/// hard-reject on `Revoked` — no auto-pin, no fallback), then
/// `SignedEnvelope::decode` (which runs `verify_strict` internally) against
/// the resolved key. Returns the typed, verified `SignedEnvelope<B>` on
/// success, or one of four distinct [`RejectReason`]s (D-08) on failure.
/// Performs no I/O and mutates no state on any path.
pub fn verify_inbound<B: BodySchema>(
    bytes: &[u8],
    keyring: &Keyring,
    now: &str,
) -> Result<SignedEnvelope<B>, RejectReason> {
    let from = peek_sender(bytes).map_err(|_| RejectReason::InvalidSignature)?;
    let vk = match keyring.active_key(&from, now) {
        KeyLookupOutcome::Active(vk) => vk,
        outcome => return Err(reject_for_outcome(&from, outcome)),
    };
    SignedEnvelope::decode(bytes, vk).map_err(|_| RejectReason::InvalidSignature)
}

/// Verify inbound cross-host envelope bytes whose body class is not known
/// in advance, at verifier instant `now`.
///
/// Phase 9 HTTP ingress sees mixed request/commit/deliver/ack traffic —
/// [`verify_inbound`] is generic over exactly one class. Identical
/// time-aware flow and reject contract as [`verify_inbound`] (D-07/D-08) —
/// the only difference is the decode call itself, which routes through
/// [`AnySignedEnvelope::decode`] so the wire `class` field picks the typed
/// body. Performs no I/O and mutates no state on any path.
pub fn verify_inbound_any(
    bytes: &[u8],
    keyring: &Keyring,
    now: &str,
) -> Result<AnySignedEnvelope, RejectReason> {
    let from = peek_sender(bytes).map_err(|_| RejectReason::InvalidSignature)?;
    let vk = match keyring.active_key(&from, now) {
        KeyLookupOutcome::Active(vk) => vk,
        outcome => return Err(reject_for_outcome(&from, outcome)),
    };
    AnySignedEnvelope::decode(bytes, vk).map_err(|_| RejectReason::InvalidSignature)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use famp::{AuthorityScope, FampSigningKey, MessageId, Principal, Timestamp};
    use famp_envelope::body::ack::{AckBody, AckDisposition};
    use famp_envelope::body::{
        Bounds, Budget, CommitBody, DeliverBody, RequestBody, TerminalStatus,
    };
    use famp_envelope::UnsignedEnvelope;

    /// A literal, canonical `now` used across this module's tests. No test
    /// reads a clock — every legacy-pinned key here carries no
    /// `valid_until`, so any canonical `now` verifies `Active`.
    const NOW: &str = "2026-07-27T00:00:00Z";

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

    /// Two-key `Bounds` (v0.5.1 §9.3 requires >= 2 of the 8 fields set;
    /// `Bounds::validate()` rejects an under-specified set).
    fn two_key_bounds() -> Bounds {
        Bounds {
            deadline: Some("2026-07-27T00:00:00Z".to_string()),
            budget: Some(Budget {
                amount: "100".to_string(),
                unit: "usd".to_string(),
            }),
            hop_limit: None,
            policy_domain: None,
            authority_scope: None,
            max_artifact_size: None,
            confidence_floor: None,
            recursion_depth: None,
        }
    }

    fn request_bytes(sk: &FampSigningKey, from: &Principal, to: &Principal) -> Vec<u8> {
        let id: MessageId = "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a6c".parse().unwrap();
        let ts = Timestamp("2026-07-27T00:00:00Z".to_string());
        let body = RequestBody {
            scope: serde_json::json!({"task": "translate"}),
            bounds: two_key_bounds(),
            natural_language_summary: None,
        };
        let unsigned = UnsignedEnvelope::<RequestBody>::new(
            id,
            from.clone(),
            to.clone(),
            AuthorityScope::Advisory,
            ts,
            body,
        );
        unsigned.sign(sk).unwrap().encode().unwrap()
    }

    fn commit_bytes(sk: &FampSigningKey, from: &Principal, to: &Principal) -> Vec<u8> {
        let id: MessageId = "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a6d".parse().unwrap();
        let ts = Timestamp("2026-07-27T00:00:00Z".to_string());
        let body = CommitBody {
            scope: serde_json::json!({"task": "translate"}),
            scope_subset: None,
            bounds: two_key_bounds(),
            accepted_policies: vec!["policy://famp/v0.7/personal".to_string()],
            delegation_permissions: None,
            reporting_obligations: None,
            terminal_condition: serde_json::json!({"type": "final_delivery"}),
            conditions: None,
            natural_language_summary: None,
        };
        let unsigned = UnsignedEnvelope::<CommitBody>::new(
            id,
            from.clone(),
            to.clone(),
            AuthorityScope::CommitLocal,
            ts,
            body,
        );
        unsigned.sign(sk).unwrap().encode().unwrap()
    }

    fn deliver_bytes(sk: &FampSigningKey, from: &Principal, to: &Principal) -> Vec<u8> {
        let id: MessageId = "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a6e".parse().unwrap();
        let ts = Timestamp("2026-07-27T00:00:00Z".to_string());
        let body = DeliverBody {
            interim: false,
            artifacts: None,
            result: Some(serde_json::json!({"text": "Bonjour le monde."})),
            usage_metrics: None,
            error_detail: None,
            provenance: Some(serde_json::json!({"signer": "agent:example.test/bob"})),
            natural_language_summary: None,
        };
        let unsigned = UnsignedEnvelope::<DeliverBody>::new(
            id,
            from.clone(),
            to.clone(),
            AuthorityScope::Advisory,
            ts,
            body,
        )
        .with_terminal_status(TerminalStatus::Completed);
        unsigned.sign(sk).unwrap().encode().unwrap()
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

        let result = verify_inbound::<AckBody>(&bytes, &keyring, NOW);
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

        let result = verify_inbound::<AckBody>(&unsigned_bytes, &keyring, NOW);
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

        let result = verify_inbound::<AckBody>(&bytes, &keyring, NOW);
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

        let result = verify_inbound::<AckBody>(&bytes, &keyring, NOW);
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

    // --- verify_inbound_any: per-class coverage (Task 2, 09-01-PLAN.md) ---
    //
    // Phase 9's HTTP ingress sees mixed classes; `verify_inbound_any` must
    // preserve the exact same two-gate reject contract as `verify_inbound`
    // for EACH of the 4 live classes (request/commit/deliver/ack).

    type BytesFn = fn(&FampSigningKey, &Principal, &Principal) -> Vec<u8>;

    fn all_class_builders() -> [(famp_envelope::MessageClass, BytesFn); 4] {
        [
            (famp_envelope::MessageClass::Request, request_bytes),
            (famp_envelope::MessageClass::Commit, commit_bytes),
            (famp_envelope::MessageClass::Deliver, deliver_bytes),
            (famp_envelope::MessageClass::Ack, signed_bytes),
        ]
    }

    #[test]
    fn verify_inbound_any_accepts_pinned_valid_for_every_class() {
        for (class, builder) in all_class_builders() {
            let sk = FampSigningKey::from_bytes([10u8; 32]);
            let vk = sk.verifying_key();
            let from: Principal = "agent:example.test/ivan".parse().unwrap();
            let to: Principal = "agent:example.test/judy".parse().unwrap();
            let bytes = builder(&sk, &from, &to);

            let mut keyring = Keyring::new();
            keyring.pin_tofu(from.clone(), vk).unwrap();
            let len_before = keyring.len();

            let result = verify_inbound_any(&bytes, &keyring, NOW);
            let envelope = result.unwrap_or_else(|e| panic!("class {class} must verify: {e:?}"));
            assert_eq!(
                envelope.class(),
                class,
                "verify_inbound_any dispatched to the wrong class"
            );
            assert_eq!(
                keyring.len(),
                len_before,
                "verify_inbound_any must not mutate the keyring (class {class})"
            );
        }
    }

    #[test]
    fn verify_inbound_any_rejects_unsigned_for_every_class() {
        for (class, builder) in all_class_builders() {
            let sk = FampSigningKey::from_bytes([11u8; 32]);
            let vk = sk.verifying_key();
            let from: Principal = "agent:example.test/kevin".parse().unwrap();
            let to: Principal = "agent:example.test/laura".parse().unwrap();
            let bytes = builder(&sk, &from, &to);
            let unsigned_bytes = strip_signature(&bytes);

            let mut keyring = Keyring::new();
            keyring.pin_tofu(from, vk).unwrap();
            let len_before = keyring.len();

            let result = verify_inbound_any(&unsigned_bytes, &keyring, NOW);
            assert!(
                matches!(result, Err(RejectReason::InvalidSignature)),
                "class {class}: expected InvalidSignature, got {result:?}"
            );
            assert_eq!(
                keyring.len(),
                len_before,
                "reject path must not mutate the keyring (class {class})"
            );
        }
    }

    #[test]
    fn verify_inbound_any_rejects_unpinned_key_for_every_class() {
        for (class, builder) in all_class_builders() {
            let sk = FampSigningKey::from_bytes([12u8; 32]);
            let from: Principal = "agent:example.test/mallory".parse().unwrap();
            let to: Principal = "agent:example.test/nathan".parse().unwrap();
            let bytes = builder(&sk, &from, &to);

            // Empty keyring: sender principal is absent entirely, checked
            // BEFORE any class-dispatching decode runs.
            let keyring = Keyring::new();
            let len_before = keyring.len();

            let result = verify_inbound_any(&bytes, &keyring, NOW);
            match result {
                Err(RejectReason::UnpinnedKey { principal }) => assert_eq!(principal, from),
                other => {
                    panic!("class {class}: expected UnpinnedKey{{ principal }}, got {other:?}")
                }
            }
            assert_eq!(
                keyring.len(),
                len_before,
                "reject path must not mutate the keyring (class {class})"
            );
        }
    }
}

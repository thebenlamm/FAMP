//! Byte loaders for the three adversarial cases.
//!
//! Every builder is a real implementation — no unimplemented stubs (checker B-5).
//! The unsigned + wrong-key builders are lifted verbatim from the Phase 3
//! monolithic adversarial.rs; the CONF-07 loader reads the committed fixture
//! and falls back to regenerating via the Phase 3 pretty-print trick if the
//! file is missing (matching the Phase 3 test's self-heal behavior).

#![allow(dead_code)]

use super::harness::Case;
use famp_canonical::{canonicalize, from_slice_strict};
use famp_core::{AuthorityScope, MessageId, Principal};
use famp_crypto::FampSigningKey;
use famp_envelope::body::{AckBody, AckDisposition, Bounds, Budget, RequestBody};
use famp_envelope::{SignedEnvelope, Timestamp, UnsignedEnvelope};
use std::path::PathBuf;
use std::str::FromStr;

pub const ALICE_SECRET: [u8; 32] = [1u8; 32];
pub const WRONG_SECRET: [u8; 32] = [99u8; 32];

pub fn alice() -> Principal {
    Principal::from_str("agent:local/alice").unwrap()
}

pub fn bob() -> Principal {
    Principal::from_str("agent:local/bob").unwrap()
}

fn ts() -> Timestamp {
    Timestamp("2026-04-13T00:00:00Z".to_string())
}

fn two_key_bounds() -> Bounds {
    Bounds {
        deadline: Some("2026-05-01T00:00:00Z".to_string()),
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

fn fixed_msg_id() -> MessageId {
    "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a6b".parse().unwrap()
}

fn canonical_bytes<B: famp_envelope::BodySchema>(signed: &SignedEnvelope<B>) -> Vec<u8> {
    let encoded = signed.encode().unwrap();
    let value: serde_json::Value = from_slice_strict(&encoded).unwrap();
    canonicalize(&value).unwrap()
}

/// CONF-05: valid Ack envelope with the `signature` field stripped, then
/// canonicalized. Runtime sees missing signature -> `MissingSignature`.
pub fn build_unsigned_bytes() -> Vec<u8> {
    let alice_sk = FampSigningKey::from_bytes(ALICE_SECRET);
    let body = AckBody {
        disposition: AckDisposition::Accepted,
        reason: None,
    };
    let signed = UnsignedEnvelope::<AckBody>::new(
        fixed_msg_id(),
        alice(),
        bob(),
        AuthorityScope::Advisory,
        ts(),
        body,
    )
    .sign(&alice_sk)
    .unwrap();
    let encoded = signed.encode().unwrap();
    let mut value: serde_json::Value = from_slice_strict(&encoded).unwrap();
    value.as_object_mut().unwrap().remove("signature");
    canonicalize(&value).unwrap()
}

/// CONF-06: RequestBody signed with WRONG_SECRET, from=alice. Bob's keyring
/// holds alice's REAL key, so verify-against-alice fails -> SignatureInvalid.
pub fn build_wrong_key_bytes() -> Vec<u8> {
    let wrong_sk = FampSigningKey::from_bytes(WRONG_SECRET);
    let body = RequestBody {
        scope: serde_json::json!({"task": "translate"}),
        bounds: two_key_bounds(),
        natural_language_summary: None,
    };
    let signed = UnsignedEnvelope::<RequestBody>::new(
        fixed_msg_id(),
        alice(),
        bob(),
        AuthorityScope::Advisory,
        ts(),
        body,
    )
    .sign(&wrong_sk)
    .unwrap();
    canonical_bytes(&signed)
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("conf-07-canonical-divergence.json")
}

/// CONF-07: pretty-printed signed Request envelope. Differs from canonical
/// form byte-wise, so the runtime's canonical pre-check rejects it BEFORE
/// signature verification runs. Reuses the Phase 3 committed fixture
/// byte-identically (D-D4); regenerates it deterministically if missing.
pub fn build_canonical_divergence_bytes() -> Vec<u8> {
    let path = fixture_path();
    if let Ok(bytes) = std::fs::read(&path) {
        return bytes;
    }
    // Regenerate deterministically from ALICE_SECRET.
    let alice_sk = FampSigningKey::from_bytes(ALICE_SECRET);
    let body = RequestBody {
        scope: serde_json::json!({"task": "translate"}),
        bounds: two_key_bounds(),
        natural_language_summary: None,
    };
    let signed = UnsignedEnvelope::<RequestBody>::new(
        fixed_msg_id(),
        alice(),
        bob(),
        AuthorityScope::Advisory,
        ts(),
        body,
    )
    .sign(&alice_sk)
    .unwrap();
    let encoded = signed.encode().unwrap();
    let value: serde_json::Value = from_slice_strict(&encoded).unwrap();
    let pretty = serde_json::to_vec_pretty(&value).unwrap();
    let canonical = canonicalize(&value).unwrap();
    assert_ne!(
        pretty, canonical,
        "CONF-07 fixture generator must produce non-canonical bytes"
    );
    pretty
}

pub fn case_bytes(case: Case) -> Vec<u8> {
    match case {
        Case::Unsigned => build_unsigned_bytes(),
        Case::WrongKey => build_wrong_key_bytes(),
        Case::CanonicalDivergence => build_canonical_divergence_bytes(),
    }
}

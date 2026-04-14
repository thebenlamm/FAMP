//! §7.1c vector 0 byte-exact regression tests.
//!
//! This is the load-bearing interop anchor for the entire crate. If any of
//! these assertions fail, canonicalization or signing-input construction has
//! drifted from `FAMP-v0.5.1-spec.md` §7.1c and interop is broken.
//!
//! Fixture files under `tests/vectors/vector_0/` were committed in Plan 01-01
//! as byte-exact copies of the spec's Python-generated hex listings per
//! PITFALLS P10. Do not regenerate them from this crate.

#![allow(clippy::unwrap_used, clippy::expect_used)]

// dev-deps that must be acknowledged per workspace lint.
use famp_core as _;
use insta as _;
use proptest as _;
use serde as _;
use thiserror as _;

use famp_canonical::{canonicalize, from_slice_strict};
use famp_crypto::{sign_value, FampSigningKey, TrustedVerifyingKey};
use famp_envelope::body::{AckBody, AckDisposition};
use famp_envelope::{AnySignedEnvelope, EnvelopeDecodeError, SignedEnvelope};
use serde_json::Value;
use std::fs;

const VECTOR_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors/vector_0");

// RFC 8032 Test 1 keypair (matches §7.1c).
const SECRET: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];
const PUBLIC: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

fn read_hex(name: &str) -> Vec<u8> {
    let s = fs::read_to_string(format!("{VECTOR_DIR}/{name}")).expect("hex fixture missing");
    hex::decode(s.trim()).expect("hex fixture malformed")
}

fn vector_0_bytes() -> Vec<u8> {
    fs::read(format!("{VECTOR_DIR}/envelope.json")).expect("vector 0 envelope.json missing")
}

fn test1_vk() -> TrustedVerifyingKey {
    TrustedVerifyingKey::from_bytes(&PUBLIC).expect("RFC 8032 Test 1 key must be non-weak")
}

#[test]
fn vector_0_decodes_through_signed_envelope() {
    // §7.1c.7 → §7.1c.8 — decode the signed wire envelope with the Test 1 key.
    let bytes = vector_0_bytes();
    let vk = test1_vk();
    let decoded = SignedEnvelope::<AckBody>::decode(&bytes, &vk)
        .expect("vector 0 must decode through the typed SignedEnvelope path");
    assert_eq!(
        decoded.body().disposition,
        AckDisposition::Accepted,
        "§7.1c.2 ack body must round-trip to disposition=accepted"
    );
    assert_eq!(decoded.body().reason, None);
}

#[test]
fn vector_0_canonical_bytes_byte_exact() {
    // §7.1c.3 — strip signature, run RFC 8785 canonicalization, compare to
    // the 324-byte canonical hex fixture committed in Plan 01-01.
    let bytes = vector_0_bytes();
    let mut value: Value = from_slice_strict(&bytes).unwrap();
    value.as_object_mut().unwrap().remove("signature");
    let canonical = canonicalize(&value).unwrap();
    let expected = read_hex("canonical.hex");
    assert_eq!(
        canonical, expected,
        "§7.1c.3 canonical bytes diverged — RFC 8785 canonicalization regression"
    );
    assert_eq!(canonical.len(), 324, "vector 0 canonical form is 324 bytes");
}

#[test]
fn vector_0_signature_reproduces_byte_exact() {
    // §7.1c.6 — re-sign the stripped envelope with the Test 1 signing key
    // and compare the raw 64-byte signature to the committed fixture.
    // Ed25519 is deterministic (RFC 8032 §5.1.6) so any divergence is a
    // canonicalization or domain-prefix regression.
    let bytes = vector_0_bytes();
    let mut value: Value = from_slice_strict(&bytes).unwrap();
    value.as_object_mut().unwrap().remove("signature");
    let sk = FampSigningKey::from_bytes(SECRET);
    let sig = sign_value(&sk, &value).unwrap();
    let expected = read_hex("signature.hex");
    let produced = sig.to_bytes();
    assert_eq!(
        produced.as_slice(),
        expected.as_slice(),
        "§7.1c.6 signature bytes diverged — signing input regression (canonicalize, domain prefix, or Ed25519 determinism)"
    );
}

#[test]
fn any_signed_envelope_dispatches_vector_0_to_ack() {
    let bytes = vector_0_bytes();
    let vk = test1_vk();
    let decoded = AnySignedEnvelope::decode(&bytes, &vk).unwrap();
    assert!(
        matches!(decoded, AnySignedEnvelope::Ack(_)),
        "vector 0 class=ack must route to AnySignedEnvelope::Ack"
    );
}

#[test]
fn any_signed_envelope_rejects_delegate_class() {
    // Dispatch short-circuits on unknown class BEFORE signature verification.
    // The fake "delegate" bytes don't need to be validly signed.
    let bogus = br#"{"class": "delegate", "signature": "xxx"}"#;
    let vk = test1_vk();
    let err = AnySignedEnvelope::decode(bogus, &vk).unwrap_err();
    assert!(
        matches!(err, EnvelopeDecodeError::UnknownClass { ref found } if found == "delegate"),
        "expected UnknownClass {{ delegate }}, got {err:?}"
    );
}

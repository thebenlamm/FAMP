//! FAMP v0.5.1 §7.1c Worked-Example byte-exact interop gate.
//!
//! This is the Phase 2 protocol gate: a cross-language conformance fixture
//! whose bytes come from `FAMP-v0.5.1-spec.md` §7.1c (externally sourced via
//! Python `jcs==0.2.1` + `cryptography==46.0.7`; see
//! `tests/vectors/famp-sig-v1/PROVENANCE.md`). If Rust disagrees with these
//! bytes, this test fails and Phase 2 is not shippable.

#![allow(clippy::expect_used, clippy::unwrap_used)]

// Silence workspace-level unused_crate_dependencies for deps used elsewhere
// in the crate but not by this integration-test compile unit.
use base64 as _;
use ed25519_dalek as _;
use famp_canonical as _;
use insta as _;
use proptest as _;
use sha2 as _;
use subtle as _;
use thiserror as _;
use zeroize as _;

use famp_crypto::{
    canonicalize_for_signature, verify_canonical_bytes, FampSignature, TrustedVerifyingKey,
};

#[derive(serde::Deserialize)]
struct WorkedExample {
    domain_prefix_hex: String,
    public_key_hex: String,
    unsigned_envelope_json: String,
    canonical_json_hex: String,
    signing_input_hex: String,
    signature_hex: String,
}

#[test]
fn section_7_1c_worked_example_byte_exact() {
    let raw = include_str!("vectors/famp-sig-v1/worked-example.json");
    let f: WorkedExample = serde_json::from_str(raw).expect("fixture parses");

    // Guard: fixture transcription sanity
    assert_eq!(
        f.domain_prefix_hex, "46414d502d7369672d763100",
        "domain_prefix_hex MUST match FAMP-sig-v1\\0"
    );
    assert!(
        f.signing_input_hex.starts_with(&f.domain_prefix_hex),
        "signing_input_hex MUST begin with domain_prefix_hex"
    );
    assert_eq!(
        f.signing_input_hex.len(),
        f.domain_prefix_hex.len() + f.canonical_json_hex.len(),
        "signing_input_hex MUST equal prefix || canonical concatenation"
    );

    // Reproduce signing input from the unsigned envelope and assert byte-exact match.
    let unsigned: serde_json::Value =
        serde_json::from_str(&f.unsigned_envelope_json).expect("unsigned envelope parses");
    let actual_signing_input =
        canonicalize_for_signature(&unsigned).expect("canonicalize_for_signature");
    let expected_signing_input = hex::decode(&f.signing_input_hex).expect("hex decode");
    assert_eq!(
        actual_signing_input, expected_signing_input,
        "canonicalize_for_signature output MUST match fixture byte-for-byte"
    );

    // Reconstruct key + signature from fixture and verify.
    let pk_bytes: [u8; 32] = hex::decode(&f.public_key_hex)
        .expect("pk hex")
        .try_into()
        .expect("pk len");
    let vk =
        TrustedVerifyingKey::from_bytes(&pk_bytes).expect("public key must pass ingress");

    let canonical = hex::decode(&f.canonical_json_hex).expect("canonical hex");
    let sig_bytes: [u8; 64] = hex::decode(&f.signature_hex)
        .expect("sig hex")
        .try_into()
        .expect("sig len");
    let sig = FampSignature::from_bytes(sig_bytes);

    verify_canonical_bytes(&vk, &canonical, &sig)
        .expect("§7.1c worked-example signature MUST verify byte-exact");
}

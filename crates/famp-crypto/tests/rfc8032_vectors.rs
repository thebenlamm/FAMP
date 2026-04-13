//! RFC 8032 §7.1 Ed25519 algorithm-level test vectors.
//!
//! These vectors exercise the **Ed25519 primitive** directly via
//! `ed25519-dalek` — the FAMP domain-separation prefix is NOT applied here
//! because RFC 8032 vectors sign the raw message. This is the algorithm gate
//! that proves the underlying primitive is wired correctly before the FAMP
//! §7.1c worked-example fixture (Plan 03) layers the protocol on top.

#![allow(clippy::expect_used, clippy::unwrap_used)]

// Silence workspace-level unused_crate_dependencies for deps used elsewhere
// in the crate but not by this integration-test compile unit.
use base64 as _;
use famp_canonical as _;
use famp_crypto as _;
use insta as _;
use proptest as _;
use subtle as _;
use thiserror as _;
use zeroize as _;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

#[derive(serde::Deserialize)]
struct Vector {
    name: String,
    secret_key_hex: String,
    public_key_hex: String,
    message_hex: String,
    signature_hex: String,
}

#[test]
fn all_rfc8032_vectors_byte_exact() {
    let raw = include_str!("vectors/rfc8032/test-vectors.json");
    let vectors: Vec<Vector> = serde_json::from_str(raw).expect("parse vectors");
    assert_eq!(
        vectors.len(),
        5,
        "RFC 8032 §7.1 has exactly 5 vectors (TEST 1, 2, 3, 1024, SHA(abc))"
    );

    for v in &vectors {
        let sk_bytes: [u8; 32] = hex::decode(&v.secret_key_hex)
            .unwrap()
            .try_into()
            .expect("secret key must be 32 bytes");
        let pk_bytes: [u8; 32] = hex::decode(&v.public_key_hex)
            .unwrap()
            .try_into()
            .expect("public key must be 32 bytes");
        let msg = hex::decode(&v.message_hex).unwrap();
        let expected_sig_bytes: [u8; 64] = hex::decode(&v.signature_hex)
            .unwrap()
            .try_into()
            .expect("signature must be 64 bytes");

        let sk = SigningKey::from_bytes(&sk_bytes);

        // Public key consistency: derived pk matches the vector.
        assert_eq!(
            sk.verifying_key().as_bytes(),
            &pk_bytes,
            "{}: derived public key mismatch",
            v.name
        );

        // Sign → byte-exact match with vector signature.
        let sig = sk.sign(&msg);
        assert_eq!(
            sig.to_bytes(),
            expected_sig_bytes,
            "{}: signature bytes mismatch",
            v.name
        );

        // Verify with verify_strict — the FAMP-wide requirement.
        let vk = VerifyingKey::from_bytes(&pk_bytes).expect("canonical point");
        let expected_sig = Signature::from_bytes(&expected_sig_bytes);
        vk.verify_strict(&msg, &expected_sig)
            .unwrap_or_else(|_| panic!("{}: verify_strict failed", v.name));
    }
}

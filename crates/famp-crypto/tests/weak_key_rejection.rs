//! Proves every named fixture under `tests/vectors/must-reject/` is rejected
//! by `TrustedVerifyingKey` ingress (SPEC §7.1b, CRYPTO-02/03).

#![allow(clippy::expect_used, clippy::unwrap_used)]

// Silence `unused_crate_dependencies` for dev-deps this integration test does
// not itself reference (they're used by sibling tests in the same crate).
use base64 as _;
use ed25519_dalek as _;
use famp_canonical as _;
use insta as _;
use proptest as _;
use serde_json as _;
use subtle as _;
use thiserror as _;
use zeroize as _;

use famp_crypto::{CryptoError, TrustedVerifyingKey};

#[derive(serde::Deserialize)]
struct WeakKeyFixture {
    name: String,
    #[allow(dead_code)]
    description: String,
    public_key_hex: String,
}

#[derive(serde::Deserialize)]
struct MalformedFixture {
    name: String,
    input: String,
}

#[test]
fn all_weak_key_fixtures_rejected_at_ingress() {
    let raw = include_str!("vectors/must-reject/weak-keys.json");
    let fixtures: Vec<WeakKeyFixture> =
        serde_json::from_str(raw).expect("weak-keys.json must parse");
    assert!(
        !fixtures.is_empty(),
        "weak-keys.json must contain at least one fixture"
    );
    for f in &fixtures {
        let bytes = hex::decode(&f.public_key_hex).expect("valid hex");
        let arr: [u8; 32] = bytes
            .try_into()
            .expect("fixture public_key_hex must be 32 bytes");
        let res = TrustedVerifyingKey::from_bytes(&arr);
        assert!(
            matches!(
                res,
                Err(CryptoError::WeakKey | CryptoError::InvalidKeyEncoding)
            ),
            "fixture '{}' MUST be rejected at ingress, got {res:?}",
            f.name
        );
    }
}

#[test]
fn all_malformed_b64_rejected() {
    let raw = include_str!("vectors/must-reject/malformed-b64.json");
    let fixtures: Vec<MalformedFixture> =
        serde_json::from_str(raw).expect("malformed-b64.json must parse");
    assert!(
        !fixtures.is_empty(),
        "malformed-b64.json must contain at least one fixture"
    );
    for f in &fixtures {
        let res = TrustedVerifyingKey::from_b64url(&f.input);
        assert!(
            res.is_err(),
            "b64 fixture '{}' MUST fail decode, got Ok",
            f.name
        );
    }
}

//! Property-tests the base64url round-trip for `FampSigningKey` and
//! `FampSignature`. Proves the encoded form never contains `=`, `+`, or `/`
//! (SPEC-19 / CRYPTO-06 strict unpadded URL-safe alphabet).

#![allow(clippy::expect_used, clippy::unwrap_used)]

use base64 as _;
use ed25519_dalek as _;
use famp_canonical as _;
use hex as _;
use insta as _;
use serde as _;
use serde_json as _;
use subtle as _;
use thiserror as _;
use zeroize as _;

use famp_crypto::{FampSignature, FampSigningKey};
use proptest::prelude::*;

proptest! {
    #[test]
    fn signing_key_b64_roundtrip(bytes in proptest::array::uniform32(any::<u8>())) {
        let sk = FampSigningKey::from_bytes(bytes);
        let s = sk.to_b64url();
        let sk2 = FampSigningKey::from_b64url(&s).expect("roundtrip decode");
        prop_assert_eq!(sk.to_b64url(), sk2.to_b64url());
        prop_assert!(!s.contains('='));
        prop_assert!(!s.contains('+'));
        prop_assert!(!s.contains('/'));
    }

    #[test]
    fn signature_b64_roundtrip(bytes in proptest::array::uniform32(any::<u8>())) {
        let mut full = [0u8; 64];
        full[..32].copy_from_slice(&bytes);
        full[32..].copy_from_slice(&bytes);
        let sig = FampSignature::from_bytes(full);
        let s = sig.to_b64url();
        let sig2 = FampSignature::from_b64url(&s).expect("roundtrip decode");
        prop_assert_eq!(sig, sig2);
        prop_assert!(!s.contains('='));
        prop_assert!(!s.contains('+'));
        prop_assert!(!s.contains('/'));
    }
}

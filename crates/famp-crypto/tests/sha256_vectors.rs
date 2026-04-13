//! CRYPTO-07 conformance gate: NIST SHA-256 Known Answer Tests as byte-exact
//! assertions over the `sha256:<hex>` artifact-id form exposed by
//! `famp_crypto::sha256_artifact_id`.
//!
//! Vectors are taken verbatim from FIPS 180-2 Appendix B / NIST CAVP.
//! If any of these fail, content-addressing is broken at the byte level
//! and NO higher layer (envelope, transport, conformance) can be trusted.

#![allow(clippy::expect_used, clippy::unwrap_used)]

// Silence workspace-level unused_crate_dependencies for deps used elsewhere
// in the crate but not by this integration-test compile unit.
use base64 as _;
use ed25519_dalek as _;
use famp_canonical as _;
use hex as _;
use insta as _;
use proptest as _;
use serde as _;
use serde_json as _;
use sha2 as _;
use subtle as _;
use thiserror as _;
use zeroize as _;

use std::fmt::Write as _;

use famp_crypto::{sha256_artifact_id, sha256_digest};

#[test]
fn nist_kat_empty_string() {
    let id = sha256_artifact_id(b"");
    assert_eq!(
        id,
        "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "SHA-256 of empty input must match FIPS 180-2 empty-string KAT"
    );
}

#[test]
fn nist_kat_abc() {
    let id = sha256_artifact_id(b"abc");
    assert_eq!(
        id,
        "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "SHA-256 of b\"abc\" must match FIPS 180-2 B.1 KAT"
    );
}

#[test]
fn nist_kat_56byte_vector() {
    let input: &[u8] = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
    let id = sha256_artifact_id(input);
    assert_eq!(
        id,
        "sha256:248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
        "SHA-256 of the 56-byte FIPS 180-2 B.2 KAT must match"
    );
}

#[test]
fn artifact_id_shape_invariants() {
    let id = sha256_artifact_id(b"anything");
    assert_eq!(id.len(), 71, "sha256: + 64 hex chars = 71 chars");
    assert!(id.starts_with("sha256:"), "prefix must be literal sha256:");
    let hex_part = &id[7..];
    assert_eq!(hex_part.len(), 64);
    assert!(
        hex_part
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "hex portion must be lowercase ASCII hex only"
    );
}

#[test]
fn digest_and_artifact_id_agree() {
    let raw: [u8; 32] = sha256_digest(b"abc");
    let id = sha256_artifact_id(b"abc");
    // Manually hex-encode raw and prepend sha256: to check agreement.
    let mut expected = String::from("sha256:");
    for b in raw {
        write!(&mut expected, "{b:02x}").unwrap();
    }
    assert_eq!(id, expected);
}

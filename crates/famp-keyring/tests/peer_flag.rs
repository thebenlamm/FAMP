//! TOFU semantics + `--peer` flag parser tests (D-B3, D-B4).
//!
//! TOFU-1: re-pinning the same (principal, key) is idempotent.
//! TOFU-2: pinning a DIFFERENT key for the same principal returns KeyConflict.
//! PEER-1: valid flag parses to (principal, key).
//! PEER-2: colon separator (`:`) instead of `=` returns InvalidPeerFlag.
//! PEER-3: malformed base64 pubkey surfaces as Crypto error.

#![allow(clippy::unwrap_used, unused_crate_dependencies)]

use famp_core::Principal;
use famp_crypto::TrustedVerifyingKey;
use famp_keyring::{parse_peer_flag, Keyring, KeyringError};
use std::str::FromStr;

// seed=[1;32] Ed25519 pubkey (base64url-unpadded) — matches two_peers fixture
const ALICE_PK_B64: &str = "iojj3XQJ8ZX9UtstPLpdcspnCb8dlBIb83SIAbQPb1w";
// seed=[2;32] Ed25519 pubkey — bob's key
const BOB_PK_B64: &str = "gTl3Dqh9F19Wo1Rmw0x-zMuNipG07jeiXfYPW4_Js5Q";

fn alice() -> Principal {
    Principal::from_str("agent:local/alice").unwrap()
}

fn alice_key() -> TrustedVerifyingKey {
    TrustedVerifyingKey::from_b64url(ALICE_PK_B64).unwrap()
}

fn bob_key() -> TrustedVerifyingKey {
    TrustedVerifyingKey::from_b64url(BOB_PK_B64).unwrap()
}

#[test]
fn tofu1_idempotent_same_key_repin() {
    let k = Keyring::new()
        .with_peer(alice(), alice_key())
        .unwrap()
        .with_peer(alice(), alice_key())
        .unwrap();
    assert_eq!(k.len(), 1);
    assert!(k.get(&alice()).is_some());
}

#[test]
fn tofu2_different_key_rejected_as_key_conflict() {
    let k = Keyring::new().with_peer(alice(), alice_key()).unwrap();
    let err = k.with_peer(alice(), bob_key()).unwrap_err();
    match err {
        KeyringError::KeyConflict { principal } => assert_eq!(principal, alice()),
        other => panic!("expected KeyConflict, got {other:?}"),
    }
}

#[test]
fn peer1_valid_flag_parses() {
    let raw = format!("agent:local/alice={ALICE_PK_B64}");
    let (principal, key) = parse_peer_flag(&raw).unwrap();
    assert_eq!(principal, alice());
    assert_eq!(key.as_bytes(), alice_key().as_bytes());
}

#[test]
fn peer2_colon_separator_rejected() {
    // `:` is ambiguous because the principal itself contains `:`; we require
    // `=` as the separator (D-B4). With no `=` present, the parser must fail
    // closed with InvalidPeerFlag.
    let raw = format!("agent:local/alice:{ALICE_PK_B64}");
    let err = parse_peer_flag(&raw).unwrap_err();
    match err {
        KeyringError::InvalidPeerFlag { .. } => {}
        other => panic!("expected InvalidPeerFlag, got {other:?}"),
    }
}

#[test]
fn peer3_malformed_base64_surfaces_crypto_error() {
    let raw = "agent:local/alice=not-valid-base64!!";
    let err = parse_peer_flag(raw).unwrap_err();
    match err {
        KeyringError::Crypto(_) => {}
        other => panic!("expected Crypto, got {other:?}"),
    }
}

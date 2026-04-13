//! Unit tests for runtime glue components.
//!
//! Full CONF-05/06/07 integration tests live in Plan 03-04 under
//! `tests/adversarial.rs`. This file is strictly isolated unit coverage
//! of the adapter, peek, canonical pre-check, recipient cross-check, and
//! unknown-sender paths.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use ed25519_dalek as _;
use rand as _;
use thiserror as _;
use tokio as _;

use famp::runtime::{
    adapter::fsm_input_from_envelope, error::RuntimeError, peek::peek_sender,
    process_one_message,
};
use famp_canonical::canonicalize;
use famp_core::{AuthorityScope, MessageId, Principal};
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_envelope::body::{AckBody, AckDisposition};
use famp_envelope::{AnySignedEnvelope, EnvelopeDecodeError, Timestamp, UnsignedEnvelope};
use famp_fsm::TaskFsm;
use famp_keyring::Keyring;
use famp_transport::TransportMessage;

// RFC 8032 Test 1 keypair (alice).
const ALICE_SECRET: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];
const ALICE_PUBLIC: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

// RFC 8032 Test 2 keypair (bob) — distinct from alice so the keyring can
// distinguish them in the unknown-sender test.
const BOB_SECRET: [u8; 32] = [
    0x4c, 0xcd, 0x08, 0x9b, 0x28, 0xff, 0x96, 0xda, 0x9d, 0xb6, 0xc3, 0x46, 0xec, 0x11, 0x4e, 0x0f,
    0x5b, 0x8a, 0x31, 0x9f, 0x35, 0xab, 0xa6, 0x24, 0xda, 0x8c, 0xf6, 0xed, 0x4f, 0xb8, 0xa6, 0xfb,
];
const BOB_PUBLIC: [u8; 32] = [
    0x3d, 0x40, 0x17, 0xc3, 0xe8, 0x43, 0x89, 0x5a, 0x92, 0xb7, 0x0a, 0xa7, 0x4d, 0x1b, 0x7e, 0xbc,
    0x9c, 0x98, 0x2c, 0xcf, 0x2e, 0xc4, 0x96, 0x8c, 0xc0, 0xcd, 0x55, 0xf1, 0x2a, 0xf4, 0x66, 0x0c,
];

fn alice() -> Principal {
    "agent:example.test/alice".parse().unwrap()
}
fn bob() -> Principal {
    "agent:example.test/bob".parse().unwrap()
}
fn carol() -> Principal {
    "agent:example.test/carol".parse().unwrap()
}

fn msg_id() -> MessageId {
    "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a6b".parse().unwrap()
}

fn ts() -> Timestamp {
    Timestamp("2026-04-13T00:00:00Z".to_string())
}

/// Build a canonical (RFC 8785) byte representation of an ack envelope
/// signed by `sk`. Uses the typed sign/encode path, then re-parses and
/// re-canonicalizes so the resulting bytes satisfy the runtime's canonical
/// pre-check (Step 3 of `process_one_message`).
fn canonical_ack_bytes(sk: &FampSigningKey, from: Principal, to: Principal) -> Vec<u8> {
    let body = AckBody {
        disposition: AckDisposition::Accepted,
        reason: None,
    };
    let unsigned = UnsignedEnvelope::<AckBody>::new(
        msg_id(),
        from,
        to,
        AuthorityScope::Advisory,
        ts(),
        body,
    );
    let signed = unsigned.sign(sk).expect("sign must succeed");
    let encoded = signed.encode().expect("encode must succeed");
    let value: serde_json::Value =
        famp_canonical::from_slice_strict(&encoded).expect("strict parse must succeed");
    canonicalize(&value).expect("canonicalize must succeed")
}

// ----- UNIT-1: peek_sender happy path -----

#[test]
fn unit1_peek_sender_extracts_from_field() {
    let bytes = br#"{"from":"agent:example.test/alice","class":"ack","z":1}"#;
    let p = peek_sender(bytes).expect("peek should succeed on valid 'from'");
    assert_eq!(p, alice());
}

// ----- UNIT-2: peek_sender missing 'from' field -----

#[test]
fn unit2_peek_sender_missing_from_returns_missing_field() {
    let bytes = br#"{"class":"ack"}"#;
    let err = peek_sender(bytes).expect_err("peek must fail without 'from'");
    match err {
        RuntimeError::Decode(EnvelopeDecodeError::MissingField { field }) => {
            assert_eq!(field, "from");
        }
        other => panic!("expected Decode(MissingField), got {other:?}"),
    }
}

// ----- UNIT-3: peek_sender malformed JSON -----

#[test]
fn unit3_peek_sender_malformed_json_returns_malformed() {
    let bytes = b"{not json";
    let err = peek_sender(bytes).expect_err("peek must fail on malformed JSON");
    assert!(
        matches!(err, RuntimeError::Decode(EnvelopeDecodeError::MalformedJson(_))),
        "expected Decode(MalformedJson), got {err:?}"
    );
}

// ----- UNIT-4: adapter returns None for Ack -----

#[test]
fn unit4_fsm_input_from_envelope_ack_is_none() {
    let sk = FampSigningKey::from_bytes(ALICE_SECRET);
    let vk = TrustedVerifyingKey::from_bytes(&ALICE_PUBLIC).unwrap();
    let bytes = canonical_ack_bytes(&sk, alice(), bob());
    let env = AnySignedEnvelope::decode(&bytes, &vk).expect("decode must succeed");
    assert!(matches!(env, AnySignedEnvelope::Ack(_)));
    assert!(
        fsm_input_from_envelope(&env).is_none(),
        "ack envelopes must NOT enter the FSM (D-D4)"
    );
}

// ----- UNIT-5: canonical divergence pre-check fires before decode -----

#[test]
fn unit5_canonical_divergence_detected_before_decode() {
    // Hand-built bytes with non-canonical key order: 'z' before 'a'.
    // Canonical form would sort alphabetically, so canonicalize(parsed) will
    // NOT equal these bytes and the runtime must short-circuit with
    // RuntimeError::CanonicalDivergence BEFORE signature verification runs.
    // The dummy keyring never gets consulted for signature verification.
    let bytes = br#"{"from":"agent:example.test/alice","z":1,"a":2}"#.to_vec();
    let vk = TrustedVerifyingKey::from_bytes(&ALICE_PUBLIC).unwrap();
    let keyring = Keyring::new().with_peer(alice(), vk).unwrap();
    let mut fsm = TaskFsm::new();
    let tm = TransportMessage {
        sender: alice(),
        recipient: bob(),
        bytes,
    };
    let err = process_one_message(&tm, &keyring, &mut fsm).expect_err("must diverge");
    assert!(
        matches!(err, RuntimeError::CanonicalDivergence),
        "expected CanonicalDivergence, got {err:?}"
    );
}

// ----- UNIT-6: recipient cross-check (transport vs envelope) -----

#[test]
fn unit6_recipient_mismatch_returns_typed_error() {
    let sk = FampSigningKey::from_bytes(ALICE_SECRET);
    let vk_alice = TrustedVerifyingKey::from_bytes(&ALICE_PUBLIC).unwrap();
    // Envelope from alice to bob.
    let bytes = canonical_ack_bytes(&sk, alice(), bob());
    let keyring = Keyring::new().with_peer(alice(), vk_alice).unwrap();
    let mut fsm = TaskFsm::new();
    // Transport layer says recipient is carol — must mismatch.
    let tm = TransportMessage {
        sender: alice(),
        recipient: carol(),
        bytes,
    };
    let err = process_one_message(&tm, &keyring, &mut fsm).expect_err("must mismatch");
    match err {
        RuntimeError::RecipientMismatch { transport, envelope } => {
            assert_eq!(transport, carol());
            assert_eq!(envelope, bob());
        }
        other => panic!("expected RecipientMismatch, got {other:?}"),
    }
}

// ----- UNIT-7: unknown sender rejected before decode -----

#[test]
fn unit7_unknown_sender_rejected_before_decode() {
    // Envelope signed by alice, but keyring pins only bob — alice is unknown.
    let sk_alice = FampSigningKey::from_bytes(ALICE_SECRET);
    let vk_bob = TrustedVerifyingKey::from_bytes(&BOB_PUBLIC).unwrap();
    let bytes = canonical_ack_bytes(&sk_alice, alice(), bob());
    let keyring = Keyring::new().with_peer(bob(), vk_bob).unwrap();
    let mut fsm = TaskFsm::new();
    let tm = TransportMessage {
        sender: alice(),
        recipient: bob(),
        bytes,
    };
    let err = process_one_message(&tm, &keyring, &mut fsm).expect_err("must reject");
    match err {
        RuntimeError::UnknownSender(p) => assert_eq!(p, alice()),
        other => panic!("expected UnknownSender, got {other:?}"),
    }
    // Bonus: BOB_SECRET must have been referenced somewhere in this file to
    // keep unused-const lints quiet. Reference it in an assertion here.
    let _ = BOB_SECRET[0];
}

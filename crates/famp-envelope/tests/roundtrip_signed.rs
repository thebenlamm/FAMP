//! Per-class signed round-trip tests (CONTEXT.md D-D2).
//!
//! Build typed `UnsignedEnvelope<B>::new(...)`, `sign()`, `encode()`, then
//! `SignedEnvelope::<B>::decode()` and assert semantic equality. Exercises
//! the full pipeline end-to-end once per shipped class.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::needless_pass_by_value
)]

use famp_canonical as _;
use hex as _;
use insta as _;
use proptest as _;
use serde as _;
use serde_json as _;
use thiserror as _;

use famp_core::{AuthorityScope, MessageId, Principal};
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_envelope::body::{
    AckBody, AckDisposition, Bounds, Budget, CommitBody, ControlAction, ControlBody, ControlTarget,
    DeliverBody, ErrorCategory, ErrorDetail, RequestBody, TerminalStatus,
};
use famp_envelope::{Causality, Relation, SignedEnvelope, Timestamp, UnsignedEnvelope};

// RFC 8032 Test 1 keypair — same as vector_zero.
const SECRET: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];
const PUBLIC: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

fn keys() -> (FampSigningKey, TrustedVerifyingKey) {
    (
        FampSigningKey::from_bytes(SECRET),
        TrustedVerifyingKey::from_bytes(&PUBLIC).unwrap(),
    )
}

fn alice() -> Principal {
    "agent:example.test/alice".parse().unwrap()
}
fn bob() -> Principal {
    "agent:example.test/bob".parse().unwrap()
}
fn ts() -> Timestamp {
    Timestamp("2026-04-13T00:00:00Z".to_string())
}
fn id() -> MessageId {
    "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a6b".parse().unwrap()
}
fn other_id() -> MessageId {
    "01890a3b-1111-7222-8333-444444444444".parse().unwrap()
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

fn roundtrip<B>(unsigned: UnsignedEnvelope<B>)
where
    B: famp_envelope::BodySchema + PartialEq + std::fmt::Debug,
{
    let (sk, vk) = keys();
    let signed = unsigned.clone().sign(&sk).expect("sign must succeed");
    let bytes = signed.encode().expect("encode must succeed");
    let decoded =
        SignedEnvelope::<B>::decode(&bytes, &vk).expect("decode of self-signed must succeed");
    assert_eq!(
        decoded.body(),
        &unsigned.body,
        "body must round-trip byte-identical"
    );
    assert_eq!(decoded.class(), unsigned.class);
    assert_eq!(decoded.scope(), unsigned.scope);
    assert_eq!(decoded.from_principal(), &unsigned.from);
    assert_eq!(decoded.to_principal(), &unsigned.to);
}

#[test]
fn request_roundtrip() {
    let body = RequestBody {
        scope: serde_json::json!({"task": "translate"}),
        bounds: two_key_bounds(),
        natural_language_summary: Some("translate to french".to_string()),
    };
    let e = UnsignedEnvelope::<RequestBody>::new(
        id(),
        alice(),
        bob(),
        AuthorityScope::Advisory,
        ts(),
        body,
    );
    roundtrip(e);
}

#[test]
fn commit_roundtrip() {
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
    let e = UnsignedEnvelope::<CommitBody>::new(
        id(),
        alice(),
        bob(),
        AuthorityScope::CommitLocal,
        ts(),
        body,
    );
    roundtrip(e);
}

#[test]
fn deliver_interim_roundtrip() {
    let body = DeliverBody {
        interim: true,
        artifacts: None,
        result: Some(serde_json::json!({"progress": 0.5})),
        usage_metrics: None,
        error_detail: None,
        provenance: None,
        natural_language_summary: None,
    };
    // interim=true → no terminal_status on envelope
    let e = UnsignedEnvelope::<DeliverBody>::new(
        id(),
        alice(),
        bob(),
        AuthorityScope::Advisory,
        ts(),
        body,
    );
    roundtrip(e);
}

#[test]
fn deliver_terminal_roundtrip() {
    let body = DeliverBody {
        interim: false,
        artifacts: None,
        result: Some(serde_json::json!({"text": "Bonjour le monde."})),
        usage_metrics: None,
        error_detail: None,
        provenance: Some(serde_json::json!({"signer": "did:example:translator"})),
        natural_language_summary: None,
    };
    let e = UnsignedEnvelope::<DeliverBody>::new(
        id(),
        alice(),
        bob(),
        AuthorityScope::Advisory,
        ts(),
        body,
    )
    .with_terminal_status(TerminalStatus::Completed);
    roundtrip(e);
}

#[test]
fn deliver_terminal_failed_roundtrip() {
    let body = DeliverBody {
        interim: false,
        artifacts: None,
        result: None,
        usage_metrics: None,
        error_detail: Some(ErrorDetail {
            category: ErrorCategory::Internal,
            message: "timeout".to_string(),
            diagnostic: None,
        }),
        provenance: Some(serde_json::json!({"signer": "did:example:translator"})),
        natural_language_summary: None,
    };
    let e = UnsignedEnvelope::<DeliverBody>::new(
        id(),
        alice(),
        bob(),
        AuthorityScope::Advisory,
        ts(),
        body,
    )
    .with_terminal_status(TerminalStatus::Failed);
    roundtrip(e);
}

#[test]
fn ack_roundtrip() {
    let body = AckBody {
        disposition: AckDisposition::Accepted,
        reason: None,
    };
    let e = UnsignedEnvelope::<AckBody>::new(
        id(),
        alice(),
        bob(),
        AuthorityScope::Advisory,
        ts(),
        body,
    )
    .with_causality(Causality {
        rel: Relation::Acknowledges,
        referenced: other_id(),
    });
    roundtrip(e);
}

#[test]
fn control_cancel_roundtrip() {
    let body = ControlBody {
        target: ControlTarget::Task,
        action: ControlAction::Cancel,
        disposition: None,
        reason: Some("user aborted".to_string()),
        affected_ids: None,
    };
    let e = UnsignedEnvelope::<ControlBody>::new(
        id(),
        alice(),
        bob(),
        AuthorityScope::Advisory,
        ts(),
        body,
    );
    roundtrip(e);
}

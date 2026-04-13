//! Per-body proptest round-trip (CONTEXT.md D-D5 — narrow, typed, deterministic).
//!
//! Scope: build typed `UnsignedEnvelope<B>` from a small body strategy,
//! `sign → encode → decode`, assert semantic equality. Plus one tamper-last-
//! byte check per body that asserts `SignatureInvalid | MalformedJson |
//! InvalidSignatureEncoding` — any typed failure is acceptable, the point
//! is "no panic, no generic error".
//!
//! Strategies are shallow on purpose (max depth 2, max 3 opaque keys) so
//! shrink output is debuggable. The broader adversarial matrix lives in
//! Phase 3 CONF-05/06/07.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::unnested_or_patterns
)]

use famp_canonical as _;
use hex as _;
use insta as _;
use serde as _;
use thiserror as _;

use famp_core::{AuthorityScope, MessageId, Principal};
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_envelope::body::{
    AckBody, AckDisposition, Bounds, Budget, CommitBody, ControlAction, ControlBody,
    ControlTarget, DeliverBody, RequestBody, TerminalStatus,
};
use famp_envelope::{SignedEnvelope, Timestamp, UnsignedEnvelope};
use proptest::prelude::*;
use serde_json::json;

const SECRET: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];
const PUBLIC: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

fn sk() -> FampSigningKey {
    FampSigningKey::from_bytes(SECRET)
}
fn vk() -> TrustedVerifyingKey {
    TrustedVerifyingKey::from_bytes(&PUBLIC).unwrap()
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

// ---------------- strategies ----------------

/// Shallow 2-key `Bounds` — always valid under §9.3 ≥2 rule.
fn bounds_strategy() -> impl Strategy<Value = Bounds> {
    (1u64..=10, 1u8..=5).prop_map(|(hop, depth)| Bounds {
        deadline: Some("2026-05-01T00:00:00Z".to_string()),
        budget: Some(Budget {
            amount: "100".to_string(),
            unit: "usd".to_string(),
        }),
        hop_limit: Some(hop),
        policy_domain: None,
        authority_scope: None,
        max_artifact_size: None,
        confidence_floor: None,
        recursion_depth: Some(depth),
    })
}

fn request_body_strategy() -> impl Strategy<Value = RequestBody> {
    bounds_strategy().prop_map(|b| RequestBody {
        scope: json!({"task": "translate"}),
        bounds: b,
        natural_language_summary: None,
    })
}

fn commit_body_strategy() -> impl Strategy<Value = CommitBody> {
    bounds_strategy().prop_map(|b| CommitBody {
        scope: json!({"task": "translate"}),
        scope_subset: None,
        bounds: b,
        accepted_policies: vec!["policy://famp/v0.7/personal".to_string()],
        delegation_permissions: None,
        reporting_obligations: None,
        terminal_condition: json!({"type": "final_delivery"}),
        conditions: None,
        natural_language_summary: None,
    })
}

fn deliver_body_strategy() -> impl Strategy<Value = (DeliverBody, Option<TerminalStatus>)> {
    // interim=true with no terminal_status OR interim=false + completed with provenance
    prop_oneof![
        Just((
            DeliverBody {
                interim: true,
                artifacts: None,
                result: Some(json!({"progress": 0.5})),
                usage_metrics: None,
                error_detail: None,
                provenance: None,
                natural_language_summary: None,
            },
            None
        )),
        Just((
            DeliverBody {
                interim: false,
                artifacts: None,
                result: Some(json!({"text": "ok"})),
                usage_metrics: None,
                error_detail: None,
                provenance: Some(json!({"signer": "did:example:translator"})),
                natural_language_summary: None,
            },
            Some(TerminalStatus::Completed)
        )),
    ]
}

fn ack_body_strategy() -> impl Strategy<Value = AckBody> {
    prop_oneof![
        Just(AckDisposition::Accepted),
        Just(AckDisposition::Rejected),
        Just(AckDisposition::Received),
    ]
    .prop_map(|d| AckBody {
        disposition: d,
        reason: None,
    })
}

fn control_body_strategy() -> impl Strategy<Value = ControlBody> {
    any::<bool>().prop_map(|with_reason| ControlBody {
        target: ControlTarget::Task,
        action: ControlAction::Cancel,
        disposition: None,
        reason: if with_reason {
            Some("aborted".to_string())
        } else {
            None
        },
        affected_ids: None,
    })
}

// ---------------- generic roundtrip + tamper helpers ----------------

fn sign_roundtrip<B>(body: B, terminal: Option<TerminalStatus>) -> Result<(), String>
where
    B: famp_envelope::BodySchema + PartialEq + std::fmt::Debug,
{
    let mut e = UnsignedEnvelope::<B>::new(
        id(),
        alice(),
        bob(),
        AuthorityScope::Advisory,
        ts(),
        body,
    );
    if let Some(t) = terminal {
        e = e.with_terminal_status(t);
    }
    let expected_body = e.body.clone();
    let signed = e.sign(&sk()).map_err(|e| format!("{e:?}"))?;
    let bytes = signed.encode().map_err(|e| format!("{e:?}"))?;
    let decoded =
        SignedEnvelope::<B>::decode(&bytes, &vk()).map_err(|e| format!("{e:?}"))?;
    if decoded.body() != &expected_body {
        return Err("body round-trip diverged".to_string());
    }
    Ok(())
}

fn tamper_fails<B>(body: B, terminal: Option<TerminalStatus>) -> Result<(), String>
where
    B: famp_envelope::BodySchema,
{
    let mut e = UnsignedEnvelope::<B>::new(
        id(),
        alice(),
        bob(),
        AuthorityScope::Advisory,
        ts(),
        body,
    );
    if let Some(t) = terminal {
        e = e.with_terminal_status(t);
    }
    let signed = e.sign(&sk()).map_err(|e| format!("{e:?}"))?;
    let mut bytes = signed.encode().map_err(|e| format!("{e:?}"))?;
    // Tamper with the signature base64url character near the middle, which
    // keeps the JSON structurally valid but yields a different b64url string.
    // Find the `"signature":"` substring in the JSON and flip a middle char.
    let hay = String::from_utf8(bytes.clone()).unwrap();
    if let Some(start) = hay.find("\"signature\":\"") {
        let sig_start = start + "\"signature\":\"".len();
        let mid = sig_start + 10; // well into the base64 value
        if mid < bytes.len() && bytes[mid] != b'"' {
            bytes[mid] = if bytes[mid] == b'A' { b'B' } else { b'A' };
        }
    }
    match SignedEnvelope::<B>::decode(&bytes, &vk()) {
        Err(famp_envelope::EnvelopeDecodeError::SignatureInvalid)
        | Err(famp_envelope::EnvelopeDecodeError::InvalidSignatureEncoding(_))
        | Err(famp_envelope::EnvelopeDecodeError::MalformedJson(_)) => Ok(()),
        Err(other) => Err(format!("unexpected typed error: {other:?}")),
        Ok(_) => Err("tampered bytes unexpectedly verified".to_string()),
    }
}

// ---------------- proptest blocks ----------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn request_prop_roundtrip(b in request_body_strategy()) {
        sign_roundtrip::<RequestBody>(b, None).map_err(TestCaseError::fail)?;
    }

    #[test]
    fn request_prop_tamper(b in request_body_strategy()) {
        tamper_fails::<RequestBody>(b, None).map_err(TestCaseError::fail)?;
    }

    #[test]
    fn commit_prop_roundtrip(b in commit_body_strategy()) {
        sign_roundtrip::<CommitBody>(b, None).map_err(TestCaseError::fail)?;
    }

    #[test]
    fn commit_prop_tamper(b in commit_body_strategy()) {
        tamper_fails::<CommitBody>(b, None).map_err(TestCaseError::fail)?;
    }

    #[test]
    fn deliver_prop_roundtrip((b, t) in deliver_body_strategy()) {
        sign_roundtrip::<DeliverBody>(b, t).map_err(TestCaseError::fail)?;
    }

    #[test]
    fn deliver_prop_tamper((b, t) in deliver_body_strategy()) {
        tamper_fails::<DeliverBody>(b, t).map_err(TestCaseError::fail)?;
    }

    #[test]
    fn ack_prop_roundtrip(b in ack_body_strategy()) {
        sign_roundtrip::<AckBody>(b, None).map_err(TestCaseError::fail)?;
    }

    #[test]
    fn ack_prop_tamper(b in ack_body_strategy()) {
        tamper_fails::<AckBody>(b, None).map_err(TestCaseError::fail)?;
    }

    #[test]
    fn control_prop_roundtrip(b in control_body_strategy()) {
        sign_roundtrip::<ControlBody>(b, None).map_err(TestCaseError::fail)?;
    }

    #[test]
    fn control_prop_tamper(b in control_body_strategy()) {
        tamper_fails::<ControlBody>(b, None).map_err(TestCaseError::fail)?;
    }
}

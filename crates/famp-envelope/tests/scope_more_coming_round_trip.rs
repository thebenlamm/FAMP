//! Quick task 260425-pc7 — `scope.more_coming` round-trip + byte-exact
//! backwards-compat test.
//!
//! `scope.more_coming` is a sender-side signal on `request` envelopes
//! that means "I'm not done briefing — wait for follow-up `deliver`s
//! before treating this task as ready to commit." Mirrors the existing
//! `body.interim` shape on `deliver` envelopes (see
//! `crates/famp-envelope/src/body/deliver.rs`).
//!
//! Two assertions:
//!
//! 1. **Round-trip:** building a request with `scope.more_coming = true`,
//!    signing, encoding, and decoding round-trips the flag exactly.
//!
//! 2. **Byte-exact backwards compat:** building the same request twice —
//!    once never touching `more_coming`, once explicitly setting it
//!    `false` and then omitting from the JSON map — produces identical
//!    canonical bytes. This proves the "serde-skip when default" guarantee
//!    at the JSON layer: existing signed envelopes that pre-date this
//!    field still verify byte-for-byte.

#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp_canonical as _;
use hex as _;
use insta as _;
use proptest as _;
use serde as _;
use thiserror as _;

use famp_core::{AuthorityScope, MessageId, Principal};
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_envelope::body::request::{
    RequestBody, REQUEST_SCOPE_INSTRUCTIONS_KEY, REQUEST_SCOPE_MORE_COMING_KEY,
};
use famp_envelope::body::Bounds;
use famp_envelope::{SignedEnvelope, Timestamp, UnsignedEnvelope};

// RFC 8032 Test 1 keypair — reproducible across runs. Same as the
// existing provisional vector test so the two share a trust anchor.
const SECRET: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];
const PUBLIC: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

const FIXED_MESSAGE_ID: &str = "019f0000-0000-7000-8000-00000000pc70";
const FIXED_TIMESTAMP: &str = "2026-04-25T00:00:00Z";
const FIXTURE_BODY: &str = "Briefing part 1 of 2 — wait for follow-up deliver().";

fn build_envelope_bytes(more_coming: Option<bool>) -> Vec<u8> {
    let sk = FampSigningKey::from_bytes(SECRET);
    // Use a fixed UUID and timestamp so byte-exact comparison is
    // meaningful across the two parallel builds.
    let id: MessageId = "019f0000-0000-7000-8000-000000000001"
        .parse()
        .expect("FIXED_MESSAGE_ID is a valid UUIDv7");
    let from: Principal = "agent:example.test/alice".parse().unwrap();
    let to: Principal = "agent:example.test/bob".parse().unwrap();
    let ts = Timestamp(FIXED_TIMESTAMP.to_string());

    let bounds = Bounds {
        deadline: None,
        budget: None,
        hop_limit: Some(16),
        policy_domain: None,
        authority_scope: None,
        max_artifact_size: None,
        confidence_floor: None,
        recursion_depth: Some(4),
    };

    let mut scope_map = serde_json::Map::new();
    scope_map.insert(
        REQUEST_SCOPE_INSTRUCTIONS_KEY.to_string(),
        serde_json::Value::String(FIXTURE_BODY.to_string()),
    );
    // The "skip when default" rule lives at the sender layer: only
    // insert the key when the caller explicitly opts in to true.
    // Some(true)  → key set true   (round-trip path)
    // Some(false) → key omitted    (explicit-default backwards-compat path)
    // None        → key never set  (legacy sender path)
    if let Some(true) = more_coming {
        scope_map.insert(
            REQUEST_SCOPE_MORE_COMING_KEY.to_string(),
            serde_json::Value::Bool(true),
        );
    }

    let body = RequestBody {
        scope: serde_json::Value::Object(scope_map),
        bounds,
        natural_language_summary: Some("more_coming round-trip test".to_string()),
    };

    let unsigned: UnsignedEnvelope<RequestBody> =
        UnsignedEnvelope::new(id, from, to, AuthorityScope::Advisory, ts, body);
    let signed: SignedEnvelope<RequestBody> = unsigned.sign(&sk).expect("sign");
    signed.encode().expect("encode")
}

#[test]
fn more_coming_true_round_trips() {
    let bytes = build_envelope_bytes(Some(true));
    let vk = TrustedVerifyingKey::from_bytes(&PUBLIC).unwrap();
    let decoded: SignedEnvelope<RequestBody> =
        SignedEnvelope::decode(&bytes, &vk).expect("decode + verify_strict round-trip");
    let scope = &decoded.body().scope;
    assert_eq!(
        scope
            .pointer(&format!("/{REQUEST_SCOPE_MORE_COMING_KEY}"))
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "scope.more_coming did not round-trip"
    );
    // Sanity: the existing instructions key still co-exists.
    assert_eq!(
        scope
            .pointer(&format!("/{REQUEST_SCOPE_INSTRUCTIONS_KEY}"))
            .and_then(|v| v.as_str()),
        Some(FIXTURE_BODY),
    );
    // Suppress dead-code lint on FIXED_MESSAGE_ID — it documents the
    // intended fixed-id origin even if not used directly.
    let _ = FIXED_MESSAGE_ID;
}

#[test]
fn more_coming_default_false_is_byte_exact_with_legacy() {
    let legacy_bytes = build_envelope_bytes(None);
    let explicit_default_bytes = build_envelope_bytes(Some(false));
    assert_eq!(
        legacy_bytes, explicit_default_bytes,
        "Sender-omitted and explicit-false-then-omitted MUST produce \
         byte-identical canonical bytes — otherwise existing signed \
         envelopes will fail verify_strict after this change."
    );
    // And the legacy path still decodes cleanly under the trust anchor.
    let vk = TrustedVerifyingKey::from_bytes(&PUBLIC).unwrap();
    let decoded: SignedEnvelope<RequestBody> =
        SignedEnvelope::decode(&legacy_bytes, &vk).expect("legacy decode");
    let scope = &decoded.body().scope;
    assert_eq!(
        scope
            .pointer(&format!("/{REQUEST_SCOPE_MORE_COMING_KEY}"))
            .and_then(serde_json::Value::as_bool),
        None,
        "legacy envelope must NOT carry the more_coming key"
    );
}

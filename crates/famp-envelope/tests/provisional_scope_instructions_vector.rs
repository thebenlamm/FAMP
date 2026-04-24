//! Quick task 260424-7z5 — PROVISIONAL conformance vector generator.
//!
//! This test is `#[ignore]` by default. It is the source-of-truth generator
//! for the provisional conformance vector at
//! `tests/fixtures/provisional/request-scope-instructions.json`, which
//! captures a byte-exact signed request envelope whose body carries
//! `scope.instructions` per ADR 0001.
//!
//! ## Regenerating
//!
//! ```sh
//! cargo test -p famp-envelope --test provisional_scope_instructions_vector \
//!     -- --ignored write_provisional_scope_instructions_vector --nocapture
//! ```
//!
//! ## Validating (not `#[ignore]`)
//!
//! The `decode_provisional_scope_instructions_vector` test loads the on-disk
//! fixture and asserts:
//! - `SignedEnvelope::decode` succeeds (implies `verify_strict` passed),
//! - `body.scope.instructions` equals the fixture prose,
//! - `body.natural_language_summary` equals the fixture title.
//!
//! This is NOT a normative Level 2 vector. A Level 2 conformance loader
//! MUST NOT include the `provisional/` path in its glob.

#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp_canonical as _;
use hex as _;
use insta as _;
use proptest as _;
use serde as _;
use thiserror as _;

use std::path::PathBuf;

use famp_core::{AuthorityScope, MessageId, Principal};
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_envelope::body::request::{RequestBody, REQUEST_SCOPE_INSTRUCTIONS_KEY};
use famp_envelope::body::Bounds;
use famp_envelope::{SignedEnvelope, Timestamp, UnsignedEnvelope};

// RFC 8032 Test 1 keypair — reproducible across regenerations.
const SECRET: [u8; 32] = [
    0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c, 0xc4,
    0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae, 0x7f, 0x60,
];
const PUBLIC: [u8; 32] = [
    0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64, 0x07, 0x3a,
    0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68, 0xf7, 0x07, 0x51, 0x1a,
];

// Fixed UUIDv7 and timestamp so the fixture bytes are stable across
// regenerations. The UUID is a real UUIDv7 (version nibble `7` at offset 14).
const FIXED_MESSAGE_ID: &str = "019f0000-0000-7000-8000-000000007a75";
const FIXED_TIMESTAMP: &str = "2026-04-24T00:00:00Z";
const FIXTURE_TITLE: &str = "provisional fixture: scope.instructions round-trip";
const FIXTURE_BODY: &str =
    "This fixture locks the provisional scope.instructions convention (ADR 0001). \
     Do not edit by hand — regenerate via the ignored test.";

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("provisional")
        .join("request-scope-instructions.json")
}

fn build_fixture_envelope_bytes() -> Vec<u8> {
    let sk = FampSigningKey::from_bytes(SECRET);
    let id: MessageId = FIXED_MESSAGE_ID
        .parse()
        .expect("FIXED_MESSAGE_ID is a valid UUIDv7");
    let from: Principal = "agent:example.test/alice".parse().unwrap();
    let to: Principal = "agent:example.test/bob".parse().unwrap();
    let ts = Timestamp(FIXED_TIMESTAMP.to_string());

    // §9.3 requires ≥2 bounds keys. Match the production `build_request_envelope`
    // defaults (hop_limit = 16, recursion_depth = 4) so the vector reflects
    // what a real `famp send --new-task --body` call emits.
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

    let body = RequestBody {
        scope: serde_json::Value::Object(scope_map),
        bounds,
        natural_language_summary: Some(FIXTURE_TITLE.to_string()),
    };

    let unsigned: UnsignedEnvelope<RequestBody> =
        UnsignedEnvelope::new(id, from, to, AuthorityScope::Advisory, ts, body);
    let signed: SignedEnvelope<RequestBody> = unsigned.sign(&sk).expect("sign");
    signed.encode().expect("encode")
}

#[test]
#[ignore = "regenerates the on-disk provisional fixture — run explicitly"]
fn write_provisional_scope_instructions_vector() {
    let bytes = build_fixture_envelope_bytes();
    let path = fixture_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, &bytes).expect("write fixture");
    eprintln!("wrote {} bytes to {}", bytes.len(), path.display());

    // Paranoia: decode what we just wrote. Fails loudly if the helper
    // itself is buggy (so we never ship a fixture that can't round-trip).
    let vk = TrustedVerifyingKey::from_bytes(&PUBLIC).unwrap();
    let decoded: SignedEnvelope<RequestBody> =
        SignedEnvelope::decode(&bytes, &vk).expect("decode round-trip");
    let scope = &decoded.body().scope;
    assert_eq!(
        scope.pointer("/instructions").and_then(|v| v.as_str()),
        Some(FIXTURE_BODY),
        "regenerated fixture is missing scope.instructions"
    );
}

#[test]
fn decode_provisional_scope_instructions_vector() {
    let path = fixture_path();
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "could not read provisional vector at {}: {e}. \
             Regenerate via: cargo test -p famp-envelope --test \
             provisional_scope_instructions_vector -- --ignored \
             write_provisional_scope_instructions_vector --nocapture",
            path.display()
        )
    });
    let vk = TrustedVerifyingKey::from_bytes(&PUBLIC).unwrap();
    let decoded: SignedEnvelope<RequestBody> = SignedEnvelope::decode(&bytes, &vk)
        .expect("provisional vector must verify under RFC 8032 Test-1 pubkey");

    let scope = &decoded.body().scope;
    assert_eq!(
        scope.pointer("/instructions").and_then(|v| v.as_str()),
        Some(FIXTURE_BODY),
        "provisional vector body.scope.instructions drifted"
    );
    assert_eq!(
        decoded.body().natural_language_summary.as_deref(),
        Some(FIXTURE_TITLE),
        "provisional vector body.natural_language_summary drifted"
    );
}

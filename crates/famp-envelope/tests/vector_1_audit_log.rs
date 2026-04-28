//! AUDIT-05 fixture gate: v0.5.2 `audit_log` signed envelope dispatches.

#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp_crypto::TrustedVerifyingKey;
use famp_envelope::{AnySignedEnvelope, FAMP_SPEC_VERSION};
use serde::Deserialize;
use std::fs;

const VECTOR_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vectors/vector_1");

#[derive(Debug, Deserialize)]
struct Keys {
    verifying_key_b64url: String,
}

#[test]
fn audit_log_envelope_roundtrips_at_v0_5_2() {
    let bytes = fs::read(format!("{VECTOR_DIR}/envelope.json")).unwrap();
    let keys: Keys =
        serde_json::from_slice(&fs::read(format!("{VECTOR_DIR}/keys.json")).unwrap()).unwrap();
    let vk = TrustedVerifyingKey::from_b64url(&keys.verifying_key_b64url).unwrap();
    let any = AnySignedEnvelope::decode(&bytes, &vk).expect("v0.5.2 audit_log decode");
    match any {
        AnySignedEnvelope::AuditLog(env) => {
            assert_eq!(FAMP_SPEC_VERSION, "0.5.2");
            assert_eq!(env.body().event, "user_login");
            assert_eq!(
                env.body().subject.as_deref(),
                Some("agent:example.test/alice")
            );
        }
        other => panic!("expected AuditLog dispatch, got {other:?}"),
    }
}

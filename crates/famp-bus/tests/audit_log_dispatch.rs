#![allow(clippy::unwrap_used, unused_crate_dependencies)]

use famp_bus::AnyBusEnvelope;
use serde_json::json;

fn audit_log_envelope() -> serde_json::Value {
    json!({
        "famp": "0.5.2",
        "class": "audit_log",
        "scope": "standalone",
        "id": "01890000-0000-7000-8000-000000000001",
        "from": "agent:example.test/alice",
        "to": "agent:example.test/bob",
        "authority": "advisory",
        "ts": "2026-04-27T12:00:00Z",
        "body": { "event": "user_login" }
    })
}

#[test]
fn anybusenvelope_dispatches_audit_log() {
    let bytes = famp_canonical::canonicalize(&audit_log_envelope()).unwrap();
    let any = AnyBusEnvelope::decode(&bytes).unwrap();
    assert!(matches!(any, AnyBusEnvelope::AuditLog(_)));
}

#[test]
fn anybusenvelope_rejects_signed() {
    let mut json = audit_log_envelope();
    json.as_object_mut()
        .unwrap()
        .insert("signature".into(), serde_json::Value::String("AAAA".into()));
    let bytes = serde_json::to_vec(&json).unwrap();
    let r = AnyBusEnvelope::decode(&bytes);
    assert!(r.is_err());
}

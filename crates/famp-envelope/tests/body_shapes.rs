//! Body-shape tests — Plan 01-02 Tasks 2 and 3.
//!
//! Pure body tests: no envelope header, no signatures, no decode pipeline.
//! Those belong to Plan 03. These tests lock:
//!   * `deny_unknown_fields` at envelope and nested depth
//!   * ENV-09 narrowing — `CommitBody` rejects `capability_snapshot`
//!   * ENV-12 narrowing — `ControlAction` is cancel-only
//!   * cross-field validation on `DeliverBody`
//!   * byte-stable round-trips through `serde_json::Value`

#![allow(clippy::unwrap_used)]

// Dev-dep / workspace-dep acknowledgements (mirrors smoke.rs / errors.rs pattern
// — workspace lint `unused_crate_dependencies` fires per-test-crate).
use famp_canonical as _;
use famp_core as _;
use famp_crypto as _;
use hex as _;
use insta as _;
use proptest as _;
use thiserror as _;

use famp_envelope::body::{
    AckBody, AckDisposition, CommitBody, ControlBody, DeliverBody, RequestBody, TerminalStatus,
};
use famp_envelope::EnvelopeDecodeError;

fn load(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read_to_string(path).unwrap()
}

fn roundtrip_value<T: serde::Serialize + serde::de::DeserializeOwned>(json: &str) {
    let typed: T = serde_json::from_str(json).unwrap();
    let re = serde_json::to_value(&typed).unwrap();
    let orig: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(
        re, orig,
        "round-trip through typed struct must preserve semantic value"
    );
}

// -------------------------------------------------------------------------
// RequestBody
// -------------------------------------------------------------------------

#[test]
fn request_body_roundtrip() {
    let json = load("roundtrip/request.json");
    roundtrip_value::<RequestBody>(&json);
}

#[test]
fn request_body_missing_bounds_fails() {
    let bad = r#"{"scope":{"task":"translate"}}"#;
    let result: Result<RequestBody, _> = serde_json::from_str(bad);
    assert!(result.is_err());
}

// -------------------------------------------------------------------------
// CommitBody — ENV-09 narrowing
// -------------------------------------------------------------------------

#[test]
fn commit_body_roundtrip() {
    let json = load("roundtrip/commit.json");
    roundtrip_value::<CommitBody>(&json);
}

#[test]
fn commit_body_rejects_capability_snapshot() {
    let json = load("adversarial/commit_with_capability_snapshot.json");
    let result: Result<CommitBody, _> = serde_json::from_str(&json);
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("capability_snapshot") || msg.contains("unknown field"),
        "expected serde unknown-field rejection for capability_snapshot, got: {msg}"
    );
}

// -------------------------------------------------------------------------
// DeliverBody — cross-field validation
// -------------------------------------------------------------------------

#[test]
fn deliver_interim_body_roundtrip() {
    let json = load("roundtrip/deliver_interim.json");
    roundtrip_value::<DeliverBody>(&json);
}

#[test]
fn deliver_terminal_body_roundtrip() {
    let json = load("roundtrip/deliver_terminal.json");
    roundtrip_value::<DeliverBody>(&json);
}

#[test]
fn deliver_interim_with_terminal_status_fails() {
    let json = load("roundtrip/deliver_interim.json");
    let body: DeliverBody = serde_json::from_str(&json).unwrap();
    let err = body
        .validate_against_terminal_status(Some(&TerminalStatus::Completed))
        .unwrap_err();
    assert!(matches!(
        err,
        EnvelopeDecodeError::InterimWithTerminalStatus
    ));
}

#[test]
fn deliver_terminal_without_status_fails() {
    let json = load("roundtrip/deliver_terminal.json");
    let body: DeliverBody = serde_json::from_str(&json).unwrap();
    let err = body.validate_against_terminal_status(None).unwrap_err();
    assert!(matches!(err, EnvelopeDecodeError::TerminalWithoutStatus));
}

#[test]
fn deliver_failed_without_error_detail_fails() {
    // A deliver that is non-interim, status=failed, but no error_detail present.
    let bad_json = r#"{"interim": false, "provenance": {"signer": "x"}}"#;
    let body: DeliverBody = serde_json::from_str(bad_json).unwrap();
    let err = body
        .validate_against_terminal_status(Some(&TerminalStatus::Failed))
        .unwrap_err();
    assert!(matches!(err, EnvelopeDecodeError::MissingErrorDetail));
}

#[test]
fn deliver_completed_without_provenance_fails() {
    let bad_json = r#"{"interim": false, "result": {"text": "hi"}}"#;
    let body: DeliverBody = serde_json::from_str(bad_json).unwrap();
    let err = body
        .validate_against_terminal_status(Some(&TerminalStatus::Completed))
        .unwrap_err();
    assert!(matches!(err, EnvelopeDecodeError::MissingProvenance));
}

// -------------------------------------------------------------------------
// Unknown-field at depth (D-D3 nested requirement)
// -------------------------------------------------------------------------

#[test]
fn unknown_body_field_nested_rejected() {
    let json = load("adversarial/unknown_body_field_nested.json");
    let result: Result<RequestBody, _> = serde_json::from_str(&json);
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("evil_key") || msg.contains("unknown field"),
        "expected nested unknown-field rejection, got: {msg}"
    );
}

// -------------------------------------------------------------------------
// AckBody — matches vector 0 body exactly
// -------------------------------------------------------------------------

#[test]
fn ack_body_matches_vector_0_body() {
    let body = AckBody {
        disposition: AckDisposition::Accepted,
        reason: None,
    };
    let s = serde_json::to_string(&body).unwrap();
    assert_eq!(s, r#"{"disposition":"accepted"}"#);
}

#[test]
fn ack_body_all_dispositions_roundtrip_and_reject_unknown() {
    for (wire, variant) in [
        ("accepted", AckDisposition::Accepted),
        ("rejected", AckDisposition::Rejected),
        ("received", AckDisposition::Received),
        ("completed", AckDisposition::Completed),
        ("failed", AckDisposition::Failed),
        ("cancelled", AckDisposition::Cancelled),
    ] {
        let json = format!(r#"{{"disposition":"{wire}"}}"#);
        let decoded: AckBody = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.disposition, variant);
    }
    let bad = r#"{"disposition":"bogus"}"#;
    let result: Result<AckBody, _> = serde_json::from_str(bad);
    assert!(result.is_err());
}

#[test]
fn ack_body_roundtrip_fixture() {
    let json = load("roundtrip/ack.json");
    roundtrip_value::<AckBody>(&json);
}

// -------------------------------------------------------------------------
// ControlBody — ENV-12 cancel-only narrowing
// -------------------------------------------------------------------------

#[test]
fn control_cancel_roundtrip() {
    let json = load("roundtrip/control_cancel.json");
    roundtrip_value::<ControlBody>(&json);
}

#[test]
fn control_supersede_rejected() {
    let json = load("adversarial/control_supersede.json");
    let result: Result<ControlBody, _> = serde_json::from_str(&json);
    let err = result.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("supersede") || msg.contains("unknown variant"),
        "expected ENV-12 narrowing to reject `supersede`, got: {msg}"
    );
}

#[test]
fn control_close_rejected() {
    let bad = r#"{"target":"task","action":"close"}"#;
    let result: Result<ControlBody, _> = serde_json::from_str(bad);
    assert!(result.is_err());
}

#[test]
fn control_cancel_if_not_started_rejected() {
    let bad = r#"{"target":"task","action":"cancel_if_not_started"}"#;
    let result: Result<ControlBody, _> = serde_json::from_str(bad);
    assert!(result.is_err());
}

#[test]
fn control_revert_transfer_rejected() {
    let bad = r#"{"target":"task","action":"revert_transfer"}"#;
    let result: Result<ControlBody, _> = serde_json::from_str(bad);
    assert!(result.is_err());
}

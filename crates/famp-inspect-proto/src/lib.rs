//! FAMP v0.10 Inspector RPC types. No I/O. Types only.
//!
//! Imported by both `famp-inspect-server` (handler crate, mounted by
//! the broker) and `famp-inspect-client` (UDS client). Wire-shape
//! single source of truth for the `famp.inspect.*` RPC namespace.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json as _;

/// Discriminator for a `famp.inspect.*` RPC call. Carried as the
/// `kind` payload of the `BusMessage::Inspect { kind }` variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum InspectKind {
    Broker(InspectBrokerRequest),
    Identities(InspectIdentitiesRequest),
    /// Phase 2 task inspection request.
    Tasks(InspectTasksRequest),
    /// Phase 2 message inspection request.
    Messages(InspectMessagesRequest),
}

// ===== Broker =====

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectBrokerRequest {}

/// Reply to `InspectKind::Broker`.
///
/// The client renders this for `famp inspect broker` against a
/// *running* broker. Dead-broker states are produced entirely
/// client-side and never reach this type (see
/// `famp-inspect-client::peer_pid` and `BrokerDownState`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectBrokerReply {
    pub pid: u32,
    pub socket_path: String,
    /// Wall-clock startup time (Unix epoch seconds, u64).
    /// Set in `BrokerState::new()` (D-07). NEVER socket file mtime
    /// (D-08).
    pub started_at_unix_seconds: u64,
    /// `CARGO_PKG_VERSION` of the answering broker process.
    pub build_version: String,
}

// ===== Identities =====

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectIdentitiesRequest {}

/// One row per registered session identity in `BrokerState.clients`.
///
/// INSP-IDENT-03: this struct MUST NOT contain any field whose name
/// contains `surfaced`, `double`, or `received` (other than
/// `last_received_at_unix_seconds`). The double-print failure mode
/// is not observable at the broker - see SPEC.md INSP-IDENT-03 for
/// the rejected-counter rationale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityRow {
    pub name: String,
    pub listen_mode: bool,
    /// Captured from `BusMessage::Register` at registration time
    /// (D-01 + D-02). Never refreshed: if the client chdir's after
    /// registering, this reflects where the agent was born.
    pub cwd: Option<String>,
    pub registered_at_unix_seconds: u64,
    pub last_activity_unix_seconds: u64,
    pub mailbox_unread: u64,
    pub mailbox_total: u64,
    /// `(none)` when the mailbox is empty.
    pub last_sender: String,
    pub last_received_at_unix_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectIdentitiesReply {
    pub rows: Vec<IdentityRow>,
}

// ===== Tasks reply (Phase 2 - D-01/D-02 wire commitment) =====

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectTasksRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<uuid::Uuid>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub full: bool,
}

/// Reply to `InspectKind::Tasks`.
///
/// **Wire commitment (D-02):** the serde tag form `tag = "kind",
/// rename_all = "snake_case"` is locked as v0.10-era protocol. Do not
/// change tag, content discriminator, or rename style without a
/// deliberate `FAMP_SPEC_VERSION` bump - vector-pack reproducibility
/// when Gate B fires depends on this.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InspectTasksReply {
    /// INSP-TASK-01/02: list of tasks grouped by `task_id`, with orphan
    /// rows surfaced via `TaskRow::orphan`.
    List(TaskListReply),
    /// INSP-TASK-03: envelope chain summary for a specific `task_id`.
    Detail(TaskDetailReply),
    /// INSP-TASK-04: detail plus per-envelope canonical JCS bytes.
    DetailFull(TaskDetailFullReply),
    /// INSP-RPC-03: 500ms budget exceeded before walk+dispatch completed.
    BudgetExceeded { elapsed_ms: u64 },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskListReply {
    pub rows: Vec<TaskRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskRow {
    /// `task_id` as stored in the `TaskRecord`. It is a String rather than
    /// a UUID because orphan rows can use the nil UUID or an invalid/empty id.
    pub task_id: String,
    /// One of `REQUESTED | COMMITTED | COMPLETED | FAILED | CANCELLED`.
    pub state: String,
    pub peer: String,
    pub opened_at_unix_seconds: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_send_at_unix_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_recv_at_unix_seconds: Option<u64>,
    pub terminal: bool,
    pub envelope_count: u64,
    /// Seconds since the most recent of `opened/last_send/last_recv`.
    pub last_transition_age_seconds: u64,
    /// True when `task_id` is nil, empty, or not parseable as a UUID.
    pub orphan: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskDetailReply {
    pub task_id: String,
    pub envelopes: Vec<TaskEnvelopeSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskEnvelopeSummary {
    pub envelope_id: String,
    pub sender: String,
    pub recipient: String,
    /// One of `REQUESTED | COMMITTED | COMPLETED | FAILED | CANCELLED`.
    pub fsm_transition: String,
    pub timestamp: String,
    pub sig_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskDetailFullReply {
    pub task_id: String,
    pub envelopes: Vec<TaskEnvelopeFull>,
}

/// One envelope in the `--full` chain. Per D-09, missing legs carry
/// `bytes: None` and a non-empty `reason`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskEnvelopeFull {
    pub envelope_id: String,
    /// Canonical JCS (RFC 8785) bytes observed by this node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<String>,
    /// Populated iff `bytes.is_none()`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// INSP-TASK-02 orphan criterion: a `task_id` is an orphan when it is
/// the nil UUID, empty, or unparseable as a UUID. Locked by planner per
/// CONTEXT.md "Claude's Discretion" and RESEARCH.md Finding 6.
pub fn is_orphan_task_id(task_id: &str) -> bool {
    if task_id.is_empty() {
        return true;
    }
    uuid::Uuid::parse_str(task_id).map_or(true, |u| u.is_nil())
}

// ===== Messages reply (Phase 2 - D-01/D-02 wire commitment) =====

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectMessagesRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail: Option<u64>,
}

/// Reply to `InspectKind::Messages`.
///
/// **Wire commitment (D-02):** see `InspectTasksReply`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InspectMessagesReply {
    /// INSP-MSG-01/02/03: envelope metadata rows, never message bodies.
    List(MessageListReply),
    /// INSP-RPC-03: 500ms budget exceeded.
    BudgetExceeded { elapsed_ms: u64 },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageListReply {
    pub rows: Vec<MessageRow>,
}

/// Envelope metadata only - never the body itself (INSP-MSG-01).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageRow {
    pub sender: String,
    pub recipient: String,
    /// `causality.ref` if present, else `body.details.task`, else new-task envelope `id`, else empty.
    pub task_id: String,
    pub class: String,
    /// FSM-relevant state derived from envelope fields.
    pub state: String,
    pub timestamp: String,
    pub body_bytes: u64,
    /// First 12 hex chars of `sha256(body_bytes_canonical)`.
    pub body_sha256_prefix: String,
}

// ===== INSP-IDENT-03: schema-level rejection of forbidden field names =====

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod ident_03_schema_tests {
    use super::IdentityRow;

    /// INSP-IDENT-03: enumerate the field names of `IdentityRow` via
    /// `serde_json` round-trip and assert no field name contains
    /// `surfaced` / `double` / `received` (other than the
    /// `last_received_at_unix_seconds` whitelist).
    #[test]
    fn identity_row_has_no_forbidden_fields() {
        let row = IdentityRow {
            name: String::new(),
            listen_mode: false,
            cwd: None,
            registered_at_unix_seconds: 0,
            last_activity_unix_seconds: 0,
            mailbox_unread: 0,
            mailbox_total: 0,
            last_sender: String::new(),
            last_received_at_unix_seconds: None,
        };
        let value = serde_json::to_value(&row).unwrap();
        let object = value.as_object().unwrap();
        for key in object.keys() {
            let lowered = key.to_lowercase();
            assert!(
                !lowered.contains("surfaced"),
                "INSP-IDENT-03 violation: field `{key}` contains `surfaced`"
            );
            assert!(
                !lowered.contains("double"),
                "INSP-IDENT-03 violation: field `{key}` contains `double`"
            );
            let received_ok = key == "last_received_at_unix_seconds";
            assert!(
                received_ok || !lowered.contains("received"),
                "INSP-IDENT-03 violation: field `{key}` contains `received` (whitelist: last_received_at_unix_seconds)"
            );
        }
    }
}

// ===== Codec round-trip smoke (sibling of famp-bus proto.rs roundtrip_busmessage) =====

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod codec_roundtrip {
    use super::*;
    use famp_canonical::canonicalize;

    #[test]
    fn inspectkind_broker_roundtrips() {
        let v = InspectKind::Broker(InspectBrokerRequest::default());
        let bytes = canonicalize(&v).unwrap();
        let decoded: InspectKind = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn inspectkind_identities_roundtrips() {
        let v = InspectKind::Identities(InspectIdentitiesRequest::default());
        let bytes = canonicalize(&v).unwrap();
        let decoded: InspectKind = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn inspectkind_tasks_roundtrips() {
        let v = InspectKind::Tasks(InspectTasksRequest::default());
        let bytes = canonicalize(&v).unwrap();
        let decoded: InspectKind = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn inspectkind_messages_roundtrips() {
        let v = InspectKind::Messages(InspectMessagesRequest::default());
        let bytes = canonicalize(&v).unwrap();
        let decoded: InspectKind = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn inspecttasksreply_list_roundtrips() {
        let v = InspectTasksReply::List(TaskListReply { rows: vec![] });
        let bytes = canonicalize(&v).unwrap();
        let decoded: InspectTasksReply = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["kind"], "list");
    }

    #[test]
    fn inspecttasksreply_detail_roundtrips() {
        let v = InspectTasksReply::Detail(TaskDetailReply {
            task_id: "abc".into(),
            envelopes: vec![],
        });
        let bytes = canonicalize(&v).unwrap();
        let decoded: InspectTasksReply = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn inspecttasksreply_detail_full_roundtrips() {
        let v = InspectTasksReply::DetailFull(TaskDetailFullReply {
            task_id: "abc".into(),
            envelopes: vec![TaskEnvelopeFull {
                envelope_id: "e1".into(),
                bytes: None,
                reason: Some("not_observed_locally".into()),
            }],
        });
        let bytes = canonicalize(&v).unwrap();
        let decoded: InspectTasksReply = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn inspecttasksreply_budget_exceeded_roundtrips() {
        let v = InspectTasksReply::BudgetExceeded { elapsed_ms: 500 };
        let bytes = canonicalize(&v).unwrap();
        let decoded: InspectTasksReply = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["kind"], "budget_exceeded");
        assert_eq!(json["elapsed_ms"], 500);
    }

    #[test]
    fn inspectmessagesreply_list_roundtrips() {
        let v = InspectMessagesReply::List(MessageListReply { rows: vec![] });
        let bytes = canonicalize(&v).unwrap();
        let decoded: InspectMessagesReply = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["kind"], "list");
    }

    #[test]
    fn inspectmessagesreply_budget_exceeded_roundtrips() {
        let v = InspectMessagesReply::BudgetExceeded { elapsed_ms: 500 };
        let bytes = canonicalize(&v).unwrap();
        let decoded: InspectMessagesReply = famp_canonical::from_slice_strict(&bytes).unwrap();
        assert_eq!(v, decoded);
    }

    #[test]
    fn is_orphan_task_id_classifies_correctly() {
        assert!(is_orphan_task_id(""));
        assert!(is_orphan_task_id("00000000-0000-0000-0000-000000000000"));
        assert!(is_orphan_task_id("not-a-uuid"));
        assert!(!is_orphan_task_id("019d9ba2-2d30-7ae2-ba77-9e55863ac7f7"));
    }
}

// REVISION (blocker 3 fix): Prove INSP-TASK-04 Assumption A1.
// The `--full` rendering path canonicalizes a parsed envelope Value
// back into bytes. If `canonicalize(from_slice(canonical_bytes))`
// is NOT byte-for-byte equal to `canonical_bytes`, then `--full`
// output cannot be used to verify a federation peer's signature.
#[cfg(test)]
#[test]
#[allow(clippy::unwrap_used, clippy::expect_used)]
fn canonicalize_roundtrip() {
    const VECTOR_0_HEX: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../famp-envelope/tests/vectors/vector_0/canonical.hex"
    ));

    let original_bytes = decode_hex(VECTOR_0_HEX.trim());
    let parsed: serde_json::Value =
        serde_json::from_slice(&original_bytes).expect("vector 0 must be valid JSON");
    let recanonicalized =
        famp_canonical::canonicalize(&parsed).expect("canonicalization must succeed");

    assert_eq!(
        recanonicalized.as_slice(),
        original_bytes.as_slice(),
        "INSP-TASK-04 Assumption A1 FAILED: canonicalize(from_slice(vector_0)) != vector_0\n\
         left  (re-canonicalized, {} bytes): {}\n\
         right (original vector 0, {} bytes): {}",
        recanonicalized.len(),
        String::from_utf8_lossy(&recanonicalized),
        original_bytes.len(),
        String::from_utf8_lossy(&original_bytes),
    );
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
fn decode_hex(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0, "hex fixture must have even length");
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

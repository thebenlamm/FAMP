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
    /// Phase 2. Server returns `NotYetImplemented` in Phase 1.
    Tasks(InspectTasksRequest),
    /// Phase 2. Server returns `NotYetImplemented` in Phase 1.
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

// ===== Tasks (Phase 2 surface - Phase 1 server returns NotYetImplemented) =====

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectTasksRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<uuid::Uuid>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub full: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectTasksReply {
    /// Phase 1: always `Err("not_yet_implemented")`. Phase 2 fills
    /// this in.
    pub not_yet_implemented: bool,
}

// ===== Messages (Phase 2 surface - Phase 1 server returns NotYetImplemented) =====

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectMessagesRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InspectMessagesReply {
    pub not_yet_implemented: bool,
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
}

//! FAMP v0.10 Inspector RPC server handlers.
//!
//! Mounted by the broker process (Wave 2 wires this into the
//! `BusMessage::Inspect` arm of `Broker::handle`). All handlers
//! are synchronous and take `&BrokerStateView` -- INSP-RPC-02
//! read-only enforcement at compile time.
//!
//! Tokio isolation: this crate does not depend on tokio. Every
//! mailbox read is performed by the broker executor before calling
//! `dispatch`; the resulting metadata is passed in via
//! `BrokerCtx.mailbox_metadata`.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use famp_bus::BrokerStateView;
use famp_canonical as _;
use famp_envelope as _;
use famp_fsm as _;
use famp_inspect_proto::{
    IdentityRow, InspectBrokerReply, InspectIdentitiesReply, InspectKind, InspectMessagesReply,
    InspectTasksReply,
};
use serde as _;

/// Per-mailbox metadata pre-read by the broker executor before
/// calling `dispatch`. Keyed by canonical agent name.
#[derive(Debug, Clone, Default)]
pub struct MailboxMeta {
    pub unread: u64,
    pub total: u64,
    pub last_sender: Option<String>,
    pub last_received_at_unix_seconds: Option<u64>,
}

/// Out-of-band context for inspect handlers. The broker executor
/// fills this in once per inspect call.
#[derive(Debug, Clone)]
pub struct BrokerCtx {
    /// Broker process PID (`std::process::id()` from the broker
    /// executor entry point).
    pub pid: u32,
    /// Resolved socket path the broker is listening on.
    pub socket_path: String,
    /// `CARGO_PKG_VERSION` of the broker process.
    pub build_version: String,
    /// Pre-read mailbox metadata, keyed by registered agent name.
    pub mailbox_metadata: BTreeMap<String, MailboxMeta>,
}

/// Top-level inspector RPC dispatch. Returns the typed reply
/// serialized as a `serde_json::Value` so it can ride back as
/// `BusReply::InspectOk { payload }`.
pub fn dispatch(state: &BrokerStateView, ctx: &BrokerCtx, kind: &InspectKind) -> serde_json::Value {
    match kind {
        InspectKind::Broker(_) => {
            serde_json::to_value(inspect_broker(state, ctx)).unwrap_or(serde_json::Value::Null)
        }
        InspectKind::Identities(_) => {
            serde_json::to_value(inspect_identities(state, ctx)).unwrap_or(serde_json::Value::Null)
        }
        InspectKind::Tasks(_) => serde_json::to_value(InspectTasksReply {
            not_yet_implemented: true,
        })
        .unwrap_or(serde_json::Value::Null),
        InspectKind::Messages(_) => serde_json::to_value(InspectMessagesReply {
            not_yet_implemented: true,
        })
        .unwrap_or(serde_json::Value::Null),
    }
}

/// INSP-BROKER-01: HEALTHY reply. PID, socket path, `started_at`, build version.
fn inspect_broker(state: &BrokerStateView, ctx: &BrokerCtx) -> InspectBrokerReply {
    InspectBrokerReply {
        pid: ctx.pid,
        socket_path: ctx.socket_path.clone(),
        started_at_unix_seconds: to_epoch_seconds(state.started_at),
        build_version: ctx.build_version.clone(),
    }
}

/// INSP-IDENT-01 / INSP-IDENT-02: one row per registered identity.
fn inspect_identities(state: &BrokerStateView, ctx: &BrokerCtx) -> InspectIdentitiesReply {
    let rows = state
        .clients
        .iter()
        .map(|c| {
            let meta = ctx
                .mailbox_metadata
                .get(&c.name)
                .cloned()
                .unwrap_or_default();
            IdentityRow {
                name: c.name.clone(),
                listen_mode: c.listen_mode,
                cwd: c.cwd.clone(),
                registered_at_unix_seconds: to_epoch_seconds(c.registered_at),
                last_activity_unix_seconds: to_epoch_seconds(c.last_activity),
                mailbox_unread: meta.unread,
                mailbox_total: meta.total,
                last_sender: meta.last_sender.unwrap_or_else(|| "(none)".to_string()),
                last_received_at_unix_seconds: meta.last_received_at_unix_seconds,
            }
        })
        .collect();
    InspectIdentitiesReply { rows }
}

fn to_epoch_seconds(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use famp_bus::ClientStateView;

    fn empty_state() -> BrokerStateView {
        BrokerStateView {
            started_at: SystemTime::now(),
            clients: vec![],
        }
    }

    fn ctx_with(pid: u32, sock: &str) -> BrokerCtx {
        BrokerCtx {
            pid,
            socket_path: sock.into(),
            build_version: env!("CARGO_PKG_VERSION").to_string(),
            mailbox_metadata: BTreeMap::new(),
            task_data: None,
            message_data: None,
        }
    }

    #[test]
    fn dispatch_broker_returns_pid_and_socket_path() {
        let state = empty_state();
        let ctx = ctx_with(4242, "/tmp/test.sock");
        let value = dispatch(
            &state,
            &ctx,
            &InspectKind::Broker(famp_inspect_proto::InspectBrokerRequest::default()),
        );
        assert_eq!(value["pid"], 4242);
        assert_eq!(value["socket_path"], "/tmp/test.sock");
        assert!(value["started_at_unix_seconds"].as_u64().unwrap() > 0);
        assert!(!value["build_version"].as_str().unwrap().is_empty());
    }

    #[test]
    fn dispatch_identities_renders_rows_with_mailbox_meta() {
        let now = SystemTime::now();
        let state = BrokerStateView {
            started_at: now,
            clients: vec![ClientStateView {
                name: "alice".into(),
                pid: Some(999),
                bind_as: None,
                cwd: Some("/Users/alice".into()),
                listen_mode: true,
                registered_at: now,
                last_activity: now,
                joined: vec![],
            }],
        };
        let mut meta = BTreeMap::new();
        meta.insert(
            "alice".to_string(),
            MailboxMeta {
                unread: 3,
                total: 5,
                last_sender: Some("bob".into()),
                last_received_at_unix_seconds: Some(to_epoch_seconds(now)),
            },
        );
        let ctx = BrokerCtx {
            pid: 1,
            socket_path: "/tmp/x".into(),
            build_version: "test".into(),
            mailbox_metadata: meta,
            task_data: None,
            message_data: None,
        };
        let value = dispatch(
            &state,
            &ctx,
            &InspectKind::Identities(famp_inspect_proto::InspectIdentitiesRequest::default()),
        );
        let rows = value["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["name"], "alice");
        assert_eq!(rows[0]["listen_mode"], true);
        assert_eq!(rows[0]["cwd"], "/Users/alice");
        assert_eq!(rows[0]["mailbox_unread"], 3);
        assert_eq!(rows[0]["mailbox_total"], 5);
        assert_eq!(rows[0]["last_sender"], "bob");
    }

    #[test]
    fn dispatch_identities_empty_metadata_emits_none_sender() {
        let state = BrokerStateView {
            started_at: SystemTime::now(),
            clients: vec![ClientStateView {
                name: "alice".into(),
                pid: Some(999),
                bind_as: None,
                cwd: None,
                listen_mode: false,
                registered_at: SystemTime::now(),
                last_activity: SystemTime::now(),
                joined: vec![],
            }],
        };
        let ctx = ctx_with(1, "/tmp/x");
        let value = dispatch(
            &state,
            &ctx,
            &InspectKind::Identities(famp_inspect_proto::InspectIdentitiesRequest::default()),
        );
        let rows = value["rows"].as_array().unwrap();
        assert_eq!(rows[0]["mailbox_unread"], 0);
        assert_eq!(rows[0]["mailbox_total"], 0);
        assert_eq!(rows[0]["last_sender"], "(none)");
    }

    #[test]
    fn dispatch_tasks_without_snapshot_returns_empty_list() {
        let state = empty_state();
        let ctx = ctx_with(1, "/tmp/x");
        let value = dispatch(
            &state,
            &ctx,
            &InspectKind::Tasks(famp_inspect_proto::InspectTasksRequest::default()),
        );
        assert_eq!(value["kind"], "list");
        assert_eq!(value["rows"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn dispatch_tasks_returns_list_with_rows_when_snapshot_populated() {
        let state = empty_state();
        let snapshot = TaskSnapshot {
            records: vec![
                TaskSnapshotRow {
                    task_id: "019d9ba2-2d30-7ae2-ba77-9e55863ac7f7".into(),
                    state: "COMMITTED".into(),
                    peer: "agent:local.bus/x".into(),
                    opened_at: "2026-05-10T18:00:00Z".into(),
                    last_send_at: None,
                    last_recv_at: None,
                    terminal: false,
                },
                TaskSnapshotRow {
                    task_id: "00000000-0000-0000-0000-000000000000".into(),
                    state: "COMMITTED".into(),
                    peer: "agent:local.bus/y".into(),
                    opened_at: "2026-05-10T18:01:00Z".into(),
                    last_send_at: None,
                    last_recv_at: None,
                    terminal: false,
                },
            ],
        };
        let ctx = BrokerCtx {
            task_data: Some(snapshot),
            ..ctx_with(1, "/tmp/x")
        };
        let value = dispatch(
            &state,
            &ctx,
            &InspectKind::Tasks(famp_inspect_proto::InspectTasksRequest::default()),
        );
        assert_eq!(value["kind"], "list");
        assert_eq!(value["rows"].as_array().unwrap().len(), 2);
        assert_eq!(value["rows"][0]["orphan"], true);
        assert_eq!(value["rows"][1]["orphan"], false);
    }

    #[test]
    fn dispatch_tasks_orphan_classification_covers_empty_nil_and_valid() {
        let state = empty_state();
        let snapshot = TaskSnapshot {
            records: vec![
                TaskSnapshotRow {
                    task_id: "".into(),
                    state: "REQUESTED".into(),
                    peer: "agent:local.bus/empty".into(),
                    opened_at: "2026-05-10T18:00:00Z".into(),
                    last_send_at: None,
                    last_recv_at: None,
                    terminal: false,
                },
                TaskSnapshotRow {
                    task_id: "00000000-0000-0000-0000-000000000000".into(),
                    state: "REQUESTED".into(),
                    peer: "agent:local.bus/nil".into(),
                    opened_at: "2026-05-10T18:01:00Z".into(),
                    last_send_at: None,
                    last_recv_at: None,
                    terminal: false,
                },
                TaskSnapshotRow {
                    task_id: "019d9ba2-2d30-7ae2-ba77-9e55863ac7f7".into(),
                    state: "COMMITTED".into(),
                    peer: "agent:local.bus/valid".into(),
                    opened_at: "2026-05-10T18:02:00Z".into(),
                    last_send_at: None,
                    last_recv_at: None,
                    terminal: false,
                },
            ],
        };
        let ctx = BrokerCtx {
            task_data: Some(snapshot),
            ..ctx_with(1, "/tmp/x")
        };
        let value = dispatch(
            &state,
            &ctx,
            &InspectKind::Tasks(famp_inspect_proto::InspectTasksRequest::default()),
        );
        let rows = value["rows"].as_array().unwrap();
        assert_eq!(rows[0]["orphan"], true);
        assert_eq!(rows[1]["orphan"], true);
        assert_eq!(rows[2]["orphan"], false);
    }

    #[test]
    fn dispatch_tasks_id_returns_detail_or_detail_full() {
        let state = empty_state();
        let task_id: uuid::Uuid = "019d9ba2-2d30-7ae2-ba77-9e55863ac7f7".parse().unwrap();
        let env = serde_json::json!({
            "id": "env-1",
            "from": "agent:local.bus/alice",
            "to": "agent:local.bus/bob",
            "class": "commit",
            "ts": "2026-05-10T18:00:00Z",
            "causality": { "ref": task_id.to_string() },
            "body": { "details": { "task": task_id.to_string() } }
        });
        let mut by_recipient = BTreeMap::new();
        by_recipient.insert("bob".to_string(), vec![env]);
        let ctx = BrokerCtx {
            task_data: Some(TaskSnapshot::default()),
            message_data: Some(MessageSnapshot { by_recipient }),
            ..ctx_with(1, "/tmp/x")
        };
        let detail = dispatch(
            &state,
            &ctx,
            &InspectKind::Tasks(famp_inspect_proto::InspectTasksRequest {
                id: Some(task_id),
                full: false,
            }),
        );
        assert_eq!(detail["kind"], "detail");
        assert_eq!(detail["envelopes"].as_array().unwrap().len(), 1);

        let full = dispatch(
            &state,
            &ctx,
            &InspectKind::Tasks(famp_inspect_proto::InspectTasksRequest {
                id: Some(task_id),
                full: true,
            }),
        );
        assert_eq!(full["kind"], "detail_full");
        assert!(full["envelopes"][0]["bytes"].as_str().unwrap().contains("\"env-1\""));
    }

    #[test]
    fn dispatch_messages_without_snapshot_returns_empty_list() {
        let state = empty_state();
        let ctx = ctx_with(1, "/tmp/x");
        let value = dispatch(
            &state,
            &ctx,
            &InspectKind::Messages(famp_inspect_proto::InspectMessagesRequest::default()),
        );
        assert_eq!(value["kind"], "list");
        assert_eq!(value["rows"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn dispatch_messages_filters_to_and_applies_tail_in_original_order() {
        let state = empty_state();
        let envs: Vec<_> = (0..5)
            .map(|i| {
                serde_json::json!({
                    "id": format!("env-{i}"),
                    "from": "agent:local.bus/alice",
                    "to": "agent:local.bus/bob",
                    "class": "deliver",
                    "ts": format!("2026-05-10T18:0{i}:00Z"),
                    "body": { "details": { "task": "019d9ba2-2d30-7ae2-ba77-9e55863ac7f7", "mode": "completed", "terminal": true } }
                })
            })
            .collect();
        let mut by_recipient = BTreeMap::new();
        by_recipient.insert("bob".to_string(), envs);
        by_recipient.insert("alice".to_string(), vec![serde_json::json!({ "body": {} })]);
        let ctx = BrokerCtx {
            message_data: Some(MessageSnapshot { by_recipient }),
            ..ctx_with(1, "/tmp/x")
        };
        let value = dispatch(
            &state,
            &ctx,
            &InspectKind::Messages(famp_inspect_proto::InspectMessagesRequest {
                to: Some("bob".into()),
                tail: Some(3),
            }),
        );
        let rows = value["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0]["timestamp"], "2026-05-10T18:02:00Z");
        assert_eq!(rows[2]["timestamp"], "2026-05-10T18:04:00Z");
    }

    #[test]
    fn dispatch_messages_hash_prefix_is_12_hex_chars() {
        let state = empty_state();
        let mut by_recipient = BTreeMap::new();
        by_recipient.insert(
            "alice".to_string(),
            vec![serde_json::json!({
                "from": "agent:local.bus/bob",
                "to": "agent:local.bus/alice",
                "class": "request",
                "ts": "2026-05-10T18:00:00Z",
                "body": { "x": 1 }
            })],
        );
        let ctx = BrokerCtx {
            message_data: Some(MessageSnapshot { by_recipient }),
            ..ctx_with(1, "/tmp/x")
        };
        let value = dispatch(
            &state,
            &ctx,
            &InspectKind::Messages(famp_inspect_proto::InspectMessagesRequest {
                to: Some("alice".into()),
                tail: None,
            }),
        );
        let prefix = value["rows"][0]["body_sha256_prefix"].as_str().unwrap();
        assert_eq!(prefix.len(), 12);
        assert!(prefix.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn derive_fsm_state_maps_completed_correctly() {
        let env = serde_json::json!({
            "class": "deliver",
            "body": { "details": { "mode": "completed", "terminal": true } }
        });
        assert_eq!(derive_fsm_state(&env), "COMPLETED");
    }

    #[test]
    fn derive_fsm_state_maps_failed_correctly() {
        let env = serde_json::json!({
            "class": "deliver",
            "body": { "details": { "mode": "failed", "terminal": true } }
        });
        assert_eq!(derive_fsm_state(&env), "FAILED");
    }

    #[test]
    fn derive_fsm_state_maps_cancelled_correctly() {
        let env = serde_json::json!({
            "class": "control",
            "body": { "details": { "mode": "cancelled" } }
        });
        assert_eq!(derive_fsm_state(&env), "CANCELLED");
    }

    #[test]
    fn derive_fsm_state_maps_committed_for_non_terminal_deliver() {
        let env = serde_json::json!({
            "class": "deliver",
            "body": { "details": { "mode": "in_progress", "terminal": false } }
        });
        assert_eq!(derive_fsm_state(&env), "COMMITTED");
    }
}

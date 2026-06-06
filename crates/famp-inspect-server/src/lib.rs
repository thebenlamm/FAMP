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

use famp_bus::BrokerStateView;
use famp_fsm as _;
use famp_inspect_proto::InspectKind;
use serde as _;

mod broker;
mod identities;
mod messages;
mod parse;
mod tasks;
mod waiters;
pub use messages::message_row;

/// Per-mailbox metadata pre-read by the broker executor before
/// calling `dispatch`. Keyed by canonical agent name.
#[derive(Debug, Clone, Default)]
pub struct MailboxMeta {
    pub unread: u64,
    pub total: u64,
    pub last_sender: Option<String>,
    pub last_received_at_unix_seconds: Option<u64>,
}

/// Pre-walked task data from the broker executor.
///
/// Plain data type (no `famp-taskdir` import in this crate — `INSP-RPC-02`
/// dep-graph gate). The executor converts task records before passing them to
/// [`BrokerCtx`].
#[derive(Debug, Clone, Default)]
pub struct TaskSnapshot {
    pub records: Vec<TaskSnapshotRow>,
}

#[derive(Debug, Clone)]
pub struct TaskSnapshotRow {
    /// `task_id` as stored in the `TaskRecord` - String, not UUID.
    pub task_id: String,
    /// One of `REQUESTED | COMMITTED | COMPLETED | FAILED | CANCELLED`.
    pub state: String,
    pub peer: String,
    /// RFC3339 timestamp string from `TaskRecord.opened_at`.
    pub opened_at: String,
    pub last_send_at: Option<String>,
    pub last_recv_at: Option<String>,
    pub terminal: bool,
}

/// Pre-read mailbox envelopes for registered identities, keyed by
/// canonical agent name.
#[derive(Debug, Clone, Default)]
pub struct MessageSnapshot {
    pub by_recipient: BTreeMap<String, Vec<serde_json::Value>>,
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
    /// D-06 lazy pre-read; `Some` only when `InspectKind::Tasks` arrives.
    pub task_data: Option<TaskSnapshot>,
    /// D-06 lazy pre-read; `Some` only when `InspectKind::Messages` arrives.
    pub message_data: Option<MessageSnapshot>,
}

/// Top-level inspector RPC dispatch. Returns the typed reply
/// serialized as a `serde_json::Value` so it can ride back as
/// `BusReply::InspectOk { payload }`.
pub fn dispatch(state: &BrokerStateView, ctx: &BrokerCtx, kind: &InspectKind) -> serde_json::Value {
    match kind {
        InspectKind::Broker(_) => serde_json::to_value(broker::inspect_broker(state, ctx))
            .unwrap_or(serde_json::Value::Null),
        InspectKind::Identities(_) => {
            serde_json::to_value(identities::inspect_identities(state, ctx))
                .unwrap_or(serde_json::Value::Null)
        }
        InspectKind::Tasks(req) => serde_json::to_value(tasks::inspect_tasks(state, ctx, req))
            .unwrap_or(serde_json::Value::Null),
        InspectKind::Messages(req) => {
            serde_json::to_value(messages::inspect_messages(state, ctx, req))
                .unwrap_or(serde_json::Value::Null)
        }
        InspectKind::Waiters(_) => {
            serde_json::to_value(waiters::inspect_waiters(state)).unwrap_or(serde_json::Value::Null)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::parse::{derive_fsm_state, to_epoch_seconds};
    use famp_bus::ClientStateView;
    use std::time::SystemTime;

    fn empty_state() -> BrokerStateView {
        BrokerStateView {
            started_at: SystemTime::now(),
            clients: vec![],
            waiters: vec![],
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
            waiters: vec![],
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
            waiters: vec![],
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
    fn dispatch_tasks_merges_mailbox_only_orphans_with_taskdir_rows() {
        let state = empty_state();
        let snapshot = TaskSnapshot {
            records: vec![TaskSnapshotRow {
                task_id: "019d9ba2-2d30-7ae2-ba77-9e55863ac7f7".into(),
                state: "COMMITTED".into(),
                peer: "agent:local.bus/known".into(),
                opened_at: "2026-05-10T18:00:00Z".into(),
                last_send_at: None,
                last_recv_at: None,
                terminal: false,
            }],
        };
        let mut by_recipient = BTreeMap::new();
        by_recipient.insert(
            "known".to_string(),
            vec![serde_json::json!({
                "from": "agent:local.bus/alice",
                "to": "agent:local.bus/known",
                "class": "notice",
                "ts": "2026-05-10T18:00:00Z",
                "body": { "details": { "task": "019d9ba2-2d30-7ae2-ba77-9e55863ac7f7" } }
            })],
        );
        by_recipient.insert(
            "orphan".to_string(),
            vec![serde_json::json!({
                "from": "agent:local.bus/alice",
                "to": "agent:local.bus/orphan",
                "class": "notice",
                "ts": "2026-05-10T18:01:00Z",
                "body": { "details": { "task": "00000000-0000-0000-0000-000000000000" } }
            })],
        );
        let ctx = BrokerCtx {
            task_data: Some(snapshot),
            message_data: Some(MessageSnapshot { by_recipient }),
            ..ctx_with(1, "/tmp/x")
        };
        let value = dispatch(
            &state,
            &ctx,
            &InspectKind::Tasks(famp_inspect_proto::InspectTasksRequest::default()),
        );
        let rows = value["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["orphan"], true);
        assert_eq!(rows[0]["task_id"], "00000000-0000-0000-0000-000000000000");
        assert_eq!(rows[1]["orphan"], false);
    }

    #[test]
    fn dispatch_tasks_orphan_classification_covers_empty_nil_and_valid() {
        let state = empty_state();
        let snapshot = TaskSnapshot {
            records: vec![
                TaskSnapshotRow {
                    task_id: String::new(),
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
        let task_id_str = "019d9ba2-2d30-7ae2-ba77-9e55863ac7f7";
        let task_id = task_id_str.parse().unwrap();
        let env = serde_json::json!({
            "id": "env-1",
            "from": "agent:local.bus/alice",
            "to": "agent:local.bus/bob",
            "class": "commit",
            "ts": "2026-05-10T18:00:00Z",
            "causality": { "ref": task_id_str },
            "body": { "details": { "task": task_id_str } }
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
        assert!(full["envelopes"][0]["bytes"]
            .as_str()
            .unwrap()
            .contains("\"env-1\""));
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
    fn dispatch_messages_unfiltered_tail_uses_global_timestamp_order() {
        let state = empty_state();
        let mut by_recipient = BTreeMap::new();
        by_recipient.insert(
            "alice".to_string(),
            vec![serde_json::json!({
                "id": "old-lexical-first",
                "from": "agent:local.bus/bob",
                "to": "agent:local.bus/alice",
                "class": "deliver",
                "ts": "2026-05-10T18:00:00Z",
                "body": { "details": { "task": "019d9ba2-2d30-7ae2-ba77-9e55863ac7f7" } }
            })],
        );
        by_recipient.insert(
            "zed".to_string(),
            vec![serde_json::json!({
                "id": "new-lexical-last",
                "from": "agent:local.bus/bob",
                "to": "agent:local.bus/zed",
                "class": "deliver",
                "ts": "2026-05-10T18:03:00Z",
                "body": { "details": { "task": "019d9ba2-2d30-7ae2-ba77-9e55863ac7f7" } }
            })],
        );
        by_recipient.insert(
            "maria".to_string(),
            vec![serde_json::json!({
                "id": "newest-lexical-middle",
                "from": "agent:local.bus/bob",
                "to": "agent:local.bus/maria",
                "class": "deliver",
                "ts": "2026-05-10T18:05:00Z",
                "body": { "details": { "task": "019d9ba2-2d30-7ae2-ba77-9e55863ac7f7" } }
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
                to: None,
                tail: Some(1),
            }),
        );
        let rows = value["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["recipient"], "agent:local.bus/maria");
        assert_eq!(rows[0]["timestamp"], "2026-05-10T18:05:00Z");
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
    fn envelope_without_task_metadata_has_empty_task_id() {
        let state = empty_state();
        let mut by_recipient = BTreeMap::new();
        by_recipient.insert(
            "alice".to_string(),
            vec![serde_json::json!({
                "id": "ordinary-envelope-id",
                "from": "agent:local.bus/bob",
                "to": "agent:local.bus/alice",
                "class": "notice",
                "ts": "2026-05-10T18:00:00Z",
                "body": { "message": "not a task envelope" }
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
        assert_eq!(value["rows"][0]["task_id"], "");
    }

    #[test]
    fn new_task_audit_log_uses_envelope_id_as_task_id() {
        let state = empty_state();
        let mut by_recipient = BTreeMap::new();
        by_recipient.insert(
            "alice".to_string(),
            vec![serde_json::json!({
                "id": "019d9ba2-2d30-7ae2-ba77-9e55863ac7f7",
                "from": "agent:local.bus/bob",
                "to": "agent:local.bus/alice",
                "class": "audit_log",
                "ts": "2026-05-10T18:00:00Z",
                "body": {
                    "event": "famp.send.new_task",
                    "details": { "mode": "new_task", "summary": "hello" }
                }
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
        assert_eq!(
            value["rows"][0]["task_id"],
            "019d9ba2-2d30-7ae2-ba77-9e55863ac7f7"
        );
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

    #[test]
    fn dispatch_waiters_empty_when_no_pending_awaits() {
        let state = empty_state();
        let ctx = ctx_with(1, "/tmp/x");
        let value = dispatch(
            &state,
            &ctx,
            &InspectKind::Waiters(famp_inspect_proto::InspectWaitersRequest::default()),
        );
        assert_eq!(value["rows"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn dispatch_waiters_returns_waiter_rows() {
        use famp_bus::WaiterStateView;

        let state = BrokerStateView {
            started_at: SystemTime::now(),
            clients: vec![],
            waiters: vec![
                WaiterStateView {
                    name: "alice".into(),
                    mailbox: "alice".into(),
                    cursor: 0,
                    deadline_ms: 30000,
                },
                WaiterStateView {
                    name: "alice".into(),
                    mailbox: "#planning".into(),
                    cursor: 512,
                    deadline_ms: 30000,
                },
            ],
        };
        let ctx = ctx_with(1, "/tmp/x");
        let value = dispatch(
            &state,
            &ctx,
            &InspectKind::Waiters(famp_inspect_proto::InspectWaitersRequest::default()),
        );
        let rows = value["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["name"], "alice");
        assert_eq!(rows[0]["mailbox"], "alice");
        assert_eq!(rows[0]["cursor"], 0);
        assert_eq!(rows[0]["deadline_ms"], 30000_u64);
        assert_eq!(rows[1]["mailbox"], "#planning");
        assert_eq!(rows[1]["cursor"], 512);
    }
}

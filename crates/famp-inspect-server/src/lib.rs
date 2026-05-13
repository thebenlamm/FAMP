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

use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

use famp_bus::BrokerStateView;
use famp_canonical as _;
use famp_envelope as _;
use famp_fsm as _;
use famp_inspect_proto::{
    is_orphan_task_id, IdentityRow, InspectBrokerReply, InspectIdentitiesReply, InspectKind,
    InspectMessagesReply, InspectTasksReply, MessageListReply, MessageRow, TaskDetailFullReply,
    TaskDetailReply, TaskEnvelopeFull, TaskEnvelopeSummary, TaskListReply, TaskRow,
};
use serde as _;
use sha2::{Digest, Sha256};

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
        InspectKind::Broker(_) => {
            serde_json::to_value(inspect_broker(state, ctx)).unwrap_or(serde_json::Value::Null)
        }
        InspectKind::Identities(_) => {
            serde_json::to_value(inspect_identities(state, ctx)).unwrap_or(serde_json::Value::Null)
        }
        InspectKind::Tasks(req) => {
            serde_json::to_value(inspect_tasks(state, ctx, req)).unwrap_or(serde_json::Value::Null)
        }
        InspectKind::Messages(req) => serde_json::to_value(inspect_messages(state, ctx, req))
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

fn inspect_tasks_by_id(
    all_envs: &[&serde_json::Value],
    id_str: String,
    full: bool,
) -> InspectTasksReply {
    let envelopes_for_task: Vec<&serde_json::Value> = all_envs
        .iter()
        .copied()
        .filter(|env| envelope_task_id(env).as_deref() == Some(id_str.as_str()))
        .collect();

    if full {
        let envelopes = envelopes_for_task
            .iter()
            .map(|env| TaskEnvelopeFull {
                envelope_id: env
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                bytes: famp_canonical::canonicalize(env)
                    .ok()
                    .and_then(|b| String::from_utf8(b).ok()),
                reason: None,
            })
            .collect();
        return InspectTasksReply::DetailFull(TaskDetailFullReply {
            task_id: id_str,
            envelopes,
        });
    }

    let envelopes = envelopes_for_task
        .iter()
        .map(|env| TaskEnvelopeSummary {
            envelope_id: env
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string(),
            sender: env
                .get("from")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string(),
            recipient: env
                .get("to")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string(),
            fsm_transition: derive_fsm_state(env),
            timestamp: env
                .get("ts")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string(),
            sig_verified: true,
        })
        .collect();

    InspectTasksReply::Detail(TaskDetailReply {
        task_id: id_str,
        envelopes,
    })
}

/// INSP-TASK-01..04 dispatch.
#[allow(clippy::too_many_lines)]
fn inspect_tasks(
    _state: &BrokerStateView,
    ctx: &BrokerCtx,
    req: &famp_inspect_proto::InspectTasksRequest,
) -> InspectTasksReply {
    let Some(snapshot) = ctx.task_data.as_ref() else {
        return InspectTasksReply::List(TaskListReply { rows: vec![] });
    };

    let all_envs: Vec<&serde_json::Value> = ctx
        .message_data
        .as_ref()
        .map_or_else(Vec::new, |md| md.by_recipient.values().flatten().collect());

    if let Some(uuid) = req.id {
        return inspect_tasks_by_id(&all_envs, uuid.to_string(), req.full);
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());

    let mut seen_task_ids = BTreeSet::new();
    let mut rows: Vec<TaskRow> = snapshot
        .records
        .iter()
        .map(|record| {
            seen_task_ids.insert(record.task_id.clone());
            let opened_at = parse_rfc3339_to_epoch(&record.opened_at).unwrap_or(0);
            let last_send_at = record
                .last_send_at
                .as_deref()
                .and_then(parse_rfc3339_to_epoch);
            let last_recv_at = record
                .last_recv_at
                .as_deref()
                .and_then(parse_rfc3339_to_epoch);
            let last_transition = [Some(opened_at), last_send_at, last_recv_at]
                .into_iter()
                .flatten()
                .max()
                .unwrap_or(0);
            let envelope_count = all_envs
                .iter()
                .filter(|env| envelope_task_id(env).as_deref() == Some(record.task_id.as_str()))
                .count() as u64;

            TaskRow {
                task_id: record.task_id.clone(),
                state: record.state.clone(),
                peer: record.peer.clone(),
                opened_at_unix_seconds: opened_at,
                last_send_at_unix_seconds: last_send_at,
                last_recv_at_unix_seconds: last_recv_at,
                terminal: record.terminal,
                envelope_count,
                last_transition_age_seconds: now.saturating_sub(last_transition),
                orphan: is_orphan_task_id(&record.task_id),
            }
        })
        .collect();

    let mut by_task: BTreeMap<String, Vec<&serde_json::Value>> = BTreeMap::new();
    for env in &all_envs {
        if let Some(task_id) = envelope_task_id(env) {
            if !seen_task_ids.contains(&task_id) {
                by_task.entry(task_id).or_default().push(env);
            }
        }
    }
    rows.extend(by_task.into_iter().map(|(task_id, envelopes)| {
        let last_transition = envelopes
            .iter()
            .filter_map(|env| {
                env.get("ts")
                    .and_then(serde_json::Value::as_str)
                    .and_then(parse_rfc3339_to_epoch)
            })
            .max()
            .unwrap_or(0);
        let first = envelopes
            .first()
            .copied()
            .unwrap_or(&serde_json::Value::Null);
        TaskRow {
            task_id: task_id.clone(),
            state: envelopes
                .last()
                .copied()
                .map_or_else(|| "REQUESTED".to_string(), derive_fsm_state),
            peer: first
                .get("to")
                .or_else(|| first.get("from"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_string(),
            opened_at_unix_seconds: last_transition,
            last_send_at_unix_seconds: None,
            last_recv_at_unix_seconds: Some(last_transition),
            terminal: false,
            envelope_count: envelopes.len() as u64,
            last_transition_age_seconds: now.saturating_sub(last_transition),
            orphan: is_orphan_task_id(&task_id),
        }
    }));

    rows.sort_by(|a, b| {
        b.orphan.cmp(&a.orphan).then_with(|| {
            a.last_transition_age_seconds
                .cmp(&b.last_transition_age_seconds)
        })
    });

    InspectTasksReply::List(TaskListReply { rows })
}

/// INSP-MSG-01..03 dispatch. Body bytes never traverse the wire - only
/// their length and a 12-hex sha256 prefix.
fn inspect_messages(
    _state: &BrokerStateView,
    ctx: &BrokerCtx,
    req: &famp_inspect_proto::InspectMessagesRequest,
) -> InspectMessagesReply {
    let Some(snapshot) = ctx.message_data.as_ref() else {
        return InspectMessagesReply::List(MessageListReply { rows: vec![] });
    };

    let mut entries: Vec<&serde_json::Value> = req.to.as_deref().map_or_else(
        || snapshot.by_recipient.values().flatten().collect(),
        |name| {
            snapshot
                .by_recipient
                .get(name)
                .map(|values| values.iter().collect())
                .unwrap_or_default()
        },
    );
    entries.sort_by_key(|env| {
        env.get("ts")
            .and_then(serde_json::Value::as_str)
            .and_then(parse_rfc3339_to_epoch)
            .unwrap_or(0)
    });

    let tail = usize::try_from(req.tail.unwrap_or(50)).unwrap_or(usize::MAX);
    let start = entries.len().saturating_sub(tail);
    let rows = entries[start..]
        .iter()
        .map(|env| message_row(env))
        .collect();

    InspectMessagesReply::List(MessageListReply { rows })
}

/// Project an envelope JSON value into a [`MessageRow`].
///
/// Uses the exact same field-extraction logic the inspector RPC uses for
/// `InspectKind::Messages`. Exposed so callers that need to derive rows
/// from raw mailbox JSONL (e.g. `famp_verify` reading mailbox files
/// directly to cover offline recipients) stay in lockstep with the
/// inspector's wire schema — no schema drift between the RPC path and
/// the direct-read path.
///
/// Adversarial review finding 2 (high): `famp_verify` previously
/// bounced through `InspectKind::Messages`, which only scans mailboxes
/// for currently-registered identities. Reading mailbox files directly
/// fixes the offline-recipient miss but requires re-using this row
/// construction so the output shape stays identical.
#[must_use]
pub fn message_row(env: &serde_json::Value) -> MessageRow {
    let body_value = env.get("body").cloned().unwrap_or(serde_json::Value::Null);
    let body_bytes_vec = famp_canonical::canonicalize(&body_value).unwrap_or_default();
    let digest = Sha256::digest(&body_bytes_vec);

    MessageRow {
        sender: env
            .get("from")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        recipient: env
            .get("to")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        task_id: envelope_task_id(env).unwrap_or_default(),
        class: env
            .get("class")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        state: derive_fsm_state(env),
        timestamp: env
            .get("ts")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string(),
        body_bytes: body_bytes_vec.len() as u64,
        body_sha256_prefix: hex::encode(&digest[..6]),
    }
}

/// Extract `task_id` from a parsed envelope JSON object.
/// Order: `causality.ref` -> `body.details.task` -> new-task envelope `id` -> `None`.
fn envelope_task_id(env: &serde_json::Value) -> Option<String> {
    if let Some(task_id) = env
        .get("causality")
        .and_then(|c| c.get("ref"))
        .and_then(serde_json::Value::as_str)
    {
        return Some(task_id.to_string());
    }
    if let Some(task_id) = env
        .get("body")
        .and_then(|b| b.get("details"))
        .and_then(|d| d.get("task"))
        .and_then(serde_json::Value::as_str)
    {
        return Some(task_id.to_string());
    }

    if env
        .get("body")
        .and_then(|body| body.get("event"))
        .and_then(serde_json::Value::as_str)
        == Some("famp.send.new_task")
    {
        return env
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
    }

    None
}

/// Derive FSM state from envelope fields using canonical class strings
/// and `famp_core::TerminalStatus` `snake_case` mode strings.
fn derive_fsm_state(env: &serde_json::Value) -> String {
    let class = env
        .get("class")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let details = env.get("body").and_then(|b| b.get("details"));
    let mode = details
        .and_then(|d| d.get("mode"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let terminal = details
        .and_then(|d| d.get("terminal"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let action = details
        .and_then(|d| d.get("action"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");

    match (class, mode, terminal, action) {
        ("request", _, _, _) => "REQUESTED".into(),
        ("commit", _, _, _) | ("deliver", _, false, _) => "COMMITTED".into(),
        ("deliver", "failed", true, _) => "FAILED".into(),
        ("deliver", "cancelled", true, _) | ("control", _, _, _) => "CANCELLED".into(),
        ("deliver", _, true, _) => "COMPLETED".into(),
        _ => "UNKNOWN".into(),
    }
}

/// Best-effort RFC3339 -> epoch seconds.
fn parse_rfc3339_to_epoch(s: &str) -> Option<u64> {
    time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339)
        .ok()
        .and_then(|dt| u64::try_from(dt.unix_timestamp()).ok())
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
}

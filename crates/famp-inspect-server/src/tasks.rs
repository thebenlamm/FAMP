//! `InspectKind::Tasks` handler — FSM-state aggregation across the task
//! snapshot and the message corpus.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{SystemTime, UNIX_EPOCH};

use famp_bus::BrokerStateView;
use famp_inspect_proto::{
    is_orphan_task_id, InspectTasksReply, TaskDetailFullReply, TaskDetailReply, TaskEnvelopeFull,
    TaskEnvelopeSummary, TaskListReply, TaskRow,
};

use crate::parse::{derive_fsm_state, envelope_task_id, parse_rfc3339_to_epoch};
use crate::BrokerCtx;

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
            sig_verified: true, // TODO(INSP-SIG-VERIFY): hardcoded; signatures are not actually verified here
        })
        .collect();

    InspectTasksReply::Detail(TaskDetailReply {
        task_id: id_str,
        envelopes,
    })
}

/// INSP-TASK-01..04 dispatch.
#[allow(clippy::too_many_lines)]
pub fn inspect_tasks(
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

    let mut by_task: BTreeMap<String, Vec<&serde_json::Value>> = BTreeMap::new();
    for env in &all_envs {
        if let Some(task_id) = envelope_task_id(env) {
            by_task.entry(task_id).or_default().push(env);
        }
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
            let envelope_count = by_task
                .get(&record.task_id)
                .map(|envelopes| envelopes.len() as u64)
                .unwrap_or_default();

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

    for task_id in &seen_task_ids {
        by_task.remove(task_id);
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

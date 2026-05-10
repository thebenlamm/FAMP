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

/// INSP-BROKER-01: HEALTHY reply. PID, socket path, started_at, build version.
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
        }
    }

    #[test]
    fn dispatch_broker_returns_pid_and_socket_path() {
        let state = empty_state();
        let ctx = ctx_with(4242, "/tmp/test.sock");
        let value = dispatch(&state, &ctx, &InspectKind::Broker(Default::default()));
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
        };
        let value = dispatch(&state, &ctx, &InspectKind::Identities(Default::default()));
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
        let value = dispatch(&state, &ctx, &InspectKind::Identities(Default::default()));
        let rows = value["rows"].as_array().unwrap();
        assert_eq!(rows[0]["mailbox_unread"], 0);
        assert_eq!(rows[0]["mailbox_total"], 0);
        assert_eq!(rows[0]["last_sender"], "(none)");
    }

    #[test]
    fn dispatch_tasks_returns_not_yet_implemented() {
        let state = empty_state();
        let ctx = ctx_with(1, "/tmp/x");
        let value = dispatch(&state, &ctx, &InspectKind::Tasks(Default::default()));
        assert_eq!(value["not_yet_implemented"], true);
    }

    #[test]
    fn dispatch_messages_returns_not_yet_implemented() {
        let state = empty_state();
        let ctx = ctx_with(1, "/tmp/x");
        let value = dispatch(&state, &ctx, &InspectKind::Messages(Default::default()));
        assert_eq!(value["not_yet_implemented"], true);
    }
}

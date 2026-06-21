//! `InspectKind::Identities` handler.

use famp_bus::BrokerStateView;
use famp_inspect_proto::{IdentityListReply, IdentityRow, InspectIdentitiesReply};

use crate::parse::to_epoch_seconds;
use crate::BrokerCtx;

/// INSP-IDENT-01 / INSP-IDENT-02: one row per registered identity.
pub fn inspect_identities(state: &BrokerStateView, ctx: &BrokerCtx) -> InspectIdentitiesReply {
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
    InspectIdentitiesReply::List(IdentityListReply { rows })
}

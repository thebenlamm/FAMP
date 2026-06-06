//! `InspectKind::Waiters` handler.

use famp_bus::BrokerStateView;
use famp_inspect_proto::{InspectWaitersReply, WaiterRow};

/// INSP-WAIT-01: one row per (parked await × subscribed mailbox).
///
/// The view already fans out rows in `BrokerStateView.waiters`, so this
/// handler is just a projection of the pre-computed view into the reply type.
pub fn inspect_waiters(state: &BrokerStateView) -> InspectWaitersReply {
    let rows = state
        .waiters
        .iter()
        .map(|w| WaiterRow {
            name: w.name.clone(),
            mailbox: w.mailbox.clone(),
            cursor: w.cursor,
            deadline_ms: w.deadline_ms,
        })
        .collect();
    InspectWaitersReply { rows }
}

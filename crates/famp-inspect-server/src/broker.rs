//! `InspectKind::Broker` handler.

use famp_bus::BrokerStateView;
use famp_inspect_proto::{BrokerInfoReply, InspectBrokerReply};

use crate::parse::to_epoch_seconds;
use crate::BrokerCtx;

/// INSP-BROKER-01: HEALTHY reply. PID, socket path, `started_at`, build version.
pub fn inspect_broker(state: &BrokerStateView, ctx: &BrokerCtx) -> InspectBrokerReply {
    InspectBrokerReply::Info(BrokerInfoReply {
        pid: ctx.pid,
        socket_path: ctx.socket_path.clone(),
        started_at_unix_seconds: to_epoch_seconds(state.started_at),
        build_version: ctx.build_version.clone(),
    })
}

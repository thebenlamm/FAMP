//! `famp_whoami` MCP tool — Phase 02 plan 02-09 implementation.
//!
//! FREE-PASS tool (D-05): does NOT require an active identity. Used to
//! introspect session state — most importantly to confirm whether
//! `famp_register` has taken effect.
//!
//! Opens the bus client lazily via [`session::ensure_bus`] (idempotent;
//! per D-10 the connection is canonical-holder shape with `bind_as: None`)
//! and forwards `BusMessage::Whoami {}` to the broker. The broker returns
//! `WhoamiOk { active, joined }` reflecting THIS connection's bound name
//! (or `None` if the session has not registered yet) plus the canonical
//! holder's `joined` channel set.
//!
//! ## Output shape
//!
//! ```json
//! { "active": "<name>"|null, "joined": ["#x", "#y"] }
//! ```

use famp_bus::{BusErrorKind, BusMessage, BusReply};
use serde_json::Value;

use crate::cli::mcp::session;
use crate::cli::mcp::tools::ToolError;

/// Dispatch a `famp_whoami` tool call.
pub async fn call(_input: &Value) -> Result<Value, ToolError> {
    session::ensure_bus()
        .await
        .map_err(|kind| ToolError::new(kind, "failed to connect to local broker"))?;

    let mut guard = session::state().lock().await;
    let Some(bus) = guard.bus.as_mut() else {
        return Err(ToolError::new(
            BusErrorKind::BrokerUnreachable,
            "bus connection closed concurrently",
        ));
    };
    let reply_result = bus.send_recv(BusMessage::Whoami {}).await;
    drop(guard);
    let reply = reply_result.map_err(|e| {
        ToolError::new(
            BusErrorKind::BrokerUnreachable,
            format!("broker round-trip failed: {e:?}"),
        )
    })?;
    match reply {
        BusReply::WhoamiOk { active, joined } => Ok(serde_json::json!({
            "active": active,
            "joined": joined,
        })),
        BusReply::Err { kind, message } => Err(ToolError::new(kind, message)),
        other => Err(ToolError::new(
            BusErrorKind::Internal,
            format!("unexpected reply to Whoami: {other:?}"),
        )),
    }
}

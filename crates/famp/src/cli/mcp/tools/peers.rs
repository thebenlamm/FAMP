//! `famp_peers` MCP tool — Phase 02 plan 02-09 implementation.
//!
//! Sends `BusMessage::Sessions {}` to the broker and projects the
//! resulting `SessionRow` table into a v0.9 "online peers" shape:
//!
//! ```json
//! { "online": ["alice", "bob", ...] }
//! ```
//!
//! This is the only tool in the `tools/*` set that does NOT delegate to a
//! CLI `run_at_structured` entry point — `cli::sessions::run_at_structured`
//! returns full `SessionRow` rows (name + pid + joined), but the v0.8 MCP
//! surface keeps `famp_peers` simple by returning just the live names.

use famp_bus::{BusErrorKind, BusMessage, BusReply};
use serde_json::Value;

use crate::cli::mcp::session;
use crate::cli::mcp::tools::ToolError;

/// Dispatch a `famp_peers` tool call.
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
    let reply_result = bus.send_recv(BusMessage::Sessions {}).await;
    drop(guard);
    let reply = reply_result.map_err(|e| {
        ToolError::new(
            BusErrorKind::BrokerUnreachable,
            format!("broker round-trip failed: {e:?}"),
        )
    })?;
    match reply {
        BusReply::SessionsOk { rows } => {
            let online: Vec<String> = rows.into_iter().map(|r| r.name).collect();
            Ok(serde_json::json!({ "online": online }))
        }
        BusReply::Err { kind, message } => Err(ToolError::new(kind, message)),
        other => Err(ToolError::new(
            BusErrorKind::Internal,
            format!("unexpected reply to Sessions: {other:?}"),
        )),
    }
}

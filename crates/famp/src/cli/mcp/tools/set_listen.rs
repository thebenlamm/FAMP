//! `famp_set_listen` MCP tool — Fix 1 (2026-05-12).
//!
//! Flip the canonical holder's listen-mode flag on the broker without
//! re-registering. Re-registering would re-drain the mailbox from
//! offset 0 (Register drain-from-start); `set_listen` is the cheap
//! in-place mutation an agent uses to opt into/out of Stop-hook
//! auto-wake mid-session.
//!
//! Sends `BusMessage::SetListen { listen }` to the local broker via
//! the lazily-opened `BusClient` from `cli::mcp::session`. The
//! dispatcher's pre-registration gate (D-05) ensures this tool is only
//! reachable after a successful `famp_register`; proxy connections are
//! rejected broker-side with `NotRegistered` (slot ownership is
//! canonical-holder-only).
//!
//! ## Output shape
//!
//! ```json
//! { "listen_mode": <bool> }
//! ```
//!
//! Echoes the post-mutation flag so the caller can confirm without
//! issuing a separate `famp_inspect` round-trip.

use famp_bus::{BusErrorKind, BusMessage, BusReply};
use serde_json::Value;

use crate::cli::mcp::session;
use crate::cli::mcp::tools::ToolError;

/// Dispatch a `famp_set_listen` tool call.
pub async fn call(input: &Value) -> Result<Value, ToolError> {
    let listen = input
        .get("listen")
        .and_then(Value::as_bool)
        .ok_or_else(|| {
            ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                "missing required field: listen (boolean)",
            )
        })?;

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
    let reply_result = bus.send_recv(BusMessage::SetListen { listen }).await;
    drop(guard);
    let reply = reply_result.map_err(|e| {
        ToolError::new(
            BusErrorKind::BrokerUnreachable,
            format!("broker round-trip failed: {e:?}"),
        )
    })?;
    match reply {
        BusReply::SetListenOk { listen_mode } => Ok(serde_json::json!({
            "listen_mode": listen_mode,
        })),
        BusReply::Err { kind, message } => Err(ToolError::new(kind, message)),
        other => Err(ToolError::new(
            BusErrorKind::Internal,
            format!("unexpected reply to SetListen: {other:?}"),
        )),
    }
}

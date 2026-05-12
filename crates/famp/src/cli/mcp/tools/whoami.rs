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
//! {
//!   "active": "<name>"|null,
//!   "joined": ["#x", "#y"],
//!   "last_send": {                       // optional
//!     "task_id":        "<uuidv7>",      // SendOk.task_id (new envelope id)
//!     "thread_task_id": "<uuidv7>",      // reply-mode only: originating thread id
//!     "to_peer":        "bob",           // mutually exclusive with to_channel
//!     "to_channel":     "#planning",     // mutually exclusive with to_peer
//!     "ts":             "<rfc3339-utc>"
//!   }
//! }
//! ```
//!
//! ### `last_send` (resilience hook)
//!
//! `last_send` is included only when this session has performed at least
//! one successful `famp_send`. It mirrors the `task_id` + recipient
//! recorded by `tools::send::call` on the `Ok` arm of the broker round-trip.
//!
//! Purpose: Claude Code's stdio MCP transport occasionally surfaces
//! `[Tool result missing due to internal error]` to the model even
//! though the underlying JSON-RPC call succeeded (the broker delivered;
//! the JSON-RPC response was simply dropped on the way back). When that
//! happens the agent has no `task_id` to thread a reply against and no
//! way to confirm whether to retry. `famp_whoami` now returns `last_send`
//! so the agent can recover: read the `task_id`, then call `famp_verify`
//! to confirm the message landed before deciding whether to resend.

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
        BusReply::WhoamiOk { active, joined } => {
            // Read the last-send recovery hint (session-local, no broker
            // round-trip). Cheap on every whoami; harmless when absent
            // (None pre-first-send). Serialized via Serialize on
            // `LastSend` so the optional `to_peer` / `to_channel`
            // discriminant comes through cleanly.
            let last_send = session::last_send().await;
            let mut out = serde_json::json!({
                "active": active,
                "joined": joined,
            });
            if let Some(ls) = last_send {
                if let Some(obj) = out.as_object_mut() {
                    let v = serde_json::to_value(&ls).map_err(|e| {
                        ToolError::new(
                            BusErrorKind::Internal,
                            format!("failed to serialize last_send: {e}"),
                        )
                    })?;
                    obj.insert("last_send".to_string(), v);
                }
            }
            Ok(out)
        }
        BusReply::Err { kind, message } => Err(ToolError::new(kind, message)),
        other => Err(ToolError::new(
            BusErrorKind::Internal,
            format!("unexpected reply to Whoami: {other:?}"),
        )),
    }
}

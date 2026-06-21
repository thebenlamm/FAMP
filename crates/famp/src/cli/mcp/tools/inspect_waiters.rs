//! `famp_inspect_waiters` MCP tool.
//!
//! Read-only; works without registration (same as `famp_verify` and
//! `famp_channel_log`). Connects to the broker via the inspect RPC and
//! returns the waiter rows directly.
//!
//! ## Output shape
//!
//! ```json
//! { "rows": [{ "name": "alice", "mailbox": "#planning", "cursor": 1024, "deadline_ms": 82000 }] }
//! ```

use famp_bus::BusErrorKind;
use famp_inspect_client::{call as inspect_call, raw_connect_probe, ProbeOutcome};
use famp_inspect_proto::{InspectKind, InspectWaitersReply, InspectWaitersRequest};
use serde_json::Value;

use crate::bus_client::resolve_sock_path;
use crate::cli::mcp::tools::ToolError;

/// Dispatch a `famp_inspect_waiters` tool call.
pub async fn call(_input: &Value) -> Result<Value, ToolError> {
    let sock = resolve_sock_path();

    let ProbeOutcome::Healthy { mut stream } = raw_connect_probe(&sock).await else {
        return Err(ToolError::new(
            BusErrorKind::BrokerUnreachable,
            "broker not running",
        ));
    };

    let payload = inspect_call(
        &mut stream,
        InspectKind::Waiters(InspectWaitersRequest::default()),
    )
    .await
    .map_err(|e| ToolError::new(BusErrorKind::Internal, format!("inspect waiters rpc: {e}")))?;

    let reply: InspectWaitersReply = serde_json::from_value(payload).map_err(|e| {
        ToolError::new(
            BusErrorKind::Internal,
            format!("waiters reply schema mismatch: {e}"),
        )
    })?;

    match reply {
        InspectWaitersReply::List(list) => {
            Ok(serde_json::to_value(&list).unwrap_or_else(|_| serde_json::json!({"rows": []})))
        }
        InspectWaitersReply::BudgetExceeded { elapsed_ms } => Err(ToolError::new(
            BusErrorKind::Internal,
            format!("inspect budget exceeded ({elapsed_ms}ms) — broker busy, retry"),
        )),
    }
}

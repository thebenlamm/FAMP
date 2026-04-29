//! `famp_leave` MCP tool — NEW in Phase 02 plan 02-09.
//!
//! Thin wrapper over `cli::leave::run_at_structured`. Sends
//! `BusMessage::Leave { channel }` to the broker and surfaces the typed
//! outcome.
//!
//! ## Input contract
//!
//! - `channel: string` — required. Accepts both `"#planning"` and
//!   `"planning"` (CLI's `normalize_channel` adds the leading `#`).
//!
//! ## Output shape
//!
//! ```json
//! { "channel": "#planning" }
//! ```

use famp_bus::BusErrorKind;
use serde_json::Value;

use crate::bus_client::resolve_sock_path;
use crate::cli::error::CliError;
use crate::cli::leave::{run_at_structured, LeaveArgs};
use crate::cli::mcp::tools::ToolError;

/// Dispatch a `famp_leave` tool call.
pub async fn call(input: &Value) -> Result<Value, ToolError> {
    let channel = input
        .get("channel")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                "missing required field: channel (string)",
            )
        })?
        .to_string();

    let args = LeaveArgs {
        channel,
        act_as: None,
    };
    match run_at_structured(&resolve_sock_path(), args).await {
        Ok(out) => Ok(serde_json::json!({ "channel": out.channel })),
        Err(CliError::BusError { kind, message }) => Err(ToolError::new(kind, message)),
        Err(CliError::NotRegisteredHint { .. }) => Err(ToolError::not_registered()),
        Err(CliError::BrokerUnreachable) => Err(ToolError::new(
            BusErrorKind::BrokerUnreachable,
            "broker unreachable",
        )),
        Err(e) => Err(ToolError::new(BusErrorKind::Internal, e.to_string())),
    }
}

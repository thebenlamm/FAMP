//! `famp_join` MCP tool — NEW in Phase 02 plan 02-09.
//!
//! Thin wrapper over `cli::join::run_at_structured`. Sends
//! `BusMessage::Join { channel }` to the broker and surfaces the typed
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
//! { "channel": "#planning", "members": ["alice","bob"], "drained": <count> }
//! ```
//!
//! `drained` is the *count* of typed envelopes the broker drained on
//! join (Phase-1 D-09 wire shape; the MCP tool surfaces only the count
//! to match `cli::join`'s ergonomics).

use famp_bus::BusErrorKind;
use serde_json::Value;

use crate::bus_client::resolve_sock_path;
use crate::cli::error::CliError;
use crate::cli::join::{run_at_structured, JoinArgs};
use crate::cli::mcp::session;
use crate::cli::mcp::tools::ToolError;

/// Dispatch a `famp_join` tool call.
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

    let args = JoinArgs {
        channel,
        // Carry MCP session's bound identity through so
        // `cli::join::run_at_structured`'s `resolve_identity()` does not
        // fall back to wires.tsv. dispatch_tool guarantees
        // active_identity is Some by this point.
        act_as: session::active_identity().await,
    };
    match run_at_structured(&resolve_sock_path(), args).await {
        Ok(out) => Ok(serde_json::json!({
            "channel": out.channel,
            "members": out.members,
            "drained": out.drained.len(),
        })),
        Err(CliError::BusError { kind, message }) => Err(ToolError::new(kind, message)),
        Err(CliError::NotRegisteredHint { .. }) => Err(ToolError::not_registered()),
        Err(CliError::BrokerUnreachable) => Err(ToolError::new(
            BusErrorKind::BrokerUnreachable,
            "broker unreachable",
        )),
        Err(e) => Err(ToolError::new(BusErrorKind::Internal, e.to_string())),
    }
}

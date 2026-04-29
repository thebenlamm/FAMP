//! `famp_await` MCP tool — Phase 02 plan 02-09 implementation.
//!
//! Thin wrapper over `cli::await_cmd::run_at_structured`. Sends
//! `BusMessage::Await { timeout_ms, task }` to the broker and returns one
//! of two output shapes:
//!
//! ```json
//! { "envelope": <typed-envelope> }   // on AwaitOk
//! { "timeout": true }                // on AwaitTimeout
//! ```
//!
//! ## Input contract
//!
//! - `timeout_seconds: u64` — optional, default 30 (matches the CLI's
//!   `--timeout 30s`).
//! - `task_id: string (uuid)` — optional. When present, the broker
//!   returns only envelopes whose task matches.

use famp_bus::BusErrorKind;
use serde_json::Value;

use crate::bus_client::resolve_sock_path;
use crate::cli::await_cmd::{run_at_structured, AwaitArgs};
use crate::cli::error::CliError;
use crate::cli::mcp::session;
use crate::cli::mcp::tools::ToolError;

/// Dispatch a `famp_await` tool call.
pub async fn call(input: &Value) -> Result<Value, ToolError> {
    let timeout_secs: u64 = match input.get("timeout_seconds") {
        None | Some(Value::Null) => 30,
        Some(Value::Number(n)) => n.as_u64().ok_or_else(|| {
            ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                "field timeout_seconds must be a non-negative integer",
            )
        })?,
        Some(_) => {
            return Err(ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                "field timeout_seconds must be a non-negative integer",
            ));
        }
    };
    let timeout = humantime::Duration::from(std::time::Duration::from_secs(timeout_secs));

    let task = match input.get("task_id") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(uuid::Uuid::parse_str(s).map_err(|_| {
            ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                format!("field task_id is not a valid UUID: {s:?}"),
            )
        })?),
        Some(_) => {
            return Err(ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                "field task_id must be a UUID string",
            ));
        }
    };

    let args = AwaitArgs {
        timeout,
        task,
        // Carry MCP session's bound identity through so
        // `cli::await_cmd::run_at_structured`'s `resolve_identity()` does
        // not fall back to wires.tsv. dispatch_tool guarantees
        // active_identity is Some by this point.
        act_as: session::active_identity().await,
    };

    match run_at_structured(&resolve_sock_path(), args).await {
        Ok(out) if out.timed_out => Ok(serde_json::json!({ "timeout": true })),
        Ok(out) => Ok(out.envelope.map_or_else(
            || serde_json::json!({ "timeout": true }),
            |env| serde_json::json!({ "envelope": env }),
        )),
        Err(CliError::BusError { kind, message }) => Err(ToolError::new(kind, message)),
        Err(CliError::NotRegisteredHint { .. }) => Err(ToolError::not_registered()),
        Err(CliError::BrokerUnreachable) => Err(ToolError::new(
            BusErrorKind::BrokerUnreachable,
            "broker unreachable",
        )),
        Err(e) => Err(ToolError::new(BusErrorKind::Internal, e.to_string())),
    }
}

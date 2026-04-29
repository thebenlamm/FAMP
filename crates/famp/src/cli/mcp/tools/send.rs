//! `famp_send` MCP tool — Phase 02 plan 02-09 implementation.
//!
//! Thin wrapper over `cli::send::run_at_structured`: parses the v0.8 MCP
//! input shape (`peer` / `channel` / `mode` / `task_id` / `title` / `body`
//! / `more_coming`), builds a [`SendArgs`], and delegates the bus
//! round-trip to the canonical CLI implementation.
//!
//! ## Output shape
//!
//! ```json
//! { "task_id": "<uuidv7>", "delivered": "<debug>" }
//! ```

use famp_bus::BusErrorKind;
use serde_json::Value;

use crate::bus_client::resolve_sock_path;
use crate::cli::error::CliError;
use crate::cli::mcp::tools::ToolError;
use crate::cli::send::{run_at_structured, SendArgs};

/// Dispatch a `famp_send` tool call.
pub async fn call(input: &Value) -> Result<Value, ToolError> {
    let args = parse_input(input)?;
    match run_at_structured(&resolve_sock_path(), args).await {
        Ok(out) => Ok(serde_json::json!({
            "task_id": out.task_id,
            "delivered": out.delivered,
        })),
        Err(CliError::BusError { kind, message }) => Err(ToolError::new(kind, message)),
        Err(CliError::NotRegisteredHint { .. }) => Err(ToolError::not_registered()),
        Err(CliError::BrokerUnreachable) => Err(ToolError::new(
            BusErrorKind::BrokerUnreachable,
            "broker unreachable",
        )),
        Err(CliError::SendArgsInvalid { reason }) => {
            Err(ToolError::new(BusErrorKind::EnvelopeInvalid, reason))
        }
        Err(e) => Err(ToolError::new(BusErrorKind::Internal, e.to_string())),
    }
}

/// Parse the v0.8 MCP `famp_send` input shape into a [`SendArgs`].
///
/// Strict typing for `more_coming`: if the field is present and not a JSON
/// boolean, reject with a message naming the field and expected type so
/// `mcp_malformed_input::mcp_famp_send_rejects_non_bool_more_coming` can
/// observe the field-name + "boolean" substrings in the error response.
fn parse_input(input: &Value) -> Result<SendArgs, ToolError> {
    let mode = input.get("mode").and_then(Value::as_str).ok_or_else(|| {
        ToolError::new(
            BusErrorKind::EnvelopeInvalid,
            "missing required field: mode (string)",
        )
    })?;

    let peer = input
        .get("peer")
        .and_then(Value::as_str)
        .map(str::to_string);
    let channel = input
        .get("channel")
        .and_then(Value::as_str)
        .map(str::to_string);
    if peer.is_none() && channel.is_none() {
        return Err(ToolError::new(
            BusErrorKind::EnvelopeInvalid,
            "exactly one of peer or channel is required",
        ));
    }

    let task_id = input
        .get("task_id")
        .and_then(Value::as_str)
        .map(str::to_string);

    let title = input
        .get("title")
        .and_then(Value::as_str)
        .map(str::to_string);
    let body = input
        .get("body")
        .and_then(Value::as_str)
        .map(str::to_string);

    // STRICT: more_coming MUST be a JSON boolean if present.
    let more_coming = match input.get("more_coming") {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => {
            return Err(ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                "field more_coming must be a boolean",
            ));
        }
    };

    let (new_task, task, terminal) = match mode {
        "new_task" => {
            let summary = title
                .as_deref()
                .or(body.as_deref())
                .unwrap_or_default()
                .to_string();
            (Some(summary), None, false)
        }
        "deliver" => (None, task_id, false),
        "terminal" => (None, task_id, true),
        other => {
            return Err(ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                format!("invalid mode {other:?}: expected new_task | deliver | terminal"),
            ));
        }
    };

    Ok(SendArgs {
        to: peer,
        channel,
        new_task,
        task,
        terminal,
        body,
        more_coming,
        act_as: None,
    })
}

//! `famp_inbox` MCP tool — Phase 02 plan 02-09 implementation.
//!
//! Thin wrapper over `cli::inbox::list::run_at_structured`. Sends
//! `BusMessage::Inbox` to the broker and surfaces the typed envelopes +
//! cursor.
//!
//! ## Input contract
//!
//! - `action: "list" | "ack"` — required (v0.8 surface compatibility).
//!   Currently only `"list"` is wired through the bus path; `"ack"` is
//!   handled client-side.
//! - `since: u64` — optional cursor offset, default 0.
//! - `include_terminal: bool` — optional, default `false` per MCP-04.
//!   STRICT bool — a non-bool surfaces `EnvelopeInvalid` with a message
//!   naming both the field and the expected type so MCP clients can
//!   self-correct.
//!
//! ## Output shape
//!
//! ```json
//! { "entries": [<envelope>, ...], "next_offset": <u64> }
//! ```
//!
//! `entries` (NOT `envelopes`) preserves the v0.8 MCP-tool output
//! convention so existing clients/tests do not need to be re-shaped on
//! this field name. Each entry is the typed envelope `serde_json::Value`
//! straight from the broker, with `task_id` accessible via the FAMP
//! envelope's `causality.ref` projection (test fixture-driven).

use famp_bus::BusErrorKind;
use serde_json::Value;

use crate::bus_client::resolve_sock_path;
use crate::cli::error::CliError;
use crate::cli::inbox::list::{run_at_structured, ListArgs};
use crate::cli::mcp::session;
use crate::cli::mcp::tools::ToolError;

/// Dispatch a `famp_inbox` tool call.
pub async fn call(input: &Value) -> Result<Value, ToolError> {
    // `since`: optional u64. Default 0 (broker treats None as 0 too).
    let since = match input.get("since") {
        None | Some(Value::Null) => None,
        Some(Value::Number(n)) => n.as_u64(),
        Some(_) => {
            return Err(ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                "field since must be a non-negative integer",
            ));
        }
    };

    // STRICT: include_terminal MUST be a JSON boolean if present.
    let include_terminal = match input.get("include_terminal") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => {
            return Err(ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                "field include_terminal must be a boolean",
            ));
        }
    };

    let args = ListArgs {
        since,
        include_terminal,
        // Carry MCP session's bound identity through so
        // `cli::inbox::list::run_at_structured`'s `resolve_identity()`
        // does not fall back to wires.tsv. dispatch_tool guarantees
        // active_identity is Some by this point.
        act_as: session::active_identity().await,
    };

    match run_at_structured(&resolve_sock_path(), args).await {
        Ok(out) => {
            // Project each envelope into the v0.8 MCP-tool entry shape:
            // include `task_id` (extracted from `causality.ref`) at the
            // top level alongside the raw envelope, so tests/clients can
            // grab `task_id` without re-walking the envelope structure.
            let entries: Vec<Value> = out
                .envelopes
                .iter()
                .map(|env| {
                    let task_id = env
                        .get("causality")
                        .and_then(|c| c.get("ref"))
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    serde_json::json!({
                        "task_id": task_id,
                        "envelope": env,
                    })
                })
                .collect();
            Ok(serde_json::json!({
                "entries": entries,
                "next_offset": out.next_offset,
            }))
        }
        Err(CliError::BusError { kind, message }) => Err(ToolError::new(kind, message)),
        Err(CliError::NotRegisteredHint { .. }) => Err(ToolError::not_registered()),
        Err(CliError::BrokerUnreachable) => Err(ToolError::new(
            BusErrorKind::BrokerUnreachable,
            "broker unreachable",
        )),
        Err(e) => Err(ToolError::new(BusErrorKind::Internal, e.to_string())),
    }
}

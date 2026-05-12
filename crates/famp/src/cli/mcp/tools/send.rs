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
//! {
//!   "task_id": "<uuidv7>",
//!   "delivered": "<debug>",
//!   "delivered_rows": [{"to_kind": "agent", "to_name": "alice", "ok": true, "woken": true}],
//!   "woken": true
//! }
//! ```
//!
//! `woken` is true iff at least one recipient row in `delivered_rows`
//! reports `woken: true` — i.e. at least one recipient was parked on
//! `famp_await` at the moment the message landed and got woken with
//! `AwaitOk`. `false` means no recipient was actively listening; the
//! message is in the mailbox awaiting the next `Inbox` / `Await`.
//!
//! Caller policy: surface `woken` to the user for visibility only — do
//! not alter timeout, retry, or back-off behavior based on this field.

use famp_bus::BusErrorKind;
use serde_json::Value;

use crate::bus_client::resolve_sock_path;
use crate::cli::error::CliError;
use crate::cli::mcp::session::{self, LastSend};
use crate::cli::mcp::tools::ToolError;
use crate::cli::send::{run_at_structured, SendArgs};

/// Dispatch a `famp_send` tool call.
pub async fn call(input: &Value) -> Result<Value, ToolError> {
    let mut args = parse_input(input)?;
    // Carry the MCP session's bound identity through so
    // `cli::send::run_at_structured`'s `resolve_identity()` (D-01) does not
    // fall through to the cwd-based wires.tsv path. The dispatch_tool
    // gate (server.rs) guarantees active_identity is Some by the time we
    // reach this code path.
    args.act_as = session::active_identity().await;
    // Capture the target before `args` is moved into `run_at_structured` so
    // we can stamp `LastSend.to_peer` / `to_channel` on success. This is
    // the resilience hook for the Claude Code "Tool result missing due to
    // internal error" failure mode: the broker delivers, but the model
    // never sees the response. After such a drop the agent calls
    // `famp_whoami` to learn `task_id` + recipient, then `famp_verify` to
    // confirm delivery before deciding whether to retry.
    let to_peer = args.to.clone();
    let to_channel = args.channel.clone();
    // `thread_task_id` captures the ORIGINATING task uuid for reply-mode
    // sends. The inspector keys reply envelopes by `causality.ref`, not
    // by the reply's own envelope id, so `famp_verify` needs the thread
    // id to find the row. For `open` (new-task) mode `args.task` is
    // None and we leave the field unset — the SendOk task_id and the
    // inspector's row task_id coincide for new-task envelopes.
    let thread_task_id = args.task.clone();
    match run_at_structured(&resolve_sock_path(), args).await {
        Ok(out) => {
            let woken_any = out.delivered_rows.iter().any(|row| row.woken);
            // Record last-send AFTER the broker confirmed `SendOk` (we
            // reach this arm only when `run_at_structured` returned Ok).
            // Timestamp uses the same RFC 3339 second-precision shape as
            // the envelope path in `cli::send::build_envelope_value` so
            // operators see a consistent format across both surfaces.
            let ts = time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_else(|_| "<unformatted>".to_string());
            session::set_last_send(LastSend {
                task_id: out.task_id.clone(),
                thread_task_id,
                to_peer,
                to_channel,
                ts,
            })
            .await;
            Ok(serde_json::json!({
                "task_id": out.task_id,
                "delivered": out.delivered,
                "delivered_rows": out.delivered_rows,
                "woken": woken_any,
            }))
        }
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

    // STRICT: expect_reply MUST be a JSON boolean if present.
    let expect_reply = match input.get("expect_reply") {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::Bool(true)) => true,
        Some(_) => {
            return Err(ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                "field expect_reply must be a boolean",
            ));
        }
    };

    let (new_task, task, terminal) = match mode {
        // Preferred: open starts a thread; reply closes it by default.
        "open" | "new_task" => {
            let summary = title
                .as_deref()
                .or(body.as_deref())
                .unwrap_or_default()
                .to_string();
            (Some(summary), None, false)
        }
        // reply closes the thread unless expect_reply: true keeps it open.
        "reply" => (None, task_id, !expect_reply),
        // Legacy aliases kept for backward compatibility.
        "deliver" => (None, task_id, false),
        "terminal" | "deliver_terminal" => (None, task_id, true),
        other => {
            return Err(ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                format!(
                    "invalid mode {other:?}: expected open | reply | new_task | deliver | terminal"
                ),
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
        // Filled in by `call()` from `session::active_identity()` after
        // `parse_input` returns. Left as `None` here so this helper stays
        // pure (no async / no session access).
        act_as: None,
    })
}

//! `famp_verify` MCP tool — resilience hook for the Claude Code
//! `[Tool result missing due to internal error]` failure mode.
//!
//! ## Why this tool exists
//!
//! Claude Code's stdio MCP transport occasionally drops the
//! `tools/call` JSON-RPC response on its way back to the model, even
//! when the broker successfully processed the request. For `famp_send`
//! this means the recipient's mailbox has the message but the sender
//! agent has no `task_id` and no idea whether to retry.
//!
//! `famp_verify` lets an agent confirm delivery WITHOUT re-sending:
//! given a `task_id` (recovered from `famp_whoami.last_send`, or
//! remembered out-of-band), it asks the broker's inspector RPC whether
//! that `task_id` appears in the recipient's mailbox metadata.
//!
//! ## Why FREE-PASS (no `famp_register` required)
//!
//! Recovery must work even when session-state has been lost (cold
//! restart, fresh window after a crash). The verify path uses the
//! inspector socket directly via `famp_inspect_client::connect_and_call`,
//! which performs its own `Hello { bind_as: None }` handshake. No
//! per-session identity binding is involved. So `server.rs::dispatch_tool`
//! routes this tool through the FREE-PASS arm alongside
//! `famp_register` and `famp_whoami`.
//!
//! ## Input shape
//!
//! ```json
//! { "task_id": "<uuidv7>", "peer": "bob" }   // peer optional
//! ```
//!
//! - `task_id` (required): the `UUIDv7` to look up. The right value
//!   depends on what was sent:
//!   - For `mode="open"` (new task) sends, the inspector keys envelopes
//!     by the envelope's own `id`, so `famp_send.task_id` ==
//!     `famp_whoami.last_send.task_id` == what to pass here.
//!   - For `mode="reply"` sends, the inspector keys reply envelopes by
//!     `causality.ref` (the thread's originating task id). The reply's
//!     own envelope id is NOT directly visible in
//!     `MessageRow.task_id`. To verify a reply landed, pass the
//!     ORIGINATING thread's task_id — surfaced for recovery as
//!     `famp_whoami.last_send.thread_task_id`.
//!
//!   When in doubt: pass `last_send.thread_task_id` if present, else
//!   `last_send.task_id`. A positive `delivered: true` confirms an
//!   envelope on that thread reached the recipient — which is the
//!   question the agent's recovery flow actually needs answered.
//! - `peer` (optional): if present, narrows the inspector query to that
//!   recipient's mailbox. Strongly recommended — without it the
//!   inspector returns the broker-wide envelope log and the tool scans
//!   linearly. With it the scan is mailbox-scoped.
//!
//! ## Output shape
//!
//! ```json
//! {
//!   "delivered": true,
//!   "task_id":   "<uuidv7>",
//!   "row":       { "sender": "...", "recipient": "...", "class": "...",
//!                  "state": "...", "timestamp": "...", "body_bytes": 42,
//!                  "body_sha256_prefix": "..." }
//! }
//! ```
//!
//! When `delivered: false`, the `row` field is omitted. The `task_id`
//! is echoed back so a hung-up retry loop has a stable handle.

use famp_bus::BusErrorKind;
use famp_inspect_client::{connect_and_call, InspectClientError};
use famp_inspect_proto::{InspectKind, InspectMessagesReply, InspectMessagesRequest, MessageRow};
use serde_json::Value;

use crate::bus_client::resolve_sock_path;
use crate::cli::mcp::tools::ToolError;

/// Dispatch a `famp_verify` tool call.
pub async fn call(input: &Value) -> Result<Value, ToolError> {
    let task_id = input
        .get("task_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                "missing required field: task_id (string)",
            )
        })?
        .to_string();
    if task_id.is_empty() {
        return Err(ToolError::new(
            BusErrorKind::EnvelopeInvalid,
            "task_id must not be empty",
        ));
    }
    let peer = input
        .get("peer")
        .and_then(Value::as_str)
        .map(str::to_string);

    let sock = resolve_sock_path();
    let payload = connect_and_call(
        &sock,
        InspectKind::Messages(InspectMessagesRequest {
            to: peer.clone(),
            tail: None,
        }),
    )
    .await
    .map_err(|e| match e {
        InspectClientError::Io(_) => ToolError::new(
            BusErrorKind::BrokerUnreachable,
            "broker not running (inspector socket unreachable)",
        ),
        // `connect_and_call` returns UnexpectedReply("broker not running")
        // for any of DownClean/StaleSocket/OrphanHolder. Project all
        // those onto BrokerUnreachable since the agent's recovery path is
        // identical: nothing to retry against until the broker is up.
        InspectClientError::UnexpectedReply(msg) => {
            ToolError::new(BusErrorKind::BrokerUnreachable, msg)
        }
        InspectClientError::FrameTooLarge => ToolError::new(
            BusErrorKind::Internal,
            "inspector frame exceeded MAX_FRAME_BYTES",
        ),
        InspectClientError::Canonical(msg) => ToolError::new(
            BusErrorKind::Internal,
            format!("inspector canonical-JSON error: {msg}"),
        ),
    })?;

    let reply: InspectMessagesReply = serde_json::from_value(payload).map_err(|e| {
        ToolError::new(
            BusErrorKind::Internal,
            format!("inspect messages reply schema mismatch: {e}"),
        )
    })?;

    match reply {
        InspectMessagesReply::List(list) => Ok(scan_for_task(task_id, peer.as_deref(), list.rows)),
        InspectMessagesReply::BudgetExceeded { elapsed_ms } => Err(ToolError::new(
            BusErrorKind::Internal,
            format!("inspector budget exceeded after {elapsed_ms}ms"),
        )),
    }
}

/// Find the first envelope row whose `task_id` matches. If `peer` is
/// supplied the inspector already filtered by recipient, but defense-in-
/// depth: callers passing a `peer` that does not match any envelope row
/// must still see `delivered: false`. The inspector's `to` filter
/// matches `recipient` exactly (per `famp_inspect_server`'s
/// `gather_messages`); we trust it and don't re-filter here.
fn scan_for_task(task_id: String, _peer: Option<&str>, rows: Vec<MessageRow>) -> Value {
    let hit = rows.into_iter().find(|row| row.task_id == task_id);
    match hit {
        Some(row) => {
            // MessageRow is `#[derive(Serialize)]` via the proto crate;
            // round-trip it through serde_json so the output shape matches
            // the inspect-proto wire schema verbatim.
            let row_v = serde_json::to_value(&row).unwrap_or_else(|_| Value::Null);
            serde_json::json!({
                "delivered": true,
                "task_id":   task_id,
                "row":       row_v,
            })
        }
        None => serde_json::json!({
            "delivered": false,
            "task_id":   task_id,
        }),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn row(task_id: &str, recipient: &str) -> MessageRow {
        MessageRow {
            sender: "agent:local.bus/alice".into(),
            recipient: recipient.into(),
            task_id: task_id.into(),
            class: "audit_log".into(),
            state: "COMMITTED".into(),
            timestamp: "2026-05-12T18:00:00Z".into(),
            body_bytes: 42,
            body_sha256_prefix: "a1b2c3d4e5f6".into(),
        }
    }

    #[test]
    fn scan_for_task_finds_match_and_emits_full_row() {
        let rows = vec![
            row("aaaa", "agent:local.bus/bob"),
            row("bbbb", "agent:local.bus/bob"),
        ];
        let out = scan_for_task("bbbb".into(), Some("bob"), rows);
        assert_eq!(out["delivered"], Value::Bool(true));
        assert_eq!(out["task_id"], Value::String("bbbb".into()));
        // Row is echoed back with proto field names intact.
        assert_eq!(out["row"]["recipient"], "agent:local.bus/bob");
        assert_eq!(out["row"]["task_id"], "bbbb");
    }

    #[test]
    fn scan_for_task_returns_not_delivered_when_absent() {
        let rows = vec![row("aaaa", "agent:local.bus/bob")];
        let out = scan_for_task("missing".into(), Some("bob"), rows);
        assert_eq!(out["delivered"], Value::Bool(false));
        assert_eq!(out["task_id"], Value::String("missing".into()));
        assert!(
            out.get("row").is_none(),
            "row must be omitted on miss: {out}"
        );
    }

    #[test]
    fn scan_for_task_handles_empty_mailbox() {
        let out = scan_for_task("anything".into(), None, vec![]);
        assert_eq!(out["delivered"], Value::Bool(false));
    }
}

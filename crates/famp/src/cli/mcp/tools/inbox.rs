//! `famp_inbox` MCP tool — wraps `cli::inbox::{list, ack}` structured
//! entry points (Phase 02 plan 02-05 reshape).
//!
//! Input shape (JSON):
//! ```json
//! {
//!   "action": "list" | "ack",
//!   "since":  123,         // optional byte offset for list
//!   "include_terminal": false, // optional bool for list; default false
//!   "offset": 456          // required for ack
//! }
//! ```
//!
//! Output shape for `list`:
//! ```json
//! { "envelopes": [ ... typed envelopes ... ], "next_offset": 789 }
//! ```
//!
//! Output shape for `ack`:
//! ```json
//! { "acked": true, "offset": 456 }
//! ```
//!
//! NOTE: plan 02-09 will rewire this to call into a session-bound
//! `BusClient` instead of resolving identity per-call. The shape here is
//! the v0.9 wire shape; the session-binding plumbing is owned by 02-09.

use serde_json::Value;

use crate::bus_client::resolve_sock_path;
use crate::cli::error::CliError;
use crate::cli::inbox::{ack, list};
use crate::cli::mcp::session::IdentityBinding;

/// Dispatch a `famp_inbox` tool call.
pub async fn call(binding: &IdentityBinding, input: &Value) -> Result<Value, CliError> {
    let action = input["action"]
        .as_str()
        .ok_or_else(|| CliError::SendArgsInvalid {
            reason: "famp_inbox: missing required field 'action'".to_string(),
        })?;

    let sock = resolve_sock_path();

    match action {
        "list" => {
            let since = input["since"].as_u64();
            let include_terminal = match input.get("include_terminal") {
                None | Some(Value::Null) => false,
                Some(Value::Bool(b)) => *b,
                Some(_) => {
                    return Err(CliError::SendArgsInvalid {
                        reason: "famp_inbox: 'include_terminal' must be a boolean".to_string(),
                    });
                }
            };
            let args = list::ListArgs {
                since,
                include_terminal,
                act_as: Some(binding.identity.clone()),
            };
            let outcome = list::run_at_structured(&sock, args).await?;
            Ok(serde_json::json!({
                "envelopes": outcome.envelopes,
                "next_offset": outcome.next_offset,
            }))
        }
        "ack" => {
            let offset = input["offset"]
                .as_u64()
                .ok_or_else(|| CliError::SendArgsInvalid {
                    reason: "famp_inbox action=ack requires 'offset'".to_string(),
                })?;
            let args = ack::AckArgs {
                offset,
                act_as: Some(binding.identity.clone()),
            };
            let outcome = ack::run_at_structured(&sock, args).await?;
            Ok(serde_json::json!({
                "acked": outcome.acked,
                "offset": outcome.offset,
            }))
        }
        other => Err(CliError::SendArgsInvalid {
            reason: format!("famp_inbox: unknown action '{other}'; expected list|ack"),
        }),
    }
}

//! `famp_send` MCP tool — wraps `cli::send::run_at_structured`.
//!
//! Input shape (JSON):
//! ```json
//! {
//!   "peer": "<alias>",
//!   "mode": "new_task" | "deliver" | "terminal",
//!   "task_id": "<uuid>",   // required for deliver / terminal
//!   "title":   "<text>",   // new_task: natural-language summary (short)
//!   "body":    "<text>",   // new_task: full task content (lands in scope.instructions)
//!   "more_coming": true    // new_task only: signal more briefing follows
//! }
//! ```
//!
//! Output shape on success:
//! ```json
//! { "task_id": "<uuid>", "delivered": "<debug>" }
//! ```
//!
//! ## Phase 02 Plan 02-04 transition
//!
//! Plan 02-04 swapped `cli::send` from the v0.8 HTTPS path to the v0.9
//! UDS bus path. `run_at_structured` now takes a broker socket path
//! (not a `FAMP_HOME`) and returns `SendOutcome { task_id, delivered }`
//! (no `state` field — the bus broker reports `Vec<Delivered>`, not a
//! local FSM state). Plan 02-09 will rewire the MCP layer end-to-end
//! against the bus; this file is the minimal patch needed to keep the
//! workspace compiling between 02-04 and 02-09. The MCP tool's tool-call
//! semantics in the interim:
//!
//! - The session's bound identity is forwarded via `--as` (D-10 proxy)
//!   so the broker's per-op liveness check has a name to look up. The
//!   long-running MCP server is itself a registered holder (per D-10),
//!   so this proxy hop is local-host trivial.
//! - The `home` path on `IdentityBinding` is no longer used by `send`;
//!   the broker socket is resolved via `bus_client::resolve_sock_path`.

use serde_json::Value;

use crate::cli::error::CliError;
use crate::cli::mcp::session::IdentityBinding;
use crate::cli::send::{run_at_structured, SendArgs};

/// Dispatch a `famp_send` tool call.
///
/// `input` is the `arguments` object from the MCP `tools/call` request.
/// Returns a JSON value suitable for embedding in the MCP content array.
pub async fn call(binding: &IdentityBinding, input: &Value) -> Result<Value, CliError> {
    // Resolve the bus socket once. Plan 02-09 will replace the binding's
    // `home` field with `bus_client::BusClient` directly; for now the
    // socket comes from the standard resolver.
    let sock = crate::bus_client::resolve_sock_path();
    // The MCP session's bound identity becomes the D-10 proxy `bind_as`.
    // `IdentityBinding.identity` carries the name set by `famp_register`.
    let act_as = Some(binding.identity.clone());
    let peer = input["peer"]
        .as_str()
        .ok_or_else(|| CliError::SendArgsInvalid {
            reason: "famp_send: missing required field 'peer'".to_string(),
        })?
        .to_string();

    let mode = input["mode"]
        .as_str()
        .ok_or_else(|| CliError::SendArgsInvalid {
            reason: "famp_send: missing required field 'mode'".to_string(),
        })?;

    // Title doubles as the new_task summary; body is optional free-form text.
    let title = input["title"].as_str().map(str::to_string);
    let body_text = input["body"].as_str().map(str::to_string);
    let task_id_str = input["task_id"].as_str().map(str::to_string);
    // `more_coming` is meaningful only in new_task mode (clap's `requires`
    // attribute enforces this on the CLI path; the MCP path can't lean on
    // clap, so we silently ignore it for deliver/terminal). Quick-260425-pc7.
    //
    // Type-strict per the BL-02 / `famp_inbox_list_rejects_non_bool_include_terminal`
    // precedent: silent coercion of `"true"` / `1` / `null` / `{}` to false
    // is exactly the failure mode the project already chose to reject on
    // sibling MCP tools. Mirror that contract here.
    let more_coming = match input.get("more_coming") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => {
            return Err(CliError::SendArgsInvalid {
                reason: "famp_send: 'more_coming' must be a boolean".to_string(),
            });
        }
    };

    let args = match mode {
        "new_task" => SendArgs {
            to: Some(peer),
            channel: None,
            new_task: title.or_else(|| body_text.clone()),
            task: None,
            terminal: false,
            body: body_text,
            more_coming,
            act_as: act_as.clone(),
        },
        "deliver" => SendArgs {
            to: Some(peer),
            channel: None,
            new_task: None,
            task: Some(task_id_str.ok_or_else(|| CliError::SendArgsInvalid {
                reason: "famp_send mode=deliver requires 'task_id'".to_string(),
            })?),
            terminal: false,
            body: body_text,
            more_coming: false,
            act_as: act_as.clone(),
        },
        "terminal" => SendArgs {
            to: Some(peer),
            channel: None,
            new_task: None,
            task: Some(task_id_str.ok_or_else(|| CliError::SendArgsInvalid {
                reason: "famp_send mode=terminal requires 'task_id'".to_string(),
            })?),
            terminal: true,
            body: body_text,
            more_coming: false,
            act_as,
        },
        other => {
            return Err(CliError::SendArgsInvalid {
                reason: format!(
                    "famp_send: unknown mode '{other}'; expected new_task|deliver|terminal"
                ),
            });
        }
    };

    let outcome = run_at_structured(&sock, args).await?;
    Ok(serde_json::json!({
        "task_id":   outcome.task_id,
        "delivered": outcome.delivered,
    }))
}

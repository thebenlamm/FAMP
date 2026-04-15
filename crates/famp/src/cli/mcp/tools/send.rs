//! `famp_send` MCP tool — wraps `cli::send::run_at_structured`.
//!
//! Input shape (JSON):
//! ```json
//! {
//!   "peer": "<alias>",
//!   "mode": "new_task" | "deliver" | "terminal",
//!   "task_id": "<uuid>",   // required for deliver / terminal
//!   "title":   "<text>",   // used as new_task summary or body
//!   "body":    "<text>"    // optional free-form body text
//! }
//! ```
//!
//! Output shape on success:
//! ```json
//! { "task_id": "<uuid>", "state": "<state>" }
//! ```

use std::path::Path;

use serde_json::Value;

use crate::cli::error::CliError;
use crate::cli::send::{run_at_structured, SendArgs};

/// Dispatch a `famp_send` tool call.
///
/// `input` is the `arguments` object from the MCP `tools/call` request.
/// Returns a JSON value suitable for embedding in the MCP content array.
pub async fn call(home: &Path, input: &Value) -> Result<Value, CliError> {
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

    let args = match mode {
        "new_task" => SendArgs {
            to: peer,
            new_task: title.or_else(|| body_text.clone()),
            task: None,
            terminal: false,
            body: body_text,
        },
        "deliver" => SendArgs {
            to: peer,
            new_task: None,
            task: Some(task_id_str.ok_or_else(|| CliError::SendArgsInvalid {
                reason: "famp_send mode=deliver requires 'task_id'".to_string(),
            })?),
            terminal: false,
            body: body_text,
        },
        "terminal" => SendArgs {
            to: peer,
            new_task: None,
            task: Some(task_id_str.ok_or_else(|| CliError::SendArgsInvalid {
                reason: "famp_send mode=terminal requires 'task_id'".to_string(),
            })?),
            terminal: true,
            body: body_text,
        },
        other => {
            return Err(CliError::SendArgsInvalid {
                reason: format!(
                    "famp_send: unknown mode '{other}'; expected new_task|deliver|terminal"
                ),
            });
        }
    };

    let outcome = run_at_structured(home, args).await?;
    Ok(serde_json::json!({
        "task_id": outcome.task_id,
        "state":   outcome.state,
    }))
}

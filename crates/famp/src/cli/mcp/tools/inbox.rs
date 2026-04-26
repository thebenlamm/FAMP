//! `famp_inbox` MCP tool — wraps `cli::inbox::{list, ack}`.
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
//! { "entries": [ { "offset": ..., "task_id": "...", ... }, ... ] }
//! ```
//!
//! Output shape for `ack`:
//! ```json
//! { "ok": true }
//! ```

use serde_json::Value;

use crate::cli::error::CliError;
use crate::cli::inbox::{ack, list};
use crate::cli::mcp::session::IdentityBinding;

/// Dispatch a `famp_inbox` tool call.
pub async fn call(binding: &IdentityBinding, input: &Value) -> Result<Value, CliError> {
    let home = binding.home.as_path();
    let action = input["action"]
        .as_str()
        .ok_or_else(|| CliError::SendArgsInvalid {
            reason: "famp_inbox: missing required field 'action'".to_string(),
        })?;

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
            let mut buf = Vec::<u8>::new();
            list::run_list(home, since, include_terminal, &mut buf)?;

            // Parse line-by-line into a JSON array. A malformed line is a
            // hard tool-call failure — silently mapping it to `null` (the
            // pre-fix behaviour) made downstream agents miss messages or
            // mis-handle inbox state without ever seeing an error.
            let text = std::str::from_utf8(&buf).map_err(|e| CliError::Io {
                path: std::path::PathBuf::new(),
                source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
            })?;
            let mut entries: Vec<Value> = Vec::new();
            for (idx, line) in text.lines().filter(|l| !l.is_empty()).enumerate() {
                let parsed: Value =
                    serde_json::from_str(line).map_err(|err| CliError::SendArgsInvalid {
                        reason: format!("famp_inbox: list line {idx} is not valid JSON: {err}"),
                    })?;
                entries.push(parsed);
            }

            Ok(serde_json::json!({ "entries": entries }))
        }
        "ack" => {
            let offset = input["offset"]
                .as_u64()
                .ok_or_else(|| CliError::SendArgsInvalid {
                    reason: "famp_inbox action=ack requires 'offset'".to_string(),
                })?;
            ack::run_ack(home, offset).await?;
            Ok(serde_json::json!({ "ok": true }))
        }
        other => Err(CliError::SendArgsInvalid {
            reason: format!("famp_inbox: unknown action '{other}'; expected list|ack"),
        }),
    }
}

//! `famp_whoami` MCP tool — returns the current session identity binding.
//!
//! Output shape:
//! - registered:   `{ "identity": "<name>", "source": "explicit" }`
//! - unregistered: `{ "identity": null,     "source": "unregistered" }`
//!
//! Per CONTEXT.md: "Never errors (unless the JSON-RPC framing layer
//! itself fails)." This function therefore returns `Ok` unconditionally.

use serde_json::Value;

use crate::cli::error::CliError;
use crate::cli::mcp::session::{self, BindingSource};

/// Dispatch a `famp_whoami` tool call. Ignores `_input`.
pub async fn call(_input: &Value) -> Result<Value, CliError> {
    match session::current().await {
        Some(b) => {
            let source = match b.source {
                BindingSource::Explicit => "explicit",
            };
            Ok(serde_json::json!({
                "identity": b.identity,
                "source":   source,
            }))
        }
        None => Ok(serde_json::json!({
            "identity": Value::Null,
            "source":   "unregistered",
        })),
    }
}

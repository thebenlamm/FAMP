//! `famp_await` MCP tool — wraps `cli::await_cmd::run_at_structured`.
//!
//! Input shape (JSON):
//! ```json
//! {
//!   "timeout_seconds": 30,       // optional; default 30
//!   "task_id": "<uuid>"          // optional task filter
//! }
//! ```
//!
//! Output shape on success:
//! ```json
//! { "offset": 123, "task_id": "...", "from": "...", "class": "...", "body": {...} }
//! ```

use std::path::Path;

use serde_json::Value;

use crate::cli::await_cmd::{run_at_structured, AwaitArgs};
use crate::cli::error::CliError;

/// Dispatch a `famp_await` tool call.
pub async fn call(home: &Path, input: &Value) -> Result<Value, CliError> {
    let timeout_secs = input["timeout_seconds"].as_u64().unwrap_or(30);
    let timeout = format!("{timeout_secs}s");
    let task = input["task_id"].as_str().map(str::to_string);

    let args = AwaitArgs { timeout, task };
    let outcome = run_at_structured(home, args).await?;
    Ok(serde_json::json!({
        "offset":  outcome.offset,
        "task_id": outcome.task_id,
        "from":    outcome.from,
        "class":   outcome.class,
        "body":    outcome.body,
    }))
}

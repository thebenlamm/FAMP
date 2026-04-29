//! `famp_await` MCP tool — wraps `cli::await_cmd::run_at_structured`.
//!
//! ## v0.9 transitional shape (plan 02-06)
//!
//! Plan 02-06 rewires `cli::await_cmd::run_at_structured` from the v0.8
//! `home: &Path` polling shape to the v0.9 `sock: &Path` bus shape. The
//! MCP tool gets the corresponding update so the crate keeps compiling
//! through wave 4. Plan 02-09 (MCP rewire wave) re-shapes the input
//! object, the `IdentityBinding` plumbing, and the per-tool error
//! mapping; this wrapper is only the minimum-blast-radius adapter.
//!
//! Input shape (JSON, transitional — plan 02-09 finalizes):
//! ```json
//! {
//!   "timeout_seconds": 30,       // optional; default 30
//!   "task_id": "<uuid>"          // optional task filter (UUID)
//! }
//! ```
//!
//! Output shape on success:
//! ```json
//! { "envelope": { ... } }       // on AwaitOk
//! { "timeout": true }            // on AwaitTimeout
//! ```

use serde_json::Value;

use crate::bus_client::resolve_sock_path;
use crate::cli::await_cmd::{run_at_structured, AwaitArgs, AwaitOutcome};
use crate::cli::error::CliError;
use crate::cli::mcp::session::IdentityBinding;

/// Dispatch a `famp_await` tool call.
pub async fn call(_binding: &IdentityBinding, input: &Value) -> Result<Value, CliError> {
    let timeout_secs = input["timeout_seconds"].as_u64().unwrap_or(30);
    let timeout_str = format!("{timeout_secs}s");
    // Parse via humantime::Duration so the value funnels through the
    // same `AwaitArgs.timeout` field the CLI clap parser drives.
    let timeout: humantime::Duration =
        timeout_str
            .parse()
            .map_err(|_| CliError::InvalidDuration {
                value: timeout_str.clone(),
            })?;

    // task_id is now a typed UUID (plan 02-06). Reject non-UUID input
    // up-front rather than silently dropping the filter — plan 02-09
    // will harden the structured input contract.
    let task = match input["task_id"].as_str() {
        Some(s) => Some(uuid::Uuid::parse_str(s).map_err(|e| CliError::SendArgsInvalid {
            reason: format!("famp_await: invalid task_id: {e}"),
        })?),
        None => None,
    };

    let args = AwaitArgs {
        timeout,
        task,
        act_as: None,
    };
    let sock = resolve_sock_path();
    let outcome: AwaitOutcome = run_at_structured(&sock, args).await?;
    if outcome.timed_out {
        Ok(serde_json::json!({"timeout": true}))
    } else {
        Ok(serde_json::json!({
            "envelope": outcome.envelope.unwrap_or(serde_json::Value::Null),
        }))
    }
}

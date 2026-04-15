//! `famp_peers` MCP tool — wraps `cli::config::read_peers` and `cli::peer::add`.
//!
//! Input shape (JSON):
//! ```json
//! {
//!   "action":    "list" | "add",
//!   "alias":     "<alias>",      // required for add
//!   "endpoint":  "<url>",        // required for add
//!   "pubkey":    "<base64url>",  // required for add
//!   "principal": "<principal>"   // optional for add
//! }
//! ```
//!
//! Output shape for `list`:
//! ```json
//! { "peers": [ { "alias": "...", "endpoint": "...", ... }, ... ] }
//! ```
//!
//! Output shape for `add`:
//! ```json
//! { "ok": true }
//! ```

use std::path::Path;

use serde_json::Value;

use crate::cli::config::read_peers;
use crate::cli::error::CliError;
use crate::cli::paths;
use crate::cli::peer::add::run_add_at;

/// Dispatch a `famp_peers` tool call.
pub fn call(home: &Path, input: &Value) -> Result<Value, CliError> {
    let action = input["action"]
        .as_str()
        .ok_or_else(|| CliError::SendArgsInvalid {
            reason: "famp_peers: missing required field 'action'".to_string(),
        })?;

    match action {
        "list" => {
            let peers_path = paths::peers_toml_path(home);
            let peers = read_peers(&peers_path)?;
            let arr: Vec<Value> = peers
                .peers
                .into_iter()
                .map(|p| {
                    serde_json::json!({
                        "alias":    p.alias,
                        "endpoint": p.endpoint,
                        "pubkey_b64": p.pubkey_b64,
                        "principal": p.principal,
                    })
                })
                .collect();
            Ok(serde_json::json!({ "peers": arr }))
        }
        "add" => {
            let alias = input["alias"]
                .as_str()
                .ok_or_else(|| CliError::SendArgsInvalid {
                    reason: "famp_peers action=add requires 'alias'".to_string(),
                })?
                .to_string();
            let endpoint = input["endpoint"]
                .as_str()
                .ok_or_else(|| CliError::SendArgsInvalid {
                    reason: "famp_peers action=add requires 'endpoint'".to_string(),
                })?
                .to_string();
            let pubkey = input["pubkey"]
                .as_str()
                .ok_or_else(|| CliError::SendArgsInvalid {
                    reason: "famp_peers action=add requires 'pubkey'".to_string(),
                })?
                .to_string();
            let principal = input["principal"].as_str().map(str::to_string);
            run_add_at(home, alias, endpoint, pubkey, principal)?;
            Ok(serde_json::json!({ "ok": true }))
        }
        other => Err(CliError::SendArgsInvalid {
            reason: format!("famp_peers: unknown action '{other}'; expected list|add"),
        }),
    }
}

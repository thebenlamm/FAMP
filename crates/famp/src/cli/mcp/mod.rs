//! `famp mcp` — stdio JSON-RPC server exposing six FAMP tools.
//!
//! This module is a thin adapter layer. No business logic lives here;
//! all tool implementations call existing `cli::{send, await_cmd, inbox, peer}`
//! entry points directly.
//!
//! ## Wire format
//!
//! Hand-rolled MCP-compliant Content-Length framing (LSP-style):
//! ```text
//! Content-Length: <N>\r\n
//! \r\n
//! <N bytes of UTF-8 JSON>
//! ```
//! Each message is a JSON-RPC 2.0 object. This server implements only
//! `initialize`, `tools/list`, and `tools/call`.

pub mod error_kind;
pub mod server;
pub mod session;
pub mod tools;

use std::path::PathBuf;

use crate::cli::error::CliError;

/// CLI args for `famp mcp`. No subcommands — the server runs until stdin closes.
#[derive(clap::Args, Debug)]
pub struct McpArgs {}

/// Production entry point.
///
/// Reads `FAMP_LOCAL_ROOT` (the backing-store directory under which
/// per-identity agent dirs live) with default `$HOME/.famp-local`.
/// **Does NOT read `FAMP_HOME`** — under variant **B-strict** the MCP
/// server starts unbound and clients must call `famp_register` before
/// using any messaging tool. See
/// `.planning/phases/01-session-bound-mcp-identity/01-CONTEXT.md`.
pub async fn run(_args: McpArgs) -> Result<(), CliError> {
    let local_root = resolve_local_root()?;
    server::run(local_root).await
}

/// Resolve `FAMP_LOCAL_ROOT` with default `$HOME/.famp-local`.
fn resolve_local_root() -> Result<PathBuf, CliError> {
    if let Some(v) = std::env::var_os("FAMP_LOCAL_ROOT") {
        let p = PathBuf::from(v);
        if p.as_os_str().is_empty() {
            return Err(CliError::HomeNotSet);
        }
        return Ok(p);
    }
    let home = std::env::var_os("HOME").ok_or(CliError::HomeNotSet)?;
    Ok(PathBuf::from(home).join(".famp-local"))
}

//! `famp mcp` — stdio JSON-RPC server exposing four FAMP tools.
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

use crate::cli::error::CliError;
use crate::cli::home;

/// CLI args for `famp mcp`. No subcommands — the server runs until stdin closes.
#[derive(clap::Args, Debug)]
pub struct McpArgs {}

/// Production entry point. Resolves `FAMP_HOME` and starts the stdio server.
pub async fn run(_args: McpArgs) -> Result<(), CliError> {
    let home = home::resolve_famp_home()?;
    server::run(home).await
}

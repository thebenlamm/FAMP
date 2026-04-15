//! Stdio MCP server main loop. Full implementation in Task 2.

use std::path::PathBuf;

use crate::cli::error::CliError;

/// Placeholder — replaced in Task 2 with the real Content-Length framed
/// JSON-RPC stdio server.
pub async fn run(_home: PathBuf) -> Result<(), CliError> {
    // Task 2 wires the full server loop here.
    // Return immediately so `famp mcp` exits 0 during compilation checks.
    tokio::task::yield_now().await; // makes the function genuinely async
    Ok(())
}

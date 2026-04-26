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

/// Production entry point.
///
/// **Transitional behavior (this plan only).** Resolves `FAMP_HOME` and
/// pre-seeds the session binding so existing E2E tests
/// (`mcp_stdio_tool_calls.rs`, `e2e_two_daemons.rs`) keep passing
/// through wave 2. Plan 01-03 deletes the seeding line entirely; from
/// that point forward `FAMP_HOME` is no longer honored at MCP startup
/// and clients MUST call `famp_register`. **B-strict, no grace
/// period** — this comment block disappears in 01-03.
pub async fn run(_args: McpArgs) -> Result<(), CliError> {
    let home = home::resolve_famp_home()?;
    // TRANSITIONAL — removed in 01-03.
    let identity = home
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();
    let binding = crate::cli::mcp::session::IdentityBinding {
        identity,
        home,
        source: crate::cli::mcp::session::BindingSource::Explicit,
    };
    // TEST-ONLY SEAM (also removed in 01-03 alongside the rest of the seed):
    // FAMP_TEST_SUPPRESS_BINDING_SEED=1 makes the server start in an
    // unbound state so 01-02's gating tests can exercise NotRegistered
    // without rewriting the harness for a no-FAMP_HOME-allowed mode.
    if std::env::var_os("FAMP_TEST_SUPPRESS_BINDING_SEED").is_none() {
        let _prev = crate::cli::mcp::session::set(binding).await;
    }
    server::run().await
}

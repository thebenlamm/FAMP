//! `famp inbox ack` — client-side cursor advance.
//!
//! Phase 02 plan 02-05: per RESEARCH §6, "the broker does not track
//! per-session Inbox cursors — the client is authoritative." Therefore
//! `inbox ack` is a LOCAL file write only — NO broker round-trip,
//! NO `Hello.bind_as` proxy, NO `BusMessage` is sent.
//!
//! The cursor write reuses `cli::broker::cursor_exec::execute_advance_cursor`
//! (atomic temp+rename + `sync_all` + chmod 0o600). The `--offset` value
//! is REQUIRED — per RESEARCH §6 it is taken from the `next_offset`
//! field of a prior `famp inbox list` output that the user pipes in.

use std::path::Path;

use crate::bus_client::{bus_dir, resolve_sock_path};
use crate::cli::broker::cursor_exec::execute_advance_cursor;
use crate::cli::error::CliError;
use crate::cli::identity::resolve_identity;

/// CLI args for `famp inbox ack`.
#[derive(clap::Args, Debug)]
pub struct AckArgs {
    /// Cursor offset to advance to. REQUIRED; obtain from `famp inbox list`
    /// `next_offset` footer.
    #[arg(long)]
    pub offset: u64,
    /// Override identity (D-01).
    #[arg(long = "as")]
    pub act_as: Option<String>,
}

/// Structured outcome — used by the MCP `famp_inbox` tool wrapper
/// (plan 02-09). Always reports `acked: true` on success; failure
/// surfaces as `CliError`.
pub struct AckOutcome {
    pub acked: bool,
    pub offset: u64,
}

/// Run `famp inbox ack` against the bus directory derived from `sock`.
/// Pure local-file path: no broker round-trip.
pub async fn run_at_structured(sock: &Path, args: AckArgs) -> Result<AckOutcome, CliError> {
    let identity = resolve_identity(args.act_as.as_deref())?;
    let dir = bus_dir(sock);
    execute_advance_cursor(dir, &identity, args.offset)
        .await
        .map_err(|e| CliError::Io {
            path: dir.to_path_buf(),
            source: e,
        })?;
    Ok(AckOutcome {
        acked: true,
        offset: args.offset,
    })
}

/// Top-level CLI entry — writes the cursor and prints
/// `{"acked":true,"offset":N}` to stdout.
pub async fn run(args: AckArgs) -> Result<(), CliError> {
    let sock = resolve_sock_path();
    let outcome = run_at_structured(&sock, args).await?;
    println!(
        "{}",
        serde_json::json!({ "acked": outcome.acked, "offset": outcome.offset })
    );
    Ok(())
}

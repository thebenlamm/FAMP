//! `famp inbox` — non-blocking list + manual cursor ack.
//!
//! - `famp inbox list [--since <offset>]`: read every entry past
//!   `--since` (defaults to 0 — the whole file) and print one JSON
//!   line per entry in the same locked shape `famp await` emits.
//!   Does NOT advance the cursor.
//! - `famp inbox ack <offset>`: advance the cursor to `<offset>`.
//!   Prints nothing. Does not validate that the offset is on a line
//!   boundary — the caller (Phase 4 MCP wrapper) is trusted.

use crate::cli::error::CliError;
use crate::cli::home;

pub mod ack;
pub mod list;

#[derive(clap::Args, Debug)]
pub struct InboxArgs {
    #[command(subcommand)]
    pub command: InboxCommand,
}

#[derive(clap::Subcommand, Debug)]
pub enum InboxCommand {
    /// List inbox entries; with `--since`, only past that byte offset.
    List(InboxListArgs),
    /// Advance the cursor without printing.
    Ack(InboxAckArgs),
}

#[derive(clap::Args, Debug)]
pub struct InboxListArgs {
    #[arg(long)]
    pub since: Option<u64>,
}

#[derive(clap::Args, Debug)]
pub struct InboxAckArgs {
    pub offset: u64,
}

/// Top-level entry point. Dispatches on subcommand. Async because
/// `ack` calls `InboxCursor::advance` which is async.
pub async fn run(args: InboxArgs) -> Result<(), CliError> {
    let home = home::resolve_famp_home()?;
    match args.command {
        InboxCommand::List(list_args) => {
            let mut stdout = std::io::stdout();
            list::run_list(&home, list_args.since, &mut stdout)
        }
        InboxCommand::Ack(ack_args) => ack::run_ack(&home, ack_args.offset).await,
    }
}

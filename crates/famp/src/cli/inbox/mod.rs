//! `famp inbox` — bus-backed list + client-side cursor ack.
//!
//! Phase 02 plan 02-05: rewires from v0.8 file-reader semantics to the
//! local UDS broker. Identity binding is connection-level via D-10
//! `Hello.bind_as`. Cursor management remains client-side (RESEARCH §6).
//!
//! Subcommands:
//!
//! - `famp inbox list [--since <offset>] [--include-terminal] [--as <name>]`
//!   Connects with `Hello { bind_as: Some(identity) }`, sends
//!   `BusMessage::Inbox { since, include_terminal }`, prints one JSONL
//!   line per typed envelope to stdout, then a `{"next_offset":N}`
//!   footer.
//!
//! - `famp inbox ack --offset <N> [--as <name>]`
//!   Atomic local cursor advance via `cli::broker::cursor_exec`. NO
//!   broker round-trip — purely a temp+rename file write at
//!   `<bus_dir>/mailboxes/.<identity>.cursor`.

use crate::cli::error::CliError;

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
    List(list::ListArgs),
    /// Advance the cursor to `--offset` without contacting the broker.
    Ack(ack::AckArgs),
}

/// Top-level entry point. Dispatches on subcommand.
pub async fn run(args: InboxArgs) -> Result<(), CliError> {
    match args.command {
        InboxCommand::List(list_args) => list::run(list_args).await,
        InboxCommand::Ack(ack_args) => ack::run(ack_args).await,
    }
}

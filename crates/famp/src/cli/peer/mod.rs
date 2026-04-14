//! `famp peer` subcommand family. Phase 3 Plan 03-02.
//!
//! Currently exposes only `famp peer add <alias> --endpoint --pubkey [--principal]`.
//! Future subcommands (`list`, `remove`) land in Phase 3 Plan 03-04.

use crate::cli::error::CliError;

pub mod add;

#[derive(clap::Args, Debug)]
pub struct PeerArgs {
    #[command(subcommand)]
    pub command: PeerCommand,
}

#[derive(clap::Subcommand, Debug)]
pub enum PeerCommand {
    /// Register a new peer in `peers.toml`.
    Add(PeerAddArgs),
}

#[derive(clap::Args, Debug)]
pub struct PeerAddArgs {
    /// Local alias (`famp send --to <alias>`).
    pub alias: String,
    /// `https://host:port` — the peer's inbox endpoint.
    #[arg(long)]
    pub endpoint: String,
    /// base64url-unpadded ed25519 verifying key (32 raw bytes decoded).
    #[arg(long)]
    pub pubkey: String,
    /// Optional peer principal (`agent:authority/name`). Defaults to
    /// `agent:localhost/self` so tests can point at the Phase 2 `famp listen`
    /// self-keyring without a separate flag. Callers interoperating with a
    /// real federated agent should set this explicitly.
    #[arg(long)]
    pub principal: Option<String>,
}

/// Dispatch `famp peer ...`.
pub fn run(args: PeerArgs) -> Result<(), CliError> {
    match args.command {
        PeerCommand::Add(a) => add::run_add(a.alias, a.endpoint, a.pubkey, a.principal),
    }
}

//! `famp peer` subcommand family. Phase 3 Plan 03-02.
//!
//! Subcommands:
//! - `add`: Register a peer manually with explicit fields
//! - `import`: Import a peer card from JSON (output of `famp info`/`famp setup`)

use crate::cli::error::CliError;

pub mod add;
pub mod import;

#[derive(clap::Args, Debug)]
pub struct PeerArgs {
    #[command(subcommand)]
    pub command: PeerCommand,
}

#[derive(clap::Subcommand, Debug)]
pub enum PeerCommand {
    /// Register a new peer in `peers.toml`.
    Add(PeerAddArgs),
    /// Import a peer from JSON (reads from stdin or --card).
    Import(PeerImportArgs),
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

#[derive(clap::Args, Debug)]
pub struct PeerImportArgs {
    /// Peer card JSON. If not provided, reads from stdin.
    #[arg(long)]
    pub card: Option<String>,
}

/// Dispatch `famp peer ...`.
pub fn run(args: PeerArgs) -> Result<(), CliError> {
    match args.command {
        PeerCommand::Add(a) => add::run_add(a.alias, a.endpoint, a.pubkey, a.principal),
        PeerCommand::Import(a) => import::run_import(a.card),
    }
}

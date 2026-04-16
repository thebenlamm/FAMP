//! FAMP CLI surface. D-02: subcommand logic lives in the lib crate so
//! integration tests can call it directly without `assert_cmd`.

use clap::{Parser, Subcommand};

pub mod await_cmd;
pub mod config;
pub mod error;
pub mod home;
pub mod inbox;
pub mod info;
pub mod init;
pub mod listen;
pub mod mcp;
pub mod paths;
pub mod peer;
pub mod perms;
pub mod send;
pub mod setup;

pub use error::CliError;
pub use init::InitOutcome;
pub use listen::ListenArgs;

#[derive(Parser, Debug)]
#[command(name = "famp", version, about = "FAMP v0.5.1 reference CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize a FAMP home directory.
    Init(InitArgs),
    /// One-command setup: init + port selection + peer card output.
    Setup(setup::SetupArgs),
    /// Output this agent's peer card (for sharing with other agents).
    Info(info::InfoArgs),
    /// Run the FAMP daemon: bind the HTTPS listener and append inbound
    /// signed envelopes to `~/.famp/inbox.jsonl`.
    Listen(ListenArgs),
    /// Manage the peer registry (`peers.toml`).
    Peer(peer::PeerArgs),
    /// Send an envelope to a peer — new task, deliver, or terminal.
    Send(send::SendArgs),
    /// Block until a new inbox entry arrives past the cursor.
    #[command(name = "await")]
    Await(await_cmd::AwaitArgs),
    /// Inspect the inbox (list + cursor ack).
    Inbox(inbox::InboxArgs),
    /// Start the MCP stdio JSON-RPC server (four tools: `famp_send`, `famp_await`,
    /// `famp_inbox`, `famp_peers`). Reads Content-Length-framed JSON-RPC from
    /// stdin; writes framed responses to stdout.
    Mcp(mcp::McpArgs),
}

#[derive(clap::Args, Debug)]
pub struct InitArgs {
    /// Overwrite an existing FAMP home (atomic replace).
    #[arg(long)]
    pub force: bool,
}

/// Top-level CLI dispatcher. Called from `bin/famp.rs`.
pub fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Commands::Init(args) => init::run(args).map(|_| ()),
        Commands::Setup(args) => setup::run(args).map(|_| ()),
        Commands::Info(args) => info::run(args).map(|_| ()),
        Commands::Listen(args) => {
            // Only the `Listen` arm boots tokio; `Init` stays sync so
            // `famp init` does not pay the multi-thread runtime cost.
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| CliError::Io {
                    path: std::path::PathBuf::new(),
                    source: e,
                })?;
            rt.block_on(listen::run(args))
        }
        Commands::Peer(args) => peer::run(args),
        Commands::Send(args) => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| CliError::Io {
                    path: std::path::PathBuf::new(),
                    source: e,
                })?;
            rt.block_on(send::run(args))
        }
        Commands::Await(args) => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| CliError::Io {
                    path: std::path::PathBuf::new(),
                    source: e,
                })?;
            rt.block_on(await_cmd::run(args))
        }
        Commands::Inbox(args) => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| CliError::Io {
                    path: std::path::PathBuf::new(),
                    source: e,
                })?;
            rt.block_on(inbox::run(args))
        }
        Commands::Mcp(args) => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| CliError::Io {
                    path: std::path::PathBuf::new(),
                    source: e,
                })?;
            rt.block_on(mcp::run(args))
        }
    }
}

//! FAMP CLI surface. D-02: subcommand logic lives in the lib crate so
//! integration tests can call it directly without `assert_cmd`.

use clap::{Parser, Subcommand};

pub mod await_cmd;
pub mod broker;
pub mod config;
pub mod error;
pub mod home;
pub mod identity;
pub mod inbox;
pub mod info;
pub mod init;
pub mod join;
pub mod leave;
pub mod listen;
pub mod mcp;
pub mod paths;
pub mod peer;
pub mod perms;
pub mod register;
pub mod send;
pub mod sessions;
pub mod setup;
pub mod util;
pub mod whoami;

pub use broker::BrokerArgs;
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
    /// Run the local-first UDS broker daemon (Phase 02). Auto-spawned by
    /// `bus_client::spawn::spawn_broker_if_absent`; rarely invoked
    /// directly by humans.
    Broker(BrokerArgs),
    /// Register an identity with the local broker and hold the slot for
    /// the lifetime of this process. Long-lived foreground subcommand
    /// (Phase 02 / D-10): `famp register alice` is the canonical holder
    /// of `alice`; later one-shot CLI commands (`send`, `inbox`,
    /// `await`, `join`, `leave`, `whoami`, `sessions --me`) ride on
    /// this process via `Hello { bind_as = "alice" }` (the proxy
    /// shape). Variant for `Commands::Register`; the dispatch arm
    /// below boots a multi-thread tokio runtime and calls
    /// `register::run`.
    Register(register::RegisterArgs),
    /// Join a channel. Accepts `#name` or bare `name`. D-10 proxy:
    /// the broker mutates the canonical holder's `joined` set, NOT
    /// this connection's, so the one-shot CLI process exiting does
    /// not auto-leave.
    Join(join::JoinArgs),
    /// Leave a channel. Same D-10 proxy semantics as `join`.
    Leave(leave::LeaveArgs),
    /// List currently registered sessions held by live `famp register`
    /// processes. Read-only; reads broker memory (NOT the diagnostic
    /// `sessions.jsonl`). With `--me`, filters to the caller's resolved
    /// identity and uses `Hello.bind_as` proxy for liveness validation.
    Sessions(sessions::SessionsArgs),
    /// Print the active identity (per D-10 proxy `bind_as`) and the
    /// canonical holder's joined channels.
    Whoami(whoami::WhoamiArgs),
}

#[derive(clap::Args, Debug)]
pub struct InitArgs {
    /// Overwrite an existing FAMP home (atomic replace).
    #[arg(long)]
    pub force: bool,
}

/// Build a multi-thread tokio runtime and block on `fut`. Shared by every
/// async dispatch arm in [`run`] so each match arm stays a single-line
/// `block_on_async(...)` call and the dispatcher does not balloon with
/// repeated runtime-construction boilerplate.
fn block_on_async<F>(fut: F) -> Result<(), CliError>
where
    F: std::future::Future<Output = Result<(), CliError>>,
{
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| CliError::Io {
            path: std::path::PathBuf::new(),
            source: e,
        })?;
    rt.block_on(fut)
}

/// Top-level CLI dispatcher. Called from `bin/famp.rs`.
pub fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        // Sync arms (no tokio runtime needed).
        Commands::Init(args) => init::run(args).map(|_| ()),
        Commands::Setup(args) => setup::run(&args).map(|_| ()),
        Commands::Info(args) => info::run(&args).map(|_| ()),
        Commands::Peer(args) => peer::run(args),
        // Async arms: each boots a multi-thread tokio runtime via
        // `block_on_async` and dispatches into the subcommand's
        // `async fn run`. Only async-required arms pay the runtime cost.
        Commands::Listen(args) => block_on_async(listen::run(args)),
        Commands::Send(args) => block_on_async(send::run(args)),
        Commands::Await(args) => block_on_async(await_cmd::run(args)),
        Commands::Inbox(args) => block_on_async(inbox::run(args)),
        Commands::Mcp(args) => block_on_async(mcp::run(args)),
        Commands::Broker(args) => block_on_async(broker::run(args)),
        Commands::Register(args) => block_on_async(register::run(args)),
        Commands::Join(args) => block_on_async(join::run(args)),
        Commands::Leave(args) => block_on_async(leave::run(args)),
        Commands::Sessions(args) => block_on_async(sessions::run(args)),
        Commands::Whoami(args) => block_on_async(whoami::run(args)),
    }
}

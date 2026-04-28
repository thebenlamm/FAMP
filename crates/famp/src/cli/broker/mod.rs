//! `famp broker` subcommand — Phase 02 plan 02-02.
//!
//! Wraps the frozen Phase-1 `famp_bus::Broker` in a tokio UDS daemon.
//! `famp broker --socket <path>` binds the listener, runs the accept
//! loop, and exits cleanly on SIGINT/SIGTERM (or after 5 minutes of
//! idle per BROKER-04).
//!
//! Module layout:
//!   - `accept`: per-client read/write task using `UnixStream::into_split`
//!   - `cursor_exec`: `Out::AdvanceCursor` executor (atomic temp+rename)
//!   - `idle`: `wait_or_never` helper for the 5-min idle-timer arm
//!   - `mailbox_env`: `DiskMailboxEnv` (`BrokerEnv` impl backed by famp-inbox)
//!   - `nfs_check`: best-effort NFS-mount detector (BROKER-05)
//!   - `sessions_log`: append-only `~/.famp/sessions.jsonl` writer (CLI-11)

use std::path::PathBuf;

use crate::cli::error::CliError;

pub mod accept;
pub mod cursor_exec;
pub mod idle;
pub mod mailbox_env;
pub mod nfs_check;
pub mod sessions_log;

/// Args for `famp broker`.
#[derive(clap::Args, Debug, Clone)]
pub struct BrokerArgs {
    /// Override the broker socket path. Defaults to
    /// `$FAMP_BUS_SOCKET` or `~/.famp/bus.sock` per
    /// `bus_client::resolve_sock_path`.
    #[arg(long)]
    pub socket: Option<PathBuf>,
}

/// Production entry point for `famp broker`. Filled in Task 3.
#[allow(clippy::unused_async)] // Task 3 fills in the body with `.await` calls.
pub async fn run(_args: BrokerArgs) -> Result<(), CliError> {
    unimplemented!("Task 3: broker run loop")
}

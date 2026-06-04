//! `famp daemon` subcommand surface.
//!
//! Phase 5: ships `install`, `uninstall`, `status`, `restart`.
//!
//! install and status are fully implemented in this phase (Plans 02/05).
//! uninstall and restart are compiling stubs filled in Plan 04.

use clap::{Args, Subcommand};

use crate::cli::error::CliError;

pub mod install;
pub mod restart;
pub mod status;
pub mod uninstall;

/// Shared Linux helper (parse_linger). cfg-gated to Linux at this declaration
/// so neither macOS builds nor tests attempt to compile it.
#[cfg(target_os = "linux")]
pub mod linux;

#[derive(Args, Debug)]
pub struct DaemonArgs {
    #[command(subcommand)]
    pub command: DaemonSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum DaemonSubcommand {
    /// Install the FAMP broker as a persistent user-level service (launchd on
    /// macOS, systemd --user on Linux). Writes the service file and loads it.
    /// Idempotent: re-running is safe when the service is already installed.
    Install(install::DaemonInstallArgs),
    /// Uninstall the FAMP broker service. Stops and removes the service file.
    /// Idempotent: safe to run even when the service is not currently loaded.
    Uninstall(uninstall::DaemonUninstallArgs),
    /// Report the current daemon service status (three states: not-installed,
    /// installed-but-down, running). Exits 0 when running, 1 when not installed,
    /// 2 when installed but the broker process is not responding.
    Status(status::DaemonStatusArgs),
    /// Restart the daemon, picking up a new on-disk binary after `cargo install`.
    Restart(restart::DaemonRestartArgs),
}

/// Async dispatcher. `install`, `uninstall`, `restart` are sync fns called
/// directly; `status` is async (calls the broker inspect probe).
pub async fn run(args: DaemonArgs) -> Result<(), CliError> {
    match args.command {
        DaemonSubcommand::Install(args) => install::run(args),
        DaemonSubcommand::Uninstall(args) => uninstall::run(args),
        DaemonSubcommand::Status(args) => status::run(args).await,
        DaemonSubcommand::Restart(args) => restart::run(args),
    }
}

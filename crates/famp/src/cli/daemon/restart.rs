//! `famp daemon restart` — restart the FAMP broker service, picking up a
//! freshly installed on-disk binary (DAEMON-05).
//!
//! macOS: `launchctl kickstart -k gui/$UID/com.famp.broker`
//!   The `-k` flag kills the currently-running process and relaunches it from
//!   the on-disk binary at the path locked in the plist. This is the correct
//!   invocation for binary pickup after `cargo install --force`. Only when the
//!   plist shape itself changes (e.g. new arguments) is the full unload/reload
//!   cycle required.
//!
//! Linux: `systemctl --user restart famp-broker.service`
//!   systemd reads the ExecStart path from the unit file and re-executes it,
//!   picking up the new binary on disk.

use std::path::PathBuf;
use std::process::Command;

use clap::Args;

use crate::cli::daemon::install::DaemonError;
use crate::cli::error::CliError;

#[derive(Debug, Args)]
pub struct DaemonRestartArgs {
    /// Override the install target home (defaults to `dirs::home_dir()`).
    /// Hidden flag - used by integration tests to redirect to a tempdir.
    #[arg(long, env = "FAMP_INSTALL_TARGET_HOME", hide = true)]
    pub home: Option<PathBuf>,
}

/// macOS: restart the service via `launchctl kickstart -k`, picking up the
/// on-disk binary (DAEMON-05 binary-pickup guarantee).
///
/// `kickstart -k` kills the running process and relaunches from the binary
/// locked in the plist, picking up the new binary on disk.
#[cfg(target_os = "macos")]
fn restart_macos(uid: u32) -> Result<(), DaemonError> {
    let status = Command::new("launchctl")
        .args(["kickstart", "-k", &format!("gui/{uid}/com.famp.broker")])
        .status()
        .map_err(|e| DaemonError::Io {
            path: PathBuf::from("launchctl"),
            source: e,
        })?;
    if !status.success() {
        return Err(DaemonError::LaunchctlFailed(status.code().unwrap_or(-1)));
    }
    Ok(())
}

/// Linux: restart the service via `systemctl --user restart`, picking up the
/// on-disk binary.
#[cfg(target_os = "linux")]
fn restart_linux() -> Result<(), DaemonError> {
    let status = Command::new("systemctl")
        .args(["--user", "restart", "famp-broker.service"])
        .status()
        .map_err(|_| DaemonError::SystemctlAbsent)?;
    if !status.success() {
        return Err(DaemonError::SystemctlFailed(status.code().unwrap_or(-1)));
    }
    Ok(())
}

pub fn run(_args: DaemonRestartArgs) -> Result<(), CliError> {
    #[cfg(target_os = "macos")]
    {
        let uid = u32::from(nix::unistd::getuid());
        restart_macos(uid)?;
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        restart_linux()?;
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Err(CliError::Daemon(DaemonError::UnsupportedPlatform))
    }
}

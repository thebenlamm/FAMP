//! `famp daemon uninstall` — removes the platform service and unregisters it.
//!
//! Plan 04 replaces the Plan 02 stub with the idempotent implementation:
//!   - macOS: `launchctl bootout gui/$UID <plist>` (tolerate failure — not loaded)
//!     then remove the plist file if it exists
//!   - Linux: `systemctl --user disable --now famp-broker.service` (tolerate failure)
//!     then remove the unit file if it exists + daemon-reload (tolerate)
//!   - Both platforms: return Ok on a clean (already-uninstalled) system
//!
//! DAEMON-04: idempotent — calling uninstall twice exits 0 both times with no
//! orphan launchd/systemd registrations.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Args;

use crate::cli::daemon::install::DaemonError;
use crate::cli::error::CliError;

#[derive(Debug, Args)]
pub struct DaemonUninstallArgs {
    /// Override the install target home (defaults to `dirs::home_dir()`).
    /// Hidden flag - used by integration tests to redirect to a tempdir.
    #[arg(long, env = "FAMP_INSTALL_TARGET_HOME", hide = true)]
    pub home: Option<std::path::PathBuf>,
}

/// Uninstall the FAMP broker service from `home`.
///
/// Idempotent: tolerates the service not being registered (bootout/disable
/// failure is ignored) and the plist/unit file not existing. Returns Ok
/// on a clean system (DAEMON-04).
#[allow(clippy::needless_return)] // explicit `return` per cfg branch; only one compiles per platform
pub fn run_at(home: &Path, err: &mut dyn Write) -> Result<(), DaemonError> {
    writeln!(err, "Uninstalling FAMP broker service...").ok();

    #[cfg(target_os = "macos")]
    {
        let plist_path = home
            .join("Library")
            .join("LaunchAgents")
            .join("com.famp.broker.plist");

        // Step 1: bootout — tolerate failure (service may not be loaded).
        // DAEMON-04: idempotent. launchctl bootout fails if not loaded; ignore it.
        let uid = u32::from(nix::unistd::getuid());
        let _ = Command::new("launchctl")
            .args([
                "bootout",
                &format!("gui/{uid}"),
                plist_path.to_str().unwrap_or_default(),
            ])
            .status();
        writeln!(
            err,
            "  [1/2] launchctl bootout gui/{uid}: ok (tolerated any failure)"
        )
        .ok();

        // Step 2: remove the plist file if it exists.
        if plist_path.exists() {
            std::fs::remove_file(&plist_path).map_err(|source| DaemonError::Io {
                path: plist_path.clone(),
                source,
            })?;
            writeln!(err, "  [2/2] removed {}", plist_path.display()).ok();
        } else {
            writeln!(
                err,
                "  [2/2] plist not found at {} (already uninstalled)",
                plist_path.display()
            )
            .ok();
        }

        writeln!(err).ok();
        writeln!(err, "daemon uninstall complete.").ok();
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        let unit_path = home
            .join(".config")
            .join("systemd")
            .join("user")
            .join("famp-broker.service");

        // Step 1: disable --now — tolerate failure (service may not be enabled/loaded).
        let _ = Command::new("systemctl")
            .args(["--user", "disable", "--now", "famp-broker.service"])
            .status();
        writeln!(
            err,
            "  [1/3] systemctl --user disable --now: ok (tolerated any failure)"
        )
        .ok();

        // Step 2: remove the unit file if it exists.
        if unit_path.exists() {
            std::fs::remove_file(&unit_path).map_err(|source| DaemonError::Io {
                path: unit_path.clone(),
                source,
            })?;
            writeln!(err, "  [2/3] removed {}", unit_path.display()).ok();
        } else {
            writeln!(
                err,
                "  [2/3] unit file not found at {} (already uninstalled)",
                unit_path.display()
            )
            .ok();
        }

        // Step 3: daemon-reload so systemd forgets the unit — tolerate failure.
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .status();
        writeln!(
            err,
            "  [3/3] systemctl --user daemon-reload: ok (tolerated any failure)"
        )
        .ok();

        writeln!(err).ok();
        writeln!(err, "daemon uninstall complete.").ok();
        return Ok(());
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = err;
        return Err(DaemonError::UnsupportedPlatform);
    }
}

/// Resolve home dir and call `run_at` against stderr.
#[allow(clippy::needless_pass_by_value)]
pub fn run(args: DaemonUninstallArgs) -> Result<(), CliError> {
    let home = match args.home {
        Some(p) => p,
        None => dirs::home_dir().ok_or_else(|| CliError::Io {
            path: PathBuf::from("$HOME"),
            source: std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "could not resolve home directory",
            ),
        })?,
    };
    let mut stderr = std::io::stderr().lock();
    run_at(&home, &mut stderr)?;
    Ok(())
}

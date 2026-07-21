//! `famp daemon restart` — restart the FAMP broker service, picking up a
//! freshly installed on-disk binary (DAEMON-05).
//!
//! macOS: `launchctl bootout` + `bootstrap` + `kickstart` against the
//! LaunchAgent plist. A plain `kickstart -k` does **not** refresh launchd's
//! Lightweight Code Requirement (LWCR) when `just install` / `cargo install
//! --force` replaces the binary with a new cdhash — that wedges the agent in
//! an unrecoverable `EX_CONFIG` (exit 78) crash loop (issue #20). Full
//! bootout+bootstrap recomputes the LWCR against the current binary.
//!
//! Linux: `systemctl --user restart famp-broker.service`
//!   systemd re-reads ExecStart and re-executes the on-disk binary.
//!
//! Readiness (issue #9): after the service manager returns, poll until the
//! broker answers a Hello handshake (`raw_connect_probe` → Healthy), then
//! print one success line. Do **not** sleep a fixed interval and do **not**
//! return Ok while the socket is still down.
//!
//! Side effect (document in help): restart drops all in-memory registrations
//! and parked `famp await` waiters. Listen-mode agents stop auto-waking until
//! they re-register.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use clap::Args;
use famp_inspect_client::{raw_connect_probe, ProbeOutcome};
use famp_inspect_proto::{InspectBrokerReply, InspectBrokerRequest, InspectKind};

use crate::cli::daemon::install::DaemonError;
use crate::cli::error::CliError;

/// How long to wait for the broker Hello after the service manager returns.
const RESTART_READY_BUDGET: Duration = Duration::from_secs(5);
/// Backoff between readiness probes.
const RESTART_READY_POLL: Duration = Duration::from_millis(100);

#[derive(Debug, Args)]
pub struct DaemonRestartArgs {
    /// Override the install target home (defaults to `dirs::home_dir()`).
    /// Hidden flag - used by integration tests to redirect to a tempdir.
    #[arg(long, env = "FAMP_INSTALL_TARGET_HOME", hide = true)]
    pub home: Option<PathBuf>,
}

/// macOS LaunchAgent plist path for the broker service.
#[cfg(target_os = "macos")]
fn plist_path(home: &Path) -> PathBuf {
    home.join("Library")
        .join("LaunchAgents")
        .join("com.famp.broker.plist")
}

/// macOS: full unload/reload so launchd refreshes the LWCR for the on-disk
/// binary (issue #20), then kickstart to ensure the job is running now.
///
/// Order:
/// 1. Probe registration — fail with `NotInstalled` if the label is absent.
/// 2. `bootout` — tolerate failure (job may already be unloaded mid-crash-loop).
/// 3. `bootstrap` — recompute LWCR against the current binary; must succeed.
/// 4. `kickstart` — start immediately (RunAtLoad alone can race the readiness
///    poll if KeepAlive has not yet respawned).
#[cfg(target_os = "macos")]
fn restart_macos(home: &Path, uid: u32) -> Result<(), DaemonError> {
    if !super::status::launchctl_is_registered("com.famp.broker", uid) {
        return Err(DaemonError::NotInstalled);
    }

    let plist = plist_path(home);
    if !plist.exists() {
        return Err(DaemonError::NotInstalled);
    }
    let plist_str = plist.to_str().unwrap_or_default();
    let domain = format!("gui/{uid}");
    let service = format!("gui/{uid}/com.famp.broker");

    // bootout: tolerate failure — a crash-looping job may already be half-out,
    // and "not loaded" is a successful precondition for bootstrap.
    let _ = Command::new("launchctl")
        .args(["bootout", &domain, plist_str])
        .status();

    let status = Command::new("launchctl")
        .args(["bootstrap", &domain, plist_str])
        .status()
        .map_err(|e| DaemonError::Io {
            path: PathBuf::from("launchctl"),
            source: e,
        })?;
    if !status.success() {
        return Err(DaemonError::LaunchctlFailed(status.code().unwrap_or(-1)));
    }

    let status = Command::new("launchctl")
        .args(["kickstart", &service])
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
        .map_err(|e| {
            // Only NotFound means systemctl is genuinely absent. Any other IO
            // error (permission denied, fork failure, …) must preserve the real
            // errno — mislabeling it "systemctl not found" violates the project's
            // no-swallow / no-mislabel convention. Mirrors install_linux's shape.
            if e.kind() == std::io::ErrorKind::NotFound {
                DaemonError::SystemctlAbsent
            } else {
                DaemonError::Io {
                    path: PathBuf::from("systemctl"),
                    source: e,
                }
            }
        })?;
    if !status.success() {
        return Err(DaemonError::SystemctlFailed(status.code().unwrap_or(-1)));
    }
    Ok(())
}

/// Poll until the broker answers Hello, then fetch pid/build for the success
/// line. Bounded by `RESTART_READY_BUDGET` with `RESTART_READY_POLL` backoff.
///
/// Returns `(pid, socket_path, build_version)` on success.
async fn wait_until_healthy() -> Result<(u32, String, String), DaemonError> {
    let sock = crate::bus_client::resolve_sock_path();
    let sock_display = sock.display().to_string();
    let deadline = Instant::now() + RESTART_READY_BUDGET;

    loop {
        match raw_connect_probe(&sock).await {
            ProbeOutcome::Healthy { mut stream } => {
                let kind = InspectKind::Broker(InspectBrokerRequest::default());
                if let Ok(payload) = famp_inspect_client::call(&mut stream, kind).await {
                    if let Ok(InspectBrokerReply::Info(info)) =
                        serde_json::from_value::<InspectBrokerReply>(payload)
                    {
                        return Ok((info.pid, info.socket_path, info.build_version));
                    }
                    // Schema mismatch / budget-exceeded — treat as not ready yet.
                }
                // Connect succeeded but inspect call failed — retry.
            }
            ProbeOutcome::DownClean
            | ProbeOutcome::StaleSocket
            | ProbeOutcome::OrphanHolder { .. }
            | ProbeOutcome::PermissionDenied => {
                // Not ready yet.
            }
        }

        if Instant::now() >= deadline {
            let waited_ms = u64::try_from(RESTART_READY_BUDGET.as_millis()).unwrap_or(u64::MAX);
            return Err(DaemonError::RestartTimedOut {
                waited_ms,
                socket: sock_display,
            });
        }
        tokio::time::sleep(RESTART_READY_POLL).await;
    }
}

pub async fn run(args: DaemonRestartArgs) -> Result<(), CliError> {
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

    #[cfg(target_os = "macos")]
    {
        let uid = u32::from(nix::unistd::getuid());
        restart_macos(&home, uid)?;
    }
    #[cfg(target_os = "linux")]
    {
        let _ = home; // home unused on Linux path; unit path is under XDG
        restart_linux()?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = home;
        return Err(CliError::Daemon(DaemonError::UnsupportedPlatform));
    }

    let (pid, socket, build) = wait_until_healthy().await?;
    println!("broker restarted  pid={pid}  socket={socket}  build={build}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_timed_out_names_socket_and_budget() {
        let err = DaemonError::RestartTimedOut {
            waited_ms: 5000,
            socket: "/tmp/bus.sock".into(),
        };
        let msg = err.to_string();
        assert!(
            msg.contains("5000ms") && msg.contains("/tmp/bus.sock"),
            "RestartTimedOut must name budget and socket; got: {msg}"
        );
        assert!(
            msg.contains("inspect broker"),
            "RestartTimedOut must point at the inspect surface; got: {msg}"
        );
    }

    #[test]
    fn ready_budget_is_bounded_not_a_fixed_sleep() {
        // Guard: the readiness budget must stay short enough for scripts
        // (`restart && famp …`) and long enough for launchd/systemd respawn.
        // A fixed multi-second sleep is forbidden — this constant is the
        // upper bound on polling, not a sleep duration.
        assert!(RESTART_READY_BUDGET <= Duration::from_secs(10));
        assert!(RESTART_READY_POLL < RESTART_READY_BUDGET);
        assert!(RESTART_READY_POLL >= Duration::from_millis(50));
    }
}

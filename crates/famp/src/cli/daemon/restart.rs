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
//! Hardening (adversarial review of 7ef6a46):
//! - Each probe/inspect attempt is wrapped in `tokio::time::timeout` against
//!   the remaining budget so a stalling socket peer cannot hang past the 5s
//!   readiness ceiling (`raw_connect_probe` / `read_frame` have no internal
//!   timeout).
//! - A Healthy broker is accepted only when its argv contains
//!   `--no-idle-exit` (the daemon plist/unit form). Auto-spawn orphans use
//!   `broker --socket …` without that flag; accepting them would leave the
//!   daemon job unbound while restart reported success (#20 second-order trap).
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

/// How long to wait for a daemon-managed broker Hello after the service
/// manager returns. This is a hard ceiling: each probe attempt is itself
/// timed against the remaining budget.
const RESTART_READY_BUDGET: Duration = Duration::from_secs(5);
/// Backoff between readiness probes when the socket is not yet ready.
const RESTART_READY_POLL: Duration = Duration::from_millis(100);
/// Cap on a single probe/inspect attempt so one stalling peer cannot burn
/// the whole budget in one hung `read_frame`.
const RESTART_PROBE_ATTEMPT_CAP: Duration = Duration::from_millis(500);

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

/// True when `cmdline` is the daemon-managed broker form (plist/unit argv
/// includes `--no-idle-exit`). Auto-spawn uses `broker --socket …` without
/// that flag — see `bus_client::spawn`.
fn cmdline_is_daemon_managed(cmdline: &str) -> bool {
    cmdline.contains("--no-idle-exit")
}

/// Read the process command line for `pid`, if the process still exists.
///
/// Linux: `/proc/<pid>/cmdline` (NUL-separated → spaces).
/// macOS: `ps -o command= -p <pid>` (same surface issue #20's recovery used).
fn process_cmdline(pid: u32) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let raw = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
        if raw.is_empty() {
            return None;
        }
        Some(
            raw.iter()
                .map(|&b| if b == 0 { b' ' } else { b })
                .map(char::from)
                .collect::<String>()
                .trim()
                .to_owned(),
        )
    }
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("ps")
            .args(["-o", "command=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let s = String::from_utf8(output.stdout).ok()?;
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        None
    }
}

fn is_daemon_managed_pid(pid: u32) -> bool {
    process_cmdline(pid).is_some_and(|c| cmdline_is_daemon_managed(&c))
}

/// One readiness attempt: Hello + inspect broker. No internal deadline —
/// callers must wrap with `tokio::time::timeout`.
///
/// Returns `Some((pid, socket, build))` on a successful inspect Info reply.
async fn probe_broker_info(sock: &Path) -> Option<(u32, String, String)> {
    match raw_connect_probe(sock).await {
        ProbeOutcome::Healthy { mut stream } => {
            let kind = InspectKind::Broker(InspectBrokerRequest::default());
            let payload = famp_inspect_client::call(&mut stream, kind).await.ok()?;
            match serde_json::from_value::<InspectBrokerReply>(payload).ok()? {
                InspectBrokerReply::Info(info) => {
                    Some((info.pid, info.socket_path, info.build_version))
                }
                InspectBrokerReply::BudgetExceeded { .. } => None,
            }
        }
        ProbeOutcome::DownClean
        | ProbeOutcome::StaleSocket
        | ProbeOutcome::OrphanHolder { .. }
        | ProbeOutcome::PermissionDenied => None,
    }
}

/// Poll until a **daemon-managed** broker answers Hello within the budget.
///
/// Returns `(pid, socket_path, build_version)` on success.
async fn wait_until_healthy() -> Result<(u32, String, String), DaemonError> {
    let sock = crate::bus_client::resolve_sock_path();
    let sock_display = sock.display().to_string();
    let deadline = Instant::now() + RESTART_READY_BUDGET;
    let mut saw_orphan_pid: Option<u32> = None;

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }

        // Each attempt is hard-capped so a stalling peer cannot hang past
        // the overall budget (probe + inspect have no internal timeouts).
        let attempt = remaining.min(RESTART_PROBE_ATTEMPT_CAP);
        match tokio::time::timeout(attempt, probe_broker_info(&sock)).await {
            Ok(Some((pid, socket, build))) if is_daemon_managed_pid(pid) => {
                return Ok((pid, socket, build));
            }
            Ok(Some((pid, _, _))) => {
                // Healthy FAMP broker, but not the daemon form — keep
                // polling in case the daemon later displaces it; remember
                // the pid so a timed-out budget can name the trap.
                saw_orphan_pid = Some(pid);
            }
            Ok(None) | Err(_) => {
                // Not ready, or this attempt hit its per-probe cap.
            }
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        tokio::time::sleep(RESTART_READY_POLL.min(remaining)).await;
    }

    let waited_ms = u64::try_from(RESTART_READY_BUDGET.as_millis()).unwrap_or(u64::MAX);
    if let Some(pid) = saw_orphan_pid {
        return Err(DaemonError::OrphanBrokerHoldsSocket {
            pid,
            socket: sock_display,
        });
    }
    Err(DaemonError::RestartTimedOut {
        waited_ms,
        socket: sock_display,
    })
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
    fn orphan_broker_error_names_pid_and_flag() {
        let err = DaemonError::OrphanBrokerHoldsSocket {
            pid: 4242,
            socket: "/tmp/bus.sock".into(),
        };
        let msg = err.to_string();
        assert!(
            msg.contains("4242") && msg.contains("--no-idle-exit") && msg.contains("/tmp/bus.sock"),
            "OrphanBrokerHoldsSocket must name pid, flag, and socket; got: {msg}"
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
        // Per-attempt cap must be strictly less than the overall budget so
        // a single hung peer cannot consume the whole ceiling.
        assert!(RESTART_PROBE_ATTEMPT_CAP < RESTART_READY_BUDGET);
        assert!(RESTART_PROBE_ATTEMPT_CAP >= Duration::from_millis(100));
    }

    #[test]
    fn cmdline_daemon_managed_requires_no_idle_exit() {
        assert!(cmdline_is_daemon_managed(
            "/Users/x/.cargo/bin/famp broker --no-idle-exit"
        ));
        assert!(
            !cmdline_is_daemon_managed("/Users/x/.cargo/bin/famp broker --socket /tmp/bus.sock"),
            "auto-spawn form must NOT count as daemon-managed"
        );
        assert!(!cmdline_is_daemon_managed(""));
        assert!(!cmdline_is_daemon_managed("famp broker"));
    }

    #[test]
    fn live_daemon_pid_is_detected_when_running() {
        // Soft integration: if a daemon-managed broker is live on this
        // host, `process_cmdline` + `cmdline_is_daemon_managed` must agree
        // with `ps`. Skips cleanly when nothing is running.
        let output = Command::new("pgrep")
            .args(["-f", "famp broker --no-idle-exit"])
            .output();
        let Ok(out) = output else {
            return;
        };
        if !out.status.success() {
            return;
        }
        let Some(pid_str) = std::str::from_utf8(&out.stdout)
            .ok()
            .and_then(|s| s.lines().next())
        else {
            return;
        };
        let Ok(pid) = pid_str.trim().parse::<u32>() else {
            return;
        };
        assert!(
            is_daemon_managed_pid(pid),
            "live pid {pid} from pgrep must classify as daemon-managed"
        );
    }
}

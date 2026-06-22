//! `famp daemon status` — three-state daemon status with distinct exit codes.
//!
//! DAEMON-03: reports not-installed / installed-down / running with distinct
//! output and distinct exit codes (1 / 2 / 0).
//!
//! D-03 (Decision B): the Running render prints the daemon build_version from
//! `InspectBrokerReply.build_version` so a user can diagnose version skew
//! without a connect-time round-trip. Client build is logged at connect; daemon
//! build surfaces here.
//!
//! D-09: reports linger state on all platforms (Option<bool>, None on macOS,
//! Some on Linux via `loginctl show-user --property=Linger`).
//!
//! T-05-12 (security): the launchctl registration probe uses EXIT CODE ONLY.
//! stdout and stderr are redirected to Stdio::null(). No text parsing of
//! `launchctl print` output — man page: "NOT API in any sense at all".

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::Args;
use serde::Serialize;

use crate::cli::error::CliError;

#[derive(Debug, Args)]
pub struct DaemonStatusArgs {
    /// Output status as JSON.
    #[arg(long)]
    pub json: bool,

    /// Override the install target home (defaults to `dirs::home_dir()`).
    /// Hidden flag - used by integration tests to redirect to a tempdir.
    #[arg(long, env = "FAMP_INSTALL_TARGET_HOME", hide = true)]
    pub home: Option<PathBuf>,
}

// ─── State render enum ───────────────────────────────────────────────────────

/// Three-state daemon status render for `famp daemon status`.
///
/// - `NotInstalled` — the platform service file does not exist (exit 1)
/// - `InstalledDown` — registered/installed but broker is not reachable (exit 2)
/// - `Running` — registered and broker is healthy (exit 0)
#[derive(Serialize)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
enum DaemonStateRender {
    NotInstalled {
        platform_path: String,
    },
    InstalledDown {
        platform_path: String,
        evidence: String,
        /// Linger state: None on macOS (not applicable), Some on Linux.
        linger: Option<bool>,
    },
    Running {
        pid: u32,
        socket_path: String,
        /// The daemon process's own CARGO_PKG_VERSION (`InspectBrokerReply.build_version`).
        /// D-03 (Decision B): this is the daemon-build surface; client build is logged at connect.
        build_version: String,
        /// Linger state: None on macOS (not applicable), Some on Linux.
        linger: Option<bool>,
    },
    Busy {
        platform_path: String,
        elapsed_ms: u64,
        /// Linger state: None on macOS (not applicable), Some on Linux.
        linger: Option<bool>,
    },
}

// ─── Pure helper functions (unit-testable without launchctl/inspect) ─────────

/// Map a `DaemonStateRender` variant to its exit code.
///
/// - `Running` → 0 (success)
/// - `InstalledDown` → 2 (installed but not running)
/// - `NotInstalled` → 1 (not installed)
const fn exit_code(r: &DaemonStateRender) -> i32 {
    match r {
        DaemonStateRender::Running { .. } => 0,
        DaemonStateRender::InstalledDown { .. } | DaemonStateRender::Busy { .. } => 2,
        DaemonStateRender::NotInstalled { .. } => 1,
    }
}

/// Render a `DaemonStateRender` as a human-readable string.
///
/// The `Running` render MUST include the daemon `build_version` so a user can
/// diagnose version skew per D-03.
fn render_human(r: &DaemonStateRender) -> String {
    match r {
        DaemonStateRender::NotInstalled { platform_path } => {
            format!("state: NOT_INSTALLED  service file not found at {platform_path}")
        }
        DaemonStateRender::InstalledDown {
            platform_path,
            evidence,
            linger,
        } => {
            let linger_suffix = match linger {
                Some(true) => "  linger=yes",
                Some(false) => "  linger=no",
                None => "",
            };
            format!(
                "state: INSTALLED_DOWN  service={platform_path}  evidence={evidence}{linger_suffix}"
            )
        }
        DaemonStateRender::Running {
            pid,
            socket_path,
            build_version,
            linger,
        } => {
            let linger_suffix = match linger {
                Some(true) => "  linger=yes",
                Some(false) => "  linger=no",
                None => "",
            };
            format!(
                "state: RUNNING  pid={pid}  socket={socket_path}  broker build: {build_version}{linger_suffix}"
            )
        }
        DaemonStateRender::Busy {
            platform_path,
            elapsed_ms,
            linger,
        } => {
            let linger_suffix = match linger {
                Some(true) => "  linger=yes",
                Some(false) => "  linger=no",
                None => "",
            };
            format!(
                "state: BUSY  service={platform_path}  evidence=budget_exceeded: {elapsed_ms}ms{linger_suffix}"
            )
        }
    }
}

// ─── Platform helpers ─────────────────────────────────────────────────────────

/// Returns the platform-specific path to the FAMP broker service file.
///
/// macOS: `{home}/Library/LaunchAgents/com.famp.broker.plist`
/// Linux: `{home}/.config/systemd/user/famp-broker.service`
fn platform_service_path(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library")
            .join("LaunchAgents")
            .join("com.famp.broker.plist")
    }
    #[cfg(target_os = "linux")]
    {
        home.join(".config")
            .join("systemd")
            .join("user")
            .join("famp-broker.service")
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        // Fallback for other platforms — will always appear "not installed"
        home.join("famp-broker.service")
    }
}

/// macOS: probe whether the LaunchAgent label is registered with launchd.
///
/// Uses EXIT CODE ONLY. stdout/stderr → Stdio::null().
/// Do NOT parse launchctl print text output — man page: "NOT API in any sense at all".
/// exit 0 = registered; exit 113 = "Could not find service" (not registered).
#[cfg(target_os = "macos")]
pub(crate) fn launchctl_is_registered(label: &str, uid: u32) -> bool {
    Command::new("launchctl")
        .args(["print", &format!("gui/{uid}/{label}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Linux: probe whether the systemd user unit is active.
///
/// Returns true iff `systemctl --user is-active famp-broker.service` exits 0.
#[cfg(target_os = "linux")]
fn systemctl_is_active() -> bool {
    Command::new("systemctl")
        .args(["--user", "is-active", "famp-broker.service"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Linux: run `loginctl show-user <user> --property=Linger` and parse the result.
///
/// Returns `Some(true)` if linger is enabled, `Some(false)` if disabled,
/// `None` if the command fails or output is unparseable.
#[cfg(target_os = "linux")]
fn query_linger() -> Option<bool> {
    use crate::cli::daemon::linux::parse_linger;
    let user = std::env::var("USER").unwrap_or_else(|_| "root".to_string());
    let output = Command::new("loginctl")
        .args(["show-user", &user, "--property=Linger"])
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    Some(parse_linger(&text))
}

// ─── Async run ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_lines)]
pub async fn run(args: DaemonStatusArgs) -> Result<(), CliError> {
    use famp_inspect_client::{raw_connect_probe, ProbeOutcome};
    use famp_inspect_proto::{InspectBrokerReply, InspectBrokerRequest, InspectKind};

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

    let service_path = platform_service_path(&home);
    let service_path_str = service_path.to_string_lossy().into_owned();

    // Step 1: Does the service file exist?
    if !service_path.exists() {
        let render = DaemonStateRender::NotInstalled {
            platform_path: service_path_str,
        };
        return emit_and_exit(&render, args.json);
    }

    // Step 2: Is the service registered with the OS service manager?
    #[cfg(target_os = "macos")]
    let registered = {
        let uid = u32::from(nix::unistd::getuid());
        launchctl_is_registered("com.famp.broker", uid)
    };
    #[cfg(target_os = "linux")]
    let registered = systemctl_is_active();
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let registered = false;

    if !registered {
        #[cfg(target_os = "linux")]
        let linger = query_linger();
        #[cfg(not(target_os = "linux"))]
        let linger: Option<bool> = None;

        let render = DaemonStateRender::InstalledDown {
            platform_path: service_path_str,
            evidence: "not_registered_with_service_manager".to_string(),
            linger,
        };
        return emit_and_exit(&render, args.json);
    }

    // Step 3: Is the broker actually healthy? Use the same probe as `inspect broker`.
    let sock = crate::bus_client::resolve_sock_path();
    let outcome = raw_connect_probe(&sock).await;

    let render = match outcome {
        ProbeOutcome::Healthy { mut stream } => {
            let kind = InspectKind::Broker(InspectBrokerRequest::default());
            match famp_inspect_client::call(&mut stream, kind).await {
                Ok(payload) => match serde_json::from_value::<InspectBrokerReply>(payload) {
                    Ok(InspectBrokerReply::Info(info)) => {
                        #[cfg(target_os = "linux")]
                        let linger = query_linger();
                        #[cfg(not(target_os = "linux"))]
                        let linger: Option<bool> = None;

                        DaemonStateRender::Running {
                            pid: info.pid,
                            socket_path: info.socket_path,
                            build_version: info.build_version,
                            linger,
                        }
                    }
                    Ok(InspectBrokerReply::BudgetExceeded { elapsed_ms }) => {
                        #[cfg(target_os = "linux")]
                        let linger = query_linger();
                        #[cfg(not(target_os = "linux"))]
                        let linger: Option<bool> = None;

                        DaemonStateRender::Busy {
                            platform_path: service_path_str,
                            elapsed_ms,
                            linger,
                        }
                    }
                    Err(e) => {
                        #[cfg(target_os = "linux")]
                        let linger = query_linger();
                        #[cfg(not(target_os = "linux"))]
                        let linger: Option<bool> = None;

                        DaemonStateRender::InstalledDown {
                            platform_path: service_path_str,
                            evidence: format!("schema_mismatch: {e}"),
                            linger,
                        }
                    }
                },
                Err(e) => {
                    #[cfg(target_os = "linux")]
                    let linger = query_linger();
                    #[cfg(not(target_os = "linux"))]
                    let linger: Option<bool> = None;

                    DaemonStateRender::InstalledDown {
                        platform_path: service_path_str,
                        evidence: format!("inspect_call_failed: {e}"),
                        linger,
                    }
                }
            }
        }
        ProbeOutcome::DownClean => {
            #[cfg(target_os = "linux")]
            let linger = query_linger();
            #[cfg(not(target_os = "linux"))]
            let linger: Option<bool> = None;
            DaemonStateRender::InstalledDown {
                platform_path: service_path_str,
                evidence: "no_socket_file".to_string(),
                linger,
            }
        }
        ProbeOutcome::StaleSocket => {
            #[cfg(target_os = "linux")]
            let linger = query_linger();
            #[cfg(not(target_os = "linux"))]
            let linger: Option<bool> = None;
            DaemonStateRender::InstalledDown {
                platform_path: service_path_str,
                evidence: "connect_econnrefused".to_string(),
                linger,
            }
        }
        ProbeOutcome::OrphanHolder {
            hello_reject_summary,
        } => {
            #[cfg(target_os = "linux")]
            let linger = query_linger();
            #[cfg(not(target_os = "linux"))]
            let linger: Option<bool> = None;
            DaemonStateRender::InstalledDown {
                platform_path: service_path_str,
                evidence: format!("hello_rejected: {hello_reject_summary}"),
                linger,
            }
        }
        ProbeOutcome::PermissionDenied => {
            #[cfg(target_os = "linux")]
            let linger = query_linger();
            #[cfg(not(target_os = "linux"))]
            let linger: Option<bool> = None;
            DaemonStateRender::InstalledDown {
                platform_path: service_path_str,
                evidence: "connect_eacces".to_string(),
                linger,
            }
        }
    };

    emit_and_exit(&render, args.json)
}

fn emit_and_exit(render: &DaemonStateRender, json: bool) -> Result<(), CliError> {
    if json {
        let s = serde_json::to_string_pretty(render)
            .map_err(|e| CliError::Generic(format!("json serialize: {e}")))?;
        println!("{s}");
    } else {
        println!("{}", render_human(render));
    }

    match exit_code(render) {
        0 => Ok(()),
        code => Err(CliError::Exit(code)),
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// DAEMON-03: exit code mapping — each state maps to a distinct exit code.
    #[test]
    fn status_exit_codes() {
        let not_installed = DaemonStateRender::NotInstalled {
            platform_path: "/tmp/com.famp.broker.plist".to_string(),
        };
        let installed_down = DaemonStateRender::InstalledDown {
            platform_path: "/tmp/com.famp.broker.plist".to_string(),
            evidence: "not_registered".to_string(),
            linger: None,
        };
        let running = DaemonStateRender::Running {
            pid: 1234,
            socket_path: "/tmp/bus.sock".to_string(),
            build_version: "0.11.0".to_string(),
            linger: None,
        };

        assert_eq!(exit_code(&not_installed), 1, "NotInstalled must exit 1");
        assert_eq!(exit_code(&installed_down), 2, "InstalledDown must exit 2");
        assert_eq!(exit_code(&running), 0, "Running must exit 0");
    }

    /// DAEMON-03: each render variant produces distinct human-readable output.
    #[test]
    fn status_render_distinct() {
        let not_installed = DaemonStateRender::NotInstalled {
            platform_path: "/tmp/com.famp.broker.plist".to_string(),
        };
        let installed_down = DaemonStateRender::InstalledDown {
            platform_path: "/tmp/com.famp.broker.plist".to_string(),
            evidence: "not_registered".to_string(),
            linger: None,
        };
        let running = DaemonStateRender::Running {
            pid: 1234,
            socket_path: "/tmp/bus.sock".to_string(),
            build_version: "0.11.0".to_string(),
            linger: None,
        };

        let s_not = render_human(&not_installed);
        let s_down = render_human(&installed_down);
        let s_run = render_human(&running);

        assert!(
            s_not.contains("NOT_INSTALLED"),
            "NotInstalled render missing NOT_INSTALLED label: {s_not}"
        );
        assert!(
            s_down.contains("INSTALLED_DOWN"),
            "InstalledDown render missing INSTALLED_DOWN label: {s_down}"
        );
        assert!(
            s_run.contains("RUNNING"),
            "Running render missing RUNNING label: {s_run}"
        );

        // All three must be different
        assert_ne!(
            s_not, s_down,
            "NotInstalled and InstalledDown renders are identical"
        );
        assert_ne!(
            s_down, s_run,
            "InstalledDown and Running renders are identical"
        );
        assert_ne!(
            s_not, s_run,
            "NotInstalled and Running renders are identical"
        );
    }

    /// D-03 (Decision B): Running render must include the daemon build_version.
    #[test]
    fn status_running_shows_build() {
        let running = DaemonStateRender::Running {
            pid: 9999,
            socket_path: "/tmp/bus.sock".to_string(),
            build_version: "0.11.0".to_string(),
            linger: None,
        };

        let output = render_human(&running);
        assert!(
            output.contains("0.11.0"),
            "Running render must include daemon build_version '0.11.0', got: {output}"
        );
    }

    /// INSP-BROKER-02 (Busy path): BudgetExceeded renders as BUSY (alive-but-degraded),
    /// never INSTALLED_DOWN. Exit code preserved at 2.
    #[test]
    fn status_busy_render_and_exit_code() {
        let busy = DaemonStateRender::Busy {
            platform_path: "/tmp/x.plist".to_string(),
            elapsed_ms: 612,
            linger: None,
        };
        assert_eq!(exit_code(&busy), 2, "Busy must exit 2");
        let output = render_human(&busy);
        assert!(output.contains("BUSY"), "expected BUSY in: {output}");
        assert!(output.contains("612"), "expected elapsed_ms in: {output}");
        assert!(
            !output.contains("INSTALLED_DOWN"),
            "must not contain INSTALLED_DOWN: {output}"
        );
    }

    /// D-09: linger state reported in human render when Some.
    #[test]
    fn status_linger_reported() {
        let running_linger_yes = DaemonStateRender::Running {
            pid: 1,
            socket_path: "/tmp/bus.sock".to_string(),
            build_version: "0.11.0".to_string(),
            linger: Some(true),
        };
        let running_linger_no = DaemonStateRender::Running {
            pid: 1,
            socket_path: "/tmp/bus.sock".to_string(),
            build_version: "0.11.0".to_string(),
            linger: Some(false),
        };
        let running_no_linger = DaemonStateRender::Running {
            pid: 1,
            socket_path: "/tmp/bus.sock".to_string(),
            build_version: "0.11.0".to_string(),
            linger: None,
        };

        let yes_str = render_human(&running_linger_yes);
        let no_str = render_human(&running_linger_no);
        let none_str = render_human(&running_no_linger);

        assert!(
            yes_str.contains("linger=yes"),
            "expected linger=yes in: {yes_str}"
        );
        assert!(
            no_str.contains("linger=no"),
            "expected linger=no in: {no_str}"
        );
        assert!(
            !none_str.contains("linger"),
            "expected no linger in macOS render: {none_str}"
        );
    }
}

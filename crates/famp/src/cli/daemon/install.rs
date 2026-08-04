//! `famp daemon install` — writes the platform service file and loads it.
//!
//! Plan 02 implemented:
//!   - `DaemonError` thiserror enum (shared by all daemon submodules via `CliError::Daemon`)
//!   - `DaemonInstallArgs` with hidden `--home` test override
//!   - `generate_plist(home: &Path)` producing the locked guardian-reviewed plist XML
//!   - `run_at(home, err)` stub writing the plist only (no launchctl)
//!
//! Plan 04 (this file) adds:
//!   - `check_not_sandboxed` — BOOT-02: refuse install inside a sandbox (EPERM-on-bind probe)
//!   - `load_macos` — idempotent `launchctl bootstrap gui/$UID <plist>`; tolerates exit 37
//!   - `install_linux` — systemd `--user enable --now`; detect-and-instruct linger (D-08)
//!   - systemd ≥ 240 floor documented for `StandardOutput=append:` directive (DAEMON-06)
//!   - `refuses_in_sandbox` unit test (BOOT-02)

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Args;

use crate::bus_client::spawn::{strict_bind_probe, SpawnError};
use crate::cli::error::CliError;
use crate::cli::executable::{resolve_for_generated_config, FampExecutable};

// ─── Args ────────────────────────────────────────────────────────────────────

#[derive(Debug, Args)]
pub struct DaemonInstallArgs {
    /// Override the install target home (defaults to `dirs::home_dir()`).
    /// Hidden flag - used by integration tests to redirect to a tempdir.
    #[arg(long, env = "FAMP_INSTALL_TARGET_HOME", hide = true)]
    pub home: Option<PathBuf>,
}

// ─── Errors ──────────────────────────────────────────────────────────────────

/// Typed errors for the daemon install/uninstall/status/restart lifecycle.
///
/// Shared by all daemon submodules via `CliError::Daemon(#[from] DaemonError)`.
/// Variants used in Plans 04 and 05 are defined here so the `#[from]` wiring
/// in `error.rs` resolves immediately and Plans 04/05 can add their logic
/// without a module collision.
#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    /// Install-time resolution of the `famp` binary to embed in the service
    /// file failed. Carries the resolver message verbatim (it already names
    /// the actionable fix); `CliError::Daemon` renders it at the top level.
    #[error("{0}")]
    FampExecutable(String),

    /// The daemon log path derived from `home` is not valid UTF-8, so it
    /// cannot be interpolated into a systemd unit.
    #[error("daemon log path is not valid UTF-8 under {}", home.display())]
    LogPathNonUtf8 { home: PathBuf },
    /// I/O error while reading or writing a service file.
    #[error("io error at {}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Install was attempted from inside a sandboxed shell (e.g. Claude Code
    /// or Codex). Daemon install must run from a normal (unsandboxed) shell.
    #[error(
        "cannot install daemon service inside a sandbox; \
         run `famp daemon install` from a normal shell"
    )]
    SandboxedShell,

    /// `launchctl bootstrap` or related command failed with a non-idempotent
    /// exit code. The exit code is embedded for diagnostics.
    #[error("launchctl failed with exit code {0}")]
    LaunchctlFailed(i32),

    /// `systemctl` is not present on this Linux system.
    /// The daemon must be started manually with `famp broker --no-idle-exit`.
    #[error("systemctl not found; start broker manually with `famp broker --no-idle-exit`")]
    SystemctlAbsent,

    /// `systemctl --user` command failed with the given exit code.
    #[error("systemctl failed with exit code {0}")]
    SystemctlFailed(i32),

    /// This platform is neither macOS nor Linux — no service manager is
    /// supported. The user must start the broker manually.
    #[error(
        "unsupported platform: daemon install only supports macOS (launchd) \
         and Linux (systemd --user); start broker manually with \
         `famp broker --no-idle-exit`"
    )]
    UnsupportedPlatform,

    /// A lifecycle command (e.g. `famp daemon restart`) was run against a
    /// service that is not registered with the platform service manager.
    /// Names the install command so the user has an actionable next step.
    #[error("daemon service is not installed; run `famp daemon install` first")]
    NotInstalled,

    /// `famp daemon restart` asked the service manager to relaunch, but the
    /// broker never answered a Hello handshake within the readiness budget.
    /// Names the socket so the operator can compare with `famp inspect broker`.
    #[error(
        "daemon restart timed out after {waited_ms}ms waiting for broker Hello \
         at {socket}; check `famp inspect broker` and `~/.famp/broker.log` \
         (on macOS an EX_CONFIG crash loop after `just install` needs a full \
         bootout+bootstrap, which restart now performs — if this still fails, \
         try `famp daemon uninstall` then `famp daemon install`)"
    )]
    RestartTimedOut { waited_ms: u64, socket: String },

    /// A broker answered Hello on the bus socket, but its argv is not the
    /// daemon-managed form (`broker --no-idle-exit`). Auto-spawned orphans
    /// (`famp broker --socket …` without `--no-idle-exit`) bind the socket and
    /// cause the LaunchAgent/systemd job to exit cleanly on bind conflict —
    /// readiness must not report success against that trap (issue #20).
    #[error(
        "daemon restart found a healthy broker at {socket} (pid={pid}) that is \
         not daemon-managed (missing --no-idle-exit in argv); kill pid {pid} \
         or free the socket, then re-run `famp daemon restart`"
    )]
    OrphanBrokerHoldsSocket { pid: u32, socket: String },

    /// A path interpolated into the systemd unit's `ExecStart` contains
    /// whitespace. systemd tokenizes `ExecStart` on whitespace, so such a path
    /// would split into separate argv tokens and the unit would fail to start.
    /// We refuse loudly rather than write an unactivatable unit.
    #[error(
        "cannot install daemon service: path contains whitespace which systemd \
         ExecStart cannot represent: {0}; start broker manually with \
         `famp broker --no-idle-exit`"
    )]
    UnitPathHasWhitespace(String),
}

// ─── Plist generation ────────────────────────────────────────────────────────

/// Escape the XML metacharacters that are illegal inside an XML `<string>`
/// text node (`&`, `<`, `>`). `&` is replaced first so the ampersands
/// introduced by `&lt;`/`&gt;` are not themselves re-escaped.
///
/// A home directory containing one of these characters is legal on
/// macOS/Linux; without escaping it yields a malformed plist that launchd
/// silently refuses to parse, leaving the service permanently unloaded.
#[cfg(any(target_os = "macos", test))]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Generate the macOS LaunchAgent plist XML for the FAMP broker service.
///
/// The generated XML matches the locked guardian-reviewed shape (DAEMON-02):
/// - `Label` = `com.famp.broker`
/// - `ProgramArguments` = `[<abs famp binary>, "broker", "--no-idle-exit"]`
/// - `RunAtLoad` = `true`
/// - `KeepAlive` = `true` (unconditional boolean — NOT a dict)
/// - `ProcessType` = `"Background"`
/// - `StandardOutPath` = `StandardErrorPath` = `{home}/.famp/broker.log` (ABSOLUTE)
/// - NO `EnvironmentVariables` key
/// - NO `UserName` / `GroupName` key
///
/// All paths are resolved from `home` using `Path::join` — no tilde expansion,
/// no string concatenation. Guardian requirement: launchd does NOT expand `~`.
#[cfg(any(target_os = "macos", test))]
#[allow(clippy::unnecessary_wraps)] // preserves the `?`-using call site in `run_at`
pub(crate) fn generate_plist(
    home: &Path,
    executable: &FampExecutable,
) -> Result<String, DaemonError> {
    let famp_bin = executable.path();
    let log_path = home.join(".famp").join("broker.log");

    // XML-escape home-derived paths before interpolating into <string> elements.
    // A home dir containing `&`, `<`, or `>` (legal on macOS/Linux) would
    // otherwise produce a malformed plist that launchd silently refuses to load.
    // `&` MUST be escaped first, or the `&` introduced by `&lt;`/`&gt;` would be
    // double-escaped.
    let famp_bin_str = xml_escape(&famp_bin.display().to_string());
    let log_path_str = xml_escape(&log_path.display().to_string());

    // Verify the generated paths are absolute (no tilde) — defense-in-depth.
    // Path::join always produces an absolute path when `home` is absolute.
    debug_assert!(
        !famp_bin_str.contains('~'),
        "generate_plist: famp binary path contains tilde: {famp_bin_str}"
    );
    debug_assert!(
        !log_path_str.contains('~'),
        "generate_plist: log path contains tilde: {log_path_str}"
    );

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.famp.broker</string>
    <key>ProgramArguments</key>
    <array>
        <string>{famp_bin_str}</string>
        <string>broker</string>
        <string>--no-idle-exit</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>ProcessType</key>
    <string>Background</string>
    <key>StandardOutPath</key>
    <string>{log_path_str}</string>
    <key>StandardErrorPath</key>
    <string>{log_path_str}</string>
</dict>
</plist>
"#,
    );

    Ok(xml)
}

// ─── macOS: plist-change detection (reload advisory) ─────────────────────────

/// What writing the generated service file did to the copy already on disk.
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ServiceFileOutcome {
    /// No file was there before.
    Created,
    /// Byte-identical to what was already there.
    Unchanged,
    /// Different content — e.g. a different resolved `famp` binary.
    Updated,
}

/// Classify a service-file write without performing it.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn service_file_outcome(existing: Option<&str>, generated: &str) -> ServiceFileOutcome {
    match existing {
        None => ServiceFileOutcome::Created,
        Some(previous) if previous == generated => ServiceFileOutcome::Unchanged,
        Some(_) => ServiceFileOutcome::Updated,
    }
}

/// True when the operator must be told that the on-disk plist and the loaded
/// launchd job have diverged.
///
/// launchd keeps the `ProgramArguments` it was bootstrapped with: rewriting
/// the plist under an already-loaded job does NOT repoint the running service.
/// `famp daemon install` deliberately does not bootout/bootstrap here —
/// reloading drops every in-memory registration and every parked `famp await`
/// (see `cli::daemon::restart`), which an idempotent install must not do
/// silently. `famp daemon restart` is the command that performs that reload.
#[cfg(any(target_os = "macos", test))]
pub(crate) const fn needs_reload_advisory(
    outcome: ServiceFileOutcome,
    already_loaded: bool,
) -> bool {
    already_loaded && matches!(outcome, ServiceFileOutcome::Updated)
}

// ─── BOOT-02: Sandbox guard ───────────────────────────────────────────────────

/// Check that install is NOT being run from inside a sandboxed shell.
///
/// BOOT-02: uses the strict bind probe (`strict_bind_probe`) — if the
/// shell is sandboxed (Claude Code / Codex), binding a Unix socket to
/// `{home}/.famp/` fails with EPERM/EACCES, surfaced as `SpawnError::SandboxEperm`.
/// A sandboxed install would write a service file that can never bind its socket
/// (silent broken state), so we refuse before writing anything.
///
/// The probe directory must exist (ENOENT -> Ok() in the probe, which would
/// silently pass) — we create it here to ensure the probe gives a real answer.
fn check_not_sandboxed(bus_dir: &Path) -> Result<(), DaemonError> {
    // Ensure the bus_dir exists so the probe gives EPERM/EACCES (not ENOENT,
    // which the probe treats as Ok).
    std::fs::create_dir_all(bus_dir).map_err(|source| DaemonError::Io {
        path: bus_dir.to_path_buf(),
        source,
    })?;
    // Use the STRICT probe (not the lenient spawn fast-path probe): as an
    // install gate, an unexpected bind errno must fail loudly, not fail-open.
    match strict_bind_probe(bus_dir) {
        Err(SpawnError::SandboxEperm) => Err(DaemonError::SandboxedShell),
        Err(e) => Err(DaemonError::Io {
            path: bus_dir.to_path_buf(),
            source: std::io::Error::other(e.to_string()),
        }),
        Ok(()) => Ok(()),
    }
}

// ─── macOS: idempotent launchctl bootstrap ────────────────────────────────────

/// Load the LaunchAgent via `launchctl bootstrap gui/$UID <plist>`.
///
/// Idempotent (DAEMON-01): if the service is already registered in this domain,
/// return Ok WITHOUT re-bootstrapping — a non-destructive no-op that does not
/// restart a running broker. We check registration first rather than tolerating
/// a specific failure code, because real launchctl returns exit 5
/// ("Bootstrap failed: 5: Input/output error") when a label is already
/// bootstrapped — NOT exit 37 (an earlier, never-validated assumption). Exit 5
/// is a generic EIO that can mean other things, so we must not blanket-tolerate
/// it; the registration probe is the reliable idempotency signal.
#[cfg(target_os = "macos")]
fn load_macos(plist_path: &Path, uid: u32) -> Result<(), DaemonError> {
    // Already registered → idempotent no-op (do not restart a running broker).
    if super::status::launchctl_is_registered("com.famp.broker", uid) {
        return Ok(());
    }
    let plist_str = plist_path.to_str().unwrap_or_default();
    let status = Command::new("launchctl")
        .args(["bootstrap", &format!("gui/{uid}"), plist_str])
        .status()
        .map_err(|e| DaemonError::Io {
            path: plist_path.to_path_buf(),
            source: e,
        })?;
    if !status.success() {
        return Err(DaemonError::LaunchctlFailed(status.code().unwrap_or(-1)));
    }
    Ok(())
}

// ─── Linux: systemd --user install ───────────────────────────────────────────

/// Install the broker as a systemd --user service.
///
/// Writes `~/.config/systemd/user/famp-broker.service` and enables it with
/// `systemctl --user enable --now`.
///
/// DAEMON-06: systemd-absent path exits non-zero naming `famp broker --no-idle-exit`
/// as the fallback. Linger detect-and-instruct (D-08): if `loginctl show-user`
/// reports `Linger=no`, PRINT the `loginctl enable-linger <user>` command plus
/// the one consequence. Do NOT run it.
///
/// NOTE: `StandardOutput=append:` and `StandardError=append:` in the unit file
/// require systemd >= 240 (released 2018-09-22). Hosts with systemd < 240
/// (e.g. RHEL 7 with systemd 219) will fail to activate the service because of
/// the unsupported append: log directive. On such hosts, start the broker manually:
///   `famp broker --no-idle-exit`
#[cfg(target_os = "linux")]
fn install_linux(home: &Path, unit_content: &str, err: &mut dyn Write) -> Result<(), DaemonError> {
    // DAEMON-06: detect systemctl absent first.
    let systemctl_present = Command::new("sh")
        .args(["-c", "command -v systemctl"])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !systemctl_present {
        return Err(DaemonError::SystemctlAbsent);
    }

    // Build absolute paths (no tilde — systemd does not expand ~).
    let unit_dir = home.join(".config").join("systemd").join("user");
    let unit_path = unit_dir.join("famp-broker.service");

    std::fs::create_dir_all(&unit_dir).map_err(|source| DaemonError::Io {
        path: unit_dir.clone(),
        source,
    })?;

    // NOTE: StandardOutput=append: and StandardError=append: require systemd >= 240.
    // This is the committed floor for this unit file (Open Q3 RESOLVED).
    // On systemd < 240 (e.g. RHEL 7 with systemd 219) the service will fail to
    // activate; users on such hosts should start the broker manually:
    //   famp broker --no-idle-exit
    std::fs::write(&unit_path, unit_content).map_err(|source| DaemonError::Io {
        path: unit_path.clone(),
        source,
    })?;

    writeln!(err, "  [2/4] unit file written to {}", unit_path.display()).ok();

    // daemon-reload so systemd sees the new unit.
    let reload_status = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()
        .map_err(|source| DaemonError::Io {
            path: PathBuf::from("systemctl"),
            source,
        })?;
    if !reload_status.success() {
        return Err(DaemonError::SystemctlFailed(
            reload_status.code().unwrap_or(-1),
        ));
    }
    writeln!(err, "  [3/4] systemctl --user daemon-reload: ok").ok();

    // Enable and start the service.
    let enable_status = Command::new("systemctl")
        .args(["--user", "enable", "--now", "famp-broker.service"])
        .status()
        .map_err(|source| DaemonError::Io {
            path: PathBuf::from("systemctl"),
            source,
        })?;
    if !enable_status.success() {
        return Err(DaemonError::SystemctlFailed(
            enable_status.code().unwrap_or(-1),
        ));
    }
    writeln!(
        err,
        "  [4/4] systemctl --user enable --now famp-broker.service: ok"
    )
    .ok();

    // D-08: detect-and-instruct linger. Do NOT run `loginctl enable-linger`.
    let user = std::env::var("USER").unwrap_or_default();
    let linger_output = Command::new("loginctl")
        .args(["show-user", &user, "--property=Linger"])
        .output()
        .ok();

    let linger_enabled = linger_output
        .as_ref()
        .and_then(|o| std::str::from_utf8(&o.stdout).ok())
        .is_some_and(crate::cli::daemon::linux::parse_linger);

    if !linger_enabled {
        writeln!(err, "\nNote: linger is not enabled for user '{user}'.").ok();
        writeln!(
            err,
            "The broker will stop when you log out. To keep it running across logouts, run:"
        )
        .ok();
        writeln!(err, "  loginctl enable-linger {user}").ok();
        writeln!(
            err,
            "(This changes a system policy and is intentionally not run automatically.)"
        )
        .ok();
    }

    Ok(())
}

/// Generate the systemd --user unit for the FAMP broker.
///
/// Supervision limits are deliberate, not decorative. `Restart=always` on its
/// own is a footgun: systemd's stock start limit (5 starts / 10 s) can only
/// catch a service that dies *instantly*. A broker that dies after ~60 s — the
/// normal shape for a socket-holding daemon losing a bind race — restarts
/// forever at ~1/`RestartSec` and no limit ever fires. So we emit:
///   - `RestartSec=5` + `RestartSteps=5` + `RestartMaxDelaySec=5min`
///     (exponential backoff 5 s -> 5 min, so a restart storm self-throttles)
///   - `StartLimitIntervalSec=1h` + `StartLimitBurst=20` in `[Unit]`
///     (a genuine crash loop trips in minutes and leaves the unit `failed`)
///   - `MemoryMax=256M` — steady-state broker RSS is single-digit MiB, so this
///     is ~35x headroom. It cannot bite legitimate operation; it bounds a leak.
///
/// Version floors: `StandardOutput=append:` needs systemd 240 or newer (see
/// `install_linux`); `MemoryMax` needs 208; `StartLimitIntervalSec` in
/// `[Unit]` needs 230; `RestartSteps` and `RestartMaxDelaySec` need 254.
/// systemd *ignores* unknown unit-file keys with a warning rather than
/// refusing the unit, so on systemd 240..254 the unit still loads and runs —
/// it simply does not get the backoff. That degradation is intentional:
/// correctness on old hosts is preserved, and only the extra safety is lost.
#[cfg(any(target_os = "linux", test))]
pub(crate) fn generate_systemd_unit(
    home: &Path,
    executable: &FampExecutable,
) -> Result<String, DaemonError> {
    let famp_bin = executable.utf8();
    let log_path = home.join(".famp").join("broker.log");
    let log = log_path
        .to_str()
        .ok_or_else(|| DaemonError::LogPathNonUtf8 {
            home: home.to_path_buf(),
        })?;
    for path in [famp_bin, log] {
        if path.chars().any(char::is_whitespace) {
            return Err(DaemonError::UnitPathHasWhitespace(path.to_string()));
        }
    }
    Ok(format!(
        "[Unit]\nDescription=FAMP Local Bus Broker\nAfter=default.target\n\
         StartLimitIntervalSec=1h\nStartLimitBurst=20\n\n\
         [Service]\nExecStart={famp_bin} broker --no-idle-exit\nRestart=always\n\
         RestartSec=5\nRestartSteps=5\nRestartMaxDelaySec=5min\nMemoryMax=256M\n\
         StandardOutput=append:{log}\nStandardError=append:{log}\n\n\
         [Install]\nWantedBy=default.target\n"
    ))
}

// ─── Run logic ───────────────────────────────────────────────────────────────

/// Write the platform service file and load it.
///
/// BOOT-02: refuses to install if called from inside a sandboxed shell
/// (EPERM-on-bind probe via `check_not_sandboxed`). The check runs BEFORE
/// writing any file — no silent broken state.
///
/// macOS: writes the plist to `{home}/Library/LaunchAgents/com.famp.broker.plist`
/// and loads it via `launchctl bootstrap gui/$UID <plist>` (idempotent — exit 37
/// "already registered" is tolerated).
///
/// Linux: writes the systemd unit to `~/.config/systemd/user/famp-broker.service`
/// and enables it via `systemctl --user enable --now`. Detect-and-instructs
/// `loginctl enable-linger` (D-08) if linger is off. Exits non-zero if systemctl
/// is absent (DAEMON-06).
///
/// Guardian authorization (DAEMON-02): the loaded plist matches the shape
/// reviewed and approved in Plan 03 (GUARDIAN-SIGNOFF.md). The real home
/// directory is interpolated by `generate_plist(home)` — no literal placeholder.
#[allow(clippy::needless_return)] // explicit `return` per cfg branch; only one compiles per platform
pub fn run_at(home: &Path, err: &mut dyn Write) -> Result<(), DaemonError> {
    let executable = resolve_for_generated_config().map_err(|error| {
        DaemonError::FampExecutable(crate::cli::executable::flatten_error_chain(&error))
    })?;
    run_at_with_executable(home, &executable, err)
}

#[allow(clippy::needless_return)]
fn run_at_with_executable(
    home: &Path,
    executable: &FampExecutable,
    err: &mut dyn Write,
) -> Result<(), DaemonError> {
    writeln!(err, "Installing FAMP broker service...").ok();

    #[cfg(target_os = "linux")]
    let unit_content = generate_systemd_unit(home, executable)?;

    // BOOT-02: check for sandbox BEFORE writing anything.
    // The bus dir is the probe target; create_dir_all inside check_not_sandboxed
    // ensures the dir exists so EPERM/EACCES (not ENOENT) is returned in a sandbox.
    let bus_dir = home.join(".famp");
    check_not_sandboxed(&bus_dir)?;

    #[cfg(target_os = "macos")]
    {
        let agents_dir = home.join("Library").join("LaunchAgents");
        std::fs::create_dir_all(&agents_dir).map_err(|source| DaemonError::Io {
            path: agents_dir.clone(),
            source,
        })?;

        let plist_path = agents_dir.join("com.famp.broker.plist");
        let xml = generate_plist(home, executable)?;

        // Classify the write BEFORE performing it: a changed plist under an
        // already-loaded job needs an explicit reload to take effect.
        let uid = u32::from(nix::unistd::getuid());
        let already_loaded = super::status::launchctl_is_registered("com.famp.broker", uid);
        let outcome =
            service_file_outcome(std::fs::read_to_string(&plist_path).ok().as_deref(), &xml);

        std::fs::write(&plist_path, &xml).map_err(|source| DaemonError::Io {
            path: plist_path.clone(),
            source,
        })?;
        writeln!(err, "  [1/2] plist {outcome:?} at {}", plist_path.display()).ok();

        // Load the service via launchctl bootstrap (guardian-authorized action,
        // DAEMON-02 sign-off: sha256 b5d52c13eff63de697746b16da6676f2315fa2c631d2bc1b8bf21992cfbdeb3f).
        load_macos(&plist_path, uid)?;
        writeln!(
            err,
            "  [2/2] launchctl bootstrap gui/{uid}: ok (service loaded)"
        )
        .ok();

        writeln!(err).ok();
        if needs_reload_advisory(outcome, already_loaded) {
            writeln!(
                err,
                "note: com.famp.broker was already loaded, so launchd is still running the \
                 PREVIOUS ProgramArguments. The updated plist (famp binary: {}) takes effect \
                 after an explicit reload:\n  famp daemon restart\n\
                 (restart performs launchctl bootout + bootstrap + kickstart; it drops \
                 in-memory registrations and parked `famp await` waiters, which is why \
                 `famp daemon install` does not do it for you.)",
                executable.path().display()
            )
            .ok();
        }
        writeln!(err, "daemon install complete.").ok();
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        writeln!(err, "  [1/4] sandbox check: ok").ok();
        install_linux(home, &unit_content, err)?;
        writeln!(err).ok();
        writeln!(err, "daemon install complete.").ok();
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
pub fn run(args: DaemonInstallArgs) -> Result<(), CliError> {
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

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn executable_at(path: &Path) -> FampExecutable {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        FampExecutable::validate(path.to_path_buf()).unwrap()
    }

    #[test]
    fn plist_uses_exact_symlink_path_and_xml_escapes_it() {
        let dir = tempfile::tempdir().unwrap();
        let target = executable_at(&dir.path().join("real-famp"));
        #[cfg(unix)]
        {
            let link = dir.path().join("famp & <selected>");
            std::os::unix::fs::symlink(target.path(), &link).unwrap();
            let selected = FampExecutable::validate(link).unwrap();
            let xml = generate_plist(dir.path(), &selected).unwrap();
            assert!(xml.contains("famp &amp; &lt;selected&gt;"));
            assert!(!xml.contains("real-famp</string>"));
        }
    }

    #[test]
    fn systemd_unit_uses_exact_path_and_rejects_whitespace_purely() {
        let dir = tempfile::tempdir().unwrap();
        let selected = executable_at(&dir.path().join("bin/famp"));
        let unit = generate_systemd_unit(dir.path(), &selected).unwrap();
        assert!(unit.contains(&format!(
            "ExecStart={} broker --no-idle-exit",
            selected.utf8()
        )));
        assert!(!unit.contains(".cargo/bin/famp"));

        let spaced = executable_at(&dir.path().join("bin space/famp"));
        assert!(matches!(
            generate_systemd_unit(dir.path(), &spaced),
            Err(DaemonError::UnitPathHasWhitespace(_))
        ));
        assert!(!dir
            .path()
            .join(".config/systemd/user/famp-broker.service")
            .exists());
    }

    /// The generated unit must carry supervision limits. `Restart=always`
    /// WITHOUT backoff and WITHOUT a usable start limit is the configuration
    /// that lets a slow-dying daemon restart forever unnoticed — systemd's
    /// stock 5-starts/10s limit cannot fire on a service that takes ~60s to
    /// die. Asserted explicitly so the directives cannot be dropped silently.
    #[test]
    fn systemd_unit_carries_restart_backoff_and_a_memory_cap() {
        let dir = tempfile::tempdir().unwrap();
        let selected = executable_at(&dir.path().join("bin/famp"));
        let unit = generate_systemd_unit(dir.path(), &selected).unwrap();

        for directive in [
            "Restart=always",
            "RestartSec=5",
            "RestartSteps=5",
            "RestartMaxDelaySec=5min",
            "MemoryMax=256M",
            "StartLimitIntervalSec=1h",
            "StartLimitBurst=20",
        ] {
            assert!(
                unit.contains(directive),
                "generated unit is missing {directive}; got:\n{unit}"
            );
        }

        // The start-limit pair belongs to [Unit]; systemd ignores it in
        // [Service]. Assert placement, not just presence.
        let (unit_section, service_section) = unit.split_once("[Service]").unwrap();
        assert!(unit_section.contains("StartLimitIntervalSec=1h"));
        assert!(unit_section.contains("StartLimitBurst=20"));
        assert!(service_section.contains("RestartSteps=5"));
        assert!(service_section.contains("MemoryMax=256M"));
    }

    /// M2 (daemon half): `run_at` is the highest public orchestration entry
    /// for `famp daemon install`. A resolution failure must land before the
    /// service file is written, before `~/.famp/` is created by the sandbox
    /// probe, and before any service-manager command runs.
    ///
    /// This test never reaches `launchctl` / `systemctl`: the resolver fails
    /// first, which is precisely the guarantee under test.
    #[test]
    fn public_run_at_fails_before_any_mutation_when_executable_is_unresolvable() {
        use crate::cli::executable::test_support::{
            assert_tree_unchanged, snapshot_tree, MISSING_FAMP_BIN,
        };

        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        // Representative pre-existing service files on both platforms.
        std::fs::create_dir_all(home.join("Library/LaunchAgents")).unwrap();
        std::fs::create_dir_all(home.join(".config/systemd/user")).unwrap();
        let plist = home.join("Library/LaunchAgents/com.famp.broker.plist");
        let unit = home.join(".config/systemd/user/famp-broker.service");
        std::fs::write(&plist, "<!-- prior plist -->\n").unwrap();
        std::fs::write(&unit, "# prior unit\n").unwrap();
        let before = snapshot_tree(home);

        for value in [MISSING_FAMP_BIN, " ", home.to_str().unwrap()] {
            let result = temp_env::with_var("FAMP_INSTALL_FAMP_BIN", Some(value), || {
                let mut err = Vec::<u8>::new();
                run_at(home, &mut err)
            });
            assert!(
                matches!(result, Err(DaemonError::FampExecutable(_))),
                "FAMP_INSTALL_FAMP_BIN={value:?} must fail resolution, got {result:?}"
            );
            assert_tree_unchanged(home, &before, &format!("daemon install {value:?}"));
            assert!(
                !home.join(".famp").exists(),
                "sandbox probe must not create the bus dir before resolution succeeds"
            );
        }
    }

    /// H2: `famp daemon install` rewrites the service file but never reloads
    /// an already-loaded launchd job — reloading would drop every in-memory
    /// registration and parked await. The decision logic that drives the
    /// operator advisory is pure and tested here; `launchctl` itself stays
    /// out of the unit test.
    #[test]
    fn changed_plist_under_a_loaded_job_advises_an_explicit_reload() {
        use ServiceFileOutcome::{Created, Unchanged, Updated};

        assert_eq!(service_file_outcome(None, "<plist/>"), Created);
        assert_eq!(
            service_file_outcome(Some("<plist/>"), "<plist/>"),
            Unchanged
        );
        assert_eq!(
            service_file_outcome(Some("<plist>old bin</plist>"), "<plist>new bin</plist>"),
            Updated
        );

        // Only a content change under a loaded job diverges from what launchd
        // is running; every other combination is silent (idempotency).
        assert!(needs_reload_advisory(Updated, true));
        assert!(!needs_reload_advisory(Updated, false));
        assert!(!needs_reload_advisory(Unchanged, true));
        assert!(!needs_reload_advisory(Created, true));
        assert!(!needs_reload_advisory(Created, false));
    }

    /// The advisory must fire exactly when the resolved binary moves — the
    /// case this PR introduces (a reinstall that repoints `ProgramArguments`).
    #[test]
    fn repointing_the_famp_binary_is_a_plist_change() {
        let dir = tempfile::tempdir().unwrap();
        let first = executable_at(&dir.path().join("a/famp"));
        let second = executable_at(&dir.path().join("b/famp"));
        let before = generate_plist(dir.path(), &first).unwrap();
        let after = generate_plist(dir.path(), &second).unwrap();
        assert_eq!(
            service_file_outcome(Some(&before), &after),
            ServiceFileOutcome::Updated
        );
        assert_eq!(
            service_file_outcome(Some(&before), &before),
            ServiceFileOutcome::Unchanged
        );
    }

    /// DAEMON-02 (generation half): verify the generated plist matches the
    /// locked guardian-reviewed shape exactly.
    ///
    /// Guardian requirement: each bullet below is a hard invariant.
    /// The APPROVAL half (guardian sign-off before first load) is gated in Plan 03.
    #[test]
    fn plist_shape_matches_locked() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let executable = FampExecutable::validate(std::env::current_exe().unwrap()).unwrap();
        let xml = generate_plist(home, &executable).unwrap();

        // Label
        assert!(
            xml.contains("<key>Label</key>"),
            "Label key missing from plist"
        );
        assert!(
            xml.contains("<string>com.famp.broker</string>"),
            "Label value com.famp.broker missing"
        );

        // RunAtLoad
        assert!(
            xml.contains("<key>RunAtLoad</key>"),
            "RunAtLoad key missing"
        );

        // KeepAlive — unconditional <true/>, NOT a dict
        assert!(
            xml.contains("<key>KeepAlive</key>"),
            "KeepAlive key missing"
        );

        // ProcessType
        assert!(
            xml.contains("<key>ProcessType</key>"),
            "ProcessType key missing"
        );
        assert!(
            xml.contains("<string>Background</string>"),
            "ProcessType=Background missing"
        );

        // ProgramArguments: broker flag and --no-idle-exit
        assert!(
            xml.contains("<string>broker</string>"),
            "broker argument missing from ProgramArguments"
        );
        assert!(
            xml.contains("<string>--no-idle-exit</string>"),
            "--no-idle-exit flag missing from ProgramArguments"
        );

        // The <true/> tag must appear (for both RunAtLoad and KeepAlive)
        assert!(
            xml.matches("<true/>").count() >= 2,
            "expected at least 2 <true/> tags (RunAtLoad + KeepAlive), got: {xml}"
        );

        // Log path — contains .famp/broker.log for both StandardOutPath and StandardErrorPath
        assert!(
            xml.contains(".famp/broker.log"),
            ".famp/broker.log missing from StandardOutPath/StandardErrorPath"
        );
        assert_eq!(
            xml.matches(".famp/broker.log").count(),
            2,
            "expected .famp/broker.log exactly twice (StandardOutPath + StandardErrorPath)"
        );

        // No tilde anywhere (launchd does NOT expand ~)
        assert!(
            !xml.contains('~'),
            "tilde must not appear in generated plist; got: {xml}"
        );

        // No EnvironmentVariables key (T-05-05 mitigated)
        assert!(
            !xml.contains("EnvironmentVariables"),
            "EnvironmentVariables must not appear in plist; got: {xml}"
        );

        // No UserName or GroupName (T-05-06 mitigated: user-level LaunchAgent only)
        assert!(
            !xml.contains("UserName"),
            "UserName must not appear in plist; got: {xml}"
        );
        assert!(
            !xml.contains("GroupName"),
            "GroupName must not appear in plist; got: {xml}"
        );
    }

    /// The sample fixture (for guardian gate) must match generate_plist output
    /// for the representative home `/Users/USERNAME` byte-for-byte.
    ///
    /// This catches silent divergence (e.g. trailing-newline mismatch) between
    /// the generated XML and the fixture file that guardian reviews.
    #[test]
    fn sample_fixture_matches_generate_plist() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/sample-com.famp.broker.plist"
        ));
        let executable = FampExecutable::validate(std::env::current_exe().unwrap()).unwrap();
        let generated = generate_plist(std::path::Path::new("/Users/USERNAME"), &executable)
            .unwrap()
            .replace(executable.utf8(), "/Users/USERNAME/.cargo/bin/famp");
        assert_eq!(
            generated, fixture,
            "sample fixture does not match generate_plist output for /Users/USERNAME"
        );
    }

    /// BOOT-02: install must return DaemonError::SandboxedShell when
    /// `check_not_sandboxed` detects a sandboxed environment (EPERM-on-bind).
    ///
    /// Simulation: create a temp directory, make it mode 0o500 (owner rx, no
    /// write), then probe it. The bind() call fails with EACCES which the probe
    /// maps to SandboxEperm → SandboxedShell.
    ///
    /// Permissions are restored before the TempDir drops to avoid cleanup errors.
    #[test]
    fn refuses_in_sandbox() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let bus_dir = tmp.path().join(".famp");
        std::fs::create_dir_all(&bus_dir).unwrap();

        // Restrict permissions so bind() will fail with EACCES.
        std::fs::set_permissions(&bus_dir, std::fs::Permissions::from_mode(0o500)).unwrap();

        let result = check_not_sandboxed(&bus_dir);

        // Restore perms before drop (otherwise TempDir cleanup fails).
        std::fs::set_permissions(&bus_dir, std::fs::Permissions::from_mode(0o700)).ok();

        // On macOS/Linux the restricted dir produces EACCES → SandboxEperm → SandboxedShell.
        // On some CI environments the test process may be root (where EACCES is not
        // returned even for mode-0 dirs). Skip gracefully in that case.
        let is_root = nix::unistd::getuid().is_root();
        if is_root {
            // Root bypasses permission checks — skip assertion.
            return;
        }

        assert!(
            matches!(result, Err(DaemonError::SandboxedShell)),
            "expected SandboxedShell on EACCES bus_dir, got: {result:?}"
        );
    }
}

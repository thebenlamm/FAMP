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

use crate::bus_client::spawn::{preflight_bind_probe, SpawnError};
use crate::cli::error::CliError;

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
}

// ─── Plist generation ────────────────────────────────────────────────────────

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
pub(crate) fn generate_plist(home: &Path) -> Result<String, DaemonError> {
    let famp_bin = home.join(".cargo").join("bin").join("famp");
    let log_path = home.join(".famp").join("broker.log");

    let famp_bin_str = famp_bin.display().to_string();
    let log_path_str = log_path.display().to_string();

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
        <string>{famp_bin}</string>
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
    <string>{log}</string>
    <key>StandardErrorPath</key>
    <string>{log}</string>
</dict>
</plist>
"#,
        famp_bin = famp_bin_str,
        log = log_path_str,
    );

    Ok(xml)
}

// ─── BOOT-02: Sandbox guard ───────────────────────────────────────────────────

/// Check that install is NOT being run from inside a sandboxed shell.
///
/// BOOT-02: reuses the existing Phase 4 `preflight_bind_probe` — if the
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
    match preflight_bind_probe(bus_dir) {
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
/// Idempotent: tolerates exit code 37 ("service already registered").
/// Any other non-zero exit is a hard error (`DaemonError::LaunchctlFailed`).
#[cfg(target_os = "macos")]
fn load_macos(plist_path: &Path, uid: u32) -> Result<(), DaemonError> {
    let plist_str = plist_path.to_str().unwrap_or_default();
    let status = Command::new("launchctl")
        .args(["bootstrap", &format!("gui/{uid}"), plist_str])
        .status()
        .map_err(|e| DaemonError::Io {
            path: plist_path.to_path_buf(),
            source: e,
        })?;
    if !status.success() {
        let code = status.code().unwrap_or(-1);
        if code == 37 {
            // Exit 37 = "service already registered" — idempotent success.
            return Ok(());
        }
        return Err(DaemonError::LaunchctlFailed(code));
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
fn install_linux(home: &Path, err: &mut dyn Write) -> Result<(), DaemonError> {
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
    let famp_bin = home.join(".cargo").join("bin").join("famp");
    let log_path = home.join(".famp").join("broker.log");
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
    let unit_content = format!(
        "[Unit]\nDescription=FAMP Local Bus Broker\nAfter=default.target\n\n\
         [Service]\nExecStart={famp_bin} broker --no-idle-exit\nRestart=always\n\
         StandardOutput=append:{log}\nStandardError=append:{log}\n\n\
         [Install]\nWantedBy=default.target\n",
        famp_bin = famp_bin.display(),
        log = log_path.display(),
    );

    std::fs::write(&unit_path, &unit_content).map_err(|source| DaemonError::Io {
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
        .map(crate::cli::daemon::linux::parse_linger)
        .unwrap_or(false);

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
pub fn run_at(home: &Path, err: &mut dyn Write) -> Result<(), DaemonError> {
    writeln!(err, "Installing FAMP broker service...").ok();

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
        let xml = generate_plist(home)?;

        std::fs::write(&plist_path, &xml).map_err(|source| DaemonError::Io {
            path: plist_path.clone(),
            source,
        })?;
        writeln!(err, "  [1/2] plist written to {}", plist_path.display()).ok();

        // Load the service via launchctl bootstrap (guardian-authorized action,
        // DAEMON-02 sign-off: sha256 b5d52c13eff63de697746b16da6676f2315fa2c631d2bc1b8bf21992cfbdeb3f).
        let uid = u32::from(nix::unistd::getuid());
        load_macos(&plist_path, uid)?;
        writeln!(
            err,
            "  [2/2] launchctl bootstrap gui/{uid}: ok (service loaded)"
        )
        .ok();

        writeln!(err).ok();
        writeln!(err, "daemon install complete.").ok();
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        writeln!(err, "  [1/4] sandbox check: ok").ok();
        install_linux(home, err)?;
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

    /// DAEMON-02 (generation half): verify the generated plist matches the
    /// locked guardian-reviewed shape exactly.
    ///
    /// Guardian requirement: each bullet below is a hard invariant.
    /// The APPROVAL half (guardian sign-off before first load) is gated in Plan 03.
    #[test]
    fn plist_shape_matches_locked() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let xml = generate_plist(home).unwrap();

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
            "/../../.planning/phases/05-daemon-service-management-version-safety/sample-com.famp.broker.plist"
        ));
        let generated = generate_plist(std::path::Path::new("/Users/USERNAME")).unwrap();
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

//! `famp daemon install` — writes the platform service file to disk.
//!
//! This plan (02) implements:
//!   - `DaemonError` thiserror enum (shared by all daemon submodules via `CliError::Daemon`)
//!   - `DaemonInstallArgs` with hidden `--home` test override
//!   - `generate_plist(home: &Path)` producing the locked guardian-reviewed plist XML
//!   - `run_at(home, err)` writing the plist to `~/Library/LaunchAgents/`
//!     (generation + file write only; no launchctl load — that is Plan 04)
//!   - `run(args)` resolving home and calling `run_at`
//!
//! Plan 04 adds the launchctl/systemctl invocations and BOOT-02 sandbox check.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;

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

// ─── Run logic ───────────────────────────────────────────────────────────────

/// Write the plist to `{home}/Library/LaunchAgents/com.famp.broker.plist`.
///
/// This plan (02) ONLY generates and writes the plist file — it does NOT call
/// launchctl to load the service. Plan 04 adds the bootstrap invocation.
pub fn run_at(home: &Path, err: &mut dyn Write) -> Result<(), DaemonError> {
    writeln!(err, "Installing FAMP broker service...").ok();

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

    writeln!(err, "  [1/1] plist written to {}", plist_path.display()).ok();
    writeln!(err).ok();
    writeln!(
        err,
        "daemon install: plist written (service loading wired in Plan 04)."
    )
    .ok();

    Ok(())
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
}

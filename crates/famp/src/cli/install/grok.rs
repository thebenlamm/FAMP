//! `famp install-grok` subcommand handler.
//!
//! Installs Grok integration surfaces only:
//!  1. `~/.grok/config.toml :: [mcp_servers.famp]` (same shape as Codex)
//!  2. `~/.grok/skills/famp-listen/SKILL.md` (non-blocking listen-wake skill)
//!
//! Intentionally does **not** install a long blocking Stop hook — Grok's
//! UI stays free; auto-wake is via `famp listen-wake` + a persistent
//! monitor (see the skill).

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;
use toml::Value as TomlValue;

use crate::cli::error::CliError;
use crate::cli::install::toml_merge;

/// Embedded Grok skill body (non-blocking listen-wake instructions).
pub const FAMP_LISTEN_SKILL_MD: &str =
    include_str!("../../../assets/skills/famp-listen/SKILL.md");

#[derive(Debug, Args)]
pub struct InstallGrokArgs {
    /// Override the install target home (defaults to `dirs::home_dir()`).
    /// Hidden flag — used by integration tests to redirect to a tempdir.
    #[arg(long, env = "FAMP_INSTALL_TARGET_HOME", hide = true)]
    pub home: Option<PathBuf>,
}

#[allow(clippy::needless_pass_by_value)]
pub fn run(args: InstallGrokArgs) -> Result<(), CliError> {
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
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    run_at(&home, &mut stdout, &mut stderr)
}

/// Test-facing entry: explicit home + writable handles.
pub fn run_at(home: &Path, _out: &mut dyn Write, err: &mut dyn Write) -> Result<(), CliError> {
    let config_path = home.join(".grok").join("config.toml");
    let skill_path = home
        .join(".grok")
        .join("skills")
        .join("famp-listen")
        .join("SKILL.md");
    // Prefer bare `famp` when on PATH so shell/env moves stay portable;
    // fall back to absolute path only when `which` fails.
    let (command, command_note) = match which::which("famp") {
        Ok(p) => (
            "famp".to_string(),
            format!("famp (on PATH; resolved {})", p.display()),
        ),
        Err(_) => {
            let abs = home.join(".cargo").join("bin").join("famp");
            (
                abs.display().to_string(),
                format!("{} (famp not on PATH)", abs.display()),
            )
        }
    };

    writeln!(err, "Installing Grok MCP entry into {}", home.display()).ok();
    writeln!(err, "  resolved famp command: {command_note}").ok();

    let mut famp_table = toml::Table::new();
    famp_table.insert("command".into(), TomlValue::String(command));
    famp_table.insert(
        "args".into(),
        TomlValue::Array(vec![TomlValue::String("mcp".into())]),
    );
    famp_table.insert("startup_timeout_sec".into(), TomlValue::Integer(10));

    // Reuse the Codex TOML merge helper — same `[mcp_servers.famp]` shape.
    let outcome = toml_merge::upsert_codex_table(&config_path, "mcp_servers", "famp", famp_table)?;
    writeln!(
        err,
        "  [1/2] {} :: [mcp_servers.famp] -> {:?}",
        config_path.display(),
        outcome
    )
    .ok();

    install_skill(&skill_path)?;
    writeln!(
        err,
        "  [2/2] {} :: famp-listen skill installed",
        skill_path.display()
    )
    .ok();

    writeln!(err).ok();
    writeln!(
        err,
        "install-grok complete. Restart Grok sessions to pick up MCP changes. \
         MCP register(listen=true) arms `famp listen-wake --loop` (daemon); \
         Grok inject uses `famp listen-wake --as <id> --follow` (no long Stop hook)."
    )
    .ok();
    if which::which("famp").is_err() {
        writeln!(
            err,
            "  hint: famp binary not on PATH; run `cargo install famp` to install it."
        )
        .ok();
    }
    Ok(())
}

fn install_skill(path: &Path) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| CliError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(path, FAMP_LISTEN_SKILL_MD).map_err(|source| CliError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644)).map_err(
            |source| CliError::Io {
                path: path.to_path_buf(),
                source,
            },
        )?;
    }
    Ok(())
}

/// Remove only the skill file FAMP owns. Never `remove_dir_all` when the
/// directory contains extra user files.
///
/// - If `SKILL.md` matches our embedded asset (or is missing), delete it.
/// - If the skill dir then has only that file (now empty) or only our
///   file was present, remove the empty dir.
/// - If `SKILL.md` was modified by the user (content differs), leave it.
/// - Never deletes sibling user files.
pub(crate) fn remove_skill_dir(skill_dir: &Path) -> Result<(), CliError> {
    let skill_md = skill_dir.join("SKILL.md");
    match std::fs::read_to_string(&skill_md) {
        Ok(body) if body == FAMP_LISTEN_SKILL_MD => {
            std::fs::remove_file(&skill_md).map_err(|source| CliError::Io {
                path: skill_md.clone(),
                source,
            })?;
        }
        Ok(_) => {
            // User-modified skill — leave in place.
            return Ok(());
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(CliError::Io {
                path: skill_md,
                source,
            });
        }
    }

    // Remove the skill dir only when empty (or missing).
    match std::fs::read_dir(skill_dir) {
        Ok(mut entries) => {
            if entries.next().is_none() {
                std::fs::remove_dir(skill_dir).map_err(|source| CliError::Io {
                    path: skill_dir.to_path_buf(),
                    source,
                })?;
            }
            // Non-empty: leave user files alone.
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(CliError::Io {
            path: skill_dir.to_path_buf(),
            source,
        }),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn install_grok_writes_mcp_and_skill() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        run_at(home, &mut out, &mut err).unwrap();

        let cfg = home.join(".grok/config.toml");
        assert!(cfg.exists());
        let parsed: toml::Table = toml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        let famp_t = parsed["mcp_servers"]["famp"].as_table().unwrap();
        assert_eq!(
            famp_t["args"].as_array().unwrap()[0].as_str().unwrap(),
            "mcp"
        );
        assert_eq!(famp_t["startup_timeout_sec"].as_integer().unwrap(), 10);
        let cmd = famp_t["command"].as_str().unwrap();
        // Prefer bare `famp` when on PATH; otherwise absolute path ending in famp.
        assert!(
            cmd == "famp" || cmd.ends_with("/famp") || cmd.ends_with("famp"),
            "unexpected command: {cmd}"
        );

        let skill = home.join(".grok/skills/famp-listen/SKILL.md");
        assert!(skill.exists());
        let body = std::fs::read_to_string(&skill).unwrap();
        assert!(body.contains("famp listen-wake"));
        assert!(body.contains("persistent") || body.contains("--follow"));

        // Must not pollute Claude/Codex trees.
        assert!(!home.join(".claude").exists());
        assert!(!home.join(".codex").exists());
        assert!(!home.join(".famp/hook-runner.sh").exists());
    }

    #[test]
    fn remove_skill_dir_preserves_user_files() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("famp-listen");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), FAMP_LISTEN_SKILL_MD).unwrap();
        std::fs::write(skill_dir.join("notes.txt"), "user owned").unwrap();
        remove_skill_dir(&skill_dir).unwrap();
        assert!(!skill_dir.join("SKILL.md").exists());
        assert!(skill_dir.join("notes.txt").exists());
        assert!(skill_dir.exists());
    }

    #[test]
    fn remove_skill_dir_leaves_modified_skill() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("famp-listen");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "user customized skill\n").unwrap();
        remove_skill_dir(&skill_dir).unwrap();
        assert_eq!(
            std::fs::read_to_string(skill_dir.join("SKILL.md")).unwrap(),
            "user customized skill\n"
        );
    }

    #[test]
    fn install_grok_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        run_at(home, &mut out, &mut err).unwrap();
        let first = std::fs::read_to_string(home.join(".grok/config.toml")).unwrap();

        std::thread::sleep(std::time::Duration::from_secs(1));

        let mut out2 = Vec::<u8>::new();
        let mut err2 = Vec::<u8>::new();
        run_at(home, &mut out2, &mut err2).unwrap();
        let second = std::fs::read_to_string(home.join(".grok/config.toml")).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn skill_asset_mentions_monitor_and_forbids_long_stop() {
        assert!(FAMP_LISTEN_SKILL_MD.contains("famp listen-wake --as"));
        assert!(
            FAMP_LISTEN_SKILL_MD.contains("--follow")
                || FAMP_LISTEN_SKILL_MD.contains("persistent")
        );
        assert!(FAMP_LISTEN_SKILL_MD.to_lowercase().contains("never"));
        assert!(FAMP_LISTEN_SKILL_MD.contains("Stop"));
    }
}

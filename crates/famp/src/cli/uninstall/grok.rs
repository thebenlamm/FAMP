//! `famp uninstall-grok` subcommand handler.
//!
//! Reverses `install-grok` mutations under `~/.grok/` only:
//!  1. removes `~/.grok/config.toml :: [mcp_servers.famp]`
//!  2. removes FAMP-owned `~/.grok/skills/famp-listen/SKILL.md` only
//!     (never deletes user-owned sibling files; leaves modified skills)
//!  3. removes `~/.grok/hooks/famp-listen-stop.json`
//!  4. removes `~/.grok/hooks/famp-await.sh`
//!
//! Intentionally leaves `~/.claude/` alone — Claude may still own the
//! primary famp-await Stop path for Claude Code users.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;

use crate::cli::error::CliError;
use crate::cli::install::{await_hook, grok as install_grok, toml_merge};

#[derive(Debug, Args)]
pub struct UninstallGrokArgs {
    /// Override the install target home (defaults to `dirs::home_dir()`).
    /// Hidden flag — used by integration tests to redirect to a tempdir.
    #[arg(long, env = "FAMP_INSTALL_TARGET_HOME", hide = true)]
    pub home: Option<PathBuf>,
}

#[allow(clippy::needless_pass_by_value)]
pub fn run(args: UninstallGrokArgs) -> Result<(), CliError> {
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
    let skill_dir = home.join(".grok").join("skills").join("famp-listen");
    let stop_json = home
        .join(".grok")
        .join("hooks")
        .join("famp-listen-stop.json");
    let grok_await = home.join(".grok").join("hooks").join("famp-await.sh");

    writeln!(err, "Uninstalling Grok MCP entry from {}", home.display()).ok();
    let outcome = toml_merge::remove_codex_table(&config_path, "mcp_servers", "famp")?;
    writeln!(
        err,
        "  [1/4] {} :: [mcp_servers.famp] -> {:?}",
        config_path.display(),
        outcome
    )
    .ok();

    install_grok::remove_skill_dir(&skill_dir)?;
    writeln!(
        err,
        "  [2/4] {} :: famp-listen skill removed",
        skill_dir.display()
    )
    .ok();

    match std::fs::remove_file(&stop_json) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(CliError::Io {
                path: stop_json.clone(),
                source,
            });
        }
    }
    writeln!(
        err,
        "  [3/4] {} :: Stop hook json removed",
        stop_json.display()
    )
    .ok();

    await_hook::remove_shim(&grok_await)?;
    writeln!(
        err,
        "  [4/4] {} :: grok await shim removed",
        grok_await.display()
    )
    .ok();

    writeln!(err).ok();
    writeln!(err, "uninstall-grok complete. (~/.claude left untouched.)").ok();
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::cli::install;

    #[test]
    fn uninstall_after_install_removes_famp_table_skill_and_hooks() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        install::grok::run_at(home, &mut out, &mut err).unwrap();

        assert!(home.join(".grok/hooks/famp-listen-stop.json").exists());
        assert!(home.join(".grok/hooks/famp-await.sh").exists());
        // B2: install-grok never creates ~/.claude/
        assert!(!home.join(".claude").exists());

        let mut out2 = Vec::<u8>::new();
        let mut err2 = Vec::<u8>::new();
        run_at(home, &mut out2, &mut err2).unwrap();

        let cfg = home.join(".grok/config.toml");
        if cfg.exists() {
            let body = std::fs::read_to_string(&cfg).unwrap();
            if !body.trim().is_empty() {
                let parsed: toml::Table = toml::from_str(&body).unwrap();
                let has_famp = parsed
                    .get("mcp_servers")
                    .and_then(toml::Value::as_table)
                    .and_then(|t| t.get("famp"))
                    .is_some();
                assert!(!has_famp);
            }
        }
        assert!(!home.join(".grok/skills/famp-listen").exists());
        assert!(!home.join(".grok/hooks/famp-listen-stop.json").exists());
        assert!(!home.join(".grok/hooks/famp-await.sh").exists());
        assert!(!home.join(".claude").exists());
    }

    #[test]
    fn uninstall_on_clean_state_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        run_at(home, &mut out, &mut err).unwrap();
    }
}

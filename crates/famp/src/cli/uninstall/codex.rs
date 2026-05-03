//! `famp uninstall-codex` subcommand handler (D-12 inverse).

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;

use crate::cli::error::CliError;
use crate::cli::install::toml_merge;

#[derive(Debug, Args)]
pub struct UninstallCodexArgs {
    /// Override the install target home (defaults to `dirs::home_dir()`).
    /// Hidden flag - used by integration tests to redirect to a tempdir.
    #[arg(long, env = "FAMP_INSTALL_TARGET_HOME", hide = true)]
    pub home: Option<PathBuf>,
}

#[allow(clippy::needless_pass_by_value)]
pub fn run(args: UninstallCodexArgs) -> Result<(), CliError> {
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

pub fn run_at(home: &Path, _out: &mut dyn Write, err: &mut dyn Write) -> Result<(), CliError> {
    let config_path = home.join(".codex").join("config.toml");
    writeln!(err, "Uninstalling Codex MCP entry from {}", home.display()).ok();
    let outcome = toml_merge::remove_codex_table(&config_path, "mcp_servers", "famp")?;
    writeln!(
        err,
        "  {} :: [mcp_servers.famp] -> {:?}",
        config_path.display(),
        outcome
    )
    .ok();
    writeln!(err).ok();
    writeln!(err, "uninstall-codex complete.").ok();
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::cli::install;

    #[test]
    fn uninstall_after_install_removes_famp_table() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        install::codex::run_at(home, &mut out, &mut err).unwrap();

        let mut out2 = Vec::<u8>::new();
        let mut err2 = Vec::<u8>::new();
        run_at(home, &mut out2, &mut err2).unwrap();

        let cfg = home.join(".codex/config.toml");
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

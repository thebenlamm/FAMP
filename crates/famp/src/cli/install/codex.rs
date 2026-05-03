//! `famp install-codex` subcommand handler (D-12: Codex parity, MCP-only).

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;
use toml::Value;

use crate::cli::error::CliError;
use crate::cli::install::toml_merge;

#[derive(Debug, Args)]
pub struct InstallCodexArgs {
    /// Override the install target home (defaults to `dirs::home_dir()`).
    /// Hidden flag - used by integration tests to redirect to a tempdir.
    #[arg(long, env = "FAMP_INSTALL_TARGET_HOME", hide = true)]
    pub home: Option<PathBuf>,
}

#[allow(clippy::needless_pass_by_value)]
pub fn run(args: InstallCodexArgs) -> Result<(), CliError> {
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
    let famp_bin = which::which("famp")
        .ok()
        .unwrap_or_else(|| home.join(".cargo").join("bin").join("famp"));

    writeln!(err, "Installing Codex MCP entry into {}", home.display()).ok();
    writeln!(err, "  resolved famp binary: {}", famp_bin.display()).ok();

    let mut famp_table = toml::Table::new();
    famp_table.insert(
        "command".into(),
        Value::String(famp_bin.display().to_string()),
    );
    famp_table.insert(
        "args".into(),
        Value::Array(vec![Value::String("mcp".into())]),
    );
    famp_table.insert("startup_timeout_sec".into(), Value::Integer(10));

    let outcome = toml_merge::upsert_codex_table(&config_path, "mcp_servers", "famp", famp_table)?;
    writeln!(
        err,
        "  {} :: [mcp_servers.famp] -> {:?}",
        config_path.display(),
        outcome
    )
    .ok();

    writeln!(err).ok();
    writeln!(
        err,
        "install-codex complete. Restart Codex sessions to pick up changes."
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn install_codex_writes_mcp_servers_famp_table() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        run_at(home, &mut out, &mut err).unwrap();

        let cfg = home.join(".codex/config.toml");
        assert!(cfg.exists());
        let parsed: toml::Table = toml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        let famp_t = parsed["mcp_servers"]["famp"].as_table().unwrap();
        assert_eq!(
            famp_t["args"].as_array().unwrap()[0].as_str().unwrap(),
            "mcp"
        );
        assert_eq!(famp_t["startup_timeout_sec"].as_integer().unwrap(), 10);
        assert!(famp_t["command"].as_str().unwrap().contains("famp"));
    }

    #[test]
    fn install_codex_preserves_unrelated_top_level_sections() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        std::fs::write(
            home.join(".codex/config.toml"),
            "[some_other_section]\nkey = \"keep me\"\n[mcp_servers.github]\ncommand = \"/x\"\n",
        )
        .unwrap();

        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        run_at(home, &mut out, &mut err).unwrap();

        let parsed: toml::Table =
            toml::from_str(&std::fs::read_to_string(home.join(".codex/config.toml")).unwrap())
                .unwrap();
        assert_eq!(
            parsed["some_other_section"]["key"].as_str().unwrap(),
            "keep me"
        );
        assert_eq!(
            parsed["mcp_servers"]["github"]["command"].as_str().unwrap(),
            "/x"
        );
        assert!(parsed["mcp_servers"]["famp"].as_table().is_some());
    }

    #[test]
    fn install_codex_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        run_at(home, &mut out, &mut err).unwrap();
        let first = std::fs::read_to_string(home.join(".codex/config.toml")).unwrap();

        std::thread::sleep(std::time::Duration::from_secs(1));

        let mut out2 = Vec::<u8>::new();
        let mut err2 = Vec::<u8>::new();
        run_at(home, &mut out2, &mut err2).unwrap();
        let second = std::fs::read_to_string(home.join(".codex/config.toml")).unwrap();
        assert_eq!(first, second);
    }
}

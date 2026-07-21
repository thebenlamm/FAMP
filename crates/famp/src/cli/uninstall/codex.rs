//! `famp uninstall-codex` subcommand handler.
//!
//! Reverses the `install-codex` mutations:
//!  1. removes `~/.codex/config.toml :: [mcp_servers.famp]`
//!  2. removes `<project>/.codex/hooks/famp-await.sh`
//!  3. surgically removes FAMP-owned Codex Stop hook entries
//!  4. removes the matching Codex hook-trust state entries

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;
use serde_json::Value;

use crate::cli::error::CliError;
use crate::cli::install::{await_hook, codex as install_codex, json_merge, stop_entry, toml_merge};

#[derive(Debug, Args)]
pub struct UninstallCodexArgs {
    /// Override the install target home (defaults to `dirs::home_dir()`).
    /// Hidden flag - used by integration tests to redirect to a tempdir.
    #[arg(long, env = "FAMP_INSTALL_TARGET_HOME", hide = true)]
    pub home: Option<PathBuf>,

    /// Project root whose `.codex/hooks.json` should have the Stop hook removed.
    /// Defaults to the current git root, or the current directory outside git.
    #[arg(long, env = "FAMP_INSTALL_CODEX_PROJECT_DIR")]
    pub project: Option<PathBuf>,
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
    let project = match args.project {
        Some(p) => p,
        None => install_codex::default_project_root()?,
    };
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    run_at_project(&home, &project, &mut stdout, &mut stderr)
}

pub fn run_at(home: &Path, out: &mut dyn Write, err: &mut dyn Write) -> Result<(), CliError> {
    run_at_project(home, home, out, err)
}

pub fn run_at_project(
    home: &Path,
    project: &Path,
    _out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let home = install_codex::normalize_absolute_path(home)?;
    let project = install_codex::normalize_absolute_path(project)?;
    let config_path = home.join(".codex").join("config.toml");
    let hooks_path = project.join(".codex").join("hooks.json");
    let await_shim_path = project.join(".codex").join("hooks").join("famp-await.sh");

    writeln!(err, "Uninstalling Codex MCP entry from {}", home.display()).ok();
    let outcome = toml_merge::remove_codex_table(&config_path, "mcp_servers", "famp")?;
    writeln!(
        err,
        "  [1/4] {} :: [mcp_servers.famp] -> {:?}",
        config_path.display(),
        outcome
    )
    .ok();

    await_hook::remove_shim(&await_shim_path)?;
    writeln!(
        err,
        "  [2/4] {} :: await shim removed",
        await_shim_path.display()
    )
    .ok();

    let removed_trust_keys = surgical_remove_codex_stop_entry(&hooks_path, &await_shim_path, err)?;
    let mut removed_any_trust = false;
    for trust_key in &removed_trust_keys {
        let outcome =
            toml_merge::remove_nested_table(&config_path, &["hooks", "state"], trust_key)?;
        writeln!(
            err,
            "  [4/4] {} :: hooks.state.\"{}\" -> {:?}",
            config_path.display(),
            trust_key,
            outcome
        )
        .ok();
        if outcome == toml_merge::TomlMergeOutcome::Removed {
            removed_any_trust = true;
        }
    }
    let trusted_hashes = install_codex::famp_trusted_hashes("stop", &await_shim_path);
    let stale_removed = install_codex::remove_stale_codex_hook_trust(
        &config_path,
        &hooks_path,
        None,
        &trusted_hashes,
    )?;
    for trust_key in &stale_removed {
        writeln!(
            err,
            "  [4/4] {} :: stale hooks.state.\"{}\" -> Removed",
            config_path.display(),
            trust_key,
        )
        .ok();
    }
    if !stale_removed.is_empty() {
        removed_any_trust = true;
    }
    if !removed_any_trust {
        writeln!(
            err,
            "  [4/4] {} :: no Codex hook trust entry to remove",
            config_path.display()
        )
        .ok();
    }

    writeln!(err).ok();
    writeln!(err, "uninstall-codex complete.").ok();
    Ok(())
}

fn surgical_remove_codex_stop_entry(
    hooks_path: &Path,
    await_shim_path: &Path,
    err: &mut dyn Write,
) -> Result<Vec<String>, CliError> {
    let existing: Value = match std::fs::read_to_string(hooks_path) {
        Ok(s) if s.trim().is_empty() => {
            writeln!(
                err,
                "  [3/4] {} :: empty file, no Stop entry to remove",
                hooks_path.display()
            )
            .ok();
            return Ok(Vec::new());
        }
        Ok(s) => serde_json::from_str(&s).map_err(|source| CliError::JsonMergeParse {
            path: hooks_path.to_path_buf(),
            source,
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            writeln!(
                err,
                "  [3/4] {} :: file absent, no Stop entry to remove",
                hooks_path.display()
            )
            .ok();
            return Ok(Vec::new());
        }
        Err(source) => {
            return Err(CliError::JsonMergeRead {
                path: hooks_path.to_path_buf(),
                source,
            });
        }
    };

    let prior_stop: Vec<Value> = existing
        .get("hooks")
        .and_then(|hooks| hooks.get("Stop"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    if prior_stop.is_empty() {
        writeln!(
            err,
            "  [3/4] {} :: no Stop array, nothing to remove",
            hooks_path.display()
        )
        .ok();
        return Ok(Vec::new());
    }

    let famp_paths = install_codex::famp_hook_command_patterns(await_shim_path);
    let mut removed_trust_keys = Vec::new();
    let mut filtered = Vec::new();
    for (group_index, entry) in prior_stop.iter().enumerate() {
        collect_removed_trust_keys(
            hooks_path,
            group_index,
            entry,
            &famp_paths,
            &mut removed_trust_keys,
        );
        if let Some(kept) = stop_entry::remove_famp_hook_from_stop_entry(entry, &famp_paths) {
            filtered.push(kept);
        }
    }

    if removed_trust_keys.is_empty() {
        writeln!(
            err,
            "  [3/4] {} :: hooks.Stop -> NotPresent (no famp entry found)",
            hooks_path.display()
        )
        .ok();
        return Ok(Vec::new());
    }

    let outcome = if filtered.is_empty() {
        json_merge::remove_user_json(hooks_path, "hooks", "Stop")?
    } else {
        json_merge::upsert_user_json(hooks_path, "hooks", "Stop", Value::Array(filtered))?
    };
    writeln!(
        err,
        "  [3/4] {} :: hooks.Stop -> {:?}",
        hooks_path.display(),
        outcome
    )
    .ok();
    Ok(removed_trust_keys)
}

fn collect_removed_trust_keys(
    hooks_path: &Path,
    group_index: usize,
    entry: &Value,
    famp_paths: &[String],
    out: &mut Vec<String>,
) {
    if entry
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| stop_entry::is_famp_command(command, famp_paths))
    {
        out.push(install_codex::codex_hook_key(
            hooks_path,
            "stop",
            group_index,
            0,
        ));
        return;
    }

    let Some(hooks) = entry.get("hooks").and_then(Value::as_array) else {
        return;
    };
    for (handler_index, hook) in hooks.iter().enumerate() {
        if hook
            .get("command")
            .and_then(Value::as_str)
            .is_some_and(|command| stop_entry::is_famp_command(command, famp_paths))
        {
            out.push(install_codex::codex_hook_key(
                hooks_path,
                "stop",
                group_index,
                handler_index,
            ));
        }
    }
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

//! `famp uninstall-claude-code` subcommand handler - D-04 inverse of
//! `cli::install::claude_code`.
//!
//! Reverses every mutation:
//!  1. `json_merge::remove_user_json("mcpServers", "famp")` against `~/.claude.json`
//!  2. `slash_commands::remove_all` against `~/.claude/commands/`
//!  3. `hook_runner::remove_shim` against `~/.famp/hook-runner.sh`
//!  4. `await_hook::remove_shim` against `~/.claude/hooks/famp-await.sh`
//!  5. Surgical drop of both famp-tagged Stop hooks in `~/.claude/settings.json`
//!     while preserving every other hook and every other settings key.
//!
//! Idempotent: re-running on already-uninstalled state is a no-op.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;
use serde_json::Value;

use crate::cli::error::CliError;
use crate::cli::install::{await_hook, hook_runner, json_merge, slash_commands};

#[derive(Debug, Args)]
pub struct UninstallClaudeCodeArgs {
    /// Override the install target home (defaults to `dirs::home_dir()`).
    /// Hidden flag - used by integration tests to redirect to a tempdir.
    #[arg(long, env = "FAMP_INSTALL_TARGET_HOME", hide = true)]
    pub home: Option<PathBuf>,
}

#[allow(clippy::needless_pass_by_value)]
pub fn run(args: UninstallClaudeCodeArgs) -> Result<(), CliError> {
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
    let claude_json_path = home.join(".claude.json");
    let commands_dir = home.join(".claude").join("commands");
    let settings_path = home.join(".claude").join("settings.json");
    let shim_path = home.join(".famp").join("hook-runner.sh");
    let await_shim_path = home
        .join(".claude")
        .join("hooks")
        .join("famp-await.sh");

    writeln!(
        err,
        "Uninstalling Claude Code integration from {}",
        home.display()
    )
    .ok();

    let outcome = json_merge::remove_user_json(&claude_json_path, "mcpServers", "famp")?;
    writeln!(
        err,
        "  [1/5] {} :: mcpServers.famp -> {:?}",
        claude_json_path.display(),
        outcome
    )
    .ok();

    slash_commands::remove_all(&commands_dir)?;
    writeln!(
        err,
        "  [2/5] {} :: 7 slash-command markdown files removed",
        commands_dir.display()
    )
    .ok();

    hook_runner::remove_shim(&shim_path)?;
    writeln!(err, "  [3/5] {} :: hook-runner shim removed", shim_path.display()).ok();

    await_hook::remove_shim(&await_shim_path)?;
    writeln!(
        err,
        "  [4/5] {} :: await shim removed",
        await_shim_path.display()
    )
    .ok();

    surgical_remove_stop_entry(&settings_path, &shim_path, &await_shim_path, err)?;

    writeln!(err).ok();
    writeln!(err, "uninstall-claude-code complete.").ok();
    writeln!(
        err,
        "  note: the famp binary remains in ~/.cargo/bin; run `cargo uninstall famp` to remove it."
    )
    .ok();
    writeln!(
        err,
        "  note: any `.bak.<unix-ts>` files were intentionally preserved for recovery."
    )
    .ok();
    Ok(())
}

fn surgical_remove_stop_entry(
    settings_path: &Path,
    shim_path: &Path,
    await_shim_path: &Path,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let existing: Value = match std::fs::read_to_string(settings_path) {
        Ok(s) if s.trim().is_empty() => {
            writeln!(
                err,
                "  [5/5] {} :: empty file, no Stop entry to remove",
                settings_path.display()
            )
            .ok();
            return Ok(());
        }
        Ok(s) => serde_json::from_str(&s).map_err(|source| CliError::JsonMergeParse {
            path: settings_path.to_path_buf(),
            source,
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            writeln!(
                err,
                "  [5/5] {} :: file absent, no Stop entry to remove",
                settings_path.display()
            )
            .ok();
            return Ok(());
        }
        Err(source) => {
            return Err(CliError::JsonMergeRead {
                path: settings_path.to_path_buf(),
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
            "  [5/5] {} :: no Stop array, nothing to remove",
            settings_path.display()
        )
        .ok();
        return Ok(());
    }

    let famp_paths = [
        shim_path.display().to_string(),
        await_shim_path.display().to_string(),
    ];
    let filtered: Vec<Value> = prior_stop
        .iter()
        .filter_map(|elem| remove_famp_hook_from_stop_entry(elem, &famp_paths))
        .collect();

    if filtered.len() == prior_stop.len() {
        writeln!(
            err,
            "  [5/5] {} :: hooks.Stop -> NotPresent (no famp entry found)",
            settings_path.display()
        )
        .ok();
        return Ok(());
    }

    let outcome = if filtered.is_empty() {
        json_merge::remove_user_json(settings_path, "hooks", "Stop")?
    } else {
        json_merge::upsert_user_json(settings_path, "hooks", "Stop", Value::Array(filtered))?
    };
    writeln!(
        err,
        "  [5/5] {} :: hooks.Stop -> {:?}",
        settings_path.display(),
        outcome
    )
    .ok();
    Ok(())
}

fn remove_famp_hook_from_stop_entry(entry: &Value, shims: &[String]) -> Option<Value> {
    if entry
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| shims.iter().any(|s| command.starts_with(s.as_str())))
    {
        return None;
    }

    let Some(hooks) = entry.get("hooks").and_then(Value::as_array) else {
        return Some(entry.clone());
    };
    let filtered_hooks: Vec<Value> = hooks
        .iter()
        .filter(|hook| {
            !hook
                .get("command")
                .and_then(Value::as_str)
                .is_some_and(|command| shims.iter().any(|s| command.starts_with(s.as_str())))
        })
        .cloned()
        .collect();

    if filtered_hooks.len() == hooks.len() {
        return Some(entry.clone());
    }
    if filtered_hooks.is_empty() {
        return None;
    }

    let mut updated = entry.clone();
    let obj = updated.as_object_mut()?;
    obj.insert("hooks".to_string(), Value::Array(filtered_hooks));
    Some(updated)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::cli::install;

    #[test]
    fn uninstall_after_install_returns_to_clean_state() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        install::claude_code::run_at(home, &mut out, &mut err).unwrap();

        let mut out2 = Vec::<u8>::new();
        let mut err2 = Vec::<u8>::new();
        run_at(home, &mut out2, &mut err2).unwrap();

        let commands_dir = home.join(".claude/commands");
        if commands_dir.exists() {
            let entries: Vec<_> = std::fs::read_dir(&commands_dir).unwrap().collect();
            assert_eq!(entries.len(), 0);
        }
        assert!(!home.join(".famp/hook-runner.sh").exists());
        assert!(!home.join(".claude/hooks/famp-await.sh").exists());

        let claude: Value =
            serde_json::from_str(&std::fs::read_to_string(home.join(".claude.json")).unwrap())
                .unwrap();
        assert!(claude["mcpServers"]
            .as_object()
            .is_none_or(|servers| !servers.contains_key("famp")));
    }

    #[test]
    fn uninstall_preserves_unrelated_keys_in_claude_json() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        std::fs::write(
            home.join(".claude.json"),
            r#"{"numStartups":7,"mcpServers":{"github":{"command":"/x"}}}"#,
        )
        .unwrap();

        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        install::claude_code::run_at(home, &mut out, &mut err).unwrap();

        let mut out2 = Vec::<u8>::new();
        let mut err2 = Vec::<u8>::new();
        run_at(home, &mut out2, &mut err2).unwrap();

        let post: Value =
            serde_json::from_str(&std::fs::read_to_string(home.join(".claude.json")).unwrap())
                .unwrap();
        assert_eq!(post["numStartups"], serde_json::json!(7));
        assert_eq!(post["mcpServers"]["github"]["command"], "/x");
        assert!(post["mcpServers"]
            .as_object()
            .unwrap()
            .get("famp")
            .is_none());
    }

    #[test]
    fn uninstall_preserves_other_stop_hooks() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();

        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        install::claude_code::run_at(home, &mut out, &mut err).unwrap();

        let settings_path = home.join(".claude/settings.json");
        let mut settings: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let stop = settings["hooks"]["Stop"].as_array_mut().unwrap();
        stop.push(serde_json::json!({
            "matcher": "Edit|Write",
            "hooks": [{
                "type": "command",
                "command": "/some/other/hook.sh",
                "timeout": 5
            }]
        }));
        std::fs::write(
            &settings_path,
            serde_json::to_string_pretty(&settings).unwrap(),
        )
        .unwrap();

        let mut out2 = Vec::<u8>::new();
        let mut err2 = Vec::<u8>::new();
        run_at(home, &mut out2, &mut err2).unwrap();

        let post: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let stop = post["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 1);
        assert_eq!(stop[0]["matcher"], "Edit|Write");
        assert_eq!(stop[0]["hooks"][0]["command"], "/some/other/hook.sh");
    }

    #[test]
    fn uninstall_on_clean_state_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();

        run_at(home, &mut out, &mut err).unwrap();
    }

    #[test]
    fn uninstall_drops_stop_key_entirely_if_only_famp_was_there() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        install::claude_code::run_at(home, &mut out, &mut err).unwrap();

        let mut out2 = Vec::<u8>::new();
        let mut err2 = Vec::<u8>::new();
        run_at(home, &mut out2, &mut err2).unwrap();

        let settings_path = home.join(".claude/settings.json");
        let post: Value =
            serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        let has_stop = post
            .get("hooks")
            .and_then(|hooks| hooks.get("Stop"))
            .is_some();
        assert!(
            !has_stop,
            "Stop key should be removed when filtered array is empty"
        );
    }
}

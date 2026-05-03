//! `famp install-claude-code` subcommand handler (CC-01 + HOOK-04b install).
//!
//! Mutates four user-scope artifacts (D-04 atomic - install/uninstall symmetric):
//!  1. `~/.claude.json :: mcpServers.famp` - JSON merge (json_merge::upsert_user_json)
//!  2. `~/.claude/commands/famp-*.md` - 7 markdown files (slash_commands::write_all)
//!  3. `~/.famp/hook-runner.sh` - bash shim at mode 0755 (hook_runner::install_shim)
//!  4. `~/.claude/settings.json :: hooks.Stop` - array merge with sentinel
//!     `command` prefix `<home>/.famp/hook-runner.sh` (D-09 amended target)
//!
//! Idempotent: re-running is a no-op when state already matches (D-02).
//! Atomic: each JSON write goes through tempfile::NamedTempFile::persist
//! (rename(2)) and creates a `.bak.<unix-ts>` of pre-state. No partial states.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;
use serde_json::{json, Value};

use crate::cli::error::CliError;
use crate::cli::install::{hook_runner, json_merge, slash_commands};

#[derive(Debug, Args)]
pub struct InstallClaudeCodeArgs {
    /// Override the install target home (defaults to `dirs::home_dir()`).
    /// Hidden flag - used by integration tests to redirect to a tempdir.
    #[arg(long, env = "FAMP_INSTALL_TARGET_HOME", hide = true)]
    pub home: Option<PathBuf>,
}

#[allow(clippy::needless_pass_by_value)]
pub fn run(args: InstallClaudeCodeArgs) -> Result<(), CliError> {
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

/// Test-facing entry: takes explicit home + writable handles.
/// Mirrors the `init::run_at` / `setup::run_with_io` pattern.
pub fn run_at(home: &Path, _out: &mut dyn Write, err: &mut dyn Write) -> Result<(), CliError> {
    let claude_json_path = home.join(".claude.json");
    let commands_dir = home.join(".claude").join("commands");
    let settings_path = home.join(".claude").join("settings.json");
    let shim_path = home.join(".famp").join("hook-runner.sh");

    let famp_bin = which::which("famp")
        .ok()
        .unwrap_or_else(|| home.join(".cargo").join("bin").join("famp"));

    writeln!(
        err,
        "Installing Claude Code integration into {}",
        home.display()
    )
    .ok();
    writeln!(err, "  resolved famp binary: {}", famp_bin.display()).ok();

    let mcp_value: Value = json!({
        "type": "stdio",
        "command": famp_bin.display().to_string(),
        "args": ["mcp"],
    });
    let outcome = json_merge::upsert_user_json(&claude_json_path, "mcpServers", "famp", mcp_value)?;
    writeln!(
        err,
        "  [1/4] {} :: mcpServers.famp -> {:?}",
        claude_json_path.display(),
        outcome
    )
    .ok();

    slash_commands::write_all(&commands_dir)?;
    writeln!(
        err,
        "  [2/4] {} :: 7 slash-command markdown files written",
        commands_dir.display()
    )
    .ok();

    hook_runner::install_shim(&shim_path)?;
    writeln!(
        err,
        "  [3/4] {} :: bash shim installed (mode 0755)",
        shim_path.display()
    )
    .ok();

    let new_stop_array = build_stop_array(&settings_path, &shim_path)?;
    let outcome = json_merge::upsert_user_json(
        &settings_path,
        "hooks",
        "Stop",
        Value::Array(new_stop_array),
    )?;
    writeln!(
        err,
        "  [4/4] {} :: hooks.Stop -> {:?}",
        settings_path.display(),
        outcome
    )
    .ok();

    writeln!(err).ok();
    writeln!(
        err,
        "install-claude-code complete. Restart Claude Code windows to pick up changes."
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

fn build_stop_array(settings_path: &Path, shim_path: &Path) -> Result<Vec<Value>, CliError> {
    let existing: Value = match std::fs::read_to_string(settings_path) {
        Ok(s) if s.trim().is_empty() => Value::Object(serde_json::Map::new()),
        Ok(s) => serde_json::from_str(&s).map_err(|source| CliError::JsonMergeParse {
            path: settings_path.to_path_buf(),
            source,
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Object(serde_json::Map::new()),
        Err(source) => {
            return Err(CliError::JsonMergeRead {
                path: settings_path.to_path_buf(),
                source,
            });
        }
    };

    let prior_stop: &[Value] = existing
        .get("hooks")
        .and_then(|h| h.get("Stop"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    let shim_str = shim_path.display().to_string();
    let mut new_stop: Vec<Value> = prior_stop
        .iter()
        .filter(|elem| {
            !elem
                .get("command")
                .and_then(Value::as_str)
                .is_some_and(|command| command.starts_with(&shim_str))
        })
        .cloned()
        .collect();

    new_stop.push(json!({
        "type": "command",
        "command": shim_str,
        "timeout": 30,
    }));

    Ok(new_stop)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn install_writes_all_four_artifacts_under_tempdir_home() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        run_at(home, &mut out, &mut err).unwrap();

        assert!(home.join(".claude.json").exists());
        assert!(home.join(".claude/commands/famp-register.md").exists());
        assert!(home.join(".claude/commands/famp-send.md").exists());
        assert!(home.join(".claude/settings.json").exists());
        assert!(home.join(".famp/hook-runner.sh").exists());

        let claude: Value =
            serde_json::from_str(&std::fs::read_to_string(home.join(".claude.json")).unwrap())
                .unwrap();
        assert_eq!(claude["mcpServers"]["famp"]["args"], json!(["mcp"]));

        let settings: Value = serde_json::from_str(
            &std::fs::read_to_string(home.join(".claude/settings.json")).unwrap(),
        )
        .unwrap();
        let stop = settings["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 1);
        let cmd = stop[0]["command"].as_str().unwrap();
        assert!(cmd.ends_with("/.famp/hook-runner.sh"), "command = {cmd}");
        assert_eq!(stop[0]["type"], "command");
        assert!(
            stop[0].get("matcher").is_none(),
            "Stop must not have matcher"
        );
    }

    #[test]
    fn install_preserves_unrelated_keys_in_claude_json() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        std::fs::write(
            home.join(".claude.json"),
            r#"{"numStartups":42,"tipsHistory":["x"],"mcpServers":{"other":{"command":"/x"}}}"#,
        )
        .unwrap();

        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        run_at(home, &mut out, &mut err).unwrap();

        let post: Value =
            serde_json::from_str(&std::fs::read_to_string(home.join(".claude.json")).unwrap())
                .unwrap();
        assert_eq!(post["numStartups"], json!(42));
        assert_eq!(post["tipsHistory"], json!(["x"]));
        assert_eq!(post["mcpServers"]["other"]["command"], "/x");
        assert!(post["mcpServers"].get("famp").is_some());
    }

    #[test]
    fn install_is_idempotent_second_run_is_no_op_writes() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();

        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        run_at(home, &mut out, &mut err).unwrap();

        let claude_after_first = std::fs::read_to_string(home.join(".claude.json")).unwrap();
        let settings_after_first =
            std::fs::read_to_string(home.join(".claude/settings.json")).unwrap();

        std::thread::sleep(std::time::Duration::from_secs(1));

        let mut out2 = Vec::<u8>::new();
        let mut err2 = Vec::<u8>::new();
        run_at(home, &mut out2, &mut err2).unwrap();

        assert_eq!(
            claude_after_first,
            std::fs::read_to_string(home.join(".claude.json")).unwrap()
        );
        assert_eq!(
            settings_after_first,
            std::fs::read_to_string(home.join(".claude/settings.json")).unwrap()
        );
    }

    #[test]
    fn install_replaces_stale_stop_entry_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        let shim = home
            .join(".famp")
            .join("hook-runner.sh")
            .display()
            .to_string();
        std::fs::write(
            home.join(".claude/settings.json"),
            format!(
                r#"{{"hooks":{{"Stop":[{{"type":"command","command":"{shim}","timeout":99}},{{"type":"command","command":"/other/hook.sh","timeout":10}}]}}}}"#,
            ),
        )
        .unwrap();

        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        run_at(home, &mut out, &mut err).unwrap();

        let post: Value = serde_json::from_str(
            &std::fs::read_to_string(home.join(".claude/settings.json")).unwrap(),
        )
        .unwrap();
        let stop = post["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2, "should preserve other hook + replace ours");
        let ours = stop.iter().find(|e| e["command"] == shim).unwrap();
        assert_eq!(ours["timeout"], json!(30));
        let other = stop
            .iter()
            .find(|e| e["command"] == "/other/hook.sh")
            .unwrap();
        assert_eq!(other["timeout"], json!(10));
    }
}

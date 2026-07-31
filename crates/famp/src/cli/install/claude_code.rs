//! `famp install-claude-code` subcommand handler (CC-01 + HOOK-04b install).
//!
//! Mutates five user-scope artifacts (D-04 atomic - install/uninstall symmetric):
//!  1. `~/.claude.json :: mcpServers.famp` - JSON merge (`json_merge::upsert_user_json`)
//!  2. `~/.claude/commands/famp-*.md` - 7 markdown files (`slash_commands::write_all`)
//!  3. `~/.famp/hook-runner.sh` - bash shim at mode 0755 (`hook_runner::install_shim`)
//!  4. `~/.claude/settings.json :: hooks.Stop` - Claude Code hook entries
//!     for hook-runner.sh (timeout 30) and famp-await.sh (timeout 86400)
//!  5. `~/.claude/hooks/famp-await.sh` - listen-mode await shim at mode 0755
//!
//! Idempotent: re-running is a no-op when state already matches (D-02).
//! Atomic: each JSON write goes through `tempfile::NamedTempFile::persist`
//! (rename(2)) and creates a `.bak.<unix-ts>` of pre-state. No partial states.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;
use serde_json::{json, Value};

use crate::cli::error::CliError;
use crate::cli::executable::{resolve_for_generated_config, FampExecutable};
use crate::cli::install::{await_hook, hook_runner, json_merge, slash_commands, stop_entry};

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
    let executable = resolve_for_generated_config()?;
    run_at_with_executable(home, &executable, err)
}

fn run_at_with_executable(
    home: &Path,
    executable: &FampExecutable,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let claude_json_path = home.join(".claude.json");
    let commands_dir = home.join(".claude").join("commands");
    let settings_path = home.join(".claude").join("settings.json");
    let shim_path = home.join(".famp").join("hook-runner.sh");
    let await_shim_path = home.join(".claude").join("hooks").join("famp-await.sh");

    writeln!(
        err,
        "Installing Claude Code integration into {}",
        home.display()
    )
    .ok();
    writeln!(
        err,
        "  resolved famp binary: {}",
        executable.path().display()
    )
    .ok();

    let mcp_value: Value = json!({
        "type": "stdio",
        "command": executable.utf8(),
        "args": ["mcp"],
    });
    let outcome = json_merge::upsert_user_json(&claude_json_path, "mcpServers", "famp", mcp_value)?;
    writeln!(
        err,
        "  [1/5] {} :: mcpServers.famp -> {:?}",
        claude_json_path.display(),
        outcome
    )
    .ok();

    slash_commands::write_all(&commands_dir)?;
    writeln!(
        err,
        "  [2/5] {} :: 7 slash-command markdown files written",
        commands_dir.display()
    )
    .ok();

    hook_runner::install_shim(&shim_path, executable)?;
    writeln!(
        err,
        "  [3/5] {} :: bash shim installed (mode 0755)",
        shim_path.display()
    )
    .ok();

    let new_stop_array = build_stop_array(&settings_path, &shim_path, &await_shim_path)?;
    let outcome = json_merge::upsert_user_json(
        &settings_path,
        "hooks",
        "Stop",
        Value::Array(new_stop_array),
    )?;
    writeln!(
        err,
        "  [4/5] {} :: hooks.Stop -> {:?}",
        settings_path.display(),
        outcome
    )
    .ok();

    await_hook::install_shim(&await_shim_path, executable)?;
    writeln!(
        err,
        "  [5/5] {} :: listen-mode await shim installed (mode 0755)",
        await_shim_path.display()
    )
    .ok();

    writeln!(err).ok();
    writeln!(
        err,
        "install-claude-code complete. The 7 slash commands and Stop hooks are \
         live immediately in already-open windows. The MCP server registration \
         (mcpServers.famp) only takes effect after restarting Claude Code."
    )
    .ok();
    Ok(())
}

fn build_stop_array(
    settings_path: &Path,
    shim_path: &Path,
    await_shim_path: &Path,
) -> Result<Vec<Value>, CliError> {
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
        .map_or(&[], Vec::as_slice);

    // Remove ALL stale famp-owned entries (both shim paths).
    let famp_paths = [
        shim_path.display().to_string(),
        await_shim_path.display().to_string(),
    ];
    let mut new_stop: Vec<Value> = prior_stop
        .iter()
        .filter_map(|elem| stop_entry::remove_famp_hook_from_stop_entry(elem, &famp_paths))
        .collect();

    // Append both famp entries. Order matters: Claude Code executes Stop hooks
    // sequentially in array order. hook-runner.sh (fast, ~1s) must run first
    // so Edit-glob notifications fire before famp-await.sh blocks for up to 23h.
    new_stop.push(json!({
        "matcher": "",
        "hooks": [{
            "type": "command",
            "command": shim_path.display().to_string(),
            "timeout": 30,
        }],
    }));
    new_stop.push(json!({
        "matcher": "",
        "hooks": [{
            "type": "command",
            "command": await_shim_path.display().to_string(),
            "timeout": 86400,
        }],
    }));

    Ok(new_stop)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn stage_executable(path: &Path) -> PathBuf {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path.to_path_buf()
    }

    fn test_executable(dir: &Path) -> FampExecutable {
        let path = stage_executable(&dir.join("bin with space").join("famp's-tool"));
        FampExecutable::validate(path).unwrap()
    }

    /// Pin `FAMP_INSTALL_FAMP_BIN` at a staged executable so the public
    /// `run_at` path resolves hermetically. Without this the resolver would
    /// fall through to `which famp`, making the test pass or fail depending
    /// on whether the host happens to have FAMP installed (CI does not).
    /// WR-06: `temp_env` serializes the process-global env mutation.
    fn with_pinned_famp_bin<T>(test: impl FnOnce() -> T) -> T {
        let dir = tempfile::tempdir().unwrap();
        let bin = stage_executable(&dir.path().join("famp"));
        temp_env::with_var("FAMP_INSTALL_FAMP_BIN", Some(bin.as_os_str()), test)
    }

    #[test]
    fn injected_executable_is_pinned_in_mcp_and_both_rendered_hooks() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let executable = test_executable(dir.path());
        let mut err = Vec::new();
        run_at_with_executable(&home, &executable, &mut err).unwrap();
        let config: Value =
            serde_json::from_str(&std::fs::read_to_string(home.join(".claude.json")).unwrap())
                .unwrap();
        assert_eq!(config["mcpServers"]["famp"]["command"], executable.utf8());
        for script in [
            home.join(".famp/hook-runner.sh"),
            home.join(".claude/hooks/famp-await.sh"),
        ] {
            let rendered = std::fs::read_to_string(script).unwrap();
            assert!(rendered.contains(&format!(
                "FAMP_BIN={}",
                crate::cli::executable::posix_shell_literal(executable.utf8())
            )));
            assert!(!rendered.contains("@FAMP_BIN@"));
            assert!(!rendered.contains("command -v famp"));
            assert!(!rendered.contains(".cargo/bin/famp"));
        }
    }

    /// M2: the guarantee this whole abstraction exists for — a resolution
    /// failure happens BEFORE any generated configuration, hook asset or
    /// backup is touched. Exercises the real public `run_at` (which calls
    /// `resolve_for_generated_config`), not the executable-injecting
    /// `run_at_with_executable` seam the other tests use.
    ///
    /// WR-06: env mutation is process-global; `temp_env::with_var` holds the
    /// crate-wide lock so parallel tests cannot observe the override.
    #[test]
    fn public_run_at_fails_before_any_mutation_when_executable_is_unresolvable() {
        use crate::cli::executable::test_support::{
            assert_tree_unchanged, snapshot_tree, MISSING_FAMP_BIN,
        };

        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        // Representative pre-existing user state the installer would otherwise
        // rewrite: MCP config, Stop-hook settings, and an already-installed
        // shim carrying a *different* binary.
        std::fs::create_dir_all(home.join(".claude/hooks")).unwrap();
        std::fs::create_dir_all(home.join(".famp")).unwrap();
        std::fs::write(home.join(".claude.json"), "{\"numStartups\":7}\n").unwrap();
        std::fs::write(
            home.join(".claude/settings.json"),
            "{\"hooks\":{\"Stop\":[]}}\n",
        )
        .unwrap();
        std::fs::write(home.join(".famp/hook-runner.sh"), "#!/bin/sh\n# prior\n").unwrap();
        std::fs::write(
            home.join(".claude/hooks/famp-await.sh"),
            "#!/bin/sh\n# prior await\n",
        )
        .unwrap();
        let before = snapshot_tree(home);

        for value in [MISSING_FAMP_BIN, "   ", home.to_str().unwrap()] {
            let result = temp_env::with_var("FAMP_INSTALL_FAMP_BIN", Some(value), || {
                let mut out = Vec::<u8>::new();
                let mut err = Vec::<u8>::new();
                run_at(home, &mut out, &mut err)
            });
            assert!(
                matches!(result, Err(CliError::FampExecutable(_))),
                "FAMP_INSTALL_FAMP_BIN={value:?} must fail resolution, got {result:?}"
            );
            assert_tree_unchanged(home, &before, &format!("install-claude-code {value:?}"));
        }
    }

    #[test]
    fn install_writes_all_five_artifacts_under_tempdir_home() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        with_pinned_famp_bin(|| run_at(home, &mut out, &mut err)).unwrap();

        assert!(home.join(".claude.json").exists());
        assert!(home.join(".claude/commands/famp-register.md").exists());
        assert!(home.join(".claude/commands/famp-send.md").exists());
        assert!(home.join(".claude/settings.json").exists());
        assert!(home.join(".famp/hook-runner.sh").exists());
        assert!(home.join(".claude/hooks/famp-await.sh").exists());

        let claude: Value =
            serde_json::from_str(&std::fs::read_to_string(home.join(".claude.json")).unwrap())
                .unwrap();
        assert_eq!(claude["mcpServers"]["famp"]["args"], json!(["mcp"]));

        let settings: Value = serde_json::from_str(
            &std::fs::read_to_string(home.join(".claude/settings.json")).unwrap(),
        )
        .unwrap();
        let stop = settings["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(
            stop.len(),
            2,
            "expected exactly 2 Stop entries, got {}",
            stop.len()
        );

        // Entry 0: hook-runner.sh, timeout 30
        let hooks0 = stop[0]["hooks"].as_array().unwrap();
        assert_eq!(hooks0.len(), 1);
        let cmd0 = hooks0[0]["command"].as_str().unwrap();
        assert!(cmd0.ends_with("/.famp/hook-runner.sh"), "cmd0 = {cmd0}");
        assert_eq!(hooks0[0]["timeout"], 30);

        // Entry 1: famp-await.sh, timeout 86400
        let hooks1 = stop[1]["hooks"].as_array().unwrap();
        assert_eq!(hooks1.len(), 1);
        let cmd1 = hooks1[0]["command"].as_str().unwrap();
        assert!(
            cmd1.ends_with("/.claude/hooks/famp-await.sh"),
            "cmd1 = {cmd1}"
        );
        assert_eq!(hooks1[0]["timeout"], 86400);
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
        with_pinned_famp_bin(|| run_at(home, &mut out, &mut err)).unwrap();

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

        // One pinned binary for BOTH runs: idempotency compares the generated
        // files byte-for-byte, so the resolved path must not move between runs.
        let (claude_after_first, settings_after_first) = with_pinned_famp_bin(|| {
            let mut out = Vec::<u8>::new();
            let mut err = Vec::<u8>::new();
            run_at(home, &mut out, &mut err).unwrap();

            let first = (
                std::fs::read_to_string(home.join(".claude.json")).unwrap(),
                std::fs::read_to_string(home.join(".claude/settings.json")).unwrap(),
            );

            std::thread::sleep(std::time::Duration::from_secs(1));

            let mut out2 = Vec::<u8>::new();
            let mut err2 = Vec::<u8>::new();
            run_at(home, &mut out2, &mut err2).unwrap();
            first
        });

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
    fn double_install_produces_exactly_two_stop_entries() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();

        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        with_pinned_famp_bin(|| run_at(home, &mut out, &mut err)).unwrap();

        let mut out2 = Vec::<u8>::new();
        let mut err2 = Vec::<u8>::new();
        with_pinned_famp_bin(|| run_at(home, &mut out2, &mut err2)).unwrap();

        let settings: Value = serde_json::from_str(
            &std::fs::read_to_string(home.join(".claude/settings.json")).unwrap(),
        )
        .unwrap();
        let stop = settings["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(
            stop.len(),
            2,
            "double install must not accumulate entries; got {} Stop entries",
            stop.len()
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
                r#"{{"hooks":{{"Stop":[{{"matcher":"","hooks":[{{"type":"command","command":"{shim}","timeout":99}},{{"type":"command","command":"/other/hook.sh","timeout":10}}]}},{{"type":"command","command":"{shim} legacy","timeout":99}},{{"matcher":"Edit|Write","hooks":[{{"type":"command","command":"/another/hook.sh","timeout":10}}]}}]}}}}"#,
            ),
        )
        .unwrap();

        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        with_pinned_famp_bin(|| run_at(home, &mut out, &mut err)).unwrap();

        let post: Value = serde_json::from_str(
            &std::fs::read_to_string(home.join(".claude/settings.json")).unwrap(),
        )
        .unwrap();
        let stop = post["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(
            stop.len(),
            4,
            "should preserve other hooks and replace ours with 2 fresh famp entries; got {}",
            stop.len()
        );
        // Fresh hook-runner entry must exist with timeout 30
        let ours_group = stop
            .iter()
            .find(|e| {
                e["matcher"] == ""
                    && e["hooks"]
                        .as_array()
                        .is_some_and(|hooks| hooks.iter().any(|hook| hook["command"] == shim))
            })
            .unwrap();
        assert_eq!(ours_group["hooks"][0]["timeout"], json!(30));
        // Fresh await entry must exist with timeout 86400
        let await_shim = home
            .join(".claude")
            .join("hooks")
            .join("famp-await.sh")
            .display()
            .to_string();
        let await_group = stop
            .iter()
            .find(|e| {
                e["hooks"]
                    .as_array()
                    .is_some_and(|hooks| hooks.iter().any(|hook| hook["command"] == await_shim))
            })
            .unwrap();
        assert_eq!(await_group["hooks"][0]["timeout"], json!(86400));
        let other_group = stop
            .iter()
            .find(|e| {
                e["hooks"].as_array().is_some_and(|hooks| {
                    hooks.iter().any(|hook| hook["command"] == "/other/hook.sh")
                })
            })
            .unwrap();
        assert_eq!(other_group["hooks"][0]["timeout"], json!(10));
        assert!(stop.iter().all(|e| {
            e.get("command")
                .and_then(Value::as_str)
                .is_none_or(|command| !command.starts_with(&shim))
        }));
    }

    #[test]
    fn install_dedupes_wrapped_bash_stop_entry() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        let await_shim = home
            .join(".claude")
            .join("hooks")
            .join("famp-await.sh")
            .display()
            .to_string();
        // Pre-seed with the exact bug Ben hit: a wrapped `bash <shim>` Stop entry.
        let seeded_json = format!(
            concat!(
                r#"{{"hooks":{{"Stop":[{{"matcher":"","hooks":"#,
                r#"[{{"type":"command","command":"bash {await_shim}","timeout":86400}}]"#,
                r#"}}]}}}}"#,
            ),
            await_shim = await_shim,
        );
        std::fs::write(home.join(".claude/settings.json"), seeded_json).unwrap();

        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        with_pinned_famp_bin(|| run_at(home, &mut out, &mut err)).unwrap();

        let post: Value = serde_json::from_str(
            &std::fs::read_to_string(home.join(".claude/settings.json")).unwrap(),
        )
        .unwrap();
        let stop = post["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(
            stop.len(),
            2,
            "wrapped `bash <path>` entry must be reaped, not appended-alongside; got {} entries: {:#?}",
            stop.len(),
            stop
        );
        // No surviving entry should contain the wrapped form.
        for entry in stop {
            let cmd_str = serde_json::to_string(entry).unwrap();
            assert!(
                !cmd_str.contains(&format!("bash {await_shim}")),
                "wrapped form survived in entry: {entry:#?}"
            );
        }
    }
}

//! `famp install-codex` subcommand handler.
//!
//! Installs two Codex integration surfaces:
//!  1. `~/.codex/config.toml :: [mcp_servers.famp]`
//!  2. `<project>/.codex/hooks.json :: hooks.Stop` → native
//!     `famp hook codex-stop` (no shell shim on the critical path)
//!
//! Codex hook trust is pre-seeded in `~/.codex/config.toml` for the installed
//! hook entry so explicit `famp install-codex` produces a runnable integration
//! without a separate `/hooks` approval step.
//!
//! Legacy shell shim entries (`famp-await.sh`) are pruned on install so a
//! reinstall migrates cleanly to the native helper.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;
use serde_json::{json, Map, Value as JsonValue};
use toml::Value as TomlValue;

use crate::cli::error::CliError;
use crate::cli::executable::{posix_shell_literal, resolve_for_generated_config, FampExecutable};
use crate::cli::install::{await_hook, json_merge, stop_entry, toml_merge};

pub(crate) const CODEX_AWAIT_TIMEOUT_SEC: i64 = 86_400;
pub(crate) const CODEX_STOP_EVENT_LABEL: &str = "stop";

#[derive(Debug, Args)]
pub struct InstallCodexArgs {
    /// Override the install target home (defaults to `dirs::home_dir()`).
    /// Hidden flag - used by integration tests to redirect to a tempdir.
    #[arg(long, env = "FAMP_INSTALL_TARGET_HOME", hide = true)]
    pub home: Option<PathBuf>,

    /// Project root whose `.codex/hooks.json` should receive the Stop hook.
    /// Defaults to the current git root, or the current directory outside git.
    #[arg(long, env = "FAMP_INSTALL_CODEX_PROJECT_DIR")]
    pub project: Option<PathBuf>,
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
    let project = match args.project {
        Some(p) => p,
        None => default_project_root()?,
    };
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    run_at_project(&home, &project, &mut stdout, &mut stderr)
}

pub fn run_at(home: &Path, out: &mut dyn Write, err: &mut dyn Write) -> Result<(), CliError> {
    run_at_project(home, home, out, err)
}

pub(crate) fn default_project_root() -> Result<PathBuf, CliError> {
    let cwd = std::env::current_dir().map_err(|source| CliError::Io {
        path: PathBuf::from("."),
        source,
    })?;
    Ok(find_git_root(&cwd).unwrap_or(cwd))
}

pub(crate) fn find_git_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|candidate| candidate.join(".git").exists())
        .map(Path::to_path_buf)
}

pub fn run_at_project(
    home: &Path,
    project: &Path,
    _out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let executable = resolve_for_generated_config()?;
    run_at_project_with_executable(home, project, &executable, err)
}

fn run_at_project_with_executable(
    home: &Path,
    project: &Path,
    executable: &FampExecutable,
    err: &mut dyn Write,
) -> Result<(), CliError> {
    let home = normalize_absolute_path(home)?;
    let project = normalize_absolute_path(project)?;
    let config_path = home.join(".codex").join("config.toml");
    let hooks_path = project.join(".codex").join("hooks.json");
    let await_shim_path = project.join(".codex").join("hooks").join("famp-await.sh");
    let famp_bin = executable.path();

    // Probe BEFORE any mutation: a failing probe must leave every file
    // (MCP entry, legacy shim, hooks.json, hook-trust state) untouched.
    // Writing partial state (e.g. MCP entry written + shim pruned but no
    // Stop hook wired) would be strictly worse than not running at all.
    probe_codex_stop_support(famp_bin)?;

    writeln!(err, "Installing Codex MCP entry into {}", home.display()).ok();
    writeln!(err, "Installing Codex Stop hook into {}", project.display()).ok();
    writeln!(err, "  resolved famp binary: {}", famp_bin.display()).ok();

    let mut famp_table = toml::Table::new();
    famp_table.insert(
        "command".into(),
        TomlValue::String(famp_bin.display().to_string()),
    );
    famp_table.insert(
        "args".into(),
        TomlValue::Array(vec![TomlValue::String("mcp".into())]),
    );
    famp_table.insert("startup_timeout_sec".into(), TomlValue::Integer(10));

    let outcome = toml_merge::upsert_codex_table(&config_path, "mcp_servers", "famp", famp_table)?;
    writeln!(
        err,
        "  [1/4] {} :: [mcp_servers.famp] -> {:?}",
        config_path.display(),
        outcome
    )
    .ok();

    // Migration: remove legacy shell shim if present. Native helper is the
    // critical path; Claude/Grok still use the asset shim via their installers.
    await_hook::remove_shim(&await_shim_path)?;
    writeln!(
        err,
        "  [2/4] {} :: legacy await shim pruned (native hook is critical path)",
        await_shim_path.display()
    )
    .ok();

    let command = codex_stop_command(famp_bin);
    let new_stop_array = build_stop_array(&hooks_path, famp_bin, &await_shim_path, &command)?;
    let stop_index = new_stop_array.len().saturating_sub(1);
    let outcome = json_merge::upsert_user_json(
        &hooks_path,
        "hooks",
        "Stop",
        JsonValue::Array(new_stop_array),
    )?;
    writeln!(
        err,
        "  [3/4] {} :: hooks.Stop -> {:?} (command: {command})",
        hooks_path.display(),
        outcome
    )
    .ok();

    let trust_key = codex_hook_key(&hooks_path, CODEX_STOP_EVENT_LABEL, stop_index, 0);
    let trusted_hash =
        codex_command_hook_hash(CODEX_STOP_EVENT_LABEL, &command, CODEX_AWAIT_TIMEOUT_SEC);
    let trusted_hashes = famp_trusted_hashes(CODEX_STOP_EVENT_LABEL, famp_bin, &await_shim_path);
    let removed_stale = remove_stale_codex_hook_trust(
        &config_path,
        &hooks_path,
        Some(&trust_key),
        &trusted_hashes,
    )?;
    if !removed_stale.is_empty() {
        writeln!(
            err,
            "      pruned {} stale FAMP Codex hook trust entr{}",
            removed_stale.len(),
            if removed_stale.len() == 1 { "y" } else { "ies" }
        )
        .ok();
    }
    let outcome = seed_codex_hook_trust(&config_path, &trust_key, &trusted_hash)?;
    writeln!(
        err,
        "  [4/4] {} :: hooks.state.\"{}\" -> {:?}",
        config_path.display(),
        trust_key,
        outcome
    )
    .ok();

    writeln!(err).ok();
    writeln!(
        err,
        "install-codex complete. Restart Codex. Existing sessions must not be \
         considered auto-wake-ready; a fresh session will load the project Stop hook."
    )
    .ok();
    Ok(())
}

pub(crate) fn normalize_absolute_path(path: &Path) -> Result<PathBuf, CliError> {
    match path.canonicalize() {
        Ok(canonical) => Ok(canonical),
        Err(_) => std::path::absolute(path).map_err(|source| CliError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Verify the resolved `famp` binary actually supports `hook codex-stop`
/// before wiring it into the Stop hook, so `install-codex` fails loudly
/// instead of shipping a hook command Codex will treat as a blocking error.
///
/// Runs `<famp_bin> hook codex-stop --help` (NEVER the bare subcommand —
/// that parks for up to 23h) with a short timeout. Clap exits 0 printing
/// subcommand help when `hook codex-stop` is recognized, and exits 2 for an
/// unrecognized subcommand.
///
/// Never skipped: `famp_bin` reaches this function only as a validated
/// `FampExecutable`, so the resolver has already proved the path exists, is a
/// regular file, and is executable. Executing the resolved binary is the
/// intended contract of "install with this binary" — including a path pinned
/// through `FAMP_INSTALL_FAMP_BIN`, which is trusted exactly as much as the
/// installer invocation itself.
fn probe_codex_stop_support(famp_bin: &Path) -> Result<(), CliError> {
    let mut child = std::process::Command::new(famp_bin)
        .args(["hook", "codex-stop", "--help"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| codex_stop_unsupported_error(famp_bin))?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(codex_stop_unsupported_error(famp_bin))
                };
            }
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(codex_stop_unsupported_error(famp_bin));
                }
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(_) => return Err(codex_stop_unsupported_error(famp_bin)),
        }
    }
}

fn codex_stop_unsupported_error(famp_bin: &Path) -> CliError {
    CliError::Generic(format!(
        "resolved famp binary {} does not support `hook codex-stop` \
         (probe `hook codex-stop --help` failed or timed out). \
         Run `just install` first so the deployed famp binary has native \
         Codex Stop-hook support, then re-run `famp install-codex`.",
        famp_bin.display()
    ))
}

fn build_stop_array(
    hooks_path: &Path,
    famp_bin: &Path,
    await_shim_path: &Path,
    command: &str,
) -> Result<Vec<JsonValue>, CliError> {
    let existing: JsonValue = match std::fs::read_to_string(hooks_path) {
        Ok(s) if s.trim().is_empty() => JsonValue::Object(Map::new()),
        Ok(s) => serde_json::from_str(&s).map_err(|source| CliError::JsonMergeParse {
            path: hooks_path.to_path_buf(),
            source,
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => JsonValue::Object(Map::new()),
        Err(source) => {
            return Err(CliError::JsonMergeRead {
                path: hooks_path.to_path_buf(),
                source,
            });
        }
    };

    let prior_stop: &[JsonValue] = existing
        .get("hooks")
        .and_then(|h| h.get("Stop"))
        .and_then(JsonValue::as_array)
        .map_or(&[], Vec::as_slice);

    let famp_paths = famp_hook_command_patterns(famp_bin, await_shim_path);
    let mut new_stop: Vec<JsonValue> = prior_stop
        .iter()
        .filter_map(|elem| stop_entry::remove_famp_hook_from_stop_entry(elem, &famp_paths))
        .collect();

    new_stop.push(json!({
        "hooks": [{
            "type": "command",
            "command": command,
            "timeout": CODEX_AWAIT_TIMEOUT_SEC,
        }],
    }));

    Ok(new_stop)
}

/// Absolute command string installed into Codex hooks.json.
pub(crate) fn codex_stop_command(famp_bin: &Path) -> String {
    format!("{} hook codex-stop", shell_quote_path(famp_bin))
}

/// Patterns that identify FAMP-owned Codex Stop entries (native + legacy shim).
pub(crate) fn famp_hook_command_patterns(famp_bin: &Path, await_shim_path: &Path) -> Vec<String> {
    let mut seen = BTreeSet::new();
    // Native helper (quoted + unquoted binary path).
    seen.insert(codex_stop_command(famp_bin));
    seen.insert(format!("{} hook codex-stop", famp_bin.display()));
    // Legacy shell shim (quoted + unquoted).
    seen.insert(await_shim_path.display().to_string());
    seen.insert(shell_quote_path(await_shim_path));
    seen.into_iter().collect()
}

pub(crate) fn shell_quote_path(path: &Path) -> String {
    let raw = path.to_str().unwrap_or_default();
    if raw
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-'))
    {
        return raw.to_string();
    }
    posix_shell_literal(raw)
}

pub(crate) fn codex_hook_key(
    hooks_path: &Path,
    event_label: &str,
    group_index: usize,
    handler_index: usize,
) -> String {
    format!(
        "{}:{event_label}:{group_index}:{handler_index}",
        hooks_path.display()
    )
}

pub(crate) fn codex_command_hook_hash(
    event_label: &str,
    command: &str,
    timeout_sec: i64,
) -> String {
    let identity = json!({
        "event_name": event_label,
        "hooks": [{
            "async": false,
            "command": command,
            "timeout": timeout_sec,
            "type": "command",
        }],
    });
    let canonical = canonical_json(&identity);
    let serialized = serde_json::to_vec(&canonical).unwrap_or_default();
    famp_crypto::sha256_artifact_id(&serialized)
}

fn canonical_json(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(map) => {
            let mut sorted = Map::new();
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for key in keys {
                if let Some(child) = map.get(key) {
                    sorted.insert(key.clone(), canonical_json(child));
                }
            }
            JsonValue::Object(sorted)
        }
        JsonValue::Array(items) => JsonValue::Array(items.iter().map(canonical_json).collect()),
        other => other.clone(),
    }
}

fn seed_codex_hook_trust(
    config_path: &Path,
    trust_key: &str,
    trusted_hash: &str,
) -> Result<toml_merge::TomlMergeOutcome, CliError> {
    let mut trust_table = toml::Table::new();
    trust_table.insert(
        "trusted_hash".into(),
        TomlValue::String(trusted_hash.to_string()),
    );
    trust_table.insert("enabled".into(), TomlValue::Boolean(true));
    toml_merge::upsert_nested_table(config_path, &["hooks", "state"], trust_key, trust_table)
}

pub(crate) fn famp_trusted_hashes(
    event_label: &str,
    famp_bin: &Path,
    await_shim_path: &Path,
) -> Vec<String> {
    famp_hook_command_patterns(famp_bin, await_shim_path)
        .into_iter()
        .map(|command| codex_command_hook_hash(event_label, &command, CODEX_AWAIT_TIMEOUT_SEC))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn remove_stale_codex_hook_trust(
    config_path: &Path,
    hooks_path: &Path,
    keep_key: Option<&str>,
    trusted_hashes: &[String],
) -> Result<Vec<String>, CliError> {
    let prefix = format!("{}:{}:", hooks_path.display(), CODEX_STOP_EVENT_LABEL);
    // Match famp-owned entries by (this hooks.json's stop prefix) AND
    // trusted_hash, NOT by prefix alone: the same project hooks.json may hold
    // a user's own Stop-hook trust entries we must never remove. KNOWN
    // LIMITATION (accepted, harmless): the trust key is position-based
    // (`hooks_path:stop:<group>:<handler>`, see codex_hook_key). A reinstall
    // that BOTH moves famp's hook to a new position AND resolves a different
    // famp binary path leaves the prior entry behind (its key != keep_key and
    // its hash is absent from the current set). Codex matches trust by
    // key+hash against the live hook at that position, so a stale key pointing
    // at no live hook is inert — a config leak, not a functional break.
    // Broadening removal to the bare prefix would risk deleting the user's
    // unrelated Stop-hook trust, so we accept the leak.
    let hashes = trusted_hashes
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let (_outcome, removed) =
        toml_merge::remove_nested_tables_where(config_path, &["hooks", "state"], |key, table| {
            if keep_key.is_some_and(|keep| key == keep) {
                return false;
            }
            key.starts_with(&prefix)
                && table
                    .get("trusted_hash")
                    .and_then(TomlValue::as_str)
                    .is_some_and(|hash| hashes.contains(hash))
        })?;
    Ok(removed)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn with_probe_capable_famp<F: FnOnce(&FampExecutable)>(test: F) {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("famp test binary");
        std::fs::write(&bin, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        // Warm up the stub before handing it to the installer's probe. A test
        // thread that forks while this file is being written inherits the open
        // write descriptor, and the exec then fails with ETXTBSY ("Text file
        // busy") — which `probe_codex_stop_support` cannot distinguish from a
        // genuinely unsupported binary. Waiting until the stub actually runs
        // keeps that race out of the assertion.
        let mut warmed = false;
        for _ in 0..50 {
            let ran = std::process::Command::new(&bin)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|status| status.success());
            if ran {
                warmed = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(warmed, "staged probe stub never became executable: {bin:?}");

        let executable = FampExecutable::validate(bin).unwrap();
        test(&executable);
    }

    #[test]
    fn install_codex_writes_mcp_and_stop_hook() {
        with_probe_capable_famp(|executable| {
            let dir = tempfile::tempdir().unwrap();
            let home = dir.path();
            let mut err = Vec::<u8>::new();
            run_at_project_with_executable(home, home, executable, &mut err).unwrap();

            let cfg = home.join(".codex/config.toml");
            assert!(cfg.exists());
            let parsed: toml::Table =
                toml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
            let famp_t = parsed["mcp_servers"]["famp"].as_table().unwrap();
            assert_eq!(famp_t["command"].as_str(), Some(executable.utf8()));
            assert_eq!(
                famp_t["args"].as_array().unwrap()[0].as_str().unwrap(),
                "mcp"
            );
            assert_eq!(famp_t["startup_timeout_sec"].as_integer().unwrap(), 10);
            assert!(famp_t["command"].as_str().unwrap().contains("famp"));

            let project = normalize_absolute_path(home).unwrap();
            let hooks_path = project.join(".codex/hooks.json");
            let hooks: JsonValue =
                serde_json::from_str(&std::fs::read_to_string(&hooks_path).unwrap()).unwrap();
            let stop = hooks["hooks"]["Stop"].as_array().unwrap();
            assert_eq!(stop.len(), 1);
            let command = stop[0]["hooks"][0]["command"].as_str().unwrap();
            assert_eq!(command, codex_stop_command(executable.path()));
            assert!(
                command.contains("hook codex-stop"),
                "expected native codex-stop command, got {command}"
            );
            assert_eq!(stop[0]["hooks"][0]["timeout"], CODEX_AWAIT_TIMEOUT_SEC);
            assert!(
                !project.join(".codex/hooks/famp-await.sh").exists(),
                "legacy shell shim must not be installed for Codex"
            );

            let trust_key = codex_hook_key(&hooks_path, CODEX_STOP_EVENT_LABEL, 0, 0);
            let trusted_hash =
                codex_command_hook_hash(CODEX_STOP_EVENT_LABEL, command, CODEX_AWAIT_TIMEOUT_SEC);
            assert_eq!(
                parsed["hooks"]["state"][trust_key.as_str()]["trusted_hash"]
                    .as_str()
                    .unwrap(),
                trusted_hash
            );
            assert!(parsed["hooks"]["state"][trust_key.as_str()]["enabled"]
                .as_bool()
                .unwrap());
        });
    }

    /// M2 (Codex half): resolution failure on the public `run_at_project`
    /// path must precede every mutation — including the two Codex-specific
    /// ones the other installers do not have: pruning the legacy
    /// `famp-await.sh` shim and rewriting `hooks.state` trust entries.
    #[test]
    fn public_run_at_project_fails_before_any_mutation_when_executable_is_unresolvable() {
        use crate::cli::executable::test_support::{
            assert_tree_unchanged, snapshot_tree, MISSING_FAMP_BIN,
        };

        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let project = dir.path().join("project");
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        std::fs::create_dir_all(project.join(".codex/hooks")).unwrap();
        // Legacy shim install would prune, plus a FAMP-shaped trust entry
        // install would rewrite.
        let legacy_shim = project.join(".codex/hooks/famp-await.sh");
        std::fs::write(&legacy_shim, "#!/bin/sh\n# legacy shim\n").unwrap();
        let hooks_path = project.join(".codex/hooks.json");
        std::fs::write(
            &hooks_path,
            "{\"hooks\":{\"Stop\":[{\"hooks\":[{\"type\":\"command\",\"command\":\"echo stop\"}]}]}}\n",
        )
        .unwrap();
        let config_path = home.join(".codex/config.toml");
        let trust_key = codex_hook_key(&hooks_path, CODEX_STOP_EVENT_LABEL, 0, 0);
        std::fs::write(
            &config_path,
            format!(
                "[mcp_servers.famp]\ncommand = \"/prior/famp\"\n\n\
                 [hooks.state.\"{trust_key}\"]\ntrusted_hash = \"sha256:prior\"\nenabled = true\n"
            ),
        )
        .unwrap();
        let before = snapshot_tree(dir.path());

        for value in [MISSING_FAMP_BIN, "\t ", home.to_str().unwrap()] {
            let result = temp_env::with_var("FAMP_INSTALL_FAMP_BIN", Some(value), || {
                let mut out = Vec::<u8>::new();
                let mut err = Vec::<u8>::new();
                run_at_project(&home, &project, &mut out, &mut err)
            });
            assert!(
                matches!(result, Err(CliError::FampExecutable(_))),
                "FAMP_INSTALL_FAMP_BIN={value:?} must fail resolution, got {result:?}"
            );
            assert_tree_unchanged(dir.path(), &before, &format!("install-codex {value:?}"));
            assert!(
                legacy_shim.exists(),
                "legacy shim must not be pruned when resolution fails"
            );
            let config: toml::Table =
                toml::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
            assert_eq!(
                config["hooks"]["state"][trust_key.as_str()]["trusted_hash"].as_str(),
                Some("sha256:prior"),
                "hook trust state must be untouched when resolution fails"
            );
            assert_eq!(
                config["mcp_servers"]["famp"]["command"].as_str(),
                Some("/prior/famp")
            );
        }
    }

    #[test]
    fn unsupported_probe_fails_before_any_mutation() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let bin = dir.path().join("unsupported famp");
        std::fs::write(&bin, "#!/bin/sh\nexit 1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let executable = FampExecutable::validate(bin).unwrap();
        let mut err = Vec::new();
        assert!(run_at_project_with_executable(&home, &home, &executable, &mut err).is_err());
        assert!(!home.exists());
    }

    #[test]
    fn install_codex_preserves_unrelated_top_level_sections() {
        with_probe_capable_famp(|executable| {
            let dir = tempfile::tempdir().unwrap();
            let home = dir.path();
            std::fs::create_dir_all(home.join(".codex")).unwrap();
            std::fs::write(
                home.join(".codex/config.toml"),
                "[some_other_section]\nkey = \"keep me\"\n[mcp_servers.github]\ncommand = \"/x\"\n",
            )
            .unwrap();

            let mut err = Vec::<u8>::new();
            run_at_project_with_executable(home, home, executable, &mut err).unwrap();

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
        });
    }

    #[test]
    fn install_codex_preserves_unrelated_project_hooks() {
        with_probe_capable_famp(|executable| {
            let dir = tempfile::tempdir().unwrap();
            let home = dir.path();
            std::fs::create_dir_all(home.join(".codex")).unwrap();
            std::fs::write(
            home.join(".codex/hooks.json"),
            serde_json::to_string_pretty(&json!({
                "hooks": {
                    "SessionStart": [{"hooks": [{"type": "command", "command": "echo start"}]}],
                    "Stop": [{"hooks": [{"type": "command", "command": "echo stop", "timeout": 10}]}]
                }
            }))
            .unwrap(),
        )
        .unwrap();

            let mut err = Vec::<u8>::new();
            run_at_project_with_executable(home, home, executable, &mut err).unwrap();

            let hooks: JsonValue = serde_json::from_str(
                &std::fs::read_to_string(home.join(".codex/hooks.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(
                hooks["hooks"]["SessionStart"][0]["hooks"][0]["command"],
                "echo start"
            );
            let stop = hooks["hooks"]["Stop"].as_array().unwrap();
            assert_eq!(stop.len(), 2);
            assert_eq!(stop[0]["hooks"][0]["command"], "echo stop");
            assert!(stop[1]["hooks"][0]["command"]
                .as_str()
                .unwrap()
                .contains("hook codex-stop"));
        });
    }

    #[test]
    fn install_codex_is_idempotent() {
        with_probe_capable_famp(|executable| {
            let dir = tempfile::tempdir().unwrap();
            let home = dir.path();
            let mut err = Vec::<u8>::new();
            run_at_project_with_executable(home, home, executable, &mut err).unwrap();
            let first = std::fs::read_to_string(home.join(".codex/config.toml")).unwrap();

            std::thread::sleep(std::time::Duration::from_secs(1));

            let mut err2 = Vec::<u8>::new();
            run_at_project_with_executable(home, home, executable, &mut err2).unwrap();
            let second = std::fs::read_to_string(home.join(".codex/config.toml")).unwrap();
            assert_eq!(first, second);

            let hooks = std::fs::read_to_string(home.join(".codex/hooks.json")).unwrap();
            let parsed_hooks: JsonValue = serde_json::from_str(&hooks).unwrap();
            assert_eq!(parsed_hooks["hooks"]["Stop"].as_array().unwrap().len(), 1);
        });
    }

    #[test]
    fn codex_hash_matches_codex_v0_144_observed_stop_hook() {
        let command = "\"/Users/benlamm/.nvm/versions/node/v22.14.0/bin/node\" \
                       \"/Users/benlamm/Workspace/FAMP/.codex/hooks/gsd-context-monitor.js\"";
        assert_eq!(
            codex_command_hook_hash("stop", command, 10),
            "sha256:e005a9ef3e941532a132d758bec3fb7f90dd6d21124650d365a472dd561d4351"
        );
    }

    #[test]
    fn find_git_root_accepts_git_file_or_directory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        let child = root.join("a/b");
        std::fs::create_dir_all(&child).unwrap();
        std::fs::write(root.join(".git"), "gitdir: /tmp/repo.git\n").unwrap();
        assert_eq!(find_git_root(&child), Some(root.clone()));

        std::fs::remove_file(root.join(".git")).unwrap();
        std::fs::create_dir(root.join(".git")).unwrap();
        assert_eq!(find_git_root(&child), Some(root));
    }
}

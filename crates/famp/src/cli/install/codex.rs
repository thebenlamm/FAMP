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
use crate::cli::install::{await_hook, json_merge, stop_entry, toml_merge};

const CODEX_AWAIT_TIMEOUT_SEC: i64 = 86_400;
const CODEX_STOP_EVENT_LABEL: &str = "stop";

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

fn find_git_root(start: &Path) -> Option<PathBuf> {
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
    let home = normalize_absolute_path(home)?;
    let project = normalize_absolute_path(project)?;
    let config_path = home.join(".codex").join("config.toml");
    let hooks_path = project.join(".codex").join("hooks.json");
    let await_shim_path = project.join(".codex").join("hooks").join("famp-await.sh");
    let famp_bin = resolve_famp_bin(&home);

    // Probe BEFORE any mutation: a failing probe must leave every file
    // (MCP entry, legacy shim, hooks.json, hook-trust state) untouched.
    // Writing partial state (e.g. MCP entry written + shim pruned but no
    // Stop hook wired) would be strictly worse than not running at all.
    probe_codex_stop_support(&famp_bin)?;

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

    let command = codex_stop_command(&famp_bin);
    let new_stop_array = build_stop_array(&hooks_path, &famp_bin, &await_shim_path, &command)?;
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
    let trusted_hashes = famp_trusted_hashes(CODEX_STOP_EVENT_LABEL, &famp_bin, &await_shim_path);
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
        "install-codex complete. Restart Codex sessions to pick up MCP changes; \
         the project Stop hook is ready for new turns."
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

pub(crate) fn normalize_absolute_path(path: &Path) -> Result<PathBuf, CliError> {
    match path.canonicalize() {
        Ok(canonical) => Ok(canonical),
        Err(_) => std::path::absolute(path).map_err(|source| CliError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Resolve the `famp` binary path for install-time hook wiring.
///
/// Checks `FAMP_INSTALL_FAMP_BIN` first: if set and non-empty, that path
/// (normalized) is used directly and the `current_exe`/`PATH`/fallback chain
/// below is skipped entirely. Intended for integration tests (so the probe
/// exercises a real, freshly-built `famp` rather than whatever stale binary
/// happens to be first on the developer's `PATH`) and unusual deployments
/// that need to pin the wired binary explicitly.
///
/// Otherwise: prefer the running executable when it is literally named
/// `famp` (so `./target/release/famp install-codex` wires the freshly built
/// binary instead of a possibly-stale one elsewhere on `PATH`), then `PATH`
/// (`which famp`), else `~/.cargo/bin/famp`. Never use a test-harness exe.
pub(crate) fn resolve_famp_bin(home: &Path) -> PathBuf {
    if let Ok(pinned) = std::env::var("FAMP_INSTALL_FAMP_BIN") {
        if !pinned.is_empty() {
            let pinned = PathBuf::from(pinned);
            return normalize_absolute_path(&pinned).unwrap_or(pinned);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if exe.file_name().and_then(|n| n.to_str()) == Some("famp") {
            return normalize_absolute_path(&exe).unwrap_or(exe);
        }
    }
    if let Ok(p) = which::which("famp") {
        return normalize_absolute_path(&p).unwrap_or(p);
    }
    let fallback = home.join(".cargo").join("bin").join("famp");
    normalize_absolute_path(&fallback).unwrap_or(fallback)
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
/// Skipped when `famp_bin` does not exist on disk: a nonexistent resolved
/// path is already an unrunnable-binary case, and the existing `run_at`
/// install tests (which resolve a non-`famp`-named test-harness exe via
/// `current_exe`/`PATH` fallback against a tempdir `home`) depend on
/// tolerating that rather than failing at this probe.
fn probe_codex_stop_support(famp_bin: &Path) -> Result<(), CliError> {
    if !famp_bin.is_file() {
        return Ok(());
    }
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
    let raw = path.display().to_string();
    if raw
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-'))
    {
        return raw;
    }
    format!("'{}'", raw.replace('\'', "'\\''"))
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

    /// Path to the cargo-built `famp` binary (genuinely has `hook
    /// codex-stop`). Pinned via `FAMP_INSTALL_FAMP_BIN` so `run_at`'s
    /// pre-write probe exercises a real success path instead of resolving
    /// whatever `famp` happens to be first on the developer's/CI's `PATH`.
    ///
    /// Wrapped in `temp_env::with_var` (WR-06 convention, see `Cargo.toml`)
    /// so the process-global env mutation is serialized across parallel
    /// test threads rather than racing via a bare `std::env::set_var`.
    fn with_pinned_famp_bin<F: FnOnce()>(test: F) {
        let bin = assert_cmd::cargo::cargo_bin("famp");
        temp_env::with_var(
            "FAMP_INSTALL_FAMP_BIN",
            Some(bin.to_string_lossy().into_owned()),
            test,
        );
    }

    #[test]
    fn install_codex_writes_mcp_and_stop_hook() {
        with_pinned_famp_bin(|| {
            let dir = tempfile::tempdir().unwrap();
            let home = dir.path();
            let mut out = Vec::<u8>::new();
            let mut err = Vec::<u8>::new();
            run_at(home, &mut out, &mut err).unwrap();

            let cfg = home.join(".codex/config.toml");
            assert!(cfg.exists());
            let parsed: toml::Table =
                toml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
            let famp_t = parsed["mcp_servers"]["famp"].as_table().unwrap();
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

    #[test]
    fn install_codex_preserves_unrelated_top_level_sections() {
        with_pinned_famp_bin(|| {
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
        });
    }

    #[test]
    fn install_codex_preserves_unrelated_project_hooks() {
        with_pinned_famp_bin(|| {
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

            let mut out = Vec::<u8>::new();
            let mut err = Vec::<u8>::new();
            run_at(home, &mut out, &mut err).unwrap();

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
        with_pinned_famp_bin(|| {
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

//! `famp install-codex` subcommand handler.
//!
//! Installs two Codex integration surfaces:
//!  1. `~/.codex/config.toml :: [mcp_servers.famp]`
//!  2. `<project>/.codex/hooks.json :: hooks.Stop` listen-mode await hook
//!
//! Codex hook trust is pre-seeded in `~/.codex/config.toml` for the installed
//! hook entry so explicit `famp install-codex` produces a runnable integration
//! without a separate `/hooks` approval step.

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
    let famp_bin = which::which("famp")
        .ok()
        .unwrap_or_else(|| home.join(".cargo").join("bin").join("famp"));

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

    await_hook::install_shim(&await_shim_path)?;
    writeln!(
        err,
        "  [2/4] {} :: listen-mode await shim installed (mode 0755)",
        await_shim_path.display()
    )
    .ok();

    let command = shell_quote_path(&await_shim_path);
    let new_stop_array = build_stop_array(&hooks_path, &await_shim_path, &command)?;
    let stop_index = new_stop_array.len().saturating_sub(1);
    let outcome = json_merge::upsert_user_json(
        &hooks_path,
        "hooks",
        "Stop",
        JsonValue::Array(new_stop_array),
    )?;
    writeln!(
        err,
        "  [3/4] {} :: hooks.Stop -> {:?}",
        hooks_path.display(),
        outcome
    )
    .ok();

    let trust_key = codex_hook_key(&hooks_path, CODEX_STOP_EVENT_LABEL, stop_index, 0);
    let trusted_hash =
        codex_command_hook_hash(CODEX_STOP_EVENT_LABEL, &command, CODEX_AWAIT_TIMEOUT_SEC);
    let trusted_hashes = famp_trusted_hashes(CODEX_STOP_EVENT_LABEL, &await_shim_path);
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

fn build_stop_array(
    hooks_path: &Path,
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

    let famp_paths = famp_hook_command_patterns(await_shim_path);
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

pub(crate) fn famp_hook_command_patterns(await_shim_path: &Path) -> Vec<String> {
    let mut seen = BTreeSet::new();
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

pub(crate) fn famp_trusted_hashes(event_label: &str, await_shim_path: &Path) -> Vec<String> {
    famp_hook_command_patterns(await_shim_path)
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

    #[test]
    fn install_codex_writes_mcp_and_stop_hook() {
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

        let project = normalize_absolute_path(home).unwrap();
        let hooks_path = project.join(".codex/hooks.json");
        let hooks: JsonValue =
            serde_json::from_str(&std::fs::read_to_string(&hooks_path).unwrap()).unwrap();
        let stop = hooks["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 1);
        let command = stop[0]["hooks"][0]["command"].as_str().unwrap();
        assert!(command.ends_with("/.codex/hooks/famp-await.sh"));
        assert_eq!(stop[0]["hooks"][0]["timeout"], CODEX_AWAIT_TIMEOUT_SEC);
        assert!(project.join(".codex/hooks/famp-await.sh").exists());

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
    fn install_codex_preserves_unrelated_project_hooks() {
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

        let hooks: JsonValue =
            serde_json::from_str(&std::fs::read_to_string(home.join(".codex/hooks.json")).unwrap())
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
            .contains("famp-await.sh"));
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

        let hooks = std::fs::read_to_string(home.join(".codex/hooks.json")).unwrap();
        let parsed_hooks: JsonValue = serde_json::from_str(&hooks).unwrap();
        assert_eq!(parsed_hooks["hooks"]["Stop"].as_array().unwrap().len(), 1);
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

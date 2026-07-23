//! `famp install-grok` subcommand handler.
//!
//! Installs Grok integration surfaces under `~/.grok/` only (single Stop
//! arming path — adversarial B2):
//!  1. `~/.grok/config.toml :: [mcp_servers.famp]` (absolute `famp` path)
//!  2. `~/.grok/hooks/famp-await.sh` (listen-mode Stop shim)
//!  3. `~/.grok/hooks/famp-listen-stop.json` (Stop timeout 86400)
//!  4. `~/.grok/skills/famp-listen/SKILL.md` ("just register" docs)
//!
//! Does **not** touch `~/.claude/` — Grok native hooks only. Dual-host
//! machines that also ran `install-claude-code` may still load Claude Stop
//! via Grok's Claude-compat hook scan; the await shim holds a per-identity
//! singleton lock so only one Stop await parks. Optionally set
//! `[compat.claude] hooks = false` in `~/.grok/config.toml` to disable
//! Claude hook discovery entirely.
//!
//! Auto-wake: long Stop hook (`decision: block`) — same as Claude/Codex.
//! Grok host limit: 8 Stop continuations per turn.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;
use serde_json::json;
use toml::Value as TomlValue;

use crate::cli::error::CliError;
use crate::cli::install::{await_hook, toml_merge};

/// Embedded Grok skill body ("just register" + Stop auto-wake).
pub const FAMP_LISTEN_SKILL_MD: &str =
    include_str!("../../../assets/skills/famp-listen/SKILL.md");

#[derive(Debug, Args)]
pub struct InstallGrokArgs {
    /// Override the install target home (defaults to `dirs::home_dir()`).
    /// Hidden flag — used by integration tests to redirect to a tempdir.
    #[arg(long, env = "FAMP_INSTALL_TARGET_HOME", hide = true)]
    pub home: Option<PathBuf>,
}

#[allow(clippy::needless_pass_by_value)]
pub fn run(args: InstallGrokArgs) -> Result<(), CliError> {
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
    let skill_path = home
        .join(".grok")
        .join("skills")
        .join("famp-listen")
        .join("SKILL.md");
    let grok_await_shim = home.join(".grok").join("hooks").join("famp-await.sh");
    let stop_json_path = home
        .join(".grok")
        .join("hooks")
        .join("famp-listen-stop.json");
    // Absolute path only. Grok's MCP spawn env often lacks ~/.cargo/bin on
    // PATH, so bare `command = "famp"` fails with ENOENT (live smoke 2026-07-23).
    // Re-run `famp install-grok` after moving the binary.
    let famp_bin = which::which("famp")
        .ok()
        .unwrap_or_else(|| home.join(".cargo").join("bin").join("famp"));

    writeln!(err, "Installing Grok MCP entry into {}", home.display()).ok();
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

    // Reuse the Codex TOML merge helper — same `[mcp_servers.famp]` shape.
    let outcome = toml_merge::upsert_codex_table(&config_path, "mcp_servers", "famp", famp_table)?;
    writeln!(
        err,
        "  [1/4] {} :: [mcp_servers.famp] -> {:?}",
        config_path.display(),
        outcome
    )
    .ok();

    // Native Grok await shim only (B2: do not write ~/.claude/).
    await_hook::install_shim(&grok_await_shim)?;
    writeln!(
        err,
        "  [2/4] {} :: await shim installed",
        grok_await_shim.display()
    )
    .ok();

    install_stop_hook_json(&stop_json_path, &grok_await_shim)?;
    writeln!(
        err,
        "  [3/4] {} :: Stop hook (timeout 86400)",
        stop_json_path.display()
    )
    .ok();

    install_skill(&skill_path)?;
    writeln!(
        err,
        "  [4/4] {} :: famp-listen skill installed",
        skill_path.display()
    )
    .ok();

    writeln!(err).ok();
    writeln!(
        err,
        "install-grok complete. Restart Grok sessions to pick up MCP/hook changes. \
         User says \"register with famp\" → famp_register only; Stop hook auto-wakes \
         (Grok cap: 8 continuations/turn). Does not touch ~/.claude/."
    )
    .ok();
    if which::which("famp").is_err() {
        writeln!(
            err,
            "  hint: famp binary not on PATH; run `cargo install famp` to install it."
        )
        .ok();
    }
    if home.join(".claude").join("settings.json").exists() {
        writeln!(
            err,
            "  note: ~/.claude/settings.json exists (Claude install). Grok may also \
             load those Stop hooks via compat. The await shim singleton-locks per \
             identity. To load only native Grok hooks, set in ~/.grok/config.toml:\n\
             \x20   [compat.claude]\n\
             \x20   hooks = false"
        )
        .ok();
    }
    Ok(())
}

/// Write `~/.grok/hooks/famp-listen-stop.json` with absolute await shim path.
fn install_stop_hook_json(path: &Path, await_shim: &Path) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| CliError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let body = json!({
        "hooks": {
            "Stop": [
                {
                    "hooks": [
                        {
                            "type": "command",
                            "command": await_shim.display().to_string(),
                            "timeout": 86400
                        }
                    ]
                }
            ]
        }
    });
    let serialized = serde_json::to_string_pretty(&body).map_err(|source| {
        CliError::JsonMergeParse {
            path: path.to_path_buf(),
            source,
        }
    })?;
    std::fs::write(path, format!("{serialized}\n")).map_err(|source| CliError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644)).map_err(
            |source| CliError::Io {
                path: path.to_path_buf(),
                source,
            },
        )?;
    }
    Ok(())
}

fn install_skill(path: &Path) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| CliError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(path, FAMP_LISTEN_SKILL_MD).map_err(|source| CliError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644)).map_err(
            |source| CliError::Io {
                path: path.to_path_buf(),
                source,
            },
        )?;
    }
    Ok(())
}

/// Remove only the skill file FAMP owns. Never `remove_dir_all` when the
/// directory contains extra user files.
pub(crate) fn remove_skill_dir(skill_dir: &Path) -> Result<(), CliError> {
    let skill_md = skill_dir.join("SKILL.md");
    match std::fs::read_to_string(&skill_md) {
        Ok(body) if body == FAMP_LISTEN_SKILL_MD => {
            std::fs::remove_file(&skill_md).map_err(|source| CliError::Io {
                path: skill_md.clone(),
                source,
            })?;
        }
        Ok(_) => {
            return Ok(());
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(CliError::Io {
                path: skill_md,
                source,
            });
        }
    }

    match std::fs::read_dir(skill_dir) {
        Ok(mut entries) => {
            if entries.next().is_none() {
                std::fs::remove_dir(skill_dir).map_err(|source| CliError::Io {
                    path: skill_dir.to_path_buf(),
                    source,
                })?;
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(CliError::Io {
            path: skill_dir.to_path_buf(),
            source,
        }),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn install_grok_writes_mcp_skill_shim_and_stop_json() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        run_at(home, &mut out, &mut err).unwrap();

        let cfg = home.join(".grok/config.toml");
        assert!(cfg.exists());
        let parsed: toml::Table = toml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        let famp_t = parsed["mcp_servers"]["famp"].as_table().unwrap();
        assert_eq!(
            famp_t["args"].as_array().unwrap()[0].as_str().unwrap(),
            "mcp"
        );
        assert_eq!(famp_t["startup_timeout_sec"].as_integer().unwrap(), 10);
        let cmd = famp_t["command"].as_str().unwrap();
        assert!(
            cmd.ends_with("/famp") || Path::new(cmd).is_absolute(),
            "expected absolute famp path, got: {cmd}"
        );

        let skill = home.join(".grok/skills/famp-listen/SKILL.md");
        assert!(skill.exists());
        let body = std::fs::read_to_string(&skill).unwrap();
        assert!(body.contains("famp_register"));

        let grok_shim = home.join(".grok/hooks/famp-await.sh");
        assert!(grok_shim.exists(), "grok await shim missing");
        assert!(
            std::fs::read_to_string(&grok_shim)
                .unwrap()
                .contains("trying pid-correlated"),
            "shim must carry pid-correlated fallback"
        );
        // B2: never touch Claude tree.
        assert!(
            !home.join(".claude").exists(),
            "install-grok must not create ~/.claude/"
        );

        let stop_json = home.join(".grok/hooks/famp-listen-stop.json");
        assert!(stop_json.exists(), "Stop hook json missing");
        let stop: Value =
            serde_json::from_str(&std::fs::read_to_string(&stop_json).unwrap()).unwrap();
        let hooks = stop["hooks"]["Stop"][0]["hooks"].as_array().unwrap();
        assert_eq!(hooks[0]["timeout"], 86400);
        assert_eq!(
            hooks[0]["command"].as_str().unwrap(),
            grok_shim.display().to_string()
        );

        assert!(!home.join(".codex").exists());
        assert!(!home.join(".famp/hook-runner.sh").exists());
    }

    #[test]
    fn install_grok_does_not_merge_existing_claude_settings() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let settings = home.join(".claude/settings.json");
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        let prior = r#"{"hooks":{"Stop":[{"matcher":"","hooks":[{"type":"command","command":"/other/hook.sh","timeout":5}]}]}}"#;
        std::fs::write(&settings, prior).unwrap();

        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        run_at(home, &mut out, &mut err).unwrap();

        let post = std::fs::read_to_string(&settings).unwrap();
        assert_eq!(
            post, prior,
            "install-grok must not mutate Claude settings (B2 single Stop path)"
        );
        assert!(
            !home.join(".claude/hooks/famp-await.sh").exists(),
            "must not write Claude await shim"
        );
        // Native Grok Stop still installed.
        assert!(home.join(".grok/hooks/famp-listen-stop.json").exists());
    }

    #[test]
    fn remove_skill_dir_preserves_user_files() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("famp-listen");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), FAMP_LISTEN_SKILL_MD).unwrap();
        std::fs::write(skill_dir.join("notes.txt"), "user owned").unwrap();
        remove_skill_dir(&skill_dir).unwrap();
        assert!(!skill_dir.join("SKILL.md").exists());
        assert!(skill_dir.join("notes.txt").exists());
        assert!(skill_dir.exists());
    }

    #[test]
    fn remove_skill_dir_leaves_modified_skill() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("famp-listen");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "user customized skill\n").unwrap();
        remove_skill_dir(&skill_dir).unwrap();
        assert_eq!(
            std::fs::read_to_string(skill_dir.join("SKILL.md")).unwrap(),
            "user customized skill\n"
        );
    }

    #[test]
    fn install_grok_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        run_at(home, &mut out, &mut err).unwrap();
        let first = std::fs::read_to_string(home.join(".grok/config.toml")).unwrap();
        let first_stop =
            std::fs::read_to_string(home.join(".grok/hooks/famp-listen-stop.json")).unwrap();

        std::thread::sleep(std::time::Duration::from_secs(1));

        let mut out2 = Vec::<u8>::new();
        let mut err2 = Vec::<u8>::new();
        run_at(home, &mut out2, &mut err2).unwrap();
        let second = std::fs::read_to_string(home.join(".grok/config.toml")).unwrap();
        let second_stop =
            std::fs::read_to_string(home.join(".grok/hooks/famp-listen-stop.json")).unwrap();
        assert_eq!(first, second);
        assert_eq!(first_stop, second_stop);
    }

    #[test]
    fn skill_asset_is_just_register_docs() {
        assert!(FAMP_LISTEN_SKILL_MD.contains("famp_register"));
        assert!(
            FAMP_LISTEN_SKILL_MD.contains("register with famp")
                || FAMP_LISTEN_SKILL_MD.contains("just register")
        );
        assert!(FAMP_LISTEN_SKILL_MD.contains("Stop"));
        assert!(
            FAMP_LISTEN_SKILL_MD.to_lowercase().contains("optional")
                || FAMP_LISTEN_SKILL_MD.contains("fallback")
        );
    }
}

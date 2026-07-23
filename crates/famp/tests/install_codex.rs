//! install-codex writes `[mcp_servers.famp]` to `~/.codex/config.toml` and a
//! FAMP-owned Stop hook to `.codex/hooks.json`, preserving unrelated config.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use std::path::{Path, PathBuf};

use serde_json::json;

fn absolute(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|_| std::path::absolute(path).unwrap())
}

/// Path to the cargo-built `famp` binary (genuinely has `hook codex-stop`).
/// Pinned via `FAMP_INSTALL_FAMP_BIN` so `install-codex`'s pre-write probe
/// exercises a real success path instead of resolving whatever `famp`
/// happens to be first on the developer's/CI's `PATH` (which may be stale).
///
/// Wrapped in `temp_env::with_var` (WR-06 convention, see `Cargo.toml`) so
/// the process-global env mutation is serialized across parallel test
/// threads rather than racing via a bare `std::env::set_var`.
fn with_pinned_famp_bin<F: FnOnce()>(test: F) {
    let bin = assert_cmd::cargo::cargo_bin("famp");
    temp_env::with_var(
        "FAMP_INSTALL_FAMP_BIN",
        Some(bin.to_string_lossy().into_owned()),
        test,
    );
}

#[test]
fn install_codex_writes_mcp_servers_famp_table_under_tempdir_home() {
    with_pinned_famp_bin(|| {
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path();
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        famp::cli::install::codex::run_at(home, &mut out, &mut err)
            .expect("install-codex happy path");

        let cfg = home.join(".codex/config.toml");
        assert!(cfg.exists(), "config.toml missing");
        let body = std::fs::read_to_string(&cfg).unwrap();
        let parsed: toml::Table = toml::from_str(&body).unwrap();
        let famp_t = parsed["mcp_servers"]["famp"].as_table().unwrap();
        assert_eq!(
            famp_t["args"].as_array().unwrap()[0].as_str().unwrap(),
            "mcp"
        );
        assert!(famp_t["command"].as_str().unwrap().contains("famp"));
        assert_eq!(famp_t["startup_timeout_sec"].as_integer().unwrap(), 10);

        assert!(
            !home.join(".claude").exists(),
            "Codex install must not touch ~/.claude/"
        );
        assert!(
            !home.join(".famp/hook-runner.sh").exists(),
            "Codex install must not write the shim"
        );
        assert!(
            !home.join(".codex/hooks/famp-await.sh").exists(),
            "Codex install must NOT write the legacy shell shim"
        );

        let hooks: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(home.join(".codex/hooks.json")).unwrap())
                .unwrap();
        let stop = hooks["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 1);
        assert!(
            stop[0]["hooks"][0]["command"]
                .as_str()
                .unwrap()
                .contains("hook codex-stop"),
            "Codex install must point Stop at native famp hook codex-stop"
        );

        let project = absolute(home);
        let trust_key = format!("{}:stop:0:0", project.join(".codex/hooks.json").display());
        assert!(parsed["hooks"]["state"][trust_key]["trusted_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
    });
}

#[test]
fn install_codex_uses_absolute_project_paths_for_relative_project_arg() {
    with_pinned_famp_bin(|| {
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let cwd = std::env::current_dir().unwrap();
        let project_dir = tempfile::tempdir_in(&cwd).unwrap();
        let relative_project = PathBuf::from(project_dir.path().file_name().unwrap());
        let absolute_project = absolute(project_dir.path());

        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        famp::cli::install::codex::run_at_project(&home, &relative_project, &mut out, &mut err)
            .unwrap();

        let hooks_path = absolute_project.join(".codex/hooks.json");
        let hooks: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&hooks_path).unwrap()).unwrap();
        let command = hooks["hooks"]["Stop"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .to_string();
        let famp_token = command
            .split_whitespace()
            .next()
            .unwrap()
            .trim_matches(['\'', '"']);
        assert!(
            Path::new(famp_token).is_absolute(),
            "native hook famp path must be absolute, got {command:?}"
        );
        assert!(
            command.contains("hook codex-stop"),
            "hook command must invoke native codex-stop, got {command:?}"
        );

        let config_path = absolute(&home).join(".codex/config.toml");
        let parsed: toml::Table =
            toml::from_str(&std::fs::read_to_string(config_path).unwrap()).unwrap();
        let trust_key = format!("{}:stop:0:0", hooks_path.display());
        assert!(
            parsed["hooks"]["state"]
                .as_table()
                .unwrap()
                .contains_key(&trust_key),
            "absolute trust key missing: {trust_key}"
        );
    });
}

#[test]
fn install_codex_treats_empty_project_hooks_json_as_empty_config() {
    with_pinned_famp_bin(|| {
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path();
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        std::fs::write(home.join(".codex/hooks.json"), " \n\t").unwrap();

        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        famp::cli::install::codex::run_at(home, &mut out, &mut err).unwrap();

        let hooks: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(home.join(".codex/hooks.json")).unwrap())
                .unwrap();
        assert_eq!(hooks["hooks"]["Stop"].as_array().unwrap().len(), 1);
    });
}

#[test]
fn reinstall_codex_prunes_stale_famp_hook_trust_after_stop_index_churn() {
    with_pinned_famp_bin(|| {
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path();
        let project = absolute(home);
        let hooks_path = project.join(".codex/hooks.json");
        let config_path = project.join(".codex/config.toml");

        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        famp::cli::install::codex::run_at(home, &mut out, &mut err).unwrap();

        let stale_key = format!("{}:stop:0:0", hooks_path.display());
        let current_key = format!("{}:stop:1:0", hooks_path.display());
        let mut hooks: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&hooks_path).unwrap()).unwrap();
        hooks["hooks"]["Stop"].as_array_mut().unwrap().insert(
            0,
            json!({"hooks": [{"type": "command", "command": "echo keep", "timeout": 10}]}),
        );
        std::fs::write(&hooks_path, serde_json::to_string_pretty(&hooks).unwrap()).unwrap();

        let mut out2 = Vec::<u8>::new();
        let mut err2 = Vec::<u8>::new();
        famp::cli::install::codex::run_at(home, &mut out2, &mut err2).unwrap();

        let hooks: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&hooks_path).unwrap()).unwrap();
        let stop = hooks["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2);
        assert_eq!(stop[0]["hooks"][0]["command"], "echo keep");

        let parsed: toml::Table =
            toml::from_str(&std::fs::read_to_string(config_path).unwrap()).unwrap();
        let state = parsed["hooks"]["state"].as_table().unwrap();
        assert!(
            !state.contains_key(&stale_key),
            "stale FAMP trust key should be removed: {stale_key}"
        );
        assert!(
            state.contains_key(&current_key),
            "current FAMP trust key should be seeded: {current_key}"
        );
    });
}

#[test]
fn install_codex_preserves_realistic_codex_config() {
    with_pinned_famp_bin(|| {
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path();
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        let pre = r#"# user comment line
model = "gpt-4"
sandbox_mode = "workspace-write"

[shell_environment_policy]
inherit = "core"

[mcp_servers.github]
command = "/usr/local/bin/github-mcp"
args = ["serve"]

[mcp_servers.fs]
command = "/opt/fs"
args = []
"#;
        std::fs::write(home.join(".codex/config.toml"), pre).unwrap();

        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        famp::cli::install::codex::run_at(home, &mut out, &mut err).unwrap();

        let post: toml::Table =
            toml::from_str(&std::fs::read_to_string(home.join(".codex/config.toml")).unwrap())
                .unwrap();
        assert_eq!(post["model"].as_str().unwrap(), "gpt-4");
        assert_eq!(post["sandbox_mode"].as_str().unwrap(), "workspace-write");
        assert_eq!(
            post["shell_environment_policy"]["inherit"]
                .as_str()
                .unwrap(),
            "core"
        );
        assert_eq!(
            post["mcp_servers"]["github"]["command"].as_str().unwrap(),
            "/usr/local/bin/github-mcp"
        );
        assert_eq!(
            post["mcp_servers"]["fs"]["command"].as_str().unwrap(),
            "/opt/fs"
        );
        assert!(post["mcp_servers"]["famp"].as_table().is_some());
    });
}

#[test]
fn install_codex_merges_with_existing_project_hooks() {
    with_pinned_famp_bin(|| {
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path();
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        std::fs::write(
            home.join(".codex/hooks.json"),
            r#"{
  "hooks": {
    "SessionStart": [{"hooks": [{"type": "command", "command": "echo start"}]}],
    "Stop": [{"hooks": [{"type": "command", "command": "echo existing", "timeout": 10}]}]
  }
}
"#,
        )
        .unwrap();

        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        famp::cli::install::codex::run_at(home, &mut out, &mut err).unwrap();

        let hooks: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(home.join(".codex/hooks.json")).unwrap())
                .unwrap();
        assert_eq!(
            hooks["hooks"]["SessionStart"][0]["hooks"][0]["command"],
            "echo start"
        );
        let stop = hooks["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2);
        assert_eq!(stop[0]["hooks"][0]["command"], "echo existing");
        assert!(stop[1]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("hook codex-stop"));
    });
}

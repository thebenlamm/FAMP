//! install-codex -> uninstall-codex restores semantic equality on
//! `~/.codex/config.toml` and removes FAMP-owned project hook artifacts.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use serde_json::json;

const PRE: &str = r#"model = "gpt-4"
sandbox_mode = "read"

[mcp_servers.github]
command = "/usr/local/bin/github-mcp"
args = ["serve"]
"#;

#[test]
fn codex_install_then_uninstall_restores_pre_state() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    std::fs::create_dir_all(home.join(".codex")).unwrap();
    std::fs::write(home.join(".codex/config.toml"), PRE).unwrap();

    let pre_parsed: toml::Table = toml::from_str(PRE).unwrap();

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::install::codex::run_at(home, &mut out, &mut err).unwrap();

    let mut out2 = Vec::<u8>::new();
    let mut err2 = Vec::<u8>::new();
    famp::cli::uninstall::codex::run_at(home, &mut out2, &mut err2).unwrap();

    let post_parsed: toml::Table =
        toml::from_str(&std::fs::read_to_string(home.join(".codex/config.toml")).unwrap()).unwrap();
    assert_eq!(
        pre_parsed, post_parsed,
        "config.toml drifted across roundtrip\nPRE: {pre_parsed:#?}\nPOST: {post_parsed:#?}"
    );

    let hooks: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(home.join(".codex/hooks.json")).unwrap())
            .unwrap();
    assert!(hooks["hooks"]
        .as_object()
        .is_none_or(|hooks| !hooks.contains_key("Stop")));
    assert!(!home.join(".codex/hooks/famp-await.sh").exists());
}

#[test]
fn codex_install_then_uninstall_on_empty_home_leaves_clean_state() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::install::codex::run_at(home, &mut out, &mut err).unwrap();

    let mut out2 = Vec::<u8>::new();
    let mut err2 = Vec::<u8>::new();
    famp::cli::uninstall::codex::run_at(home, &mut out2, &mut err2).unwrap();

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
    assert!(!home.join(".codex/hooks/famp-await.sh").exists());
}

#[test]
fn codex_uninstall_preserves_unrelated_project_hooks() {
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

    let mut out2 = Vec::<u8>::new();
    let mut err2 = Vec::<u8>::new();
    famp::cli::uninstall::codex::run_at(home, &mut out2, &mut err2).unwrap();

    let hooks: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(home.join(".codex/hooks.json")).unwrap())
            .unwrap();
    assert_eq!(
        hooks["hooks"]["SessionStart"][0]["hooks"][0]["command"],
        "echo start"
    );
    let stop = hooks["hooks"]["Stop"].as_array().unwrap();
    assert_eq!(stop.len(), 1);
    assert_eq!(stop[0]["hooks"][0]["command"], "echo existing");
}

#[test]
fn codex_uninstall_prunes_stale_famp_hook_trust_after_index_churn() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    let project = home.canonicalize().unwrap();
    let hooks_path = project.join(".codex/hooks.json");
    let config_path = project.join(".codex/config.toml");

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::install::codex::run_at(home, &mut out, &mut err).unwrap();

    let stale_key = format!("{}:stop:0:0", hooks_path.display());
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

    let mut out3 = Vec::<u8>::new();
    let mut err3 = Vec::<u8>::new();
    famp::cli::uninstall::codex::run_at(home, &mut out3, &mut err3).unwrap();

    let parsed: toml::Table =
        toml::from_str(&std::fs::read_to_string(config_path).unwrap()).unwrap();
    assert!(
        parsed.get("hooks").is_none(),
        "all FAMP hook trust should be removed, including stale {stale_key}"
    );
    let hooks: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(hooks_path).unwrap()).unwrap();
    let stop = hooks["hooks"]["Stop"].as_array().unwrap();
    assert_eq!(stop.len(), 1);
    assert_eq!(stop[0]["hooks"][0]["command"], "echo keep");
}

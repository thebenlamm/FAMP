//! D-04: install -> uninstall roundtrip restores pre-existing user state.
//!
//! Enforced via `insta::assert_json_snapshot!` against checked-in snapshots in
//! `crates/famp/tests/snapshots/`. Snapshot review surfaces intentional schema
//! changes as reviewable diffs.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use serde_json::Value;

const REALISTIC_CLAUDE_JSON: &str = r#"{
  "numStartups": 137,
  "installMethod": "native",
  "tipsHistory": ["welcome", "first-edit", "slash-commands"],
  "cachedDynamicConfigs": {
    "modelDefault": "sonnet",
    "knownTools": ["bash", "grep", "edit"]
  },
  "mcpServers": {
    "github": {
      "type": "stdio",
      "command": "/usr/local/bin/github-mcp",
      "args": ["serve"]
    },
    "filesystem": {
      "type": "stdio",
      "command": "/opt/fs-mcp",
      "args": []
    }
  }
}"#;

const REALISTIC_SETTINGS_JSON: &str = r#"{
  "permissions": {
    "allow": ["Bash(git:*)", "WebFetch(*)"],
    "deny": []
  },
  "env": {
    "MY_VAR": "abc"
  },
  "model": "opus",
  "hooks": {
    "PreToolUse": [
      {"type": "command", "command": "/some/pre-hook.sh"}
    ],
    "Stop": [
      {"type": "command", "command": "/some/other-stop.sh", "timeout": 60}
    ]
  }
}"#;

#[test]
fn install_then_uninstall_restores_pre_state_for_realistic_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    std::fs::create_dir_all(home.join(".claude")).unwrap();
    std::fs::write(home.join(".claude.json"), REALISTIC_CLAUDE_JSON).unwrap();
    std::fs::write(home.join(".claude/settings.json"), REALISTIC_SETTINGS_JSON).unwrap();

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::install::claude_code::run_at(home, &mut out, &mut err).unwrap();

    let mut out2 = Vec::<u8>::new();
    let mut err2 = Vec::<u8>::new();
    famp::cli::uninstall::claude_code::run_at(home, &mut out2, &mut err2).unwrap();

    let post_claude: Value =
        serde_json::from_str(&std::fs::read_to_string(home.join(".claude.json")).unwrap()).unwrap();
    let post_settings: Value =
        serde_json::from_str(&std::fs::read_to_string(home.join(".claude/settings.json")).unwrap())
            .unwrap();

    insta::assert_json_snapshot!("claude_json_pre_state", post_claude);
    insta::assert_json_snapshot!("settings_json_pre_state", post_settings);

    let commands_dir = home.join(".claude/commands");
    if commands_dir.exists() {
        let entries: Vec<_> = std::fs::read_dir(&commands_dir).unwrap().collect();
        assert_eq!(entries.len(), 0, "stray files in commands/ after uninstall");
    }
    assert!(
        !home.join(".famp/hook-runner.sh").exists(),
        "shim still present"
    );
}

#[test]
fn install_then_uninstall_on_empty_home_leaves_clean_state() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::install::claude_code::run_at(home, &mut out, &mut err).unwrap();
    assert!(home.join(".claude.json").exists());

    let mut out2 = Vec::<u8>::new();
    let mut err2 = Vec::<u8>::new();
    famp::cli::uninstall::claude_code::run_at(home, &mut out2, &mut err2).unwrap();

    let post: Value =
        serde_json::from_str(&std::fs::read_to_string(home.join(".claude.json")).unwrap()).unwrap();
    let has_famp = post
        .get("mcpServers")
        .and_then(|servers| servers.get("famp"))
        .is_some();
    assert!(!has_famp, "mcpServers.famp still present after uninstall");

    let settings: Value =
        serde_json::from_str(&std::fs::read_to_string(home.join(".claude/settings.json")).unwrap())
            .unwrap();
    let has_stop_entries = settings
        .get("hooks")
        .and_then(|hooks| hooks.get("Stop"))
        .and_then(Value::as_array)
        .is_some_and(|stop| !stop.is_empty());
    assert!(!has_stop_entries, "hooks.Stop has lingering entries");
}

#[test]
fn double_uninstall_is_noop() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::install::claude_code::run_at(home, &mut out, &mut err).unwrap();

    let mut out2 = Vec::<u8>::new();
    let mut err2 = Vec::<u8>::new();
    famp::cli::uninstall::claude_code::run_at(home, &mut out2, &mut err2).unwrap();

    let mut out3 = Vec::<u8>::new();
    let mut err3 = Vec::<u8>::new();
    famp::cli::uninstall::claude_code::run_at(home, &mut out3, &mut err3).unwrap();
}

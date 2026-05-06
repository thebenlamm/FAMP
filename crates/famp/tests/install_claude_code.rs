//! CC-01 + HOOK-04b install-side integration test.
//!
//! Runs `install::claude_code::run_at` against `$HOME=$TMPDIR` and asserts
//! the five mutated artifacts (claude.json, settings.json, 7 commands files,
//! hook-runner.sh, famp-await.sh) exist with correct content + modes. No real
//! `~/.claude.json` is touched - sandbox is `tempfile::TempDir`.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use std::os::unix::fs::PermissionsExt;

use serde_json::Value;

#[test]
fn install_claude_code_writes_all_artifacts() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::install::claude_code::run_at(home, &mut out, &mut err)
        .expect("install-claude-code happy path");

    let claude_json = home.join(".claude.json");
    assert!(claude_json.exists(), "claude.json missing");
    let claude: Value =
        serde_json::from_str(&std::fs::read_to_string(&claude_json).unwrap()).unwrap();
    assert_eq!(
        claude["mcpServers"]["famp"]["args"],
        serde_json::json!(["mcp"])
    );
    assert_eq!(claude["mcpServers"]["famp"]["type"], "stdio");

    let commands_dir = home.join(".claude").join("commands");
    for name in [
        "famp-register.md",
        "famp-send.md",
        "famp-channel.md",
        "famp-join.md",
        "famp-leave.md",
        "famp-who.md",
        "famp-inbox.md",
    ] {
        let p = commands_dir.join(name);
        assert!(p.exists(), "missing slash-command file: {name}");
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644, "{name} mode = {mode:o}");
    }

    let shim = home.join(".famp").join("hook-runner.sh");
    assert!(shim.exists());
    let shim_mode = std::fs::metadata(&shim).unwrap().permissions().mode() & 0o777;
    assert_eq!(shim_mode, 0o755);

    let await_shim = home.join(".claude").join("hooks").join("famp-await.sh");
    assert!(await_shim.exists(), "famp-await.sh missing");
    let await_mode = std::fs::metadata(&await_shim).unwrap().permissions().mode() & 0o777;
    assert_eq!(await_mode, 0o755);

    let settings = home.join(".claude").join("settings.json");
    let s: Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
    let stop = s["hooks"]["Stop"].as_array().unwrap();
    assert_eq!(stop.len(), 2, "expected exactly 2 Stop entries, got {}", stop.len());

    // Entry 0: hook-runner.sh, timeout 30
    assert_eq!(stop[0]["matcher"], "");
    let hooks0 = stop[0]["hooks"].as_array().unwrap();
    assert_eq!(hooks0.len(), 1);
    assert_eq!(hooks0[0]["type"], "command");
    assert!(hooks0[0]["command"]
        .as_str()
        .unwrap()
        .ends_with("/.famp/hook-runner.sh"));
    assert_eq!(hooks0[0]["timeout"], 30);

    // Entry 1: famp-await.sh, timeout 86400
    assert_eq!(stop[1]["matcher"], "");
    let hooks1 = stop[1]["hooks"].as_array().unwrap();
    assert_eq!(hooks1.len(), 1);
    assert_eq!(hooks1[0]["type"], "command");
    assert!(hooks1[0]["command"]
        .as_str()
        .unwrap()
        .ends_with("/.claude/hooks/famp-await.sh"));
    assert_eq!(hooks1[0]["timeout"], 86400);
}

#[test]
fn install_claude_code_is_idempotent() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::install::claude_code::run_at(home, &mut out, &mut err).unwrap();
    let claude_first = std::fs::read_to_string(home.join(".claude.json")).unwrap();
    let settings_first = std::fs::read_to_string(home.join(".claude/settings.json")).unwrap();
    std::thread::sleep(std::time::Duration::from_secs(1));
    let mut out2 = Vec::<u8>::new();
    let mut err2 = Vec::<u8>::new();
    famp::cli::install::claude_code::run_at(home, &mut out2, &mut err2).unwrap();
    assert_eq!(
        claude_first,
        std::fs::read_to_string(home.join(".claude.json")).unwrap()
    );
    assert_eq!(
        settings_first,
        std::fs::read_to_string(home.join(".claude/settings.json")).unwrap()
    );
}

#[test]
fn install_claude_code_preserves_unrelated_top_level_keys() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    std::fs::create_dir_all(home.join(".claude")).unwrap();
    std::fs::write(
        home.join(".claude.json"),
        r#"{"numStartups":42,"installMethod":"native","tipsHistory":["a","b","c"],"cachedDynamicConfigs":{"x":1},"mcpServers":{"github":{"command":"/usr/local/bin/github-mcp","args":["serve"]}}}"#,
    )
    .unwrap();
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::install::claude_code::run_at(home, &mut out, &mut err).unwrap();
    let post: Value =
        serde_json::from_str(&std::fs::read_to_string(home.join(".claude.json")).unwrap()).unwrap();
    assert_eq!(post["numStartups"], serde_json::json!(42));
    assert_eq!(post["installMethod"], "native");
    assert_eq!(post["tipsHistory"], serde_json::json!(["a", "b", "c"]));
    assert_eq!(post["cachedDynamicConfigs"]["x"], serde_json::json!(1));
    assert_eq!(
        post["mcpServers"]["github"]["command"],
        "/usr/local/bin/github-mcp"
    );
    assert!(post["mcpServers"]["famp"].is_object());
}

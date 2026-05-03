//! D-12: install-codex writes `[mcp_servers.famp]` table to
//! `~/.codex/config.toml` preserving every other section. MCP-only - no slash
//! commands, no hooks.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

#[test]
fn install_codex_writes_mcp_servers_famp_table_under_tempdir_home() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::install::codex::run_at(home, &mut out, &mut err).expect("install-codex happy path");

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
}

#[test]
fn install_codex_preserves_realistic_codex_config() {
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
        toml::from_str(&std::fs::read_to_string(home.join(".codex/config.toml")).unwrap()).unwrap();
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
}

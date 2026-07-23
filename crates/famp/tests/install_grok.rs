//! install-grok writes MCP, native Grok await shim, Stop hook json, and skill.
//! Does not touch ~/.claude/ (single Stop arming path — adversarial B2).

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

#[test]
fn install_grok_writes_mcp_servers_famp_table_under_tempdir_home() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::install::grok::run_at(home, &mut out, &mut err).expect("install-grok happy path");

    let cfg = home.join(".grok/config.toml");
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

    let skill = home.join(".grok/skills/famp-listen/SKILL.md");
    assert!(skill.exists(), "famp-listen skill missing");
    let skill_body = std::fs::read_to_string(&skill).unwrap();
    assert!(skill_body.contains("famp_register"));

    let grok_shim = home.join(".grok/hooks/famp-await.sh");
    assert!(grok_shim.exists(), "grok await shim missing");
    assert!(std::fs::read_to_string(&grok_shim)
        .unwrap()
        .contains("trying pid-correlated"));
    assert!(
        std::fs::read_to_string(&grok_shim)
            .unwrap()
            .contains("stop-await-locks"),
        "shim must singleton-lock Stop await"
    );

    let stop_json = home.join(".grok/hooks/famp-listen-stop.json");
    assert!(stop_json.exists(), "Stop hook json missing");
    let stop: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&stop_json).unwrap()).unwrap();
    let hooks = stop["hooks"]["Stop"][0]["hooks"].as_array().unwrap();
    assert_eq!(hooks[0]["timeout"], 86400);
    assert_eq!(
        hooks[0]["command"].as_str().unwrap(),
        grok_shim.display().to_string()
    );

    assert!(
        !home.join(".codex").exists(),
        "Grok install must not touch ~/.codex/"
    );
    assert!(
        !home.join(".claude").exists(),
        "Grok install must not touch ~/.claude/ (B2 single Stop path)"
    );
    assert!(
        !home.join(".famp/hook-runner.sh").exists(),
        "Grok install must not write the Claude hook-runner shim"
    );
}

#[test]
fn install_grok_preserves_unrelated_mcp_servers() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    std::fs::create_dir_all(home.join(".grok")).unwrap();
    std::fs::write(
        home.join(".grok/config.toml"),
        "[mcp_servers.github]\ncommand = \"/x\"\nargs = []\n",
    )
    .unwrap();

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::install::grok::run_at(home, &mut out, &mut err).unwrap();

    let parsed: toml::Table =
        toml::from_str(&std::fs::read_to_string(home.join(".grok/config.toml")).unwrap()).unwrap();
    assert_eq!(
        parsed["mcp_servers"]["github"]["command"].as_str().unwrap(),
        "/x"
    );
    assert!(parsed["mcp_servers"]["famp"].as_table().is_some());
}

#[test]
fn install_grok_is_idempotent() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::install::grok::run_at(home, &mut out, &mut err).unwrap();
    let first = std::fs::read_to_string(home.join(".grok/config.toml")).unwrap();

    std::thread::sleep(std::time::Duration::from_secs(1));

    let mut out2 = Vec::<u8>::new();
    let mut err2 = Vec::<u8>::new();
    famp::cli::install::grok::run_at(home, &mut out2, &mut err2).unwrap();
    let second = std::fs::read_to_string(home.join(".grok/config.toml")).unwrap();
    assert_eq!(first, second);
}

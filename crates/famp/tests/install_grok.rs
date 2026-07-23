//! install-grok writes `[mcp_servers.famp]` to `~/.grok/config.toml` and the
//! famp-listen skill, without touching Claude/Codex trees or installing a
//! long Stop hook.

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
    assert!(skill_body.contains("famp listen-wake"));
    assert!(skill_body.contains("persistent"));

    // Must not pollute Claude / Codex / shared Stop-hook paths.
    assert!(
        !home.join(".claude").exists(),
        "Grok install must not touch ~/.claude/"
    );
    assert!(
        !home.join(".codex").exists(),
        "Grok install must not touch ~/.codex/"
    );
    assert!(
        !home.join(".famp/hook-runner.sh").exists(),
        "Grok install must not write the Claude hook-runner shim"
    );
    assert!(
        !home.join(".claude/hooks/famp-await.sh").exists(),
        "Grok install must not install a long Stop await shim"
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

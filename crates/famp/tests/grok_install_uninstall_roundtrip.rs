//! install-grok -> uninstall-grok restores semantic equality on
//! `~/.grok/config.toml` and removes the famp-listen skill.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

/// Pin `FAMP_INSTALL_FAMP_BIN` at the cargo-built `famp` so the installer's
/// executable resolution is hermetic. Without this the resolver falls through
/// to `which famp`, and the test would pass or fail depending on whether the
/// host happens to have FAMP installed (CI does not). Same convention as
/// `install_codex.rs`; `temp_env` serializes the process-global env mutation
/// (WR-06).
fn with_pinned_famp_bin<T>(test: impl FnOnce() -> T) -> T {
    let bin = assert_cmd::cargo::cargo_bin("famp");
    temp_env::with_var(
        "FAMP_INSTALL_FAMP_BIN",
        Some(bin.to_string_lossy().into_owned()),
        test,
    )
}

const PRE: &str = r#"model = "grok-4"

[mcp_servers.github]
command = "/usr/local/bin/github-mcp"
args = ["serve"]
"#;

#[test]
fn grok_install_then_uninstall_restores_pre_state() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    std::fs::create_dir_all(home.join(".grok")).unwrap();
    std::fs::write(home.join(".grok/config.toml"), PRE).unwrap();

    let pre_parsed: toml::Table = toml::from_str(PRE).unwrap();

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    with_pinned_famp_bin(|| famp::cli::install::grok::run_at(home, &mut out, &mut err)).unwrap();

    assert!(home.join(".grok/skills/famp-listen/SKILL.md").exists());
    assert!(home.join(".grok/hooks/famp-listen-stop.json").exists());
    assert!(home.join(".grok/hooks/famp-await.sh").exists());

    let mut out2 = Vec::<u8>::new();
    let mut err2 = Vec::<u8>::new();
    famp::cli::uninstall::grok::run_at(home, &mut out2, &mut err2).unwrap();

    let post_parsed: toml::Table =
        toml::from_str(&std::fs::read_to_string(home.join(".grok/config.toml")).unwrap()).unwrap();
    assert_eq!(
        pre_parsed, post_parsed,
        "config.toml drifted across roundtrip\nPRE: {pre_parsed:#?}\nPOST: {post_parsed:#?}"
    );
    assert!(!home.join(".grok/skills/famp-listen").exists());
    assert!(!home.join(".grok/hooks/famp-listen-stop.json").exists());
    assert!(!home.join(".grok/hooks/famp-await.sh").exists());
}

#[test]
fn grok_install_then_uninstall_on_empty_home_leaves_clean_state() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    with_pinned_famp_bin(|| famp::cli::install::grok::run_at(home, &mut out, &mut err)).unwrap();

    let mut out2 = Vec::<u8>::new();
    let mut err2 = Vec::<u8>::new();
    famp::cli::uninstall::grok::run_at(home, &mut out2, &mut err2).unwrap();

    let cfg = home.join(".grok/config.toml");
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
    assert!(!home.join(".grok/skills/famp-listen").exists());
    assert!(!home.join(".grok/hooks/famp-listen-stop.json").exists());
    assert!(!home.join(".grok/hooks/famp-await.sh").exists());
}

//! CC-10: docs/ONBOARDING.md is <=80 lines (D-13 minimal-scope invariant).

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use std::path::PathBuf;

fn onboarding_path() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("docs/ONBOARDING.md")
}

#[test]
fn onboarding_md_exists() {
    let path = onboarding_path();
    assert!(path.exists(), "docs/ONBOARDING.md missing (CC-10)");
}

#[test]
fn onboarding_md_is_at_most_80_lines() {
    let path = onboarding_path();
    let body = std::fs::read_to_string(&path).unwrap();
    let lines = body.lines().count();
    assert!(
        lines <= 80,
        "docs/ONBOARDING.md has {lines} lines (D-13 cap: 80)"
    );
}

#[test]
fn onboarding_md_has_three_required_sections() {
    let path = onboarding_path();
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains("## Install"), "Install section missing");
    assert!(
        body.contains("## Other clients"),
        "Other clients section missing"
    );
    assert!(body.contains("## Uninstall"), "Uninstall section missing");
}

#[test]
fn onboarding_md_includes_install_codex_one_liner() {
    let path = onboarding_path();
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains("famp install-codex"));
    assert!(body.contains("famp uninstall-codex"));
    assert!(body.contains("famp install-claude-code"));
    assert!(body.contains("famp uninstall-claude-code"));
}

#[test]
fn onboarding_md_excludes_d13_out_items() {
    let path = onboarding_path();
    let body = std::fs::read_to_string(&path).unwrap();
    // D-13 OUT items: troubleshooting deep-dive, hooks deep-dive, channels deep-dive.
    assert!(
        !body.to_lowercase().contains("## troubleshooting"),
        "ONBOARDING includes a Troubleshooting section (D-13 OUT)"
    );
    assert!(
        !body.to_lowercase().contains("## hooks"),
        "ONBOARDING includes a Hooks deep-dive section (D-13 OUT)"
    );
    assert!(
        !body.to_lowercase().contains("## channels"),
        "ONBOARDING includes a Channels deep-dive section (D-13 OUT)"
    );
}

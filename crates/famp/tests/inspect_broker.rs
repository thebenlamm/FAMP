//! Integration tests for `famp inspect broker`.
//! Wave 2: compile-time scaffolding + --help smoke.
//! Wave 3 will land HEALTHY/down-state integration tests.
#![allow(unused_crate_dependencies)]

use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn inspect_broker_help_lists_json_flag() {
    let out = Command::cargo_bin("famp")
        .unwrap()
        .args(["inspect", "broker", "--help"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("--json"),
        "stdout did not contain --json: {stdout}"
    );
}

#[test]
fn inspect_help_lists_broker_and_identities_only() {
    let out = Command::cargo_bin("famp")
        .unwrap()
        .args(["inspect", "--help"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("broker"),
        "missing broker subcommand: {stdout}"
    );
    assert!(
        stdout.contains("identities"),
        "missing identities subcommand: {stdout}"
    );
    assert!(
        !stdout.contains("\n  tasks"),
        "D-06 violation: tasks subcommand surfaced: {stdout}"
    );
    assert!(
        !stdout.contains("\n  messages"),
        "D-06 violation: messages subcommand surfaced: {stdout}"
    );
}

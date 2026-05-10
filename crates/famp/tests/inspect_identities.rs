//! Integration tests for `famp inspect identities`.
//! Wave 2: --help smoke. Wave 3 lands the rendering tests.
#![allow(unused_crate_dependencies)]

use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn inspect_identities_help_lists_json_flag() {
    let out = Command::cargo_bin("famp")
        .unwrap()
        .args(["inspect", "identities", "--help"])
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

//! Invoke the `personal_two_agents` example as a subprocess and assert
//! exit-code 0 + expected trace lines. CI gate for EX-01 + CONF-03.

#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use std::process::Command;

#[test]
fn personal_two_agents_exits_zero_with_expected_trace() {
    let output = Command::new(env!("CARGO"))
        .args([
            "run",
            "--quiet",
            "-p",
            "famp",
            "--example",
            "personal_two_agents",
        ])
        .output()
        .expect("failed to invoke cargo run");
    assert!(
        output.status.success(),
        "example exited non-zero: stderr={}",
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[1]"), "missing trace line 1: {stdout}");
    assert!(stdout.contains("[2]"), "missing trace line 2: {stdout}");
    assert!(stdout.contains("[3]"), "missing trace line 3: {stdout}");
    assert!(stdout.contains("[4]"), "missing trace line 4: {stdout}");
    assert!(stdout.contains("Request"), "missing Request in trace");
    assert!(stdout.contains("Commit"), "missing Commit in trace");
    assert!(stdout.contains("Deliver"), "missing Deliver in trace");
    assert!(stdout.contains("Ack"), "missing Ack in trace");
    assert!(stdout.contains("OK: personal_two_agents complete"));
}

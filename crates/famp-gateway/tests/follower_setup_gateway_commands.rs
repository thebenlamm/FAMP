//! D3 (gateway-binary half): executes the `docs/FOLLOWER-SETUP.md` section 1
//! command that probes `famp-gateway`, and proves from the binary that the
//! rejected `--help` invocation is a real failure, not a hypothetical one.
//!
//! Split from `crates/famp/tests/follower_setup_doc_accuracy.rs` by crate
//! ownership: a test invokes a binary only from that binary's own crate.
//! `Command::cargo_bin("famp-gateway")` from `crates/famp/tests/` would not
//! build under `cargo test -p famp`.

#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::process::Command;

use assert_cmd::cargo::CommandCargoExt;

fn guide_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/FOLLOWER-SETUP.md")
}

/// Pure helper over `&str`: does the text contain the binary name followed
/// by a space and the help flag, as a contiguous string? Never reads the
/// file itself, so the red-path test can feed it an in-memory mutated copy.
fn documents_broken_help_invocation(text: &str) -> bool {
    text.contains("famp-gateway --help")
}

#[test]
fn documented_path_presence_check_executes_and_exits_zero() {
    let guide = std::fs::read_to_string(guide_path()).expect("guide must be readable");
    assert!(
        guide.contains("command -v famp-gateway"),
        "guide no longer documents `command -v famp-gateway`; this test would be \
         checking something the guide no longer says"
    );

    let gateway_cmd = Command::cargo_bin("famp-gateway").expect("famp-gateway must build");
    let bin_path = PathBuf::from(gateway_cmd.get_program());
    let bin_dir = bin_path
        .parent()
        .expect("built binary's path must have a parent directory")
        .to_owned();

    let output = Command::new("/bin/sh")
        .arg("-c")
        .arg("command -v famp-gateway")
        .env_clear()
        .env("PATH", &bin_dir)
        .output()
        .expect("/bin/sh must be runnable");

    assert!(
        output.status.success(),
        "`command -v famp-gateway` must exit 0 with PATH pinned to the built binary's \
         directory ({bin_dir:?}); got {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("famp-gateway"),
        "`command -v famp-gateway` stdout must name the binary; got: {stdout}"
    );
}

#[test]
fn help_flag_invocation_fails_and_is_not_documented() {
    let output = Command::cargo_bin("famp-gateway")
        .expect("famp-gateway must build")
        .arg("--help")
        .output()
        .expect("famp-gateway must be runnable as a subprocess");

    assert!(
        !output.status.success(),
        "famp-gateway --help must exit non-zero; got {:?}",
        output.status
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unrecognized argument"),
        "expected `famp-gateway --help` to report an unrecognized argument -- this \
         proves the negative check below guards a real failure, not a hypothetical \
         one; got stderr:\n{stderr}"
    );

    let guide = std::fs::read_to_string(guide_path()).expect("guide must be readable");
    assert!(
        !documents_broken_help_invocation(&guide),
        "D1 regression: docs/FOLLOWER-SETUP.md documents `famp-gateway --help`, \
         which the binary invocation above just proved exits non-zero"
    );
}

#[test]
fn red_path_trips_on_prerepair_invocation() {
    let guide = std::fs::read_to_string(guide_path()).expect("guide must be readable");
    // Restore D1's pre-repair line, in memory only -- no working-tree
    // mutation and no restore step.
    let mutated = guide.replacen("command -v famp-gateway", "famp-gateway --help", 1);
    assert_ne!(
        guide, mutated,
        "mutation must actually change the guide, or this red-path test proves nothing"
    );
    assert!(
        documents_broken_help_invocation(&mutated),
        "red path failed to trip: the mutated guide should document the broken \
         help invocation and did not"
    );
}

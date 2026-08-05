//! Semantic accuracy gate for the Phase 20 follower setup guide.

#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

use std::path::PathBuf;
use std::process::Command;

use assert_cmd::cargo::CommandCargoExt;
use famp::pairing::PairingError;

fn root_file(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn help(args: &[&str]) -> String {
    let output = Command::cargo_bin("famp")
        .unwrap()
        .args(args)
        .output()
        .expect("shipping famp help must run");
    assert!(
        output.status.success(),
        "help failed for {args:?}: {output:?}"
    );
    String::from_utf8(output.stdout).unwrap()
}

fn before(doc: &str, first: &str, second: &str) {
    let a = doc
        .find(first)
        .unwrap_or_else(|| panic!("missing anchor: {first}"));
    let b = doc
        .find(second)
        .unwrap_or_else(|| panic!("missing anchor: {second}"));
    assert!(a < b, "expected `{first}` before `{second}`");
}

#[test]
fn follower_setup_is_ordered_and_matches_shipping_surfaces() {
    let doc = std::fs::read_to_string(root_file("docs/FOLLOWER-SETUP.md")).unwrap();
    let readme = std::fs::read_to_string(root_file("README.md")).unwrap();
    let normalized = doc.split_whitespace().collect::<Vec<_>>().join(" ");

    assert!(readme.contains("[Follower Setup](docs/FOLLOWER-SETUP.md)"));
    for args in [
        &["pair", "invite", "--help"][..],
        &["pair", "redeem", "--help"][..],
        &["pair", "status", "--help"][..],
        &["send", "--help"][..],
        &["inbox", "list", "--help"][..],
        &["inspect", "tasks", "--help"][..],
    ] {
        let output = help(args);
        assert!(output.contains("Usage:"), "missing clap usage for {args:?}");
    }

    before(
        &normalized,
        "latest/download/famp-installer.sh",
        "famp pair invite",
    );
    before(
        &normalized,
        "same trust as someone at your terminal",
        "famp pair redeem --from",
    );
    before(&normalized, "famp pair redeem --from", "famp pair status");
    before(&normalized, "famp pair status", "famp daemon restart");
    before(
        &normalized,
        "famp daemon restart",
        "phase20-ben-to-follower",
    );
    before(
        &normalized,
        "phase20-ben-to-follower",
        "famp inbox list --as <follower-name>",
    );
    before(
        &normalized,
        "phase20-follower-to-ben",
        "famp inbox list --as <ben-name>",
    );

    for required in [
        "Ben is the **inviter**",
        "**follower/redeemer**",
        "local acceptance only",
        "famp send --as <follower-name> --to agent:<ben-domain>/<ben-name> --task <ben-to-follower-task-id> --body <result> --terminal",
        "famp send --as <ben-name> --to agent:<follower-domain>/<follower-name> --task <follower-to-ben-task-id> --body <result> --terminal",
        "famp inspect tasks --id <ben-to-follower-task-id> --json",
        "famp inspect tasks --id <follower-to-ben-task-id> --json",
        "famp_inbox",
        "`famp_send` in `reply` mode",
        "COMPLETED",
        "FAILED",
        "CANCELLED",
        "human judgment remains open",
    ] {
        assert!(doc.contains(required), "missing semantic anchor: {required}");
    }

    for forbidden in [
        "famp peer export",
        "famp peer import",
        "sender's exit 0 proves",
        "Phase 21",
        "automatic remote wake",
    ] {
        assert!(
            !doc.contains(forbidden),
            "forbidden legacy/overclaim anchor: {forbidden}"
        );
    }
    assert!(doc.contains("Do not use a shared VPN"));
    assert!(doc.contains("does not auto-wake"));
}

#[test]
fn seven_pairing_messages_are_synchronized_but_do_not_claim_comprehension() {
    let doc = std::fs::read_to_string(root_file("docs/FOLLOWER-SETUP.md")).unwrap();
    let messages = [
        PairingError::CodeMalformed {
            reason: String::new(),
        }
        .to_string(),
        PairingError::Expired.to_string(),
        PairingError::AlreadyRedeemed.to_string(),
        PairingError::AttemptsExhausted.to_string(),
        PairingError::WrongCode.to_string(),
        PairingError::GatewayUnreachable {
            url: "{url}".into(),
        }
        .to_string(),
        PairingError::SameMachineRefusal.to_string(),
    ];
    for message in messages {
        assert!(
            doc.contains(&message),
            "guide drifted from PairingError: {message}"
        );
    }
    assert!(doc.contains("does **not** measure comprehension or close PAIR-05"));
}

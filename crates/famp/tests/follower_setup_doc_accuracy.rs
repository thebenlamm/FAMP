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

/// Forbidden legacy/overclaim literals for `docs/FOLLOWER-SETUP.md`. Shared
/// between the spelling-level gate below and the D3 red-path control test,
/// which demonstrates that this list alone could never have caught D1 (a
/// spelling-correct, non-zero-exit command).
const FORBIDDEN_LITERALS: &[&str] = &[
    "famp peer export",
    "famp peer import",
    "sender's exit 0 proves",
    "Phase 21",
    "automatic remote wake",
];

/// D3: commands documented in section 1's fenced install/verify block that
/// this gate actually executes against the shipping `famp` binary, paired
/// with the argv to pass. Exact string match on the trimmed documented
/// line — any edit to a section 1 command forces a deliberate update here
/// instead of drifting silently (mirrors `install_docs_accuracy.rs`'s
/// pinned-count precedent).
const EXECUTED_VERIFICATIONS: &[(&str, &[&str])] = &[("famp --version", &["--version"])];

/// D3: commands documented in section 1 that are NOT executed here, paired
/// with the reason. A command present in the block but absent from both
/// this list and `EXECUTED_VERIFICATIONS` fails the suite (see
/// `classify_section1_commands`).
const NON_HERMETIC: &[(&str, &str)] = &[
    (
        "curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-installer.sh | sh",
        "needs network access and writes to ~/.cargo/bin",
    ),
    (
        "curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-gateway-installer.sh | sh",
        "needs network access and writes to ~/.cargo/bin",
    ),
    (
        "command -v famp-gateway",
        "a PATH presence check whose execution lives in crates/famp-gateway/tests/follower_setup_gateway_commands.rs",
    ),
    (
        "famp daemon install",
        "installs a real launchd or systemd --user service on the developer's machine -- must never be executed by a test",
    ),
];

/// Extract the non-blank, trimmed lines of the first fenced code block in
/// `doc` whose contents include a line mentioning `famp-installer.sh` --
/// section 1's install/verify block. Pure over `&str`, never reads the
/// file itself, so both the green-path and red-path tests can feed it an
/// in-memory mutated copy.
fn section1_lines(doc: &str) -> Vec<String> {
    let mut in_block = false;
    let mut block_lines: Vec<String> = Vec::new();
    let mut found: Option<Vec<String>> = None;

    for line in doc.lines() {
        if line.trim_start().starts_with("```") {
            if in_block {
                if found.is_none() && block_lines.iter().any(|l| l.contains("famp-installer.sh")) {
                    found = Some(block_lines.clone());
                }
                block_lines.clear();
            }
            in_block = !in_block;
            continue;
        }
        if in_block {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                block_lines.push(trimmed.to_owned());
            }
        }
    }

    found.unwrap_or_default()
}

/// Pure classifier over `&str`: every non-blank line of section 1's fenced
/// block must be either an `EXECUTED_VERIFICATIONS` entry or a
/// `NON_HERMETIC` entry. Returns the first unclassified line as `Err`, or
/// `Ok(())` if every line is accounted for. This is the exact hole D1
/// slipped through: a spelling-correct, non-zero-exit command that no
/// prior gate executed.
fn classify_section1_commands(doc: &str) -> Result<(), String> {
    for line in section1_lines(doc) {
        let is_executed = EXECUTED_VERIFICATIONS.iter().any(|(cmd, _)| *cmd == line);
        let is_non_hermetic = NON_HERMETIC.iter().any(|(cmd, _)| *cmd == line);
        if !is_executed && !is_non_hermetic {
            return Err(line);
        }
    }
    Ok(())
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

    for forbidden in FORBIDDEN_LITERALS {
        assert!(
            !doc.contains(forbidden),
            "forbidden legacy/overclaim anchor: {forbidden}"
        );
    }
    assert!(doc.contains("Do not use a shared VPN"));
    assert!(doc.contains("does not auto-wake"));
}

#[test]
#[allow(clippy::const_is_empty)] // guards against a future accidental empty
                                 // const, not a currently-reachable case
fn section1_commands_execute_or_are_classified_non_hermetic() {
    assert!(
        !EXECUTED_VERIFICATIONS.is_empty(),
        "EXECUTED_VERIFICATIONS must not be empty -- an always-non-hermetic \
         classification list would make this gate vacuous"
    );

    let doc = std::fs::read_to_string(root_file("docs/FOLLOWER-SETUP.md")).unwrap();

    classify_section1_commands(&doc)
        .unwrap_or_else(|line| panic!("unclassified section 1 command: {line}"));

    let lines = section1_lines(&doc);
    for (cmd, _) in EXECUTED_VERIFICATIONS {
        assert!(
            lines.iter().any(|l| l == cmd),
            "EXECUTED_VERIFICATIONS entry not present in section 1's block: {cmd}"
        );
    }
    for (cmd, _) in NON_HERMETIC {
        assert!(
            lines.iter().any(|l| l == cmd),
            "NON_HERMETIC entry not present in section 1's block: {cmd}"
        );
    }

    for (cmd, argv) in EXECUTED_VERIFICATIONS {
        let output = Command::cargo_bin("famp")
            .unwrap()
            .args(*argv)
            .output()
            .unwrap_or_else(|e| panic!("failed to run documented command `{cmd}`: {e}"));
        assert!(
            output.status.success(),
            "documented command `{cmd}` exited non-zero -- this is the exact D1 hole: \
             a spelling-correct command that fails; status: {:?}, stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !output.stdout.is_empty(),
            "documented command `{cmd}` produced no stdout"
        );
    }
}

#[test]
fn section1_red_path_trips_on_prerepair_help_invocation() {
    let doc = std::fs::read_to_string(root_file("docs/FOLLOWER-SETUP.md")).unwrap();
    // Restore D1's pre-repair line, in memory only -- no working-tree
    // mutation and no restore step.
    let mutated = doc.replacen("command -v famp-gateway", "famp-gateway --help", 1);
    assert_ne!(
        doc, mutated,
        "mutation must actually change the doc, or this red-path test proves nothing"
    );

    let err = classify_section1_commands(&mutated)
        .expect_err("classifier must reject the restored pre-repair line");
    assert_eq!(
        err, "famp-gateway --help",
        "classifier must name the restored pre-repair line as the unclassified command"
    );

    // Control: the mutated copy must still be spelling-clean against the
    // pre-existing forbidden-literal gate. This demonstrates in code the
    // defect log's central claim -- D1 was invisible to flag-spelling
    // checking. If this assertion ever fails, the red-path experiment
    // above is contaminated and proves nothing.
    for forbidden in FORBIDDEN_LITERALS {
        assert!(
            !mutated.contains(forbidden),
            "control failed: mutated copy contains forbidden literal `{forbidden}` -- \
             the red-path experiment above is contaminated and proves nothing"
        );
    }
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

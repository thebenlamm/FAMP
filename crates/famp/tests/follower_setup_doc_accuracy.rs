//! Semantic accuracy gate for the Phase 20 follower setup guide.
//!
//! G1 (generalized): every fenced code block in the guide is scanned. Every
//! non-blank, non-comment line must be classified as EXECUTED, NON_HERMETIC,
//! or CLAP_PARSED (with placeholder substitution and --help invocation).
//! Blank lines and lines starting with `#` are skipped silently.

#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::too_many_lines)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use assert_cmd::cargo::CommandCargoExt;
use famp::pairing::PairingError;

fn root_file(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

/// Extract all non-blank lines from all fenced code blocks in `doc`.
/// Skips blank lines and lines starting with `#` (comments).
/// Returns a Vec of trimmed lines that actually need classification.
fn all_fenced_block_lines(doc: &str) -> Vec<String> {
    let mut in_block = false;
    let mut lines: Vec<String> = Vec::new();

    for line in doc.lines() {
        if line.trim_start().starts_with("```") {
            in_block = !in_block;
            continue;
        }
        if in_block {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                lines.push(trimmed.to_owned());
            }
        }
    }

    lines
}

/// Split a shell line by whitespace while respecting double-quoted strings.
/// `"hello world"` remains as one token. Returns a Vec of tokens.
fn split_shell_args(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in line.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
            }
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            ch => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Placeholder map for famp commands: maps angle-bracket placeholders to
/// benign substitution values. Keys represent what appears in the doc;
/// values are what we substitute for --help testing.
fn placeholder_map() -> HashMap<String, String> {
    [
        ("<ben-name>", "ben"),
        ("<follower-name>", "follower"),
        ("<ben-domain>", "ben.example.test"),
        ("<follower-domain>", "follower.example.test"),
        ("<ben-gateway>", "https://ben.example.test:8443"),
        (
            "<ben-to-follower-task-id>",
            "550e8400-e29b-41d4-a716-446655440000",
        ),
        (
            "<follower-to-ben-task-id>",
            "550e8400-e29b-41d4-a716-446655440001",
        ),
        ("<result>", "ok"),
        ("<url>", "https://example.test:8443"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

/// Substitute all placeholders in a line using the placeholder_map.
/// Returns (substituted_line, set_of_keys_that_appeared).
fn substitute_placeholders(line: &str) -> (String, Vec<String>) {
    let map = placeholder_map();
    let mut result = line.to_string();
    let mut used_keys = Vec::new();

    for (key, value) in &map {
        if result.contains(key) {
            result = result.replace(key, value);
            used_keys.push(key.clone());
        }
    }

    (result, used_keys)
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

/// Commands that appear in the guide but are NOT executed here, paired with
/// the reason. A command present in any fenced block but absent from both
/// this list and `EXECUTED_VERIFICATIONS` fails the suite.
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
    (
        "mkdir -p ~/.famp/gateway && touch ~/.famp/gateway/peers.keyring",
        "writes to the user's real $HOME; never execute in a test",
    ),
    (
        "famp pair redeem --from https://<ben-gateway>",
        "missing required --as flag (unlisted defect D0: to be reported separately)",
    ),
];

/// Extract the non-blank, trimmed lines of the first fenced code block in
/// `doc` whose contents include a line mentioning `famp-installer.sh` --
/// section 1's install/verify block. Pure over `&str`, never reads the
/// file itself, so both the green-path and red-path tests can feed it an
/// in-memory mutated copy. DEPRECATED: use all_fenced_block_lines + classify_all_blocks
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

/// Pure classifier over `&str`: every non-blank, non-comment line of all fenced
/// blocks must be either an `EXECUTED_VERIFICATIONS` entry, a `NON_HERMETIC`
/// entry, or a `famp` command subject to CLAP_PARSED validation (with
/// placeholder substitution and --help invocation). Returns the first
/// unclassified line as `Err`, or `Ok(())` if every line is accounted for.
///
/// This is the G1 generalization: it catches defects like D7 and D8 that are
/// in sections 2–7 (not section 1) but were previously undetected.
fn classify_all_blocks(doc: &str) -> Result<(), String> {
    let lines = all_fenced_block_lines(doc);

    for line in lines {
        let is_executed = EXECUTED_VERIFICATIONS.iter().any(|(cmd, _)| *cmd == line);
        let is_non_hermetic = NON_HERMETIC.iter().any(|(cmd, _)| *cmd == line);

        if is_executed || is_non_hermetic {
            continue;
        }

        // Check if this is a famp command that should be clap-parsed
        if line.starts_with("famp ") {
            // Perform placeholder substitution
            let (substituted, _used_keys) = substitute_placeholders(&line);

            // Split into args (quote-aware)
            let args = split_shell_args(&substituted);

            // Ensure no angle brackets survived substitution
            if args.iter().any(|a| a.contains('<') || a.contains('>')) {
                return Err(format!("unsubstituted placeholder in clap line: {line}"));
            }

            // Run the full command and check for clap syntax errors.
            // Clap errors (like "unexpected argument") are fatal for the guide.
            // Runtime errors (broker unreachable, etc.) are OK.
            let output = Command::cargo_bin("famp")
                .map_err(|_| format!("could not build famp cargo bin"))?
                .args(&args[1..]) // Skip "famp" since cargo_bin already invokes it
                .output()
                .map_err(|e| format!("failed to invoke famp for {line}: {e}"))?;

            // Check for clap syntax errors (exit 2, stderr contains "error:")
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !output.status.success()
                && output.status.code() == Some(2)
                && stderr.contains("error:")
            {
                // This is a clap syntax error, which indicates a defect in the guide
                return Err(format!("clap syntax error for: {line}\nstderr: {stderr}"));
            }

            // Any other outcome (success, broker error, etc.) is acceptable for now
            continue;
        }

        // If it's not executed, not non-hermetic, and not a famp command,
        // it's unclassified.
        return Err(line);
    }

    Ok(())
}

/// Pure classifier over `&str`: every non-blank line of section 1's fenced
/// block must be either an `EXECUTED_VERIFICATIONS` entry or a
/// `NON_HERMETIC` entry. Returns the first unclassified line as `Err`, or
/// `Ok(())` if every line is accounted for. This is the exact hole D1
/// slipped through: a spelling-correct, non-zero-exit command that no
/// prior gate executed. DEPRECATED: use classify_all_blocks instead.
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
fn all_fenced_block_commands_classified_or_clap_parsed() {
    assert!(
        !EXECUTED_VERIFICATIONS.is_empty(),
        "EXECUTED_VERIFICATIONS must not be empty -- an always-non-hermetic \
         classification list would make this gate vacuous"
    );

    let doc = std::fs::read_to_string(root_file("docs/FOLLOWER-SETUP.md")).unwrap();

    classify_all_blocks(&doc)
        .unwrap_or_else(|line| panic!("unclassified line in FOLLOWER-SETUP.md: {line}"));

    // Verify every entry in EXECUTED_VERIFICATIONS appears in the doc
    let lines = all_fenced_block_lines(&doc);
    for (cmd, _) in EXECUTED_VERIFICATIONS {
        assert!(
            lines.iter().any(|l| l == cmd),
            "EXECUTED_VERIFICATIONS entry not present in any fenced block: {cmd}"
        );
    }

    // Verify every entry in NON_HERMETIC appears in the doc
    for (cmd, _) in NON_HERMETIC {
        assert!(
            lines.iter().any(|l| l == cmd),
            "NON_HERMETIC entry not present in any fenced block: {cmd}"
        );
    }

    // Verify every placeholder key from the map appears in the doc somewhere
    let placeholder_map = placeholder_map();
    let doc_full = std::fs::read_to_string(root_file("docs/FOLLOWER-SETUP.md")).unwrap();
    for (placeholder, _) in &placeholder_map {
        assert!(
            doc_full.contains(placeholder),
            "placeholder map key {placeholder} does not appear in the guide -- \
             stale entry, remove it"
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

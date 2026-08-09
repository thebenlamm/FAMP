//! Contract tests for blank Phase 20 ledgers and their fail-closed validator.

#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use famp::pairing::PairingError;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn root(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn validate(mode: &str, body: &str) -> bool {
    let tmp = TempDir::new().unwrap();
    let record = tmp.path().join("record.md");
    fs::write(&record, body).unwrap();
    Command::new("sh")
        .arg(root("scripts/phase20-evidence-check.sh"))
        .arg(mode)
        .arg(record)
        .status()
        .unwrap()
        .success()
}

fn rehearsal_blank_candidate() -> String {
    let rows = vec![
        "outcome=unresolved",
        "redaction_review=<REQUIRED>",
        "redaction_findings=<REQUIRED>",
        "clean_preflight=<REQUIRED>",
        "clean_owner=<REQUIRED>",
        "clean_utc=<REQUIRED>",
        "clean_os_arch=<REQUIRED>",
        "release_famp_version=<REQUIRED>",
        "release_gateway_version=<REQUIRED>",
        "pairing_ready=<REQUIRED>",
        "task_a_id=<REQUIRED>",
        "task_a_owner=<REQUIRED>",
        "task_a_utc=<REQUIRED>",
        "task_a_state=<REQUIRED>",
        "task_b_id=<REQUIRED>",
        "task_b_owner=<REQUIRED>",
        "task_b_utc=<REQUIRED>",
        "task_b_state=<REQUIRED>",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    rows.join("\n") + "\n"
}

fn rehearsal_complete_pass() -> String {
    let rows = vec![
        "outcome=pass",
        "redaction_review=pass",
        "redaction_findings=none",
        "clean_preflight=REDACTED:signal-OK",
        "clean_owner=TestOperator",
        "clean_utc=2030-01-02T03:04:05Z",
        "clean_os_arch=REDACTED:Linux-x86_64",
        "release_famp_version=1.1.0-rc.1",
        "release_gateway_version=1.1.0-rc.1",
        "pairing_ready=yes",
        "task_a_id=aaaaaaaa-aaaa-7aaa-aaaa-aaaaaaaaaaaa",
        "task_a_owner=TestFollower",
        "task_a_utc=2030-01-02T03:05:05Z",
        "task_a_state=COMPLETED",
        "task_b_id=bbbbbbbb-bbbb-7bbb-bbbb-bbbbbbbbbbbb",
        "task_b_owner=TestOperator",
        "task_b_utc=2030-01-02T03:06:05Z",
        "task_b_state=FAILED",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    rows.join("\n") + "\n"
}

fn complete_acceptance() -> String {
    let mut rows = vec![
        "outcome=pass",
        "redaction_review=pass",
        "redaction_findings=none",
        "independent_machines=yes",
        "different_networks=yes",
        "shared_vpn=no",
        "copied_keys=no",
        "question_log=none",
        "no_coaching=yes",
        "guide_commit=0123456789abcdef",
        "guide_digest=sha256:abcdef0123456789",
        "ben_owner=Ben",
        "ben_utc=2030-01-02T03:04:05Z",
        "ben_os_arch=REDACTED:Linux-x86_64",
        "ben_famp_version=1.1.0-rc.1",
        "ben_gateway_version=1.1.0-rc.1",
        "follower_owner=Follower",
        "follower_utc=2030-01-02T03:05:05Z",
        "follower_os_arch=REDACTED:macOS-arm64",
        "follower_famp_version=1.1.0-rc.1",
        "follower_gateway_version=1.1.0-rc.1",
        "task_a_id=aaaaaaaa-aaaa-7aaa-aaaa-aaaaaaaaaaaa",
        "task_a_owner=Follower",
        "task_a_utc=2030-01-02T03:10:05Z",
        "task_a_state=COMPLETED",
        "task_b_id=bbbbbbbb-bbbb-7bbb-bbbb-bbbbbbbbbbbb",
        "task_b_owner=Ben",
        "task_b_utc=2030-01-02T03:12:05Z",
        "task_b_state=FAILED",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    for n in 1..=7 {
        rows.push(format!("message_{n}_text=SHIPPED_MESSAGE_{n}"));
        rows.push(format!("message_{n}_owner=Follower"));
        rows.push(format!("message_{n}_utc=2030-01-02T03:{n:02}:05Z"));
        rows.push(format!("message_{n}_first_paraphrase=REDACTED:action-{n}"));
        rows.push(format!("message_{n}_judgment=actionable"));
    }
    rows.join("\n") + "\n"
}

#[test]
fn blank_templates_are_visible_and_rejected() {
    for (mode, path) in [
        (
            "rehearsal",
            ".planning/phases/20-human-acceptance-gate/20-REHEARSAL-TEMPLATE.md",
        ),
        (
            "acceptance",
            ".planning/phases/20-human-acceptance-gate/20-ACCEPTANCE-TEMPLATE.md",
        ),
    ] {
        let body = fs::read_to_string(root(path)).unwrap();
        assert!(body.contains("<REQUIRED>"));
        assert!(body.contains("outcome=unresolved"));
        assert!(!validate(mode, &body));
    }
}

#[test]
fn complete_acceptance_passes_and_required_failures_fail_closed() {
    let complete = complete_acceptance();
    assert!(validate("acceptance", &complete));
    for broken in [
        complete.replace("follower_owner=Follower\n", ""),
        complete.replace(
            "follower_gateway_version=1.1.0-rc.1",
            "follower_gateway_version=<REQUIRED>",
        ),
        complete.replace(
            "task_b_id=bbbbbbbb-bbbb-7bbb-bbbb-bbbbbbbbbbbb",
            "task_b_id=aaaaaaaa-aaaa-7aaa-aaaa-aaaaaaaaaaaa",
        ),
        complete.replace("task_a_state=COMPLETED", "task_a_state=COMMITTED"),
        complete.replace("outcome=pass", "outcome=unknown"),
        format!("outcome=invalid\n{complete}"),
        complete.replace(
            "redaction_findings=none",
            "redaction_findings=/home/alice/.famp/private_key",
        ),
        complete.replace("message_7_first_paraphrase=REDACTED:action-7\n", ""),
        complete.replace("question_log=none", "question_log=<REQUIRED>"),
    ] {
        assert!(
            !validate("acceptance", &broken),
            "validator accepted broken record:\n{broken}"
        );
    }
}

#[test]
fn templates_encode_owner_time_machine_and_comprehension_contracts() {
    let acceptance = fs::read_to_string(root(
        ".planning/phases/20-human-acceptance-gate/20-ACCEPTANCE-TEMPLATE.md",
    ))
    .unwrap();
    for anchor in [
        "ben_owner=",
        "ben_utc=",
        "ben_os_arch=",
        "ben_famp_version=",
        "ben_gateway_version=",
        "follower_owner=",
        "follower_utc=",
        "follower_os_arch=",
        "follower_famp_version=",
        "follower_gateway_version=",
        "question_log=",
        "no_coaching=",
        "guide_commit=",
        "guide_digest=",
    ] {
        assert!(acceptance.contains(anchor), "missing {anchor}");
    }
    for n in 1..=7 {
        assert!(acceptance.contains(&format!("message_{n}_text=")));
        assert!(acceptance.contains(&format!("message_{n}_first_paraphrase=")));
        assert!(acceptance.contains(&format!("message_{n}_judgment=")));
    }
    for message in [
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
    ] {
        assert!(
            acceptance.contains(&message),
            "template drifted from PairingError: {message}"
        );
    }
}

#[test]
fn rehearsal_candidate_stage_aware_invariants() {
    // The rehearsal candidate's validity depends on which phase of population it is in.
    // This test proves the stage-aware invariant: each outcome class has the validator
    // behavior and structural properties required for that phase, making the gate
    // satisfiable at every checkpoint.

    let rehearsal_path = root(".planning/phases/20-human-acceptance-gate/20-REHEARSAL.md");
    assert!(
        rehearsal_path.exists(),
        "Phase 20-02 Task 1 must create the rehearsal candidate"
    );

    // Stage 1: blank candidate with outcome=unresolved and <REQUIRED> placeholders.
    // This MUST exist at Task 1 completion and MUST fail the validator.
    let blank = rehearsal_blank_candidate();
    assert!(
        blank.contains("outcome=unresolved"),
        "test helper: blank has unresolved"
    );
    assert!(
        blank.contains("<REQUIRED>"),
        "test helper: blank has placeholders"
    );
    assert!(!validate("rehearsal", &blank),
        "Branch 1 FAILED: blank candidate with outcome=unresolved + <REQUIRED> must be rejected.\n\
         Expected validator to fail, but it passed.\n\
         This is the state at 20-02 Task 1 completion — it MUST fail because fields are incomplete.");

    // Stage 2: stripped placeholders but outcome still unresolved.
    // This is a half-filled fake and MUST fail the validator.
    let half_filled = blank
        .replace(
            "clean_preflight=<REQUIRED>",
            "clean_preflight=REDACTED:signal-OK",
        )
        .replace("clean_owner=<REQUIRED>", "clean_owner=TestOperator")
        .replace("clean_utc=<REQUIRED>", "clean_utc=2030-01-02T03:04:05Z")
        .replace(
            "clean_os_arch=<REQUIRED>",
            "clean_os_arch=REDACTED:Linux-x86_64",
        );
    assert!(
        half_filled.contains("outcome=unresolved"),
        "test helper: half-filled still unresolved"
    );
    assert!(
        half_filled.contains("<REQUIRED>"),
        "test helper: half-filled still has placeholders"
    );
    assert!(
        !validate("rehearsal", &half_filled),
        "Branch 2 FAILED: partially filled record with outcome=unresolved must be rejected.\n\
         Expected validator to fail, but it passed.\n\
         This catches cases where evidence was selectively filled without changing outcome."
    );

    // Stage 3: flipped outcome to pass but placeholders remain.
    // This is fabrication and MUST fail the validator.
    let fabricated = blank.replace("outcome=unresolved", "outcome=pass");
    assert!(
        fabricated.contains("outcome=pass"),
        "test helper: fabricated has pass"
    );
    assert!(
        fabricated.contains("<REQUIRED>"),
        "test helper: fabricated still has placeholders"
    );
    assert!(
        !validate("rehearsal", &fabricated),
        "Branch 3 FAILED: outcome=pass with <REQUIRED> placeholders must be rejected.\n\
         Expected validator to fail, but it passed.\n\
         This is the fabrication case: claiming pass without populating evidence."
    );

    // Stage 4: complete pass with all fields populated.
    // This MUST pass the validator (the state after genuine Task 2 completion).
    let complete = rehearsal_complete_pass();
    assert!(
        complete.contains("outcome=pass"),
        "test helper: complete has pass"
    );
    assert!(
        !complete.contains("<REQUIRED>"),
        "test helper: complete has no placeholders"
    );
    assert!(
        validate("rehearsal", &complete),
        "Branch 4 FAILED: complete rehearsal with outcome=pass must be accepted.\n\
         Expected validator to pass, but it failed.\n\
         This is the valid end state: all evidence populated, no placeholders."
    );

    // If 20-ACCEPTANCE.md exists, it must pass the validator.
    // If it does not exist, that is also valid (not yet created).
    let acceptance_path = root(".planning/phases/20-human-acceptance-gate/20-ACCEPTANCE.md");
    if acceptance_path.exists() {
        let acceptance_body =
            fs::read_to_string(&acceptance_path).expect("acceptance record must be readable");
        assert!(
            validate("acceptance", &acceptance_body),
            "If 20-ACCEPTANCE.md exists, it must pass the acceptance validator"
        );
    }
}

/// Fill every `<REQUIRED>` placeholder in a real template with a value the
/// validator accepts, and classify the record `pass`.
///
/// Deliberately operates on the template's FULL text, prose included, rather
/// than on a synthetic list of `key=value` rows.
fn fill_template_as_pass(template: &str) -> String {
    template
        .lines()
        .map(|line| {
            let Some((key, value)) = line.split_once('=') else {
                return line.to_string();
            };
            if key.contains(' ') || (value != "<REQUIRED>" && value != "unresolved") {
                return line.to_string();
            }
            let filled = match key {
                "outcome" | "redaction_review" => "pass",
                "redaction_findings" => "none",
                "independent_machines" | "different_networks" | "no_coaching" => "yes",
                "shared_vpn" | "copied_keys" => "no",
                "task_a_id" => "task-a-0001",
                "task_b_id" => "task-b-0002",
                k if k.ends_with("_state") => "COMPLETED",
                k if k.ends_with("_os_arch") => "REDACTED:Linux/x86_64",
                k if k.ends_with("_utc") => "2026-01-01T00:00:00Z",
                _ => "filled",
            };
            format!("{key}={filled}")
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// A fully populated `pass` record built from the REAL template must validate.
///
/// Regression test for the gate defect found while populating 20-REHEARSAL.md
/// on 2026-08-09: the template's own redaction warning spelled out two of the
/// literals `phase20-evidence-check.sh` greps for, so any `pass` record
/// inherited them and was rejected as leaking a secret. The forbidden-pattern
/// grep runs only on the `pass` path, so every earlier `unresolved` state
/// missed it -- the same shape as G3, mirrored onto the success outcome.
///
/// The existing helpers could not catch this: they assemble synthetic
/// `key=value` rows, so the template's prose was never part of a validated
/// body. This test validates the template text itself.
#[test]
fn real_templates_populated_as_pass_are_accepted() {
    for (mode, path) in [
        (
            "rehearsal",
            ".planning/phases/20-human-acceptance-gate/20-REHEARSAL-TEMPLATE.md",
        ),
        (
            "acceptance",
            ".planning/phases/20-human-acceptance-gate/20-ACCEPTANCE-TEMPLATE.md",
        ),
    ] {
        let template = fs::read_to_string(root(path))
            .unwrap_or_else(|e| panic!("template {path} must be readable: {e}"));
        let populated = fill_template_as_pass(&template);
        // Prose legitimately mentions `<REQUIRED>` (the "replace every
        // <REQUIRED> value" instruction), so only field lines are checked.
        let unfilled: Vec<&str> = populated
            .lines()
            .filter(|l| {
                l.split_once('=')
                    .is_some_and(|(k, v)| v == "<REQUIRED>" && !k.contains(' '))
            })
            .collect();
        assert!(
            unfilled.is_empty(),
            "{path}: filler left field placeholders behind, extend \
             fill_template_as_pass: {unfilled:?}"
        );
        assert!(
            validate(mode, &populated),
            "{path}: a fully populated `pass` record built from the real template \
             was REJECTED by phase20-evidence-check.sh.\n\
             The most likely cause is prose in the template matching the validator's \
             forbidden secret/path grep -- describe those literals, do not spell them."
        );
    }
}

/// The live rehearsal record must never carry a secret or unredacted home path,
/// whatever outcome it currently holds.
///
/// The validator only reaches its redaction grep on the `pass` path, so this
/// stays honest while `outcome` is still awaiting a provenance call.
#[test]
fn live_rehearsal_record_is_redaction_clean_at_any_outcome() {
    let path = ".planning/phases/20-human-acceptance-gate/20-REHEARSAL.md";
    let Ok(body) = fs::read_to_string(root(path)) else {
        return; // record not created yet
    };
    let populated = body.replace("outcome=unresolved", "outcome=pass");
    assert!(
        validate("rehearsal", &populated),
        "{path}: classified `pass`, the live record does not satisfy the validator. \
         Fix the record before attesting an outcome."
    );
}

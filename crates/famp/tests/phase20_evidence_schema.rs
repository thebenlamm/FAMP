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
    // Phase 20-01 laid the fail-closed test expecting rehearsal to not exist.
    // Phase 20-02 Task 1 creates the rehearsal candidate. The invariant is that
    // the candidate exists AND has outcome=unresolved (i.e., is visibly incomplete).
    // Once Task 2 completes on a clean host, outcome will be resolved to
    // pass/product_or_guide_failure/invalid, at which point this assertion will fail
    // as expected—that is not a bug, it means Phase 20 is complete.
    let rehearsal_path = root(".planning/phases/20-human-acceptance-gate/20-REHEARSAL.md");
    assert!(
        rehearsal_path.exists(),
        "Phase 20-02 Task 1 must create the rehearsal candidate"
    );
    let rehearsal_body =
        fs::read_to_string(&rehearsal_path).expect("rehearsal candidate must be readable");
    assert!(
        rehearsal_body.contains("outcome=unresolved"),
        "rehearsal candidate must have unresolved outcome"
    );
    assert!(!root(".planning/phases/20-human-acceptance-gate/20-ACCEPTANCE.md").exists());
}

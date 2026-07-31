//! Notification shaping and final block-decision JSON emission.

use serde_json::{json, Value};
use std::io::Write;
use std::path::Path;

use famp_inspect_client::{call, raw_connect_probe, ProbeOutcome};
use famp_inspect_proto::{InspectIdentitiesReply, InspectIdentitiesRequest, InspectKind};

use crate::bus_client::resolve_sock_path;
use crate::cli::await_cmd::AwaitOutcome;

use super::log::log;
use super::transcript::validate_sender;

/// Shape + emit `{"decision":"block","reason":"..."}` for a successful wake.
/// Returns `true` if a block decision was written to `out`.
///
/// Resolves the broker socket from the environment; see
/// [`emit_block_decision_at`] for the injectable-socket variant.
// `future_not_send`: this future holds `out: &mut dyn Write` (stdout) across
// the `#26` await. It is only ever driven by `rt.block_on` in the codex-stop
// hook (never `tokio::spawn`), so it is polled on one thread and never sent
// between threads — Send is irrelevant here.
#[allow(clippy::future_not_send)]
pub async fn emit_block_decision(
    outcome: &AwaitOutcome,
    identity: &str,
    out: &mut dyn Write,
) -> bool {
    let sock = resolve_sock_path();
    emit_block_decision_at(&sock, outcome, identity, out).await
}

/// [`emit_block_decision`] against an explicit broker socket path.
///
/// Tests must use this: the `#26` unread check below otherwise talks to
/// whatever broker the developer happens to have running, so a live session
/// under the same identity would flip the result.
// `future_not_send`: holds `out: &mut dyn Write` across the `#26` await;
// driven by `block_on` only (never spawned), so never sent across threads.
#[allow(clippy::future_not_send)]
pub async fn emit_block_decision_at(
    sock: &Path,
    outcome: &AwaitOutcome,
    identity: &str,
    out: &mut dyn Write,
) -> bool {
    if outcome.aborted || outcome.timed_out || outcome.envelopes.is_empty() {
        return false;
    }

    let (mut count, mut sender, mailbox_kind, mailbox_name) = meta_from_outcome(outcome);
    if count < 1 {
        log("await timeout payload; clean stop");
        return false;
    }
    let await_batch_count = count;

    // #26: for agent mailboxes, prefer disk-ack unread over await-batch length.
    if mailbox_kind != "channel" {
        if let Some((unread, last_sender)) = actionable_unread_at(sock, identity).await {
            if unread == 0 {
                log(&format!(
                    "await batch had {await_batch_count} envelopes but disk-ack unread=0 for {identity}; no actionable wake (#26)"
                ));
                return false;
            }
            if unread != count {
                log(&format!(
                    "await batch count={count} reduced to disk-ack unread={unread} for {identity} (#26)"
                ));
            }
            count = unread;
            if !last_sender.is_empty() {
                sender = last_sender;
            }
        } else {
            log(&format!(
                "inspect identities unavailable; keeping await-batch count={count} (fail-open)"
            ));
        }
    }

    if !validate_sender(&sender) {
        log("sender failed validation; using 'unknown'");
        sender = "unknown".to_string();
    }

    let reason = build_reason(count, &sender, &mailbox_kind, &mailbox_name);
    let body = json!({
        "decision": "block",
        "reason": reason,
    });
    let Ok(serialized) = serde_json::to_string(&body) else {
        log(&format!(
            "POST-WAKE EMIT FAILURE identity={identity} mailbox={mailbox_kind}/{mailbox_name} reason=json_serialize"
        ));
        return false;
    };
    if writeln!(out, "{serialized}").is_err() {
        log(&format!(
            "POST-WAKE EMIT FAILURE identity={identity} mailbox={mailbox_kind}/{mailbox_name} reason=stdout_write"
        ));
        return false;
    }
    log(&format!(
        "emitting block decision ({} bytes); count={count} sender={sender} await_batch={await_batch_count}",
        serialized.len()
    ));
    true
}

fn meta_from_outcome(outcome: &AwaitOutcome) -> (u64, String, String, String) {
    let mut count = 0u64;
    let mut sender = "unknown".to_string();
    let (mailbox_kind, mailbox_name) = match &outcome.mailbox {
        Some(famp_bus::MailboxName::Channel(name)) => ("channel".to_string(), name.clone()),
        Some(famp_bus::MailboxName::Agent(name)) => ("agent".to_string(), name.clone()),
        None => ("agent".to_string(), String::new()),
    };
    for item in &outcome.envelopes {
        if item.get("timeout") == Some(&Value::Bool(true)) {
            continue;
        }
        count += 1;
        if let Some(s) = item
            .get("from")
            .or_else(|| item.get("sender"))
            .and_then(Value::as_str)
        {
            sender = s.to_string();
        }
    }
    (count, sender, mailbox_kind, mailbox_name)
}

fn build_reason(count: u64, sender: &str, mailbox_kind: &str, mailbox_name: &str) -> String {
    if mailbox_kind == "channel" {
        let mut chan = mailbox_name.to_string();
        if !chan.starts_with('#') {
            chan = format!("#{chan}");
        }
        if count > 1 {
            format!(
                "[FAMP listen mode] {count} new FAMP messages in channel {chan}, latest from {sender}. Call famp_channel_log({{channel: '{chan}'}}) to read them."
            )
        } else {
            format!(
                "[FAMP listen mode] New FAMP message in channel {chan} from {sender}. Call famp_channel_log({{channel: '{chan}'}}) to read it."
            )
        }
    } else if count > 1 {
        format!(
            "[FAMP listen mode] {count} new FAMP messages, latest from {sender}. Call famp_inbox to read them."
        )
    } else {
        format!("[FAMP listen mode] New FAMP message from {sender}. Call famp_inbox to read it.")
    }
}

async fn actionable_unread_at(sock: &Path, identity: &str) -> Option<(u64, String)> {
    let ProbeOutcome::Healthy { mut stream } = raw_connect_probe(sock).await else {
        return None;
    };
    let payload = call(
        &mut stream,
        InspectKind::Identities(InspectIdentitiesRequest::default()),
    )
    .await
    .ok()?;
    let reply: InspectIdentitiesReply = serde_json::from_value(payload).ok()?;
    match reply {
        InspectIdentitiesReply::List(list) => {
            for r in list.rows {
                if r.name != identity {
                    continue;
                }
                let unread = r.mailbox_unread;
                let mut sender = r.last_sender;
                if sender == "(none)" {
                    sender = "unknown".to_string();
                }
                return Some((unread, sender));
            }
            None
        }
        InspectIdentitiesReply::BudgetExceeded { .. } => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::cli::await_cmd::AwaitOutcome;
    use std::path::PathBuf;

    /// A path with no listener, so `actionable_unread_at` fails open.
    /// Hermetic: never touches the developer's live broker.
    fn dead_sock() -> PathBuf {
        std::env::temp_dir().join("famp-hook-emit-test-no-listener.sock")
    }

    #[test]
    fn agent_reason_single() {
        let r = build_reason(1, "alice", "agent", "bob");
        assert!(r.contains("New FAMP message from alice"));
        assert!(r.contains("famp_inbox"));
        assert!(!r.contains("alice's body"));
    }

    #[test]
    fn channel_reason_multi() {
        let r = build_reason(3, "alice", "channel", "planning");
        assert!(r.contains("#planning"));
        assert!(r.contains("3 new"));
        assert!(r.contains("famp_channel_log"));
    }

    #[tokio::test]
    async fn empty_outcome_does_not_emit() {
        let outcome = AwaitOutcome {
            envelopes: vec![],
            mailbox: None,
            next_offset: None,
            timed_out: true,
            diagnostic: None,
            aborted: false,
        };
        let mut buf = Vec::new();
        assert!(!emit_block_decision_at(&dead_sock(), &outcome, "dk", &mut buf).await);
        assert!(buf.is_empty());
    }

    #[tokio::test]
    async fn emits_native_json_without_jq() {
        let outcome = AwaitOutcome {
            envelopes: vec![json!({"from": "alice", "class": "request"})],
            mailbox: Some(famp_bus::MailboxName::Agent("dk".into())),
            next_offset: Some(1),
            timed_out: false,
            diagnostic: None,
            aborted: false,
        };
        // No listener on this socket, so the #26 inspect probe fails open and
        // the await-batch count is kept.
        let mut buf = Vec::new();
        let emitted = emit_block_decision_at(&dead_sock(), &outcome, "dk", &mut buf).await;
        assert!(emitted);
        let s = String::from_utf8(buf).unwrap();
        let v: Value = serde_json::from_str(s.trim()).unwrap();
        assert_eq!(v["decision"], "block");
        assert!(v["reason"].as_str().unwrap().contains("alice"));
    }
}

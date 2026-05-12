//! `famp_verify` MCP tool — resilience hook for the Claude Code
//! `[Tool result missing due to internal error]` failure mode.
//!
//! ## Why this tool exists
//!
//! Claude Code's stdio MCP transport occasionally drops the
//! `tools/call` JSON-RPC response on its way back to the model, even
//! when the broker successfully processed the request. For `famp_send`
//! this means the recipient's mailbox has the message but the sender
//! agent has no `task_id` and no idea whether to retry.
//!
//! `famp_verify` lets an agent confirm delivery WITHOUT re-sending:
//! given a `task_id` (recovered from `famp_whoami.last_send`, or
//! remembered out-of-band), it checks whether that `task_id` appears in
//! the recipient's mailbox on disk.
//!
//! ## Direct mailbox-file read (adversarial-review fix, 2026-05-12)
//!
//! Previously this tool bounced through the inspector's `InspectKind::
//! Messages` RPC. That had three correctness gaps surfaced by review:
//!
//! - **Finding 2 (high) — offline recipients silently missed.** The
//!   inspector's `read_message_snapshot` only walks mailboxes for
//!   currently-registered canonical holders. A recipient that crashed,
//!   exited, or has not yet registered has its mailbox on disk but is
//!   absent from `BrokerStateView.clients`, so the inspector returned an
//!   empty row set and `famp_verify` reported `delivered: false` despite
//!   the message being durably written.
//! - **Finding 3 (medium) — 50-row scan cap.** The inspector defaults to
//!   `tail: 50` and the previous verify call passed `tail: None`. Older
//!   delivered tasks fell off the window.
//! - **Finding 4 (medium) — thread-only matching.** Matching by
//!   `MessageRow.task_id` (which is `causality.ref` for reply envelopes)
//!   confirms "something landed on this thread," not "MY specific reply
//!   landed." A prior reply on the same thread produced false positives.
//!
//! All three are fixed by reading the recipient's mailbox JSONL file
//! directly, with no row cap and access to the full envelope (not just
//! the wire-friendly `MessageRow` projection). For schema parity with
//! the inspector RPC the tool still surfaces `row` via
//! `famp_inspect_server::message_row`, so callers see byte-identical
//! output regardless of which code path produced the hit.
//!
//! ## Why FREE-PASS (no `famp_register` required)
//!
//! Recovery must work even when session-state has been lost (cold
//! restart, fresh window after a crash). The verify path reads files
//! and does not require a broker round-trip at all, so
//! `server.rs::dispatch_tool` routes this tool through the FREE-PASS
//! arm alongside `famp_register` and `famp_whoami`.
//!
//! ## Input shape
//!
//! ```json
//! {
//!   "task_id":     "<uuidv7>",   // required: thread or new-task id
//!   "peer":        "bob",        // optional: agent recipient
//!   "channel":     "planning",   // optional: channel recipient (no '#')
//!   "envelope_id": "<uuidv7>"    // optional: exact envelope match
//! }
//! ```
//!
//! - `task_id` (required): the `UUIDv7` to look up. For `mode="open"`
//!   sends this is the envelope's own `id`. For `mode="reply"` sends
//!   pass the originating thread's task_id (surfaced as
//!   `famp_whoami.last_send.thread_task_id`).
//! - `peer` (optional): canonical agent identity to verify against.
//!   Recommended for agent DMs; without it (and without `channel`) the
//!   tool scans every mailbox under `<bus_dir>/mailboxes/`. Exactly one
//!   of `peer` or `channel` should be provided.
//! - `channel` (optional): channel name (with or without leading `#`).
//!   When present, verifies against `<bus_dir>/mailboxes/#<name>.jsonl`.
//! - `envelope_id` (optional): if supplied, requires the envelope's own
//!   `id` field to match exactly IN ADDITION TO the thread match. This
//!   distinguishes "my specific reply landed" from "some envelope on
//!   this thread landed." When absent, falls back to thread-only match
//!   (which is what open-mode sends need, since `envelope_id == task_id`
//!   there anyway).
//!
//! ## Output shape
//!
//! ```json
//! {
//!   "delivered": true,
//!   "task_id":   "<uuidv7>",
//!   "row":       { "sender": "...", "recipient": "...", "class": "...",
//!                  "state": "...", "timestamp": "...", "body_bytes": 42,
//!                  "body_sha256_prefix": "..." }
//! }
//! ```
//!
//! When `delivered: false`, the `row` field is omitted. The `task_id`
//! is echoed back so a hung-up retry loop has a stable handle.

use std::path::{Path, PathBuf};

use famp_bus::BusErrorKind;
use serde_json::Value;

use crate::bus_client::{bus_dir, resolve_sock_path};
use crate::cli::mcp::tools::ToolError;

/// Dispatch a `famp_verify` tool call.
pub async fn call(input: &Value) -> Result<Value, ToolError> {
    let task_id = input
        .get("task_id")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                "missing required field: task_id (string)",
            )
        })?
        .to_string();
    if task_id.is_empty() {
        return Err(ToolError::new(
            BusErrorKind::EnvelopeInvalid,
            "task_id must not be empty",
        ));
    }
    let peer = input
        .get("peer")
        .and_then(Value::as_str)
        .map(str::to_string);
    let channel = input
        .get("channel")
        .and_then(Value::as_str)
        .map(str::to_string);
    let envelope_id = input
        .get("envelope_id")
        .and_then(Value::as_str)
        .map(str::to_string);

    if peer.is_some() && channel.is_some() {
        return Err(ToolError::new(
            BusErrorKind::EnvelopeInvalid,
            "peer and channel are mutually exclusive",
        ));
    }

    let sock = resolve_sock_path();
    let mailboxes_dir = bus_dir(&sock).join("mailboxes");

    // Build the list of candidate mailbox files. Single-recipient calls
    // (peer or channel) read exactly one file; the broker-wide fallback
    // walks every `*.jsonl` under `mailboxes/`. The directory may not
    // exist yet if no agent has ever registered against this socket —
    // treat that as "no envelopes" rather than an error.
    let candidates = match (&peer, &channel) {
        (Some(name), None) => vec![mailboxes_dir.join(format!("{name}.jsonl"))],
        (None, Some(name)) => {
            let bare = name.trim_start_matches('#');
            vec![mailboxes_dir.join(format!("#{bare}.jsonl"))]
        }
        (None, None) => list_mailbox_files(&mailboxes_dir),
        // peer.is_some() && channel.is_some() rejected above.
        _ => unreachable!("peer/channel mutual-exclusion checked earlier"),
    };

    Ok(scan_files(&task_id, envelope_id.as_deref(), &candidates))
}

/// Enumerate `*.jsonl` files under `dir`. Missing/unreadable directory
/// → empty Vec (the "no envelopes" outcome for verify is a clean
/// `delivered: false`).
fn list_mailbox_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let path = e.path();
            (path.extension().and_then(|s| s.to_str()) == Some("jsonl")).then_some(path)
        })
        .collect()
}

/// Read every candidate mailbox file in order, returning the first
/// envelope whose thread/envelope-id match. Files that don't exist or
/// are unreadable contribute zero rows. A corrupt mid-file line is
/// surfaced as `Internal` via `ToolError` would force the caller into
/// the retry loop unnecessarily — instead `read_all` already tolerates
/// the partial-tail crash case; we tolerate file-not-found here.
fn scan_files(task_id: &str, envelope_id: Option<&str>, candidates: &[PathBuf]) -> Value {
    for path in candidates {
        let Ok(entries) = famp_inbox::read::read_all(path) else {
            continue;
        };
        if let Some(env) = entries.into_iter().find(|env| envelope_matches(env, task_id, envelope_id)) {
            let row = famp_inspect_server::message_row(&env);
            let row_v = serde_json::to_value(&row).unwrap_or(Value::Null);
            return serde_json::json!({
                "delivered": true,
                "task_id":   task_id,
                "row":       row_v,
            });
        }
    }
    serde_json::json!({
        "delivered": false,
        "task_id":   task_id,
    })
}

/// Match an envelope against the verify input:
///   1. THREAD MATCH: `causality.ref == task_id` OR `id == task_id`
///      (the latter covers `mode="open"` envelopes where the new-task
///      id IS the thread id).
///   2. ENVELOPE MATCH (Finding 4 fix): if `envelope_id` was supplied,
///      additionally require `id == envelope_id`. This proves the
///      caller's *specific* envelope landed, not merely "something on
///      the thread did."
fn envelope_matches(env: &Value, task_id: &str, envelope_id: Option<&str>) -> bool {
    let id = env.get("id").and_then(Value::as_str);
    let causality_ref = env
        .get("causality")
        .and_then(|c| c.get("ref"))
        .and_then(Value::as_str);

    let thread_hit = causality_ref == Some(task_id) || id == Some(task_id);
    if !thread_hit {
        return false;
    }
    match envelope_id {
        Some(want) => id == Some(want),
        None => true,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn write_mailbox(dir: &Path, name: &str, envelopes: &[Value]) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let path = dir.join(format!("{name}.jsonl"));
        let mut f = File::create(&path).unwrap();
        for env in envelopes {
            writeln!(f, "{}", serde_json::to_string(env).unwrap()).unwrap();
        }
        path
    }

    fn open_envelope(id: &str, from: &str, to: &str) -> Value {
        serde_json::json!({
            "id": id,
            "from": from,
            "to": to,
            "class": "request",
            "ts": "2026-05-12T18:00:00Z",
            "body": {"event": "famp.send.new_task", "details": {"summary": "x"}},
        })
    }

    fn reply_envelope(env_id: &str, thread_id: &str, from: &str, to: &str) -> Value {
        serde_json::json!({
            "id": env_id,
            "from": from,
            "to": to,
            "class": "commitment",
            "ts": "2026-05-12T18:00:01Z",
            "causality": {"ref": thread_id},
            "body": {"event": "famp.send.reply"},
        })
    }

    #[test]
    fn envelope_matches_open_mode_thread_only() {
        // Open-mode: id == task_id; no envelope_id needed.
        let env = open_envelope("0193abcd-ef01-7000-8000-000000000001", "alice", "bob");
        assert!(envelope_matches(
            &env,
            "0193abcd-ef01-7000-8000-000000000001",
            None
        ));
        // Wrong thread id misses.
        assert!(!envelope_matches(
            &env,
            "0193abcd-ef01-7000-8000-000000000999",
            None
        ));
    }

    #[test]
    fn envelope_matches_reply_thread_match() {
        // Reply-mode: causality.ref == thread; envelope's own id is the
        // reply's id. Thread-only match still hits (open-mode caller).
        let env = reply_envelope(
            "0193abcd-ef01-7000-8000-000000000002",
            "0193abcd-ef01-7000-8000-000000000001",
            "bob",
            "alice",
        );
        assert!(envelope_matches(
            &env,
            "0193abcd-ef01-7000-8000-000000000001",
            None
        ));
    }

    #[test]
    fn envelope_matches_specific_envelope_id_filters_false_positive() {
        // Finding 4: two replies on the same thread. Without
        // envelope_id, both match (false-positive risk). With
        // envelope_id, only the exact one matches.
        let reply_a = reply_envelope(
            "0193abcd-ef01-7000-8000-00000000000a",
            "0193abcd-ef01-7000-8000-000000000001",
            "bob",
            "alice",
        );
        let reply_b = reply_envelope(
            "0193abcd-ef01-7000-8000-00000000000b",
            "0193abcd-ef01-7000-8000-000000000001",
            "bob",
            "alice",
        );
        // Thread-only: both hit.
        assert!(envelope_matches(
            &reply_a,
            "0193abcd-ef01-7000-8000-000000000001",
            None
        ));
        assert!(envelope_matches(
            &reply_b,
            "0193abcd-ef01-7000-8000-000000000001",
            None
        ));
        // With envelope_id=B: only B matches.
        assert!(!envelope_matches(
            &reply_a,
            "0193abcd-ef01-7000-8000-000000000001",
            Some("0193abcd-ef01-7000-8000-00000000000b"),
        ));
        assert!(envelope_matches(
            &reply_b,
            "0193abcd-ef01-7000-8000-000000000001",
            Some("0193abcd-ef01-7000-8000-00000000000b"),
        ));
    }

    #[test]
    fn scan_files_finds_envelope_in_first_mailbox() {
        let dir = tempfile::tempdir().unwrap();
        let mb = dir.path().join("mailboxes");
        let path = write_mailbox(
            &mb,
            "bob",
            &[open_envelope(
                "0193abcd-ef01-7000-8000-000000000001",
                "alice",
                "bob",
            )],
        );
        let out = scan_files(
            "0193abcd-ef01-7000-8000-000000000001",
            None,
            &[path.clone()],
        );
        assert_eq!(out["delivered"], Value::Bool(true));
        assert_eq!(
            out["task_id"],
            Value::String("0193abcd-ef01-7000-8000-000000000001".into())
        );
        assert!(out["row"].is_object());
        assert_eq!(out["row"]["recipient"].as_str().unwrap_or(""), "bob");
    }

    #[test]
    fn scan_files_returns_not_delivered_for_unknown_task_id() {
        let dir = tempfile::tempdir().unwrap();
        let mb = dir.path().join("mailboxes");
        let path = write_mailbox(
            &mb,
            "bob",
            &[open_envelope(
                "0193abcd-ef01-7000-8000-000000000001",
                "alice",
                "bob",
            )],
        );
        let out = scan_files(
            "0193abcd-ef01-7000-8000-00000000ffff",
            None,
            &[path.clone()],
        );
        assert_eq!(out["delivered"], Value::Bool(false));
        assert!(out.get("row").is_none());
    }

    #[test]
    fn scan_files_skips_missing_mailbox_files() {
        // Missing-file MUST not error — verify is best-effort and
        // returns `delivered: false` for any cold-recipient case.
        let out = scan_files(
            "0193abcd-ef01-7000-8000-000000000001",
            None,
            &[PathBuf::from("/nonexistent/famp/mailboxes/ghost.jsonl")],
        );
        assert_eq!(out["delivered"], Value::Bool(false));
    }

    #[test]
    fn scan_files_finds_offline_recipient_envelope() {
        // Finding 2 regression: an envelope in a mailbox file whose
        // recipient never registered (or has crashed) must still be
        // findable. The direct-file read bypasses the inspector's
        // currently-registered-only walk.
        let dir = tempfile::tempdir().unwrap();
        let mb = dir.path().join("mailboxes");
        let env_id = "0193abcd-ef01-7000-8000-000000000777";
        let path = write_mailbox(
            &mb,
            "ghost", // never registered — the inspector would skip this mailbox
            &[open_envelope(env_id, "alice", "ghost")],
        );
        let out = scan_files(env_id, None, &[path.clone()]);
        assert_eq!(
            out["delivered"],
            Value::Bool(true),
            "envelopes in mailboxes of never-registered recipients must still verify-true"
        );
    }

    #[test]
    fn scan_files_no_row_cap() {
        // Finding 3 regression: write more than 50 envelopes; the
        // earliest one must still be findable. The inspector defaulted
        // to a 50-row tail; direct file read has no cap.
        let dir = tempfile::tempdir().unwrap();
        let mb = dir.path().join("mailboxes");
        let mut envelopes = Vec::new();
        // 100 distinct envelopes; we look up the FIRST one.
        for i in 0..100u32 {
            let id = format!("0193abcd-ef01-7000-8000-0000000{i:06x}");
            envelopes.push(open_envelope(&id, "alice", "bob"));
        }
        let path = write_mailbox(&mb, "bob", &envelopes);
        let first = format!("0193abcd-ef01-7000-8000-0000000{:06x}", 0u32);
        let out = scan_files(&first, None, &[path.clone()]);
        assert_eq!(
            out["delivered"],
            Value::Bool(true),
            "first-of-100 must still be findable (no 50-row cap)"
        );
    }
}

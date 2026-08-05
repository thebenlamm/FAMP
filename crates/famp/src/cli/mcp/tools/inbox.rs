//! `famp_inbox` MCP tool — Phase 02 plan 02-09 implementation.
//!
//! Thin wrapper over `cli::inbox::list::run_at_structured`. Sends
//! `BusMessage::Inbox` to the broker and surfaces the typed envelopes +
//! cursor.
//!
//! ## Input contract
//!
//! This MCP surface is **list-only**. It reads exactly two fields; anything
//! else in the input `Value` is ignored (there is no `deny_unknown_fields`
//! on this path, so a v0.8-era caller still passing `action: "list"` keeps
//! working). The on-disk `.cursor` is written through on **every** MCP
//! `famp_inbox` call (see `cursor_write_target` below): the target is
//! monotonic in the broker's `next_offset` EXCEPT for the EOF clamp, which
//! can regress a corrupt past-EOF cursor back down to the mailbox's real
//! byte size — this is how an already-corrupt on-disk cursor self-heals on
//! its next read. There is still no MCP `ack`; the `since`-based session
//! offset below remains the read-position mechanism callers control.
//!
//! - `since: u64` — optional cursor offset. When the caller omits it (or
//!   passes `null`), the MCP **session layer** remembers the previous
//!   call's `InboxOk.next_offset` (`session::inbox_offset()`) and uses
//!   THAT as the effective `since`, so a second `famp_inbox` in one
//!   session does not replay envelopes already seen (#13 — protects the
//!   agent's context from the double-print pattern
//!   `docs/CLAUDE-CODE-CONTEXT-GUIDE.md` warns about). An explicit
//!   caller-supplied `since` (INCLUDING `0`) always wins and is used
//!   as-is — `since: 0` is therefore a deliberate full-mailbox-replay
//!   recovery escape hatch, not a bug. After every successful call the
//!   session stores the RETURNED `next_offset` (never
//!   `max(stored, returned)` — mailboxes can shrink, #11/#16, and the
//!   stored value must follow the broker's clamp down).
//!
//!   **Accepted cost:** the FIRST `famp_inbox` of a session still
//!   replays the whole mailbox. `famp_register`'s `RegisterOk.drained`
//!   count is discarded (not the envelopes), and seeding the offset from
//!   register would need a wire change (out of scope for #13). This is
//!   a deliberate once-per-session cost and a recovery affordance, not a
//!   bug — do not re-file it.
//! - `include_terminal: bool` — optional, default `false` per MCP-04.
//!   STRICT bool — a non-bool surfaces `EnvelopeInvalid` with a message
//!   naming both the field and the expected type so MCP clients can
//!   self-correct.
//!
//! ## Output shape
//!
//! ```json
//! { "entries": [<envelope>, ...], "next_offset": <u64> }
//! ```
//!
//! `entries` (NOT `envelopes`) preserves the v0.8 MCP-tool output
//! convention so existing clients/tests do not need to be re-shaped on
//! this field name. Each entry is the typed envelope `serde_json::Value`
//! straight from the broker, with `task_id` accessible via the FAMP
//! envelope's `causality.ref` projection (test fixture-driven).

use famp_bus::BusErrorKind;
use famp_envelope::EnvelopeView;
use serde_json::Value;

use crate::bus_client::{bus_dir, resolve_sock_path};
use crate::cli::broker::cursor_exec::execute_advance_cursor;
use crate::cli::inbox::list::{run_at_structured, ListArgs};
use crate::cli::mcp::session;
use crate::cli::mcp::tools::ToolError;

/// Read the current on-disk cursor for `name` under `dir`'s `mailboxes/`
/// directory. Mirrors the parse pattern in
/// `cli/broker/mod.rs::read_mailbox_meta_for`: body is a single ASCII
/// decimal followed by `\n`; a missing or unparseable file is treated as
/// `0` — never blocks the already-succeeded inbox read this feeds.
fn read_disk_cursor(dir: &std::path::Path, name: &str) -> u64 {
    let cursor_path = dir.join("mailboxes").join(format!(".{name}.cursor"));
    std::fs::read_to_string(&cursor_path)
        .ok()
        .and_then(|s| s.trim_end_matches('\n').parse::<u64>().ok())
        .unwrap_or(0)
}

/// Read the byte length of `name`'s mailbox file under `dir`'s
/// `mailboxes/` directory. `0` when the file is missing or unreadable —
/// mirrors `read_disk_cursor`'s never-block posture; this is a
/// best-effort observability read, never a gate on the already-succeeded
/// inbox call it feeds.
fn read_mailbox_size(dir: &std::path::Path, name: &str) -> u64 {
    let mailbox_path = dir.join("mailboxes").join(format!("{name}.jsonl"));
    std::fs::metadata(&mailbox_path)
        .map(|m| m.len())
        .unwrap_or(0)
}

/// Compute the disk-cursor write target given the current on-disk value,
/// the broker's returned `next_offset`, and the mailbox's real byte size.
///
/// `current.max(next_offset)` preserves the existing monotonic floor: a
/// manual `since: 0` full-mailbox replay must not rewind the `unread`
/// floor that `famp inspect identities` reads from disk. `.min(mailbox_size)`
/// is the EOF clamp: without it, a cursor that ever lands past EOF is
/// unrecoverable by construction (`max` can only ever grow it further) and
/// the identity reads empty forever — indistinguishable from "no new
/// mail". The caller fires the write on `target != current`, not
/// `target > current`, because the clamp's whole purpose is to let a
/// regression from a past-EOF value actually land and self-heal an
/// already-corrupt cursor.
///
/// Benign race, noted so a future reader does not "fix" it: another agent
/// may append to the mailbox between the broker's read and the
/// `read_mailbox_size` call, which can only make `mailbox_size` LARGER and
/// therefore only relaxes the clamp. The dangerous direction (size smaller
/// than a legitimate `next_offset`) means the mailbox was truncated or
/// rotated, where clamping down is the correct answer anyway.
const fn cursor_write_target(current: u64, next_offset: u64, mailbox_size: u64) -> u64 {
    let advanced = if current >= next_offset {
        current
    } else {
        next_offset
    };
    if advanced <= mailbox_size {
        advanced
    } else {
        mailbox_size
    }
}

/// Dispatch a `famp_inbox` tool call.
pub async fn call(input: &Value) -> Result<Value, ToolError> {
    // `since`: optional u64. When the caller omits it, fall back to the
    // session's remembered offset from the previous call (#13) so a
    // second famp_inbox does not replay the whole mailbox. An explicit
    // caller value (including 0) always wins — see module doc comment.
    let since = match input.get("since") {
        None | Some(Value::Null) => session::inbox_offset().await,
        Some(Value::Number(n)) => n.as_u64(),
        Some(_) => {
            return Err(ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                "field since must be a non-negative integer",
            ));
        }
    };

    // STRICT: include_terminal MUST be a JSON boolean if present.
    let include_terminal = match input.get("include_terminal") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => {
            return Err(ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                "field include_terminal must be a boolean",
            ));
        }
    };

    // Single identity snapshot — the fix for the TOCTOU documented in
    // the module doc above. A concurrent `famp_register` landing between
    // a read-site snapshot and a write-site snapshot could otherwise
    // redirect the cursor write-through below to a DIFFERENT identity's
    // `.cursor` file than the one the broker actually read against. Bind
    // ONCE here and reuse this same local for both the broker call's
    // `act_as` AND the write-through's cursor filename. `call()` must
    // read the session's active identity exactly once — a structural
    // test pins that property.
    let identity = session::active_identity().await;

    let args = ListArgs {
        since,
        include_terminal,
        // Carry MCP session's bound identity through so
        // `cli::inbox::list::run_at_structured`'s `resolve_identity()`
        // does not fall back to wires.tsv. dispatch_tool guarantees
        // active_identity is Some by this point.
        act_as: identity.clone(),
    };

    match run_at_structured(&resolve_sock_path(), args).await {
        Ok(out) => {
            // #13: ALWAYS store the RETURNED next_offset, never
            // max(stored, returned) — mailboxes can shrink (#11, #16)
            // and the broker's clamp must be followed down. since:0
            // remains a full-replay escape hatch AND still updates the
            // stored value going forward.
            session::set_inbox_offset(Some(out.next_offset)).await;

            // Write-through to the on-disk `.{name}.cursor` so `famp inspect
            // identities` (which reads ONLY the disk cursor via
            // `read_mailbox_meta_for`) stops lagging the MCP session cursor.
            // This is the one authority `unread` is computed from; the read
            // above has already succeeded, so this is a best-effort
            // observability write, not a gate on the tool's success.
            //
            // Reuses the SAME `identity` snapshot bound above the broker
            // call — not a second read of the session's active identity —
            // so a concurrent `famp_register` cannot redirect this write to a
            // different identity's cursor file (the TOCTOU this task
            // closes). `cursor_write_target` folds in the EOF clamp: the
            // monotonic floor is deliberately NOT the same policy as
            // `session::set_inbox_offset` above (which follows the
            // broker's shrink-clamp down, #11/#16) — the disk cursor is
            // the `unread` floor read by the inspector and by
            // `register`/`join`, and must advance monotonically except
            // when clamped to EOF. Do not unify the two policies.
            if let Some(name) = identity.as_deref() {
                let dir = bus_dir(&resolve_sock_path()).to_path_buf();
                let current = read_disk_cursor(&dir, name);
                let size = read_mailbox_size(&dir, name);
                let target = cursor_write_target(current, out.next_offset, size);
                if target != current {
                    if let Err(e) = execute_advance_cursor(&dir, name, target).await {
                        eprintln!(
                            "warning: famp_inbox cursor write-through failed for {name} \
                             (target offset {target}): {e}"
                        );
                    }
                }
            }

            // Project each envelope into the v0.8 MCP-tool entry shape:
            // include `task_id` at the top level so agents can reply without
            // re-walking the envelope. For reply messages (deliver/terminal),
            // task_id comes from `causality.ref`. For new_task messages,
            // causality is absent — fall back to `envelope.id`, which the
            // broker uses as the canonical task identifier (see task_id_from
            // in broker/handle.rs). This ensures task_id is always non-null.
            //
            // Phase 14 (D-04/D-05, mechanical rendering surface #1 of 7):
            // `task_id` / `thread_state` are derived from the INNER
            // envelope (metadata, not attacker-rendered prose) — this is
            // safe to read directly even for gateway/unknown-origin
            // envelopes. The `"envelope"` field's body, in contrast, is
            // exactly the untrusted-content surface this phase closes:
            // it is replaced with `render::render_envelope_body`'s output
            // (D-07: the one shared helper), and a sibling `"origin"`
            // field is added so a machine consumer can check provenance
            // without string-matching the quarantine marker.
            let entries: Vec<Value> = out
                .envelopes
                .iter()
                .map(|stamped| {
                    let env = &stamped.envelope;
                    let task_id = env
                        .get("causality")
                        .and_then(|c| c.get("ref"))
                        .and_then(Value::as_str)
                        .or_else(|| env.get("id").and_then(Value::as_str))
                        .map(str::to_string);
                    // Infer thread state from event type so agents know
                    // whether to reply or treat the thread as closed.
                    let thread_state = EnvelopeView::new(env)
                        .body()
                        .and_then(|b| b.get("event"))
                        .and_then(Value::as_str)
                        .map_or("OPEN", |e| {
                            if e == "famp.send.deliver_terminal" {
                                "CLOSED"
                            } else {
                                "OPEN"
                            }
                        });
                    let mut rendered_env = env.clone();
                    if let Some(body) = EnvelopeView::new(env).body().cloned() {
                        let rendered_body =
                            crate::cli::render::render_envelope_body(stamped.origin, &body);
                        if let Some(obj) = rendered_env.as_object_mut() {
                            obj.insert("body".to_string(), rendered_body);
                        }
                    }
                    serde_json::json!({
                        "task_id": task_id,
                        "thread_state": thread_state,
                        "origin": stamped.origin,
                        "envelope": rendered_env,
                    })
                })
                .collect();
            Ok(serde_json::json!({
                "entries": entries,
                "next_offset": out.next_offset,
            }))
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ── cursor_write_target ──────────────────────────────────────────

    /// Normal advance: broker offset wins, under EOF.
    #[test]
    fn cursor_write_target_normal_advance() {
        assert_eq!(cursor_write_target(100, 500, 9000), 500);
    }

    /// Monotonic floor holds on a manual replay: a `since: 0` full-replay
    /// must NOT rewind the disk cursor.
    #[test]
    fn cursor_write_target_monotonic_floor_holds_on_replay() {
        assert_eq!(cursor_write_target(500, 0, 9000), 500);
    }

    /// EOF clamp on a fresh overrun — the exact live `lane-x-merge`
    /// numbers (14334 was the corrupted `.cursor` value, exactly the byte
    /// size of a DIFFERENT identity's mailbox; 9865 is `lane-x-merge`'s
    /// real mailbox size).
    #[test]
    fn cursor_write_target_eof_clamp_on_fresh_overrun() {
        assert_eq!(cursor_write_target(0, 14334, 9865), 9865);
    }

    /// EOF clamp self-heals an already-corrupt cursor — the exact live
    /// `opus-coordinator-0805` numbers. Proves the clamp can regress a
    /// past-EOF disk cursor, which is intended: this is the self-heal.
    #[test]
    fn cursor_write_target_eof_clamp_self_heals_corrupt_cursor() {
        assert_eq!(cursor_write_target(202_562, 0, 12_154), 12_154);
    }

    /// Missing mailbox file: size reads as 0, target clamps to 0.
    #[test]
    fn cursor_write_target_missing_mailbox() {
        assert_eq!(cursor_write_target(2154, 0, 0), 0);
    }

    /// Already at EOF: current == next_offset == mailbox_size, target
    /// equals current so the caller writes nothing.
    #[test]
    fn cursor_write_target_already_at_eof() {
        assert_eq!(cursor_write_target(9865, 9865, 9865), 9865);
    }

    // ── read_mailbox_size ────────────────────────────────────────────

    #[test]
    fn read_mailbox_size_missing_file_reads_zero() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert_eq!(read_mailbox_size(tmp.path(), "nobody"), 0);
    }

    #[test]
    fn read_mailbox_size_existing_file_round_trips_length() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mailboxes = tmp.path().join("mailboxes");
        std::fs::create_dir_all(&mailboxes).unwrap();
        let body = b"hello world\n";
        std::fs::write(mailboxes.join("alice.jsonl"), body).unwrap();
        assert_eq!(read_mailbox_size(tmp.path(), "alice"), body.len() as u64);
    }

    // ── TOCTOU structural assertion ──────────────────────────────────

    /// A true concurrency test is impractical at this layer: `call()`
    /// needs a live broker plus a racing `famp_register`, and the failing
    /// interleaving is not reliably reproducible. This structural
    /// assertion is the honest substitute — it pins the single-snapshot
    /// property (`call()`'s body contains EXACTLY ONE `active_identity()`
    /// read) that IS the fix, rather than trying to force the race.
    #[test]
    fn call_body_reads_active_identity_exactly_once() {
        let source = include_str!("inbox.rs");
        let start = source
            .find("pub async fn call(")
            .expect("call() signature not found in inbox.rs — did it move?");
        let body = &source[start..];
        let end = body
            .find("\n#[cfg(test)]")
            .expect("#[cfg(test)] module not found after call() — did it move?");
        let call_body = &body[..end];

        // Non-vacuity: if the slicing above ever breaks (e.g. call() is
        // renamed or restructured), fail loudly rather than silently
        // counting zero occurrences in an empty/wrong slice.
        assert!(
            call_body.contains("run_at_structured"),
            "sliced call() body does not contain run_at_structured — \
             slicing logic is broken"
        );

        // Build the needle at runtime so this test's own source text
        // never contains the literal `active_identity()` and cannot
        // inflate its own count.
        let needle = ["active", "identity()"].join("_");
        let count = call_body.matches(&needle).count();
        assert_eq!(
            count, 1,
            "expected exactly one `active_identity()` read in call(), found {count}"
        );
    }
}

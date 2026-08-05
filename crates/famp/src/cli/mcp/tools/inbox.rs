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
//! `famp_inbox` call, directly to the broker's returned `next_offset` (see
//! `call()` below) — no independent `stat()` of the mailbox is taken.
//! `mailbox_env.rs::drain`'s `next_offset` is always a complete-line
//! boundary of the mailbox (it only ever advances `running` past whole
//! `\n`-terminated records, or early-returns `file_len` at EOF), so the
//! on-disk cursor can never land past EOF or mid-record. Because the write
//! fires whenever the target differs from the current on-disk value (not
//! only when it is greater), a corrupt past-EOF cursor self-heals on the
//! very next read. There is still no MCP `ack`; the `since`-based session
//! offset below remains the read-position mechanism callers control.
//!
//! We deliberately do NOT `stat()` the mailbox file ourselves to compute an
//! EOF clamp: `famp-inbox/src/append.rs` writes a record's body and its
//! `\n` terminator as two separate `write_all` calls, so a concurrent
//! `std::fs::metadata().len()` read can observe the file mid-record. Using
//! that value to clamp the cursor could land it inside a record instead of
//! on a boundary — the broker's own `next_offset` is the only value that is
//! atomically consistent with the read it just performed. There is also no
//! `current.max(next_offset)` monotonic floor: the broker computes and
//! returns the file-end boundary regardless of the caller's `since`,
//! including `since: 0`, so a floor would only be protecting against an
//! input the broker cannot produce.
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

    // Single identity snapshot. NOT a concurrency fix — the MCP server
    // loop (`server.rs::run`) is strictly serial: it awaits
    // `dispatch_tool` inline for each request and there is no
    // `tokio::spawn` anywhere under `cli/mcp/`, so a single `famp mcp`
    // process never has two tool calls in flight and no `famp_register`
    // can land between two reads inside this function. Binding once here
    // and reusing the same local for both the broker call's `act_as` and
    // the write-through's cursor filename is a defensive invariant against
    // a future concurrent dispatcher, kept because it is also simply
    // simpler and more obviously correct than reading twice. A structural
    // test pins the single-read property.
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
            // call, purely so the write targets the same name the read was
            // just performed against (see the comment on that binding — the
            // MCP loop is serial, so this is not closing a race).
            //
            // The target is simply the broker's `out.next_offset` — no
            // separate `stat()`, no monotonic floor, no EOF clamp computed
            // here. Per the module doc above, `next_offset` is always a
            // complete-line boundary of the mailbox (it is the broker's own
            // EOF-clamped answer, returned atomically with the read it just
            // did), so it can never be mid-record or past EOF, unlike a
            // `metadata().len()` read of our own (`append.rs` writes a
            // record's body and terminator in two separate `write_all`
            // calls, so a concurrent stat can observe the file mid-record).
            // The write fires on `target != current`, not `target >
            // current`: a regression from a corrupt past-EOF on-disk value
            // down to the broker's real `next_offset` must be allowed to
            // land — that regression IS the self-heal.
            if let Some(name) = identity.as_deref() {
                let dir = bus_dir(&resolve_sock_path()).to_path_buf();
                let current = read_disk_cursor(&dir, name);
                let target = out.next_offset;
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
    // ── single-active_identity-read structural assertion ─────────────

    /// `call()`'s body reads `session::active_identity()` exactly once.
    /// This is a defensive invariant, not a fix for an observed race: the
    /// MCP server loop (`server.rs::run`) is strictly serial — it awaits
    /// `dispatch_tool` inline and there is no `tokio::spawn` anywhere
    /// under `cli/mcp/` — so a single `famp mcp` process never has two
    /// tool calls in flight, and no `famp_register` can land between two
    /// reads here. The single read is kept because it is simpler and more
    /// obviously correct than reading twice, and it stays correct if a
    /// future dispatcher ever becomes concurrent.
    ///
    /// Known weakness: this counts source *text*, not behavior. A future
    /// read via `session::state().lock().await.active_identity` (bypassing
    /// the `active_identity()` helper) would evade this check entirely.
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

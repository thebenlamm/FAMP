//! `famp_register` MCP tool — Phase 02 plan 02-09 implementation.
//!
//! Sends `BusMessage::Register { name, pid, cwd, listen }` to the local broker via the
//! lazily-opened `BusClient` from `cli::mcp::session`, and on `RegisterOk`
//! installs the canonical identity on the per-process session state via
//! [`session::set_active_identity`] (D-04 + D-10).
//!
//! Per D-10, the MCP server is the registered slot for its session — NOT
//! a proxy that rides on a separate `famp register <name>` daemon. So the
//! `pid` field carries the MCP server's own process id (`std::process::id()`).
//!
//! Identity-name validation mirrors the bash regex used by
//! `scripts/famp-local cmd_register` so the CLI surface and the MCP
//! surface agree on what is a valid name: `^[A-Za-z0-9._-]+$`. Names that
//! fail validation are rejected with `BusErrorKind::EnvelopeInvalid`
//! before the broker is contacted.
//!
//! ## Output shape
//!
//! ```json
//! {
//!   "active": "<name>",
//!   "drained": <count>,
//!   "peers": ["..."],
//!   "listen_mode": true,
//!   "wake_readiness": "unknown",
//!   "warning": "...host Stop hook..."
//! }
//! ```
//!
//! `drained` is the *count* of typed envelopes the broker drained on
//! register (Phase-1 D-09 wire shape carries the full envelopes; the MCP
//! tool surfaces only the count, matching `cli::join`'s ergonomics).
//! `peers` is the broker's `connected_names` snapshot at register time.
//! The final three fields are present when listen intent is enabled. They
//! intentionally do not claim host readiness from broker state alone.
//!
//! ## Snapshot vs. live membership
//!
//! `RegisterOk.peers` is a point-in-time snapshot of `connected_names`
//! taken at registration. It does **not** update as later agents join
//! or leave. Callers that need the current membership set must call
//! `famp_peers` (which round-trips `BusMessage::Sessions` to the
//! broker on every invocation, see `tools/peers.rs`).
//!
//! Late-joining agents will not appear in any earlier registrant's
//! `RegisterOk.peers`; this is by design, not a bug.
//!
//! ## `peers.toml` on disk (v0.8 artifact)
//!
//! Any `peers.toml` file under `~/.famp-local/agents/<name>/` is a
//! v0.8 federation trust artifact (Ed25519 pubkey + TLS fingerprint
//! pinning). v0.9's local UDS broker does **not** read it for
//! membership. Treat it as inert; live membership is owned by the
//! broker and surfaced via `famp_peers`.

use famp_bus::{BusErrorKind, BusMessage, BusReply};
use serde_json::Value;

use crate::cli::mcp::session;
use crate::cli::mcp::tools::ToolError;

/// Dispatch a `famp_register` tool call.
pub async fn call(input: &Value) -> Result<Value, ToolError> {
    // Accept both `identity` (v0.8 surface, what existing MCP clients
    // and tests pass) and `name` (the broker's wire field name) so this
    // tool is robust to either spelling.
    let name = input
        .get("identity")
        .and_then(Value::as_str)
        .or_else(|| input.get("name").and_then(Value::as_str))
        .ok_or_else(|| {
            ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                "missing required field: identity (string)",
            )
        })?
        .to_string();
    validate_identity_name(&name)?;
    // Fix 2 (2026-05-12): MCP sessions default to listen-mode ON. Agent
    // windows almost always want auto-wake; the CLI `famp register`
    // entry point retains its own default (set in the CLI subcommand
    // args), so this is a per-surface choice, not a wire change. The
    // Register frame still carries the resolved bool over the bus
    // unchanged.
    let listen = input.get("listen").and_then(Value::as_bool).unwrap_or(true);

    // STRICT: rebind MUST be a JSON boolean if present. Mirrors the
    // include_terminal shape in inbox.rs — a non-bool surfaces a typed
    // error naming both the field and the expected type so an MCP client
    // can self-correct rather than be silently coerced.
    let rebind = match input.get("rebind") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => {
            return Err(ToolError::new(
                BusErrorKind::EnvelopeInvalid,
                "field rebind must be a boolean",
            ));
        }
    };

    session::ensure_bus()
        .await
        .map_err(|(kind, detail)| ToolError::new(kind, detail))?;

    let mut guard = session::state().lock().await;

    // Rebind guard (T-058-01), checked BEFORE the broker round-trip so
    // nothing mutates on the rejected path. `Some(prev) where prev ==
    // name` (the idempotent same-name case, a real recovery affordance
    // after a `/compact` drops the register marker) and `None` (nothing
    // bound yet) both fall through unchanged.
    if let Some(message) = rebind_rejection(guard.active_identity.as_deref(), &name, rebind) {
        drop(guard);
        return Err(ToolError::new(BusErrorKind::EnvelopeInvalid, message));
    }

    let Some(bus) = guard.bus.as_mut() else {
        // ensure_bus() succeeded but the slot is empty — only possible if
        // a concurrent caller cleared `bus` (test code only). Treat as a
        // broker-unreachable since the connection is gone.
        return Err(ToolError::new(
            BusErrorKind::BrokerUnreachable,
            "bus connection closed concurrently",
        ));
    };
    let pid = std::process::id();
    let cwd = std::env::current_dir()
        .ok()
        .map(|path| path.display().to_string());
    let reply = bus
        .send_recv(BusMessage::Register {
            name: name.clone(),
            pid,
            cwd,
            listen,
            // Phase 14 D-01/D-02: an MCP-registered session IS the
            // canonical local holder (per the module doc above, it is
            // never a proxy). Declaring `Local` explicitly here is what
            // keeps this identity's OWN outbound `Send`s from resolving
            // to `Origin::Unknown` (which every rendering surface
            // treats as untrusted) — omission is fail-closed, not a
            // free pass, so this declaration is required for normal
            // local-to-local traffic to render unwrapped.
            origin: Some(famp_bus::Origin::Local),
        })
        .await
        .map_err(|e| {
            ToolError::new(
                BusErrorKind::BrokerUnreachable,
                format!("broker round-trip failed: {e:?}"),
            )
        })?;

    // D1 (260810-hac): record this window's Claude Code host address for
    // the native `SendMessage` wake ping, on a SEPARATE frame issued only
    // after the Register succeeded. See `record_wake_addr`.
    let wake_outcome = if matches!(reply, BusReply::RegisterOk { .. }) {
        record_wake_addr(bus).await
    } else {
        WakeAddrOutcome::Stored
    };
    // Fix round, 260810-hac. A poisoned connection must not stay cached:
    // `session::ensure_bus` returns early whenever `guard.bus.is_some()`,
    // so without this the untrustworthy client serves every subsequent
    // tool call on this session. Failing the registration is the honest
    // consequence — the registration lives on the connection we are
    // discarding, so it no longer holds. Same-name re-register is
    // idempotent and mailboxes are durable per name.
    if let WakeAddrOutcome::ConnectionPoisoned(detail) = wake_outcome {
        guard.bus = None;
        drop(guard);
        return Err(poisoned_connection_error(&detail));
    }

    let result = match reply {
        BusReply::RegisterOk {
            active,
            drained,
            peers,
        } => {
            guard.active_identity = Some(active.clone());
            // #13: reset the remembered inbox cursor offset on identity
            // (re)bind. This is the production rebind path — register
            // binds INLINE on this held mutex guard and cannot call
            // `session::set_active_identity` (that would re-lock this
            // same tokio Mutex and deadlock). A stale byte offset
            // against a different mailbox would read at a meaningless
            // position, so the reset must live here too, not only in
            // `set_active_identity`.
            guard.inbox_offset = None;
            guard.listen_mode = Some(listen);
            // Listen intent is broker state, not proof that a host Stop hook is
            // installed and loaded. Keep registration successful, but make the
            // cross-layer uncertainty explicit. Do NOT advertise a
            // `famp listen-wake --follow`
            // monitor here: in the Stop-hook deployment nothing writes the
            // `.wake` file, so `--follow` blocks forever, and arming
            // `--daemon` opens a second bus waiter that steals messages from
            // the Stop await (the exact race the comment at drop(guard) below
            // avoids internally). The orphaned monitor surface must not be
            // handed to users.
            let mut body = serde_json::json!({
                "active": &active,
                "drained": drained.len(),
                "peers": peers,
            });
            if listen {
                body["listen_mode"] = serde_json::json!(true);
                body["wake_readiness"] = serde_json::json!("unknown");
                body["warning"] = serde_json::json!(
                    "listen mode requires an installed and loaded host Stop hook; for Codex run `famp inspect wake --identity <name>` to verify end-to-end readiness"
                );
            }
            Ok(body)
        }
        BusReply::Err { kind, message } => Err(ToolError::new(kind, message)),
        // `BusReply` is open-coded with many ok-shaped variants. A non-Err,
        // non-RegisterOk reply is a broker protocol violation; surface as
        // Internal so the JSON-RPC layer projects to -32109.
        other => Err(ToolError::new(
            BusErrorKind::Internal,
            format!("unexpected reply to Register: {}", other.variant_name()),
        )),
    };
    drop(guard);
    // The host Stop hook is the intended wake mechanism, but registration
    // cannot prove that it is installed or loaded. Do not arm a listen-wake
    // supervisor here — a second bus waiter races the Stop await.
    result
}

/// Build the rebind-rejection message (T-058-01) when `active` is bound
/// to a DIFFERENT name than `requested` and `rebind` was not passed.
/// Returns `None` when there is nothing to reject: unbound (`active` is
/// `None`), same-name re-register (idempotent — a real recovery
/// affordance after a `/compact` drops the register marker), or
/// `rebind: true` (the explicit takeover opt-in).
///
/// Pure function so the message text — the ONLY signal a subagent gets —
/// is unit-testable without a live broker or session mutex.
fn rebind_rejection(active: Option<&str>, requested: &str, rebind: bool) -> Option<String> {
    let prev = active?;
    if prev == requested || rebind {
        return None;
    }
    Some(format!(
        "this MCP session is already bound to {prev:?}; registering as \
         {requested:?} would rebind it. If you genuinely intend to take this \
         window over and move its binding from {prev:?} to {requested:?}, \
         pass rebind: true. (If you are a subagent: this MCP session is \
         PROCESS-WIDE for the entire window, including every subagent that \
         window spawns, since subagent MCP calls arrive on the parent \
         window's famp mcp process — so registering here would silently \
         hijack the parent window's identity. Subagents must NOT call \
         famp_register.)"
    ))
}

/// The error returned when the cached bus connection had to be discarded
/// after the registration itself had already reached the broker. Names the
/// remedy, because the remedy is cheap and total: same-name re-register is
/// idempotent and mailboxes are durable per name.
fn poisoned_connection_error(detail: &str) -> ToolError {
    ToolError::new(
        BusErrorKind::BrokerUnreachable,
        format!(
            "registration reached the broker, but the bus connection failed \
             immediately afterwards and cannot be reused ({detail}). The \
             connection has been discarded; call famp_register again with the \
             same identity — re-register is idempotent and your mailbox is \
             durable, so nothing queued is lost."
        ),
    )
}

/// Verdict on the cached bus connection after a `SetWakeAddr` round-trip.
#[derive(Debug, PartialEq, Eq)]
enum WakeAddrOutcome {
    /// The address was stored (or there was none to store). The cached
    /// connection is in sync and stays installed.
    Stored,
    /// Nothing was stored, but the stream consumed exactly one well-formed
    /// frame, so it is still aligned. Log and carry on — the only cost is
    /// no wake ping for this window; the Stop hook remains authoritative
    /// (D4). Carries the line to log.
    NotStored(String),
    /// The round-trip failed mid-frame. The connection may be dead, or
    /// desynced by a partially-consumed frame, and MUST NOT be reused.
    /// Carries the detail to surface to the caller.
    ConnectionPoisoned(String),
}

/// Classify a `SetWakeAddr` round-trip. Pure so the connection verdict —
/// the part that is genuinely hard to reason about — is unit-testable
/// without a live broker.
///
/// `transport_err` is the already-formatted `{e:?}` of a `send_recv`
/// error, or `None` when `send_recv` returned `Ok`.
///
/// ## Why `Ok(unexpected)` is NOT a poisoned connection
///
/// `codec::read_frame` reads the length prefix, then `read_exact`s the
/// whole body, then deserializes. Any `Ok` return therefore means one
/// COMPLETE frame was consumed and the stream is aligned on the next
/// frame boundary — regardless of which variant came back. An unexpected
/// variant is a broker protocol oddity worth logging, not a reason to
/// throw away a working connection.
///
/// An `Err` is the opposite. Two of its causes leave the stream desynced
/// rather than merely dead: `FrameTooLarge` consumes the length prefix
/// but not the body, and an `Io` error on the body `read_exact` consumes
/// part of it. A subsequent read on that stream starts mid-body. There is
/// no timeout path in `send_recv`, so we cannot distinguish those from a
/// dead broker at this layer — and every one of them is a connection we
/// must not keep.
fn classify_set_wake_addr(
    sent: &str,
    reply: Option<&BusReply>,
    transport_err: Option<&str>,
) -> WakeAddrOutcome {
    if let Some(detail) = transport_err {
        return WakeAddrOutcome::ConnectionPoisoned(detail.to_string());
    }
    match reply {
        Some(BusReply::SetWakeAddrOk {
            wake_addr: Some(echoed),
        }) if echoed == sent => WakeAddrOutcome::Stored,
        // proto.rs documents the echo precisely so a client can observe
        // that a malformed address stored nothing. Fail-open by design —
        // but say so rather than treating it as success.
        Some(BusReply::SetWakeAddrOk { wake_addr: echoed }) => WakeAddrOutcome::NotStored(format!(
            "famp mcp: broker did not store wake address {sent:?} (echoed {echoed:?}) \
                 — no wake ping for this session; the Stop hook is unaffected"
        )),
        Some(other) => WakeAddrOutcome::NotStored(format!(
            "famp mcp: SetWakeAddr got {} — no wake ping for this session; \
             the Stop hook is unaffected",
            other.variant_name()
        )),
        None => WakeAddrOutcome::ConnectionPoisoned(
            "send_recv returned neither a reply nor an error".to_string(),
        ),
    }
}

/// D1 (260810-hac): issue the `SetWakeAddr` frame for this window, if it
/// has one.
///
/// Uses `parent_id()` — NOT `std::process::id()` — because the address
/// must name the CLAUDE CODE SESSION, and the `famp mcp` server is that
/// session's child (verified fact 1 of the spike: parent pid == session
/// pid == cc-socks basename, 4 of 4).
///
/// No socket on disk — the normal case for a non-Claude host or a session
/// without one (verified fact 4) — sends no frame at all and returns
/// [`WakeAddrOutcome::Stored`].
///
/// ## This is no longer "swallow everything" (fix round, 260810-hac)
///
/// As first shipped this function swallowed transport errors outright, on
/// the stated rule that it "MUST NOT be able to fail registration". That
/// rule rested on a premise that does not hold: it assumed a failed
/// round-trip leaves a REUSABLE connection. It does not — see
/// [`classify_set_wake_addr`] — and `session::ensure_bus` never clears
/// `guard.bus`, so the untrustworthy client stayed installed for every
/// later tool call on the session.
///
/// The caller now drops the cached connection and fails the registration
/// on [`WakeAddrOutcome::ConnectionPoisoned`]. That is not a regression in
/// robustness: the registration LIVES on that connection, so once it is
/// dropped the registration genuinely no longer holds and reporting
/// success would be a lie the Stop hook would then act on. Same-name
/// re-register is idempotent and mailboxes are durable per name, so the
/// remedy is one retry with nothing lost.
async fn record_wake_addr(bus: &mut crate::bus_client::BusClient) -> WakeAddrOutcome {
    let Some(wake_addr) = wake_addr_for_pid(
        std::os::unix::process::parent_id(),
        std::path::Path::new(CC_SOCKS_DIR),
    ) else {
        return WakeAddrOutcome::Stored;
    };
    let outcome = match bus
        .send_recv(BusMessage::SetWakeAddr {
            wake_addr: Some(wake_addr.clone()),
        })
        .await
    {
        Ok(reply) => classify_set_wake_addr(&wake_addr, Some(&reply), None),
        // Log the actual cause, never a bare string — a swallowed error
        // context is a swallowed error.
        Err(e) => classify_set_wake_addr(&wake_addr, None, Some(&format!("{e:?}"))),
    };
    if let WakeAddrOutcome::NotStored(line) = &outcome {
        eprintln!("{line}");
    }
    outcome
}

/// Default directory Claude Code exposes its per-session cross-session
/// messaging sockets in. Verified fact 1 of the 260810-hac spike: the
/// basename is the Claude Code session pid, and the `famp mcp` process's
/// PARENT pid is that same session pid (4 of 4 on the spike box).
const CC_SOCKS_DIR: &str = "/tmp/cc-socks";

/// D1: build the wake address for `pid` under `base_dir`, returning `None`
/// unless the socket actually exists on disk.
///
/// Pure and parameterized on `base_dir` (in the style of
/// [`rebind_rejection`]) so the path-building and existence-gating logic
/// is unit-testable without a live Claude Code host.
///
/// Verified fact 4: not every claude session has a sock, and not every
/// session runs `famp mcp`. `None` is the normal, silent outcome — the
/// caller must treat it as "no ping for this window", never as an error.
fn wake_addr_for_pid(pid: u32, base_dir: &std::path::Path) -> Option<String> {
    let sock = base_dir.join(format!("{pid}.sock"));
    if !sock.exists() {
        return None;
    }
    Some(format!("uds:{}", sock.display()))
}

/// Validate the identity name. Mirrors the bash regex AND length cap from
/// `scripts/famp-local cmd_register`: `^[A-Za-z0-9._-]+$`, ≤64 bytes.
///
/// IN-05: enforce the length cap here so an oversized name fails fast at
/// the MCP boundary with the right error class, not as a confusing
/// downstream error from `famp-core::identity::validate_name_or_instance_id`.
fn validate_identity_name(name: &str) -> Result<(), ToolError> {
    if name.is_empty() {
        return Err(ToolError::new(
            BusErrorKind::EnvelopeInvalid,
            "identity name must not be empty",
        ));
    }
    if name.len() > 64 {
        return Err(ToolError::new(
            BusErrorKind::EnvelopeInvalid,
            format!("identity name length {} exceeds 64 bytes", name.len()),
        ));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(ToolError::new(
            BusErrorKind::EnvelopeInvalid,
            format!("invalid identity name {name:?}: must match [A-Za-z0-9._-]+"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{classify_set_wake_addr, wake_addr_for_pid, WakeAddrOutcome, CC_SOCKS_DIR};
    use famp_bus::BusReply;

    const SENT: &str = "uds:/tmp/cc-socks/8091.sock";

    #[test]
    fn an_echoed_address_matching_what_we_sent_is_stored() {
        assert_eq!(
            classify_set_wake_addr(
                SENT,
                Some(&BusReply::SetWakeAddrOk {
                    wake_addr: Some(SENT.to_string()),
                }),
                None,
            ),
            WakeAddrOutcome::Stored
        );
    }

    #[test]
    fn a_none_echo_is_reported_not_silently_treated_as_stored() {
        // proto.rs documents the echo precisely so a client can observe
        // that a malformed address stored nothing. The pre-fix code
        // matched `SetWakeAddrOk { .. }` and could not tell the two apart.
        let outcome = classify_set_wake_addr(
            SENT,
            Some(&BusReply::SetWakeAddrOk { wake_addr: None }),
            None,
        );
        match outcome {
            WakeAddrOutcome::NotStored(line) => {
                assert!(
                    line.contains(SENT),
                    "the log must name what we sent: {line}"
                );
                assert!(
                    line.contains("Stop hook"),
                    "the log must say what still works: {line}"
                );
            }
            other => panic!("a None echo must be NotStored, got {other:?}"),
        }
    }

    #[test]
    fn an_unexpected_but_well_formed_reply_keeps_the_connection() {
        // DISAGREEMENT WITH THE REVIEW, with evidence. `codec::read_frame`
        // reads the length prefix, `read_exact`s the whole body, then
        // deserializes — so ANY `Ok` return consumed exactly one complete
        // frame and the stream is aligned on the next boundary. An
        // unexpected variant is a broker oddity to log, not a reason to
        // discard a working connection and fail the registration.
        let outcome = classify_set_wake_addr(
            SENT,
            Some(&BusReply::SetListenOk { listen_mode: true }),
            None,
        );
        assert!(
            matches!(outcome, WakeAddrOutcome::NotStored(_)),
            "a well-formed unexpected reply must NOT poison the connection, got {outcome:?}"
        );
    }

    #[test]
    fn a_transport_error_poisons_the_connection_and_carries_the_cause() {
        // `FrameTooLarge` consumes the length prefix but not the body, and
        // an Io error on the body `read_exact` consumes part of it — both
        // leave the stream desynced, and `send_recv` has no timeout path
        // that would let us tell them from a dead broker. Every Err is a
        // connection we must not keep.
        let outcome = classify_set_wake_addr(SENT, None, Some("Io(Custom { kind: BrokenPipe })"));
        match outcome {
            WakeAddrOutcome::ConnectionPoisoned(detail) => assert!(
                detail.contains("BrokenPipe"),
                "the actual cause must survive, not a bare string: {detail}"
            ),
            other => panic!("a transport error must poison the connection, got {other:?}"),
        }
    }

    #[test]
    fn wake_addr_is_none_when_the_socket_does_not_exist() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(wake_addr_for_pid(8091, dir.path()), None);
    }

    #[test]
    fn wake_addr_is_some_when_the_socket_exists() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("8091.sock"), b"").expect("create sock placeholder");
        let addr = wake_addr_for_pid(8091, dir.path()).expect("existing sock yields an address");
        assert!(
            addr.starts_with("uds:"),
            "address must carry the uds: scheme"
        );
        assert!(
            addr.ends_with("/8091.sock"),
            "address must name the pid's socket"
        );
        // A DIFFERENT pid in the same dir must still be absent — the
        // existence check is per-pid, not per-directory.
        assert_eq!(wake_addr_for_pid(9999, dir.path()), None);
    }

    #[test]
    fn wake_addr_under_the_real_cc_socks_dir_passes_broker_validation() {
        // Guards the seam between this helper's output shape and the
        // broker-side regex (`famp_bus::wake_addr_valid`). If either side
        // drifts, every ping silently stops being stored — a failure that
        // is invisible from this crate's tests alone.
        let synthetic = format!("uds:{CC_SOCKS_DIR}/8091.sock");
        assert!(
            famp_bus::wake_addr_valid(&synthetic),
            "helper output shape must satisfy the broker's shape check"
        );
    }
}

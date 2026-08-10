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
    // after the Register succeeded. Best-effort; see `record_wake_addr`.
    if matches!(reply, BusReply::RegisterOk { .. }) {
        record_wake_addr(bus).await;
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

/// D1 (260810-hac): issue the `SetWakeAddr` frame for this window, if it
/// has one.
///
/// Uses `parent_id()` — NOT `std::process::id()` — because the address
/// must name the CLAUDE CODE SESSION, and the `famp mcp` server is that
/// session's child (verified fact 1 of the spike: parent pid == session
/// pid == cc-socks basename, 4 of 4).
///
/// Every path here is best-effort and MUST NOT be able to fail
/// registration. No socket on disk — the normal case for a non-Claude host
/// or a session without one (verified fact 4) — sends no frame at all. A
/// transport error or an unexpected reply is logged to stderr and
/// swallowed. The only consequence of failure is that this window gets no
/// wake ping; the Stop hook remains its authoritative wake path (D4).
async fn record_wake_addr(bus: &mut crate::bus_client::BusClient) {
    let Some(wake_addr) = wake_addr_for_pid(
        std::os::unix::process::parent_id(),
        std::path::Path::new(CC_SOCKS_DIR),
    ) else {
        return;
    };
    match bus
        .send_recv(BusMessage::SetWakeAddr {
            wake_addr: Some(wake_addr),
        })
        .await
    {
        Ok(BusReply::SetWakeAddrOk { .. }) => {}
        Ok(unexpected) => eprintln!(
            "famp mcp: SetWakeAddr got {} — wake ping disabled for this session",
            unexpected.variant_name()
        ),
        Err(_) => eprintln!(
            "famp mcp: SetWakeAddr round-trip failed — wake ping disabled for this session"
        ),
    }
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

    use super::{wake_addr_for_pid, CC_SOCKS_DIR};

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

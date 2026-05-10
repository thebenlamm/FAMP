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
//! { "active": "<name>", "drained": <count>, "peers": ["..."] }
//! ```
//!
//! `drained` is the *count* of typed envelopes the broker drained on
//! register (Phase-1 D-09 wire shape carries the full envelopes; the MCP
//! tool surfaces only the count, matching `cli::join`'s ergonomics).
//! `peers` is the broker's `connected_names` snapshot at register time.
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
    let listen = input
        .get("listen")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    session::ensure_bus()
        .await
        .map_err(|kind| ToolError::new(kind, "failed to connect to local broker"))?;

    let mut guard = session::state().lock().await;
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
        })
        .await
        .map_err(|e| {
            ToolError::new(
                BusErrorKind::BrokerUnreachable,
                format!("broker round-trip failed: {e:?}"),
            )
        })?;

    let result = match reply {
        BusReply::RegisterOk {
            active,
            drained,
            peers,
        } => {
            guard.active_identity = Some(active.clone());
            Ok(serde_json::json!({
                "active": active,
                "drained": drained.len(),
                "peers": peers,
            }))
        }
        BusReply::Err { kind, message } => Err(ToolError::new(kind, message)),
        // `BusReply` is open-coded with many ok-shaped variants. A non-Err,
        // non-RegisterOk reply is a broker protocol violation; surface as
        // Internal so the JSON-RPC layer projects to -32109.
        other => Err(ToolError::new(
            BusErrorKind::Internal,
            format!("unexpected reply to Register: {other:?}"),
        )),
    };
    drop(guard);
    result
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

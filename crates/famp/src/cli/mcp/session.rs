//! Per-process MCP session state — Phase 2 reshape (D-04 + D-10).
//!
//! ## Why module-scope, not per-session-id keyed
//!
//! MCP stdio transport launches one `famp mcp` subprocess per client
//! window (Claude Code and Codex both do this). Within a single process
//! there is exactly one session. So session state collapses to a single
//! `OnceLock` + `Mutex` over [`SessionState`] at module scope. We do **not**
//! introduce a `HashMap<SessionId, _>` — there is no second session to
//! key off.
//!
//! ## Phase 2 shape
//!
//! Two pieces of state:
//!
//! - the optional `BusClient` field — the long-lived UDS connection to
//!   the local broker. Lazily opened on first tool call via [`ensure_bus`].
//! - the optional canonical-identity field — the name this MCP server
//!   has registered as. `None` until [`set_active_identity`] is called
//!   by `tools::register::call` after a successful `RegisterOk`.
//!
//! ## D-10: MCP is the registered slot, NOT a proxy
//!
//! Per D-10 (CONTEXT.md), the MCP server is a real long-lived process
//! that calls `famp_register` and BECOMES THE registered slot for its
//! session — it is **not** a proxy that rides on someone else's
//! `famp register` daemon. So [`ensure_bus`] opens the [`BusClient`] with
//! `bind_as: None` (canonical-holder shape). The `Register` frame
//! (sent by `tools/register.rs::call`) is what later sets the
//! canonical `state.name` for the connection on the broker side.
//!
//! Contrast: one-shot CLI subcommands (`famp send`, `famp inbox list`,
//! …) connect with `bind_as: Some(name)` and ride on a long-running
//! `famp register <name>` daemon. The MCP server stands in for that
//! daemon within a Claude Code window's lifetime.
//!
//! ## Concurrency
//!
//! `tokio::sync::Mutex`: the only writer is `tools::register::call`
//! and reads happen at most once per in-flight tool call (stdio is
//! serially driven). Contention is structurally bounded.

use std::sync::OnceLock;

use famp_bus::BusErrorKind;
use tokio::sync::Mutex;

use crate::bus_client::BusClient;

/// Metadata about the most-recent successful `famp_send` from this
/// session. Recorded by `tools::send::call` on the `Ok` arm and surfaced
/// by `tools::whoami::call` so an agent can recover when Claude Code's
/// stdio transport drops the `famp_send` tool result mid-flight
/// (`[Tool result missing due to internal error]`). The agent then calls
/// `famp_whoami` to learn the `task_id` that was assigned and decides
/// whether to retry or (more usefully) call `famp_verify` to confirm
/// delivery before retrying.
///
/// Exactly one of `to_peer` / `to_channel` is populated, matching the
/// `BusMessage::Send.to: Target` discriminant on the wire. `ts` is an
/// RFC 3339 UTC timestamp captured at the moment the broker replied
/// `SendOk` — i.e. proof that the call reached the broker, regardless
/// of whether the JSON-RPC response surfaced to the model.
///
/// ## `task_id` vs `thread_task_id` — verify semantics
///
/// For `mode="open"`, `task_id` is the new task's uuid AND the value
/// the inspector RPC will return as `MessageRow.task_id` (since the
/// envelope has no `causality.ref`, the inspector falls through to
/// `envelope.id`). One value, one verify path.
///
/// For `mode="reply"`, the broker returns `task_id = <new envelope id>`
/// in `SendOk` (which is what `task_id` here records), but the
/// inspector projects each row's `task_id` from `causality.ref` FIRST
/// (`famp_inspect_server::envelope_task_id`). That means reply
/// envelopes are keyed in the inspector by the THREAD's task id, not
/// by the reply's own envelope id. To verify a reply landed,
/// `famp_verify` must look up the thread id — surfaced here as
/// `thread_task_id`. It is `Some` only for reply-mode sends; `None`
/// for `open` (where it would duplicate `task_id`).
#[derive(Debug, Clone, serde::Serialize)]
pub struct LastSend {
    pub task_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_peer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_channel: Option<String>,
    pub ts: String,
}

/// The MCP server's per-process session state.
///
/// `bus` is lazily opened by [`ensure_bus`] on first call. `active_identity`
/// is set by `tools::register::call` after the broker confirms `RegisterOk`.
/// `last_send` is set by `tools::send::call` on every successful send so
/// `tools::whoami::call` can surface it as a recovery hint when Claude
/// Code drops the `famp_send` tool result.
pub struct SessionState {
    pub bus: Option<BusClient>,
    pub active_identity: Option<String>,
    pub last_send: Option<LastSend>,
}

/// Module-scope storage for the session state.
///
/// `OnceLock` initializes the `Mutex<SessionState>` lazily on first
/// access. The interior `Option` fields distinguish the four phases of
/// the session lifecycle:
///
/// | `bus`   | `active_identity` | Meaning                                    |
/// |---------|-------------------|--------------------------------------------|
/// | `None`  | `None`            | Pristine — server just started.            |
/// | `Some`  | `None`            | `ensure_bus` ran; not yet registered.      |
/// | `Some`  | `Some`            | Registered; ready to dispatch tool calls.  |
/// | `None`  | `Some`            | Unreachable — `set_active_identity` only   |
/// |         |                   | runs after a successful Register, which    |
/// |         |                   | requires the bus to be open.               |
pub fn state() -> &'static Mutex<SessionState> {
    static S: OnceLock<Mutex<SessionState>> = OnceLock::new();
    S.get_or_init(|| {
        Mutex::new(SessionState {
            bus: None,
            active_identity: None,
            last_send: None,
        })
    })
}

/// Open the `BusClient` if not already connected. Idempotent.
///
/// Per D-10, the MCP server is the registered slot for its session, NOT
/// a proxy. So the connection is opened with `bind_as: None`. The
/// `tools::register::call` site is responsible for sending the
/// `Register` frame that turns this anonymous-but-connected slot into
/// the canonical holder of the session's identity.
///
/// On first call: spawns the broker if absent, opens the UDS, performs
/// the BUS-06 Hello handshake. On subsequent calls: returns `Ok(())`
/// immediately (no I/O).
///
/// Errors are projected onto [`BusErrorKind`] so MCP-10's
/// exhaustive-match downstream catches them.
pub async fn ensure_bus() -> Result<(), BusErrorKind> {
    // WR-04: hold the lock across `BusClient::connect` so concurrent
    // callers can't both run the broker-spawn + Hello handshake and
    // then drop the loser's freshly-connected client on the floor
    // (which would leak a broker accept + handshake + a stranded
    // ClientState entry, plus a spurious broker.log line). Per the
    // module comment above, contention is structurally bounded —
    // stdio MCP serializes tool calls — so holding the lock is fine.
    let mut guard = state().lock().await;
    if guard.bus.is_some() {
        return Ok(());
    }
    let sock = crate::bus_client::resolve_sock_path();
    let client = BusClient::connect(&sock, None)
        .await
        .map_err(|_| BusErrorKind::BrokerUnreachable)?;
    guard.bus = Some(client);
    drop(guard);
    Ok(())
}

/// Read the active identity (a clone of the inner `Option<String>`).
/// `None` until `tools::register::call` has set it after a successful
/// `RegisterOk` from the broker.
pub async fn active_identity() -> Option<String> {
    state().lock().await.active_identity.clone()
}

/// Set the active identity. Called by `tools::register::call` after the
/// broker confirms `RegisterOk { name }`.
pub async fn set_active_identity(name: String) {
    state().lock().await.active_identity = Some(name);
}

/// Record metadata for the most-recent successful `famp_send`. Called
/// by `tools::send::call` on the `Ok` arm. Surfaced by `tools::whoami::call`
/// as a recovery hint for the Claude Code `[Tool result missing due to
/// internal error]` failure mode (the broker delivered the message but
/// the JSON-RPC result never reached the model).
pub async fn set_last_send(record: LastSend) {
    state().lock().await.last_send = Some(record);
}

/// Read the most-recent `LastSend` record (cloned). `None` until the
/// session has performed at least one successful `famp_send`.
pub async fn last_send() -> Option<LastSend> {
    state().lock().await.last_send.clone()
}

/// Clear all session state. Intentionally only available under `cfg(test)`
/// — production code never resets a session within a single process
/// lifetime.
#[cfg(test)]
pub async fn clear() {
    let mut guard = state().lock().await;
    guard.bus = None;
    guard.active_identity = None;
    guard.last_send = None;
}

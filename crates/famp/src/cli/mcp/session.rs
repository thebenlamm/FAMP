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

/// The MCP server's per-process session state.
///
/// `bus` is lazily opened by [`ensure_bus`] on first call. `active_identity`
/// is set by `tools::register::call` after the broker confirms `RegisterOk`.
pub struct SessionState {
    pub bus: Option<BusClient>,
    pub active_identity: Option<String>,
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

/// Clear all session state. Intentionally only available under `cfg(test)`
/// — production code never resets a session within a single process
/// lifetime.
#[cfg(test)]
pub async fn clear() {
    let mut guard = state().lock().await;
    guard.bus = None;
    guard.active_identity = None;
}

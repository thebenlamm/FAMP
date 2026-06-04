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

use std::path::Path;
use std::sync::OnceLock;

use famp_bus::BusErrorKind;
use tokio::sync::Mutex;

use crate::bus_client::{spawn, BusClient, BusClientError};

/// Metadata about the most-recent successful `famp_send` from this session.
///
/// Recorded by `tools::send::call` on the `Ok` arm and surfaced by
/// `tools::whoami::call` so an agent can recover when Claude Code's stdio
/// transport drops the `famp_send` tool result mid-flight
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

/// Map a [`BusClientError`] from the connect stage to a human-readable
/// detail string, binding the inner [`std::io::Error`] so the OS error
/// number is always included. The kind is always
/// [`BusErrorKind::BrokerUnreachable`]; only the detail varies.
///
/// This is a pure function (no I/O) so unit tests can exercise every arm
/// without a live broker.
fn bus_err_detail(err: BusClientError, sock: &Path) -> String {
    match err {
        BusClientError::Io(io) => format!(
            "could not connect to existing broker at {}: {io}",
            sock.display()
        ),
        BusClientError::BrokerDidNotStart(spawn_err) => match spawn_err {
            spawn::SpawnError::Io(io) => spawn_io_detail(io),
            spawn::SpawnError::SandboxEperm => spawn::SpawnError::SandboxEperm.to_string(),
            spawn::SpawnError::CurrentExe(io) => format!(
                "tried to spawn a broker but could not locate the famp executable (current-exe: {io})"
            ),
            spawn::SpawnError::BrokerDidNotStart => format!(
                "spawned a broker but it did not bind {} within 2s — check the broker log at {}",
                sock.display(),
                sock.parent()
                    .unwrap_or_else(|| Path::new("/"))
                    .join("broker.log")
                    .display()
            ),
            spawn::SpawnError::SocketPathNotUtf8 => format!(
                "broker socket path is not valid UTF-8: {}",
                sock.display()
            ),
        },
        BusClientError::HelloFailed { kind, message } => {
            format!("broker refused the Hello handshake: {kind:?}: {message}")
        }
        BusClientError::Frame(err) => format!("frame codec error talking to broker: {err}"),
        BusClientError::Decode(err) => format!("strict-parse of broker reply failed: {err}"),
        BusClientError::UnexpectedReply(msg) => format!("unexpected broker reply: {msg}"),
    }
}

fn spawn_io_detail(io: std::io::Error) -> String {
    let sandbox_hint = if matches!(
        io.raw_os_error(),
        Some(code) if code == libc::EPERM || code == libc::EACCES
    ) {
        " — if running inside a sandbox, broker process creation (fork/setsid) may be blocked; start a broker outside the sandbox or set FAMP_BUS_SOCKET to a reachable broker"
    } else {
        ""
    };
    format!("tried to spawn a broker and process creation failed (spawn io: {io}){sandbox_hint}")
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
/// Errors are projected onto a `(kind, detail)` tuple where `kind` is
/// always [`BusErrorKind::BrokerUnreachable`] and `detail` carries a
/// stage-aware message including the OS error number. Callers destructure
/// via `.map_err(|(kind, detail)| ToolError::new(kind, detail))`.
pub async fn ensure_bus() -> Result<(), (BusErrorKind, String)> {
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
        .map_err(|e| (BusErrorKind::BrokerUnreachable, bus_err_detail(e, &sock)))?;
    guard.bus = Some(client);
    drop(guard);
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::bus_client::spawn;

    /// `BusClientError::Io` maps to a connect-stage message containing the
    /// socket path AND the os-error text. ECONNREFUSED-ish (errno 111).
    #[test]
    fn bus_err_detail_io_contains_socket_and_errno() {
        let io_err = std::io::Error::from_raw_os_error(111);
        let sock = Path::new("/tmp/famp-test-k2p.sock");
        let detail = bus_err_detail(BusClientError::Io(io_err), sock);
        assert!(
            detail.contains("os error"),
            "expected 'os error' in detail, got: {detail}"
        );
        assert!(
            detail.contains("famp-test-k2p.sock"),
            "expected socket path in detail, got: {detail}"
        );
    }

    /// `BusClientError::BrokerDidNotStart(SpawnError::Io)` — fork/setsid
    /// blocked by a sandbox — maps to a spawn-stage message containing
    /// the os-error text, "sandbox", and "spawn".
    #[test]
    fn bus_err_detail_broker_did_not_start_spawn_io_contains_errno_and_sandbox() {
        let io_err = std::io::Error::from_raw_os_error(1); // EPERM — fork/setsid class
        let sock = Path::new("/tmp/famp-test-k2p.sock");
        let detail = bus_err_detail(
            BusClientError::BrokerDidNotStart(spawn::SpawnError::Io(io_err)),
            sock,
        );
        assert!(
            detail.contains("os error"),
            "expected 'os error' in detail (errno must not be swallowed), got: {detail}"
        );
        assert!(
            detail.contains("sandbox"),
            "expected 'sandbox' hint in detail, got: {detail}"
        );
        assert!(
            detail.contains("spawn"),
            "expected 'spawn' in detail, got: {detail}"
        );
    }

    #[test]
    fn bus_err_detail_sandbox_eperm_contains_remedy() {
        let sock = Path::new("/tmp/famp-test-k2p.sock");
        let detail = bus_err_detail(
            BusClientError::BrokerDidNotStart(spawn::SpawnError::SandboxEperm),
            sock,
        );
        assert!(
            detail.contains("sandbox"),
            "expected 'sandbox' in detail, got: {detail}"
        );
        assert!(
            detail.contains("famp daemon install"),
            "expected install remedy in detail, got: {detail}"
        );
    }

    #[test]
    fn bus_err_detail_non_eperm_spawn_io_does_not_claim_sandbox() {
        let io_err = std::io::Error::from_raw_os_error(2); // ENOENT, not sandbox EPERM/EACCES.
        let sock = Path::new("/tmp/famp-test-k2p.sock");
        let detail = bus_err_detail(
            BusClientError::BrokerDidNotStart(spawn::SpawnError::Io(io_err)),
            sock,
        );
        assert!(
            !detail.contains("sandbox"),
            "non-EPERM spawn io must not claim sandbox, got: {detail}"
        );
    }

    /// `BusClientError::BrokerDidNotStart(SpawnError::BrokerDidNotStart)` —
    /// genuine 2s timeout, no errno — maps to a message mentioning the
    /// timeout and pointing at the broker log. Must NOT claim an os error.
    #[test]
    fn bus_err_detail_broker_did_not_start_timeout_points_at_log() {
        let sock = Path::new("/tmp/famp-test-k2p.sock");
        let detail = bus_err_detail(
            BusClientError::BrokerDidNotStart(spawn::SpawnError::BrokerDidNotStart),
            sock,
        );
        assert!(
            detail.contains("2s") || detail.contains("2 s"),
            "expected 2s timeout mention in detail, got: {detail}"
        );
        assert!(
            detail.contains("broker.log"),
            "expected broker log pointer in detail, got: {detail}"
        );
        assert!(
            !detail.contains("os error"),
            "genuine timeout has no os error, but detail claims one: {detail}"
        );
    }

    /// JSON-RPC code regression: `BusErrorKind::BrokerUnreachable` must
    /// map to code -32108 (unchanged). Calls the real `const fn`.
    #[test]
    fn broker_unreachable_jsonrpc_code_is_minus_32108() {
        assert_eq!(
            crate::cli::mcp::error_kind::bus_error_to_jsonrpc(BusErrorKind::BrokerUnreachable).0,
            -32108
        );
    }
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

/// Record metadata for the most-recent successful `famp_send`.
///
/// Called by `tools::send::call` on the `Ok` arm. Surfaced by `tools::whoami::call`
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

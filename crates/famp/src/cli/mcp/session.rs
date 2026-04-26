//! Per-process MCP session state — the current identity binding.
//!
//! ## Why module-scope, not per-session-id keyed
//!
//! MCP stdio transport launches one `famp mcp` subprocess per client
//! window (Claude Code and Codex both do this). Within a single process
//! there is exactly one session. So session state collapses to a single
//! `Mutex<Option<IdentityBinding>>` at module scope. We do **not**
//! introduce a `HashMap<SessionId, IdentityBinding>` — there is no
//! second session to key off.
//!
//! ## Concurrency
//!
//! `tokio::sync::Mutex` (not `RwLock`): the only writer is `famp_register`
//! and reads happen at most once per in-flight tool call (stdio is
//! serially driven). Contention is structurally bounded; the simpler
//! primitive is the right pick.
//!
//! ## v0.9 sunset
//!
//! This module exists to validate the v0.9 `famp_register` tool
//! contract on the v0.8 substrate. The transport underneath is replaced
//! by the local-first bus in v0.9; the binding type itself does not
//! survive — the v0.9 broker carries identity in a different shape. Do
//! NOT promote this type to a sub-crate or re-export from `famp` root.

use std::path::PathBuf;
use std::sync::OnceLock;

use tokio::sync::Mutex;

/// How the current binding was established.
///
/// Under variant **B-strict** (per `01-CONTEXT.md`), there is exactly
/// one variant: `Explicit`. No `LegacyFampHome` variant exists —
/// `famp mcp` does not honor `FAMP_HOME` at startup. If a future plan
/// needs to add a source, the `mcp_error_kind_exhaustive` test corpus
/// and the `famp_whoami` JSON shape both must be updated in lockstep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingSource {
    /// The session called `famp_register` and the identity name
    /// resolved to a valid `$FAMP_LOCAL_ROOT/agents/<name>/` directory.
    Explicit,
}

/// The session's current identity binding.
///
/// `home` is the resolved directory the existing `cli::send`,
/// `cli::await_cmd`, `cli::inbox`, `cli::peer` modules already accept
/// as their first argument. Tools in `cli::mcp::tools::*` resolve
/// `home` via `current()` instead of via `home::resolve_famp_home()`.
#[derive(Debug, Clone)]
pub struct IdentityBinding {
    pub identity: String,
    pub home: PathBuf,
    pub source: BindingSource,
}

/// Module-scope storage for the current binding.
///
/// `OnceLock` initializes the `Mutex` lazily on first access. The
/// `Option` inside the mutex distinguishes "no register yet"
/// (`None`) from "registered" (`Some`). Tools translate `None` to
/// `CliError::NotRegistered`.
fn state() -> &'static Mutex<Option<IdentityBinding>> {
    static STATE: OnceLock<Mutex<Option<IdentityBinding>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}

/// Read the current binding (a clone, so callers don't hold the lock
/// across `.await` of downstream operations like `inbox.append`).
/// `None` means the session has not yet called `famp_register`.
pub async fn current() -> Option<IdentityBinding> {
    state().lock().await.clone()
}

/// Set the current binding, replacing any prior value.
/// Returns the previous binding (for logging / debugging).
/// Per CONTEXT.md "`famp_register` always wins": this is the
/// authoritative idempotent setter.
pub async fn set(binding: IdentityBinding) -> Option<IdentityBinding> {
    let mut guard = state().lock().await;
    guard.replace(binding)
}

/// Clear the current binding. Provided for completeness and
/// (future) test-harness use; not currently called from any
/// production path. Tests in 01-02 may call this between cases.
#[cfg_attr(not(test), allow(dead_code))]
pub async fn clear() -> Option<IdentityBinding> {
    let mut guard = state().lock().await;
    guard.take()
}

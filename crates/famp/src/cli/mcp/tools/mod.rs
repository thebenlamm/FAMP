//! MCP tool handlers — thin wrappers over existing `cli::*` entry points.
//!
//! All `tools::<X>::call` functions return [`Result<Value, ToolError>`].
//! `ToolError` carries both the typed `BusErrorKind` discriminator and a
//! free-form message string so the JSON-RPC error frame can include
//! field-naming hints that the bare enum cannot carry. The dispatcher in
//! `server.rs` projects `ToolError` onto the JSON-RPC `(code, message)`
//! pair via [`ToolError::into_parts`].

use famp_bus::BusErrorKind;

use crate::cli::error::CliError;

pub mod await_;
pub mod channel_log;
pub mod inbox;
pub mod inspect_waiters;
pub mod join;
pub mod leave;
pub mod peers;
pub mod register;
pub mod send;
pub mod set_listen;
pub mod verify;
pub mod whoami;

/// A typed tool error.
///
/// Carries both the `BusErrorKind` discriminator (used to populate
/// `data.famp_error_kind` and the JSON-RPC error code via the MCP-10
/// exhaustive map) and a human-readable message (used as the JSON-RPC
/// `error.message`). Constructed by every tool body's error path.
#[derive(Debug, Clone)]
pub struct ToolError {
    pub kind: BusErrorKind,
    pub message: String,
}

impl ToolError {
    /// Build a `ToolError` from a kind and any `Into<String>` message.
    pub fn new(kind: BusErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// The canonical "session is not registered; call `famp_register` first"
    /// shape returned by every binding-required tool when the dispatcher's
    /// pre-registration gate refuses the call (D-05).
    pub fn not_registered() -> Self {
        Self::new(
            BusErrorKind::NotRegistered,
            "session is not registered; call famp_register first",
        )
    }

    /// Decompose into `(kind, message)` — used by `server.rs::dispatch_tool`
    /// to build the JSON-RPC error frame.
    #[must_use]
    pub fn into_parts(self) -> (BusErrorKind, String) {
        (self.kind, self.message)
    }
}

/// Centralized `CliError -> ToolError` mapping — the single source of truth
/// for how the MCP tool layer projects a CLI entry-point error onto a typed
/// tool error.
///
/// Every tool that delegates to a `cli::*::run_at_structured` entry point
/// funnels its `Err` arm through this impl (`Err(e) => Err(e.into())`)
/// instead of re-spelling the identical 5-arm match. Mapping rules:
///
/// - [`CliError::BusError`] → preserve the broker's `kind` + `message`.
/// - [`CliError::NotRegisteredHint`] → the canonical
///   [`ToolError::not_registered`] shape (the holder `name` is intentionally
///   dropped; the MCP surface uses the fixed "call `famp_register` first" hint).
/// - [`CliError::BrokerUnreachable`] → `BrokerUnreachable` + `"broker unreachable"`.
/// - [`CliError::SendArgsInvalid`] → `EnvelopeInvalid` + the `reason`. Only
///   `famp_send` can actually produce this variant; folding it in here is
///   invisible to the other tools (their entry points never return it) and
///   keeps the mapping table in one place.
/// - everything else → `Internal` + the error's `Display` string (identical to
///   the previous per-tool `Err(e) => ToolError::new(Internal, e.to_string())`).
impl From<CliError> for ToolError {
    fn from(err: CliError) -> Self {
        match err {
            CliError::BusError { kind, message } => Self::new(kind, message),
            CliError::NotRegisteredHint { .. } => Self::not_registered(),
            CliError::BrokerUnreachable => {
                Self::new(BusErrorKind::BrokerUnreachable, "broker unreachable")
            }
            CliError::SendArgsInvalid { reason } => {
                Self::new(BusErrorKind::EnvelopeInvalid, reason)
            }
            other => Self::new(BusErrorKind::Internal, other.to_string()),
        }
    }
}

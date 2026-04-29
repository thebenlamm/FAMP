//! MCP tool handlers — thin wrappers over existing `cli::*` entry points.
//!
//! All `tools::<X>::call` functions return [`Result<Value, ToolError>`].
//! `ToolError` carries both the typed `BusErrorKind` discriminator and a
//! free-form message string so the JSON-RPC error frame can include
//! field-naming hints that the bare enum cannot carry. The dispatcher in
//! `server.rs` projects `ToolError` onto the JSON-RPC `(code, message)`
//! pair via [`ToolError::into_parts`].

use famp_bus::BusErrorKind;

pub mod await_;
pub mod inbox;
pub mod join;
pub mod leave;
pub mod peers;
pub mod register;
pub mod send;
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

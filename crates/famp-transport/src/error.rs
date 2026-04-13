//! Narrow, phase-local error enum for `MemoryTransport`.
//!
//! Per v0.6 precedent (`KeyringError`, `TaskFsmError`), this enum does NOT
//! convert into `ProtocolErrorKind` at the transport boundary. Mapping to
//! the protocol-level error happens in `crates/famp/src/runtime/`.

use famp_core::Principal;

#[derive(Debug, thiserror::Error)]
pub enum MemoryTransportError {
    #[error("unknown recipient: {principal}")]
    UnknownRecipient { principal: Principal },

    #[error("inbox closed for: {principal}")]
    InboxClosed { principal: Principal },
}

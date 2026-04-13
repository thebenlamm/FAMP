//! FAMP byte-oriented transport trait.
//!
//! Transports carry raw wire bytes between principals. They know nothing
//! about envelopes, canonicalization, signatures, or the task FSM — that
//! composition lives in the top `famp` crate runtime glue (Phase 3 D-D1).
//!
//! Native AFIT (Rust 1.75+); workspace pins 1.87. No `async-trait` macro.

#![forbid(unsafe_code)]

pub mod error;
pub mod memory;

use famp_core::Principal;

pub use error::MemoryTransportError;
pub use memory::MemoryTransport;

/// One addressed, opaque wire message. `bytes` is a canonicalized signed
/// envelope JSON payload — transport does not inspect it.
#[derive(Debug, Clone)]
pub struct TransportMessage {
    pub sender: Principal,
    pub recipient: Principal,
    pub bytes: Vec<u8>,
}

/// Byte-oriented, principal-addressed async transport.
///
/// Implementors: `MemoryTransport` (this crate, in-process), `HttpTransport`
/// (Phase 4, `famp-transport-http`). Both use the same error-as-associated-
/// type shape so runtime glue can be generic without boxing.
pub trait Transport {
    type Error: std::error::Error + Send + Sync + 'static;

    fn send(
        &self,
        msg: TransportMessage,
    ) -> impl std::future::Future<Output = Result<(), Self::Error>> + Send;

    fn recv(
        &self,
        as_principal: &Principal,
    ) -> impl std::future::Future<Output = Result<TransportMessage, Self::Error>> + Send;
}

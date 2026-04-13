//! Wire-plumbing only — NOT public API per CONTEXT.md D-A3.
//!
//! The full `WireEnvelope` struct and decode dispatch land in Plan 03.
//! Task 1 / Plan 01-01 only needs the signature-field name constant so
//! later plans can reference it without touching wire.rs again.

#[allow(dead_code)] // wired in Plan 03
pub(crate) const SIGNATURE_FIELD: &str = "signature";

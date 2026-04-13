//! `canonicalize` free function + `Canonicalize` blanket trait.
//!
//! Per CONTEXT.md D-01..D-03 / D-02: the free function is the primary path,
//! the trait is a thin sugar layer that delegates to it. Both are exported
//! from the crate root.

use serde::Serialize;

use crate::error::CanonicalError;

/// Canonicalize any `serde::Serialize` value to RFC 8785 JCS bytes.
///
/// This is the primary entry point for producing the byte string that gets
/// signed by `famp-crypto`. Internally delegates to `serde_jcs::to_vec`; if
/// the SEED-001 conformance gate (Plan 03) decides `serde_jcs` is not
/// trustworthy, the swap-out happens **here** and nowhere else.
///
/// # Errors
///
/// - [`CanonicalError::NonFiniteNumber`] if `value` serializes to a `NaN` or
///   `±Infinity` (RFC 8785 §3.2.2.2 forbids these).
/// - [`CanonicalError::Serialize`] for any other upstream serde failure.
pub fn canonicalize<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, CanonicalError> {
    serde_jcs::to_vec(value).map_err(CanonicalError::from_serde)
}

/// Sugar trait: `value.canonicalize()` instead of `canonicalize(&value)`.
///
/// Implemented for every `Serialize` type via blanket impl per D-02, so
/// callers do not have to import this trait to use the free function — but
/// can if they prefer the method-call form.
pub trait Canonicalize: Serialize {
    /// Equivalent to [`crate::canonical::canonicalize`] called with `self`.
    fn canonicalize(&self) -> Result<Vec<u8>, CanonicalError> {
        crate::canonical::canonicalize(self)
    }
}

impl<T: Serialize + ?Sized> Canonicalize for T {}

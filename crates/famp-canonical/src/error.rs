//! Phase 1 error surface for `famp-canonical`.
//!
//! Per CONTEXT.md D-16/D-17/D-18: keep this enum narrow and phase-appropriate.
//! The 15-category protocol error enum lives in `famp-core` (Phase 3) — do
//! NOT widen `CanonicalError` to model protocol-level dispositions here.

use thiserror::Error;

/// Errors returned by `famp-canonical` public API surfaces.
///
/// Variants are locked by Phase 1 CONTEXT.md D-17 and must not be reordered or
/// renamed without updating the SEED-001 fallback plan and all downstream
/// consumers.
#[derive(Debug, Error)]
pub enum CanonicalError {
    /// `serde_jcs` (or its inner `serde_json` writer) failed to serialize the
    /// input value into canonical bytes for a reason other than a non-finite
    /// number. Wraps the upstream `serde_json::Error`.
    #[error("serialization failed: {0}")]
    Serialize(serde_json::Error),

    /// Inbound JSON bytes on the strict-parse path were malformed (token
    /// error, EOF mid-value, invalid UTF-8, etc.).
    #[error("invalid JSON input: {0}")]
    InvalidJson(serde_json::Error),

    /// A duplicate object key was detected during `from_slice_strict` /
    /// `from_str_strict`. This is a FAMP protocol guarantee, not a hygiene
    /// preference (D-04..D-07).
    #[error("duplicate key in JSON object: {key:?}")]
    DuplicateKey {
        /// The duplicate key as it appeared in the input bytes.
        key: String,
    },

    /// A `NaN` or `±Infinity` value was encountered. RFC 8785 §3.2.2.2
    /// forbids these in canonical JSON.
    #[error("non-finite number (NaN or Infinity) not permitted by RFC 8785")]
    NonFiniteNumber,

    /// Escape hatch for any internal `serde_jcs` failure that does not map
    /// cleanly into the variants above. Should be vanishingly rare in
    /// practice; if it fires, that is evidence for the SEED-001 fallback.
    #[error("internal canonicalization error: {0}")]
    InternalCanonicalization(String),
}

impl CanonicalError {
    /// Classify a `serde_json::Error` raised from `serde_jcs::to_vec`.
    ///
    /// `serde_jcs` returns a `serde_json::Error` for both ordinary
    /// serialization failures and "Number out of range" (NaN / ±Infinity).
    /// We inspect the error's `Display` output to distinguish, which matches
    /// the approach documented in RESEARCH Pattern 2.
    pub(crate) fn from_serde(e: serde_json::Error) -> Self {
        let msg = e.to_string();
        if msg.contains("NaN") || msg.contains("infinit") || msg.contains("Infinit") {
            CanonicalError::NonFiniteNumber
        } else {
            CanonicalError::Serialize(e)
        }
    }
}

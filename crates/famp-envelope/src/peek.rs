//! Pre-decode sender extraction (lifted from `famp/src/runtime/peek.rs` in
//! Phase 4 Plan 04-01).
//!
//! `AnySignedEnvelope::decode` requires a `TrustedVerifyingKey` upfront,
//! but a receiver can only look up that key in a keyring after knowing the
//! sender. Strict-parse wire bytes (rejecting duplicate keys per
//! famp-canonical) to extract only the `from` field, parsed as a `Principal`,
//! before any signature verification runs.
//!
//! Lives in `famp-envelope` (not `crates/famp`) so both the runtime glue AND
//! the `famp-transport-http` sig-verify middleware can call the same
//! canonical two-phase decode shape without `famp-transport-http` depending
//! on `crates/famp` (Pitfall 3 in 04-RESEARCH.md).

use crate::error::EnvelopeDecodeError;
use famp_canonical::from_slice_strict;
use famp_core::Principal;
use std::str::FromStr;

/// Strictly parse wire bytes (duplicate-key-rejecting) and extract the `from`
/// field as a [`Principal`]. Performs NO signature verification.
pub fn peek_sender(bytes: &[u8]) -> Result<Principal, EnvelopeDecodeError> {
    // from_slice_strict returns famp_canonical::CanonicalError on failure;
    // EnvelopeDecodeError::MalformedJson wraps that via #[from].
    let value: serde_json::Value = from_slice_strict(bytes)?;
    let from_str = value
        .get("from")
        .and_then(serde_json::Value::as_str)
        .ok_or(EnvelopeDecodeError::MissingField { field: "from" })?;
    Principal::from_str(from_str)
        .map_err(|_| EnvelopeDecodeError::MissingField { field: "from" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peek_sender_extracts_from_field() {
        let bytes = br#"{"from":"agent:local/alice","to":"agent:local/bob"}"#;
        let p = peek_sender(bytes).expect("peek");
        assert_eq!(p.to_string(), "agent:local/alice");
    }

    #[test]
    fn peek_sender_rejects_missing_from() {
        let bytes = br#"{"to":"agent:local/bob"}"#;
        let err = peek_sender(bytes).unwrap_err();
        assert!(matches!(err, EnvelopeDecodeError::MissingField { field: "from" }));
    }

    #[test]
    fn peek_sender_rejects_duplicate_keys() {
        // from_slice_strict rejects duplicate keys per famp-canonical contract;
        // expect MalformedJson(CanonicalError::...).
        let bytes = br#"{"from":"agent:local/alice","from":"agent:local/eve"}"#;
        let err = peek_sender(bytes).unwrap_err();
        assert!(matches!(err, EnvelopeDecodeError::MalformedJson(_)));
    }
}

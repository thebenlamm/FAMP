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
use crate::view::OwnedEnvelopeView;
use famp_core::Principal;

/// Strictly parse wire bytes (duplicate-key-rejecting) and extract the `from`
/// field as a [`Principal`]. Performs NO signature verification.
///
/// Thin wrapper over [`crate::EnvelopeView::from`]: the structural view is the
/// single source of truth for the field name and parse rule. Behaviour is
/// preserved exactly — a malformed-JSON / duplicate-key body surfaces
/// [`EnvelopeDecodeError::MalformedJson`] (from the strict parse), and an
/// absent, non-string, or unparseable `from` surfaces
/// [`EnvelopeDecodeError::MissingField`].
pub fn peek_sender(bytes: &[u8]) -> Result<Principal, EnvelopeDecodeError> {
    OwnedEnvelopeView::parse(bytes)?
        .view()
        .from()
        .ok_or(EnvelopeDecodeError::MissingField { field: "from" })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
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
        assert!(matches!(
            err,
            EnvelopeDecodeError::MissingField { field: "from" }
        ));
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

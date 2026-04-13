//! Pre-decode sender extraction.
//!
//! `AnySignedEnvelope::decode` requires a `TrustedVerifyingKey` upfront,
//! but the runtime can only look up that key in the keyring after knowing
//! the sender. So we extract `from` from the raw wire bytes FIRST, parse
//! it as `Principal`, then look up the key, then call decode.
//!
//! Research Pattern 4 — verified against `dispatch.rs`'s
//! `AnySignedEnvelope::decode` sequence.

use crate::runtime::error::RuntimeError;
use famp_canonical::from_slice_strict;
use famp_core::Principal;
use famp_envelope::EnvelopeDecodeError;
use std::str::FromStr;

/// Parse the wire bytes strictly (rejecting duplicate keys) and extract the
/// `from` field as a [`Principal`] without performing signature verification.
pub fn peek_sender(bytes: &[u8]) -> Result<Principal, RuntimeError> {
    let value: serde_json::Value = from_slice_strict(bytes)
        .map_err(|e| RuntimeError::Decode(EnvelopeDecodeError::MalformedJson(e)))?;
    let from_str = value
        .get("from")
        .and_then(serde_json::Value::as_str)
        .ok_or(RuntimeError::Decode(EnvelopeDecodeError::MissingField {
            field: "from",
        }))?;
    Principal::from_str(from_str).map_err(|_| {
        RuntimeError::Decode(EnvelopeDecodeError::MissingField { field: "from" })
    })
}

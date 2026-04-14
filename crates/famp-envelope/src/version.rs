//! The single spec-version string FAMP v0.5.1 uses on the wire.
//!
//! The version-rejection gate lives in `envelope::SignedEnvelope::decode_value`
//! (see PR #2 / spec §Δ01 §19): a tampered `famp` field produces
//! `EnvelopeDecodeError::UnsupportedVersion` → `ProtocolErrorKind::Unsupported`.
//! There is exactly one such site in the decode path.

pub const FAMP_SPEC_VERSION: &str = "0.5.1";

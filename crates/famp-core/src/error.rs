//! Wire-stable protocol error vocabulary (FAMP v0.5.1 §15.1).
//!
//! `ProtocolErrorKind` is a flat unit-variant enum — exactly 15 categories,
//! matching spec §15.1 verbatim via `#[serde(rename_all = "snake_case")]`.
//! The wire strings are locked by the `error_wire_strings` integration test,
//! which any rename must update.
//!
//! Per D-22, this module deliberately does NOT provide `From<CanonicalError>`
//! or `From<CryptoError>` conversions into `ProtocolErrorKind`. Mapping upstream
//! error types onto protocol categories is the responsibility of the boundary
//! crate (envelope / transport) that actually builds the wire response.

/// The 15 wire-level protocol error categories defined by FAMP v0.5.1 §15.1.
///
/// Serde wire form is `snake_case` per-variant — asserted by the
/// `error_wire_strings` fixture.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    thiserror::Error,
)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolErrorKind {
    #[error("malformed")]
    Malformed,
    #[error("unsupported")]
    Unsupported,
    #[error("unauthorized")]
    Unauthorized,
    #[error("stale")]
    Stale,
    #[error("duplicate")]
    Duplicate,
    #[error("orphaned")]
    Orphaned,
    #[error("out_of_scope")]
    OutOfScope,
    #[error("capacity_exceeded")]
    CapacityExceeded,
    #[error("policy_blocked")]
    PolicyBlocked,
    #[error("commitment_missing")]
    CommitmentMissing,
    #[error("delegation_forbidden")]
    DelegationForbidden,
    #[error("provenance_incomplete")]
    ProvenanceIncomplete,
    #[error("conflict")]
    Conflict,
    #[error("condition_failed")]
    ConditionFailed,
    #[error("expired")]
    Expired,
}

/// Internal-plumbing wrapper pairing a `ProtocolErrorKind` with optional detail.
///
/// Intentionally does NOT implement `Serialize` — the wire shape
/// (`{ "error": "...", "detail": "..." }`) is the envelope crate's job (D-21).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{kind}: {}", detail.as_deref().unwrap_or(""))]
pub struct ProtocolError {
    pub kind: ProtocolErrorKind,
    pub detail: Option<String>,
}

impl ProtocolError {
    #[must_use]
    pub const fn new(kind: ProtocolErrorKind) -> Self {
        Self { kind, detail: None }
    }

    #[must_use]
    pub fn with_detail(kind: ProtocolErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: Some(detail.into()),
        }
    }
}

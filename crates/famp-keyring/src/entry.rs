//! `KeyEntry` — a single key binding for a principal, carrying its
//! lifecycle state and (optionally) expiry/pin/state-change timestamps.
//!
//! v1.1 (Phase 15, D15-A): a principal may hold multiple `KeyEntry` values
//! at once (rotation), each in exactly one of three states
//! ([`KeyState::Active`], [`KeyState::Retired`], [`KeyState::Revoked`]).
//! Legacy (pre-v1.1) entries are represented as `Active` with all three
//! timestamps absent — see [`KeyEntry::legacy`] and
//! [`KeyEntry::is_legacy_shaped`], which together drive the save-format
//! rule in `file_format::serialize_entry` (D15-A "save-format rule").
//!
//! No `time`/`chrono` dependency here (Phase 08 decision, re-affirmed by
//! D15-A): [`is_canonical_utc`] is a small, local, positional byte gate for
//! the canonical whole-second UTC form `YYYY-MM-DDTHH:MM:SSZ` (exactly 20
//! bytes), mirroring `famp_envelope::timestamp::is_canonical_utc_form`
//! (which is `pub(crate)` there and not importable from this crate).

use famp_crypto::TrustedVerifyingKey;

/// Lifecycle state of a single pinned key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    /// The currently-usable key for a principal. At most one `Active`
    /// entry may exist per principal (enforced at load time).
    Active,
    /// A previously-active key, superseded by a rotation. No longer
    /// usable for verification, but retained for audit/history.
    Retired,
    /// A key explicitly revoked (REVK-02, defense-in-depth alongside
    /// REVK-01 expiry). No longer usable for verification.
    Revoked,
}

impl KeyState {
    /// The exact lowercase on-disk token for this state (D15-A).
    #[must_use]
    pub const fn as_token(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Retired => "retired",
            Self::Revoked => "revoked",
        }
    }

    /// Parse one of the three legal lowercase D15-A state tokens. Any
    /// other input (including different casing) returns `None` — the
    /// caller maps that to a `MalformedEntry` at load time.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "active" => Some(Self::Active),
            "retired" => Some(Self::Retired),
            "revoked" => Some(Self::Revoked),
            _ => None,
        }
    }
}

/// One key binding for a principal.
///
/// Carries the key itself, its lifecycle state, and up to three D15-A
/// timestamp fields (`valid_until`, `pinned_at`, `state_since`), each
/// either the canonical 20-byte UTC form or absent.
#[derive(Debug, Clone)]
pub struct KeyEntry {
    key: TrustedVerifyingKey,
    state: KeyState,
    valid_until: Option<String>,
    pinned_at: Option<String>,
    state_since: Option<String>,
}

impl KeyEntry {
    /// Build a legacy (pre-v1.1) entry: `Active`, all timestamps absent.
    /// This is the shape every 2-field on-disk line parses into.
    #[must_use]
    pub const fn legacy(key: TrustedVerifyingKey) -> Self {
        Self {
            key,
            state: KeyState::Active,
            valid_until: None,
            pinned_at: None,
            state_since: None,
        }
    }

    /// Build a fully-specified v1.1 entry.
    #[must_use]
    pub const fn new(
        key: TrustedVerifyingKey,
        state: KeyState,
        valid_until: Option<String>,
        pinned_at: Option<String>,
        state_since: Option<String>,
    ) -> Self {
        Self {
            key,
            state,
            valid_until,
            pinned_at,
            state_since,
        }
    }

    #[must_use]
    pub const fn key(&self) -> &TrustedVerifyingKey {
        &self.key
    }

    #[must_use]
    pub const fn state(&self) -> KeyState {
        self.state
    }

    #[must_use]
    pub fn valid_until(&self) -> Option<&str> {
        self.valid_until.as_deref()
    }

    #[must_use]
    pub fn pinned_at(&self) -> Option<&str> {
        self.pinned_at.as_deref()
    }

    #[must_use]
    pub fn state_since(&self) -> Option<&str> {
        self.state_since.as_deref()
    }

    /// Diagnostic key identifier, delegating to `famp_crypto::key_id`.
    #[must_use]
    pub fn key_id(&self) -> String {
        famp_crypto::key_id(&self.key)
    }

    /// True iff this entry is indistinguishable from a legacy (pre-v1.1)
    /// pin: `Active` state with all three timestamps absent. Drives the
    /// D15-A save-format rule — an entry that is `is_legacy_shaped` is
    /// serialized as the 2-field legacy line, never the 6-field v1.1 line.
    #[must_use]
    pub fn is_legacy_shaped(&self) -> bool {
        self.state == KeyState::Active
            && self.valid_until.is_none()
            && self.pinned_at.is_none()
            && self.state_since.is_none()
    }
}

/// True iff `v` is the canonical whole-second UTC form `YYYY-MM-DDTHH:MM:SSZ`.
///
/// Exactly 20 bytes, digits and separators positionally checked, byte 19
/// is a literal `Z`. Locally re-derived from
/// `famp_envelope::timestamp::is_canonical_utc_form` (which is
/// `pub(crate)` there and not importable) rather than adding a `time`/
/// `chrono` dependency to this crate (Phase 08 decision, D15-A).
#[must_use]
pub fn is_canonical_utc(v: &str) -> bool {
    let bytes = v.as_bytes();
    if bytes.len() != 20 {
        return false;
    }
    let digit = |i: usize| bytes[i].is_ascii_digit();
    (0..4).all(digit)
        && bytes[4] == b'-'
        && (5..7).all(digit)
        && bytes[7] == b'-'
        && (8..10).all(digit)
        && bytes[10] == b'T'
        && (11..13).all(digit)
        && bytes[13] == b':'
        && (14..16).all(digit)
        && bytes[16] == b':'
        && (17..19).all(digit)
        && bytes[19] == b'Z'
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn dummy_key() -> TrustedVerifyingKey {
        TrustedVerifyingKey::from_b64url("iojj3XQJ8ZX9UtstPLpdcspnCb8dlBIb83SIAbQPb1w").unwrap()
    }

    #[test]
    fn legacy_entry_is_legacy_shaped() {
        let e = KeyEntry::legacy(dummy_key());
        assert!(e.is_legacy_shaped());
        assert_eq!(e.state(), KeyState::Active);
    }

    #[test]
    fn entry_with_a_timestamp_is_not_legacy_shaped() {
        let e = KeyEntry::new(
            dummy_key(),
            KeyState::Active,
            Some("2027-01-01T00:00:00Z".to_string()),
            None,
            None,
        );
        assert!(!e.is_legacy_shaped());
    }

    #[test]
    fn state_token_round_trips() {
        for s in [KeyState::Active, KeyState::Retired, KeyState::Revoked] {
            assert_eq!(KeyState::from_token(s.as_token()), Some(s));
        }
        assert_eq!(KeyState::from_token("bogus"), None);
    }

    #[test]
    fn canonical_utc_accepts_only_exact_20_byte_z_form() {
        assert!(is_canonical_utc("2026-01-01T00:00:00Z"));
        assert!(!is_canonical_utc("2026-01-01T00:00:00.123Z"));
        assert!(!is_canonical_utc("2026-01-01T00:00:00+00:00"));
        assert!(!is_canonical_utc("2026-01-01T00:00:00"));
        assert!(!is_canonical_utc("-"));
        assert!(!is_canonical_utc(""));
    }
}

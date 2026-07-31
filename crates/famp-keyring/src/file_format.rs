//! Keyring file format — line-oriented, full-line comments only.
//!
//! ```text
//! # FAMP v0.7 TOFU keyring
//! agent:local/alice  <base64url-unpadded-32-byte-pubkey>
//! agent:local/bob  <base64url-unpadded-32-byte-pubkey>
//! ```
//!
//! Grammar (D-B1, extended by D15-A for v1.1):
//! - One entry per line; fields separated by `[ \t]+`.
//! - Full-line `#` comments (first non-whitespace char is `#`) ignored.
//! - Blank lines ignored.
//! - Inline `#` after the pubkey is a parse error (rejects inline
//!   trailing comments in both legacy and v1.1 lines).
//! - `\r\n` tolerated on read; `\n`-only on write.
//! - **Legacy, 2 fields:** `{principal}  {pubkey_b64url}` — implicitly
//!   `state=active`, `valid_until=None`, `pinned_at=None`,
//!   `state_since=None`.
//! - **v1.1, 6 fields:** `{principal}  {pubkey_b64url}  {state}
//!   {valid_until}  {pinned_at}  {state_since}`. `state` is one of
//!   `active`/`retired`/`revoked` (`KeyState::from_token`); each timestamp
//!   field is either the canonical 20-byte `YYYY-MM-DDTHH:MM:SSZ` form or
//!   the literal `-` absent sentinel. Any other field count is a
//!   `MalformedEntry`.
//!
//! Save format (D-B5/D15-A): alphabetical by principal string, entries
//! within a principal sorted by pubkey b64url, EXACTLY two spaces as
//! separator, trailing `\n`, no header comments re-emitted. An entry that
//! is `Active` with all three timestamps absent is emitted as the legacy
//! 2-field line (`KeyEntry::is_legacy_shaped`); every other entry is
//! emitted as the 6-field v1.1 line. This keeps an untouched keyring
//! byte-identical across the v1.1 upgrade (T-15-08).

use crate::entry::{is_canonical_utc, KeyEntry, KeyState};
use crate::error::KeyringError;
use famp_core::Principal;
use famp_crypto::TrustedVerifyingKey;
use std::str::FromStr;

/// Absent-timestamp sentinel for the 6-field v1.1 line (D15-A).
const ABSENT: &str = "-";

pub struct ParsedEntry {
    pub principal: Principal,
    pub entry: KeyEntry,
}

/// Parse a single non-comment, non-blank line. `line_no` is 1-based and used
/// only for error reporting.
pub fn parse_line(raw: &str, line_no: usize) -> Result<ParsedEntry, KeyringError> {
    // Tolerate trailing `\r` for cross-platform sanity.
    let line = raw.strip_suffix('\r').unwrap_or(raw);

    // Reject inline `#` — full-line comments only.
    if line.contains('#') {
        return Err(KeyringError::MalformedEntry {
            line: line_no,
            reason: "inline '#' comments are not permitted".to_string(),
        });
    }

    let fields: Vec<&str> = line.split_whitespace().collect();

    let (principal_str, pubkey_str) = match fields.len() {
        2 | 6 => (fields[0], fields[1]),
        n => {
            return Err(KeyringError::MalformedEntry {
                line: line_no,
                reason: format!("expected 2 (legacy) or 6 (v1.1) fields, found {n}"),
            })
        }
    };

    let principal =
        Principal::from_str(principal_str).map_err(|e| KeyringError::MalformedEntry {
            line: line_no,
            reason: format!("invalid principal: {e}"),
        })?;
    let key = TrustedVerifyingKey::from_b64url(pubkey_str)?;

    if fields.len() == 2 {
        return Ok(ParsedEntry {
            principal,
            entry: KeyEntry::legacy(key),
        });
    }

    // 6-field v1.1 line: state, valid_until, pinned_at, state_since.
    let state_tok = fields[2];
    let state = KeyState::from_token(state_tok).ok_or_else(|| KeyringError::MalformedEntry {
        line: line_no,
        reason: format!("invalid state token '{state_tok}' (expected active/retired/revoked)"),
    })?;

    let parse_ts = |field_name: &'static str, raw: &str| -> Result<Option<String>, KeyringError> {
        if raw == ABSENT {
            return Ok(None);
        }
        if !is_canonical_utc(raw) {
            return Err(KeyringError::MalformedEntry {
                line: line_no,
                reason: format!(
                    "field '{field_name}' is not a canonical UTC timestamp or '-' sentinel: '{raw}'"
                ),
            });
        }
        Ok(Some(raw.to_string()))
    };

    let valid_until = parse_ts("valid_until", fields[3])?;
    let pinned_at = parse_ts("pinned_at", fields[4])?;
    let state_since = parse_ts("state_since", fields[5])?;

    Ok(ParsedEntry {
        principal,
        entry: KeyEntry::new(key, state, valid_until, pinned_at, state_since),
    })
}

/// Emit one canonical save-format line for `(principal, entry)`. Legacy
/// shape (`Active`, all timestamps absent) emits the 2-field legacy line;
/// every other entry emits the 6-field v1.1 line. EXACTLY two spaces as
/// field separator throughout (D-B5).
pub fn serialize_entry(principal: &Principal, entry: &KeyEntry) -> String {
    if entry.is_legacy_shaped() {
        return format!("{}  {}\n", principal, entry.key().to_b64url());
    }
    let ts = |v: Option<&str>| v.unwrap_or(ABSENT).to_string();
    format!(
        "{}  {}  {}  {}  {}  {}\n",
        principal,
        entry.key().to_b64url(),
        entry.state().as_token(),
        ts(entry.valid_until()),
        ts(entry.pinned_at()),
        ts(entry.state_since()),
    )
}

//! Keyring file format (v0.7) — line-oriented, full-line comments only.
//!
//! ```text
//! # FAMP v0.7 TOFU keyring
//! agent:local/alice  <base64url-unpadded-32-byte-pubkey>
//! agent:local/bob  <base64url-unpadded-32-byte-pubkey>
//! ```
//!
//! Grammar (D-B1):
//! - One entry per line; principal and pubkey separated by `[ \t]+`.
//! - Full-line `#` comments (first non-whitespace char is `#`) ignored.
//! - Blank lines ignored.
//! - Inline `#` after the pubkey is a parse error (v0.7 rejects inline
//!   trailing comments).
//! - `\r\n` tolerated on read; `\n`-only on write.
//!
//! Save format (D-B5): alphabetical by principal string, EXACTLY two spaces
//! as separator, trailing `\n`, no header comments re-emitted.

use crate::error::KeyringError;
use famp_core::Principal;
use famp_crypto::TrustedVerifyingKey;
use std::str::FromStr;

pub struct ParsedEntry {
    pub principal: Principal,
    pub key: TrustedVerifyingKey,
}

/// Parse a single non-comment, non-blank line. `line_no` is 1-based and used
/// only for error reporting.
pub fn parse_line(raw: &str, line_no: usize) -> Result<ParsedEntry, KeyringError> {
    // Tolerate trailing `\r` for cross-platform sanity.
    let line = raw.strip_suffix('\r').unwrap_or(raw);

    // Reject inline `#` — v0.7 is full-line comments only.
    if line.contains('#') {
        return Err(KeyringError::MalformedEntry {
            line: line_no,
            reason: "inline '#' comments are not permitted in v0.7".to_string(),
        });
    }

    let mut parts = line.split_whitespace();
    let principal_str = parts.next().ok_or_else(|| KeyringError::MalformedEntry {
        line: line_no,
        reason: "missing principal".to_string(),
    })?;
    let pubkey_str = parts.next().ok_or_else(|| KeyringError::MalformedEntry {
        line: line_no,
        reason: "missing pubkey".to_string(),
    })?;
    if parts.next().is_some() {
        return Err(KeyringError::MalformedEntry {
            line: line_no,
            reason: "unexpected trailing content".to_string(),
        });
    }

    let principal =
        Principal::from_str(principal_str).map_err(|e| KeyringError::MalformedEntry {
            line: line_no,
            reason: format!("invalid principal: {e}"),
        })?;
    let key = TrustedVerifyingKey::from_b64url(pubkey_str)?;

    Ok(ParsedEntry { principal, key })
}

/// Emit one canonical save-format line: `{principal}  {pubkey}\n` — EXACTLY
/// two spaces as separator (D-B5).
pub fn serialize_entry(principal: &Principal, key: &TrustedVerifyingKey) -> String {
    format!("{}  {}\n", principal, key.to_b64url())
}

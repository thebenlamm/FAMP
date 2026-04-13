//! FAMP v0.7 TOFU keyring — `HashMap<Principal, TrustedVerifyingKey>`.
//!
//! Narrow by absence: no Agent Card, no federation credential, no pluggable
//! trust store, no key rotation. Pinning is sticky; any conflict is fatal.
//!
//! See `.planning/phases/03-memorytransport-tofu-keyring-same-process-example/03-CONTEXT.md`
//! (D-A1..D-A4, D-B1..D-B6) for the full design rationale.

#![forbid(unsafe_code)]

// Dev-deps referenced only by integration tests in `tests/`. Silence
// `unused_crate_dependencies` for the lib-test compile unit.
#[cfg(test)]
use tempfile as _;

pub mod error;
mod file_format;
pub mod peer_flag;

pub use error::KeyringError;
pub use peer_flag::parse_peer_flag;

use famp_core::Principal;
use famp_crypto::TrustedVerifyingKey;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// Local-file TOFU keyring: `Principal -> TrustedVerifyingKey`.
///
/// Construction paths:
/// - [`Keyring::new`] — empty.
/// - [`Keyring::load_from_file`] — parse a keyring file (D-B1/D-B2).
/// - [`Keyring::with_peer`] — merge a single `(Principal, key)` binding,
///   chainable and conflict-rejecting (D-B3).
///
/// Mutation paths:
/// - [`Keyring::pin_tofu`] — first-sight pin; fails closed on key conflict.
///
/// There is deliberately no `replace`, `override`, or `force` variant.
#[derive(Debug, Default, Clone)]
pub struct Keyring {
    map: HashMap<Principal, TrustedVerifyingKey>,
}

impl Keyring {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Load a keyring file. Validates grammar, principal/pubkey parse,
    /// and rejects duplicate principals + duplicate pubkeys with line
    /// numbers in the typed error (D-B2).
    pub fn load_from_file(path: &Path) -> Result<Self, KeyringError> {
        let f = std::fs::File::open(path)?;
        let reader = BufReader::new(f);
        let mut map: HashMap<Principal, TrustedVerifyingKey> = HashMap::new();
        let mut seen_keys: HashMap<[u8; 32], Principal> = HashMap::new();
        for (idx, line) in reader.lines().enumerate() {
            let line_no = idx + 1;
            let line = line?;
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let entry = file_format::parse_line(&line, line_no)?;
            if map.contains_key(&entry.principal) {
                return Err(KeyringError::DuplicatePrincipal {
                    principal: entry.principal,
                    line: line_no,
                });
            }
            let key_bytes = *entry.key.as_bytes();
            if let Some(existing) = seen_keys.get(&key_bytes) {
                return Err(KeyringError::DuplicatePubkey {
                    existing: existing.clone(),
                    line: line_no,
                });
            }
            seen_keys.insert(key_bytes, entry.principal.clone());
            map.insert(entry.principal, entry.key);
        }
        Ok(Self { map })
    }

    /// Save the keyring to disk in canonical save format: alphabetical by
    /// principal string, exactly two spaces separator, trailing `\n` per
    /// entry, no comment header re-emitted (D-B5).
    pub fn save_to_file(&self, path: &Path) -> Result<(), KeyringError> {
        let mut keys: Vec<&Principal> = self.map.keys().collect();
        keys.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
        let mut f = std::fs::File::create(path)?;
        for p in keys {
            let line = file_format::serialize_entry(p, &self.map[p]);
            f.write_all(line.as_bytes())?;
        }
        Ok(())
    }

    /// Chainable variant of [`Keyring::pin_tofu`]. Consumes `self`, returns a
    /// new `Keyring` with the peer merged. Idempotent on re-adding the same
    /// `(principal, key)` pair; fails closed on any DIFFERENT key for a
    /// pinned principal.
    pub fn with_peer(
        mut self,
        principal: Principal,
        key: TrustedVerifyingKey,
    ) -> Result<Self, KeyringError> {
        self.pin_tofu(principal, key)?;
        Ok(self)
    }

    #[must_use]
    pub fn get(&self, principal: &Principal) -> Option<&TrustedVerifyingKey> {
        self.map.get(principal)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Insert or confirm an existing pin (D-B3 TOFU semantics).
    ///
    /// Returns `Ok(())` if the principal is unknown (first-sight pin) or if
    /// the principal is already pinned to the SAME 32-byte pubkey (idempotent
    /// re-pin). Returns `Err(KeyConflict)` if the principal is already pinned
    /// to a DIFFERENT key. There is no automatic rotation path.
    pub fn pin_tofu(
        &mut self,
        principal: Principal,
        key: TrustedVerifyingKey,
    ) -> Result<(), KeyringError> {
        if let Some(existing) = self.map.get(&principal) {
            if existing.as_bytes() != key.as_bytes() {
                return Err(KeyringError::KeyConflict { principal });
            }
            return Ok(());
        }
        self.map.insert(principal, key);
        Ok(())
    }
}

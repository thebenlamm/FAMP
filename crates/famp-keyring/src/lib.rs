//! FAMP TOFU keyring — `HashMap<Principal, Vec<KeyEntry>>`.
//!
//! v0.7: exactly one key per principal, no rotation, no revocation.
//! v1.1 (Phase 15, D15-A/D-09): extended to a per-principal lifecycle of
//! `active`/`retired`/`revoked` [`entry::KeyEntry`] values, with expiry
//! (`valid_until`) as the PRIMARY revocation mechanism (REVK-01) and a
//! signed revocation statement as defense-in-depth (REVK-02, landed in a
//! later plan). At most one `Active` entry may exist per principal at any
//! time (enforced at load and by [`Keyring::pin_tofu`]).
//!
//! See `.planning/phases/03-memorytransport-tofu-keyring-same-process-example/03-CONTEXT.md`
//! (D-A1..D-A4, D-B1..D-B6) for the original v0.7 design rationale, and
//! `.planning/phases/15-keyring-multi-key-extension-revocation/15-DECISIONS.md`
//! (D15-A/D15-B) for the v1.1 record shape and revocation-signer rule.

#![forbid(unsafe_code)]

// Dev-deps referenced only by integration tests in `tests/`. Silence
// `unused_crate_dependencies` for the lib-test compile unit.
#[cfg(test)]
use tempfile as _;

pub mod entry;
pub mod error;
mod file_format;
pub mod peer_flag;

pub use entry::{KeyEntry, KeyState};
pub use error::KeyringError;
pub use peer_flag::parse_peer_flag;

use famp_core::Principal;
use famp_crypto::TrustedVerifyingKey;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// The result of a time-aware trust lookup ([`Keyring::active_key`]).
///
/// Every variant other than `Active` is a rejection at the call site
/// (T-15-11): there is no "most recent key wins" fallback and no
/// auto-pin. `Retired` and `Revoked` carry the recorded `state_since`
/// timestamp (named `retired_at`/`revoked_at` per its D15-A semantics)
/// for operator diagnosis only — neither has been re-verified against
/// `now` beyond being loaded as already-canonical at parse time.
#[derive(Debug, Clone, Copy)]
pub enum KeyLookupOutcome<'a> {
    /// A usable, non-expired, active key.
    Active(&'a TrustedVerifyingKey),
    /// The principal has no entries at all (TRUST-02: unpinned).
    Unpinned,
    /// An `Active` entry exists but `now` is at or past its
    /// `valid_until` window (REVK-01: expiry is the PRIMARY revocation
    /// mechanism, checked regardless of any revocation record).
    Expired { valid_until: &'a str },
    /// No `Active` entry exists; the most relevant entry found is
    /// `Retired`.
    Retired { retired_at: Option<&'a str> },
    /// No `Active` entry exists; the most relevant entry found is
    /// `Revoked`. Reported in preference to `Retired` when both would
    /// otherwise apply.
    Revoked { revoked_at: Option<&'a str> },
    /// The caller-supplied `now` is not itself in the canonical 20-byte
    /// UTC form — fail closed rather than risk an unsound lexical
    /// comparison (T-15-10).
    NonCanonicalNow,
}

/// The result of a successful [`Keyring::rotate_to`] call (KEYR-02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RotationOutcome {
    /// The principal had no entries at all (or only non-`Active`
    /// entries); the new key is now pinned `Active`.
    FirstPin,
    /// The SAME 32-byte pubkey was already the principal's `Active`
    /// entry — idempotent, no mutation.
    AlreadyPinned,
    /// The principal's previous `Active` entry was moved to `Retired`
    /// (key material retained) and the new key pinned `Active`.
    Rotated {
        previous_key_id: String,
        new_key_id: String,
    },
}

/// Local-file TOFU keyring: `Principal -> Vec<KeyEntry>`.
///
/// Construction paths:
/// - [`Keyring::new`] — empty.
/// - [`Keyring::load_from_file`] — parse a keyring file (D-B1/D-B2, D15-A).
/// - [`Keyring::with_peer`] — merge a single `(Principal, key)` binding,
///   chainable and conflict-rejecting (D-B3).
///
/// Mutation paths:
/// - [`Keyring::pin_tofu`] — first-sight pin against the `Active` entry;
///   fails closed on key conflict.
///
/// There is deliberately no `replace`, `override`, or `force` variant.
#[derive(Debug, Default, Clone)]
pub struct Keyring {
    map: HashMap<Principal, Vec<KeyEntry>>,
}

impl Keyring {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Load a keyring file. Validates grammar, principal/pubkey parse,
    /// and rejects:
    /// - two `Active` entries for one principal -> `DuplicatePrincipal`
    ///   (narrowed contract from v0.7's blanket "one entry per
    ///   principal" to "at most one ACTIVE entry per principal", D15-A).
    /// - the same pubkey twice under the SAME principal ->
    ///   `DuplicateKeyEntry`.
    /// - the same pubkey under two DIFFERENT principals ->
    ///   `DuplicatePubkey` (unchanged).
    pub fn load_from_file(path: &Path) -> Result<Self, KeyringError> {
        let f = std::fs::File::open(path)?;
        let reader = BufReader::new(f);
        let mut map: HashMap<Principal, Vec<KeyEntry>> = HashMap::new();
        let mut seen_keys: HashMap<[u8; 32], Principal> = HashMap::new();
        for (idx, line) in reader.lines().enumerate() {
            let line_no = idx + 1;
            let line = line?;
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let parsed = file_format::parse_line(&line, line_no)?;
            let key_bytes = *parsed.entry.key().as_bytes();

            // Two `Active` entries for one principal is checked BEFORE the
            // pubkey-duplication checks below, so a line that repeats both
            // the SAME principal and the SAME pubkey (the rt3 regression
            // fixture) is reported as `DuplicatePrincipal` — the narrowed
            // v0.7 contract this fixture already pins — rather than the
            // newer, more specific `DuplicateKeyEntry`.
            if parsed.entry.state() == KeyState::Active {
                let already_active = map
                    .get(&parsed.principal)
                    .is_some_and(|entries| entries.iter().any(|e| e.state() == KeyState::Active));
                if already_active {
                    return Err(KeyringError::DuplicatePrincipal {
                        principal: parsed.principal,
                        line: line_no,
                    });
                }
            }

            if let Some(existing_principal) = seen_keys.get(&key_bytes) {
                if *existing_principal == parsed.principal {
                    return Err(KeyringError::DuplicateKeyEntry {
                        principal: parsed.principal,
                        line: line_no,
                    });
                }
                return Err(KeyringError::DuplicatePubkey {
                    existing: existing_principal.clone(),
                    line: line_no,
                });
            }

            seen_keys.insert(key_bytes, parsed.principal.clone());
            map.entry(parsed.principal).or_default().push(parsed.entry);
        }
        Ok(Self { map })
    }

    /// Save the keyring to disk in canonical save format: alphabetical by
    /// principal string, entries within a principal sorted by pubkey
    /// b64url, exactly two spaces separator, trailing `\n` per entry, no
    /// comment header re-emitted (D-B5, D15-A).
    pub fn save_to_file(&self, path: &Path) -> Result<(), KeyringError> {
        let mut keys: Vec<&Principal> = self.map.keys().collect();
        keys.sort_by_cached_key(ToString::to_string);
        let mut f = std::fs::File::create(path)?;
        for p in keys {
            let mut entries: Vec<&KeyEntry> = self.map[p].iter().collect();
            entries.sort_by_cached_key(|e| e.key().to_b64url());
            for entry in entries {
                let line = file_format::serialize_entry(p, entry);
                f.write_all(line.as_bytes())?;
            }
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

    /// Presence check only: returns the principal's `Active` entry's key
    /// with NO validity-window check. Intended for CLI/tests that only
    /// need "is something pinned" — every verification path MUST use
    /// [`Keyring::active_key`] instead, which is the sole time-aware
    /// trust decision.
    #[must_use]
    pub fn get(&self, principal: &Principal) -> Option<&TrustedVerifyingKey> {
        self.map
            .get(principal)?
            .iter()
            .find(|e| e.state() == KeyState::Active)
            .map(KeyEntry::key)
    }

    /// Number of principals with at least one entry (not total entries).
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// All key entries recorded for `principal`, in load/insertion order.
    #[must_use]
    pub fn entries(&self, principal: &Principal) -> &[KeyEntry] {
        self.map.get(principal).map_or(&[], Vec::as_slice)
    }

    /// The time-aware trust query (REVK-01, T-15-10, T-15-11): decide
    /// whether `principal` has a usable pinned key at instant `now`.
    ///
    /// `now` is always supplied by the caller — this function never reads
    /// a clock (REVK-01/REVK-03 determinism; enforced by a `grep` gate in
    /// this plan's acceptance criteria). Order of checks:
    /// 1. `now` must itself be in canonical UTC form, else
    ///    [`KeyLookupOutcome::NonCanonicalNow`] (fail closed).
    /// 2. No entries at all -> [`KeyLookupOutcome::Unpinned`].
    /// 3. An `Active` entry exists: compare its `valid_until` (if any) to
    ///    `now` by plain lexical `<=` — sound because both operands are
    ///    already canonical-gated (parse-time for `valid_until`, step 1
    ///    for `now`) — returning [`KeyLookupOutcome::Expired`] when the
    ///    window has closed, else [`KeyLookupOutcome::Active`].
    /// 4. No `Active` entry: prefer reporting
    ///    [`KeyLookupOutcome::Revoked`] over [`KeyLookupOutcome::Retired`]
    ///    when both are present.
    #[must_use]
    pub fn active_key<'a>(&'a self, principal: &Principal, now: &str) -> KeyLookupOutcome<'a> {
        if !entry::is_canonical_utc(now) {
            return KeyLookupOutcome::NonCanonicalNow;
        }
        let Some(entries) = self.map.get(principal) else {
            return KeyLookupOutcome::Unpinned;
        };
        if entries.is_empty() {
            return KeyLookupOutcome::Unpinned;
        }

        if let Some(active) = entries.iter().find(|e| e.state() == KeyState::Active) {
            return match active.valid_until() {
                Some(valid_until) if valid_until <= now => {
                    KeyLookupOutcome::Expired { valid_until }
                }
                _ => KeyLookupOutcome::Active(active.key()),
            };
        }

        if let Some(revoked) = entries.iter().find(|e| e.state() == KeyState::Revoked) {
            return KeyLookupOutcome::Revoked {
                revoked_at: revoked.state_since(),
            };
        }
        if let Some(retired) = entries.iter().find(|e| e.state() == KeyState::Retired) {
            return KeyLookupOutcome::Retired {
                retired_at: retired.state_since(),
            };
        }
        // Entries exist but none is Active/Retired/Revoked is
        // unreachable given `KeyState`'s exhaustive 3-variant shape, but
        // fail closed rather than panic if that ever changes.
        KeyLookupOutcome::Unpinned
    }

    /// Insert or confirm an existing pin (D-B3 TOFU semantics), operating
    /// on the principal's `Active` entry.
    ///
    /// Returns `Ok(())` if the principal is unknown (first-sight pin) or if
    /// the principal's `Active` entry is already pinned to the SAME
    /// 32-byte pubkey (idempotent re-pin). Returns `Err(KeyConflict)` if
    /// the `Active` entry is pinned to a DIFFERENT key. There is no
    /// automatic rotation path (rotation lands in a later plan against
    /// this same shape).
    pub fn pin_tofu(
        &mut self,
        principal: Principal,
        key: TrustedVerifyingKey,
    ) -> Result<(), KeyringError> {
        if let Some(entries) = self.map.get(&principal) {
            if entries
                .iter()
                .any(|e| e.state() == KeyState::Revoked && e.key().as_bytes() == key.as_bytes())
            {
                return Err(KeyringError::KeyRevoked {
                    principal,
                    key_id: famp_crypto::key_id(&key),
                });
            }
            if let Some(active) = entries.iter().find(|e| e.state() == KeyState::Active) {
                if active.key().as_bytes() != key.as_bytes() {
                    return Err(KeyringError::KeyConflict { principal });
                }
                return Ok(());
            }
        }
        self.map
            .entry(principal)
            .or_default()
            .push(KeyEntry::legacy(key));
        Ok(())
    }

    /// Pin a new key for `principal`, moving any existing `Active` entry
    /// to `Retired` rather than dropping it (KEYR-02). `now`/`valid_until`
    /// are always caller-supplied — this function never reads a clock.
    ///
    /// Every check below runs BEFORE any mutation, so every `Err` path
    /// leaves the keyring byte-identical (T-15-13):
    /// 1. `now` (and `valid_until`, if present) must be canonical UTC, else
    ///    [`KeyringError::NonCanonicalTimestamp`].
    /// 2. `new_key` already exists as a `Revoked` entry for `principal` ->
    ///    [`KeyringError::KeyRevoked`], regardless of `confirmed`
    ///    (revocation is monotonic, T-15-04).
    /// 3. No `Active` entry exists (either no entries at all, or only
    ///    `Retired`/`Revoked` ones) -> pin `new_key` `Active`, return
    ///    [`RotationOutcome::FirstPin`].
    /// 4. An `Active` entry exists with the SAME pubkey ->
    ///    [`RotationOutcome::AlreadyPinned`], no mutation.
    /// 5. An `Active` entry exists with a DIFFERENT pubkey and
    ///    `confirmed == false` -> [`KeyringError::KeyChangeRequiresConfirmation`].
    /// 6. Otherwise: retire the existing `Active` entry (key material
    ///    kept) and pin `new_key` `Active` ->
    ///    [`RotationOutcome::Rotated`].
    pub fn rotate_to(
        &mut self,
        principal: Principal,
        new_key: TrustedVerifyingKey,
        now: &str,
        valid_until: Option<&str>,
        confirmed: bool,
    ) -> Result<RotationOutcome, KeyringError> {
        if !entry::is_canonical_utc(now) {
            return Err(KeyringError::NonCanonicalTimestamp {
                value: now.to_string(),
            });
        }
        if let Some(vu) = valid_until {
            if !entry::is_canonical_utc(vu) {
                return Err(KeyringError::NonCanonicalTimestamp {
                    value: vu.to_string(),
                });
            }
        }

        let new_key_bytes = *new_key.as_bytes();
        let new_key_id = famp_crypto::key_id(&new_key);

        if let Some(entries) = self.map.get(&principal) {
            if entries
                .iter()
                .any(|e| e.state() == KeyState::Revoked && *e.key().as_bytes() == new_key_bytes)
            {
                return Err(KeyringError::KeyRevoked {
                    principal,
                    key_id: new_key_id,
                });
            }
        }

        // Single mutable borrow for the rest of this call — every branch
        // below operates on this same `entries` vector, so no error path
        // needs a second fallible lookup.
        let entries = self.map.entry(principal.clone()).or_default();
        let active_idx = entries.iter().position(|e| e.state() == KeyState::Active);

        let Some(idx) = active_idx else {
            entries.push(KeyEntry::new(
                new_key,
                KeyState::Active,
                valid_until.map(ToString::to_string),
                Some(now.to_string()),
                None,
            ));
            return Ok(RotationOutcome::FirstPin);
        };

        if *entries[idx].key().as_bytes() == new_key_bytes {
            return Ok(RotationOutcome::AlreadyPinned);
        }

        let previous_key_id = entries[idx].key_id();

        if !confirmed {
            return Err(KeyringError::KeyChangeRequiresConfirmation {
                principal,
                previous_key_id,
                new_key_id,
            });
        }

        let old = entries[idx].clone();
        entries[idx] = KeyEntry::new(
            old.key().clone(),
            KeyState::Retired,
            old.valid_until().map(ToString::to_string),
            old.pinned_at().map(ToString::to_string),
            Some(now.to_string()),
        );
        entries.push(KeyEntry::new(
            new_key,
            KeyState::Active,
            valid_until.map(ToString::to_string),
            Some(now.to_string()),
            None,
        ));

        Ok(RotationOutcome::Rotated {
            previous_key_id,
            new_key_id,
        })
    }

    /// Remove a `Retired` entry from the trust store — the ONLY path that
    /// deletes key material (KEYR-02). Refuses an `Active` entry (would
    /// silently sever the channel, T-15-14) and a `Revoked` entry (a
    /// revocation tombstone is permanent — deleting it would let a re-pin
    /// resurrect the key, T-15-04). Removes the principal entirely from
    /// the map once its entry vector becomes empty, so [`Keyring::len`]
    /// stays "number of principals with at least one entry".
    pub fn retire(&mut self, principal: &Principal, key_id: &str) -> Result<(), KeyringError> {
        let entries = self
            .map
            .get_mut(principal)
            .ok_or_else(|| KeyringError::NoSuchKeyEntry {
                principal: principal.clone(),
                key_id: key_id.to_string(),
            })?;
        let idx = entries
            .iter()
            .position(|e| e.key_id() == key_id)
            .ok_or_else(|| KeyringError::NoSuchKeyEntry {
                principal: principal.clone(),
                key_id: key_id.to_string(),
            })?;

        match entries[idx].state() {
            KeyState::Active => {
                return Err(KeyringError::CannotRetire {
                    principal: principal.clone(),
                    key_id: key_id.to_string(),
                    state: "active",
                });
            }
            KeyState::Revoked => {
                return Err(KeyringError::CannotRetire {
                    principal: principal.clone(),
                    key_id: key_id.to_string(),
                    state: "revoked",
                });
            }
            KeyState::Retired => {}
        }

        entries.remove(idx);
        if entries.is_empty() {
            self.map.remove(principal);
        }
        Ok(())
    }
}

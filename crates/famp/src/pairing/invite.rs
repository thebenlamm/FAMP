//! The on-disk pairing invite store: `~/.famp/gateway/pairing.json`.
//!
//! (mode 0600), the ONE authoritative invite record, written by the
//! gateway's redemption route and read/advanced by `famp pair status`
//! (PAIR-02/PAIR-03).
//!
//! `save_atomic` is NEW code, not a copy of `Keyring::save_to_file` — that
//! function is a plain `File::create` + `write_all` with no rename and no
//! fsync, a pre-existing Phase 15 gap this store must not inherit: PAIR-02's
//! hard-abort and PAIR-03's persisted-before-processing guarantees are
//! strictly stronger than a non-atomic write delivers. Every
//! read-modify-write goes through [`StoreLock`]; a bare [`InviteStore::load`]
//! for display does not need it.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::pairing::PairingError;

/// One pairing invite, keyed by `id` (a `uuid::Uuid::now_v7()` string).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InviteRecord {
    pub id: String,
    pub principal: String,
    /// Hex-encoded SHA-256 digest of the code — never the code itself, and
    /// never anything from which the code can be reconstructed.
    pub code_digest: String,
    pub created_at: String,
    /// `created_at` plus 24 hours (PAIR-03's explicit 24-hour window).
    pub expires_at: String,
    pub attempts: u32,
    pub state: InviteState,
}

/// Single-use invite lifecycle: `Pending` -> `Redeemed` -> `Pinned`, or
/// `Pending` -> `Revoked` (Plan 02's `famp pair revoke`).
///
/// The pin write itself happens ONLY inside `famp pair status` (never the
/// redemption endpoint, never a background loop) — this enum's
/// `Redeemed` variant is the observable evidence that transition is
/// still pending.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum InviteState {
    Pending,
    Redeemed {
        by: String,
        key_id: String,
        pubkey_b64url: String,
        at: String,
    },
    Pinned {
        at: String,
    },
    Revoked {
        at: String,
    },
}

/// The server-side brute-force budget (PAIR-02).
///
/// Against a 2048-word, five-word code the search space is `2048^5` =
/// `2^55`, so a budget of five guesses is roughly a 1-in-7-quadrillion
/// success probability per invite; the budget exists to make an online
/// guessing campaign pointless, not to make the code itself short.
pub const MAX_ATTEMPTS: u32 = 5;

impl InviteRecord {
    /// True iff `now >= expires_at` under plain lexical string comparison.
    ///
    /// Both operands are canonical whole-second UTC `YYYY-MM-DDTHH:MM:SSZ`
    /// strings (`famp_keyring::entry::is_canonical_utc`), so lexical order
    /// agrees with chronological order. `now` always originates from the
    /// verifier's OWN wall clock (`famp_gateway::clock::now_canonical_utc`)
    /// and NEVER from any field carried in a redemption request (T-18-11) —
    /// a caller cannot supply a `now` that back-dates its own code's
    /// expiry.
    #[must_use]
    pub fn is_expired(&self, now: &str) -> bool {
        debug_assert!(
            famp_keyring::entry::is_canonical_utc(now),
            "now must be canonical UTC: {now:?}"
        );
        debug_assert!(
            famp_keyring::entry::is_canonical_utc(&self.expires_at),
            "expires_at must be canonical UTC: {:?}",
            self.expires_at
        );
        now >= self.expires_at.as_str()
    }
}

/// The outcome of classifying a presented code digest against the store,
/// as decided by [`InviteStore::decide`].
///
/// Read-only: producing one of these values never mutates the store —
/// every state transition is a separate, later `&mut self` call
/// ([`InviteStore::burn_attempt`], [`InviteStore::consume`]) that a
/// caller runs only after re-loading the store under [`StoreLock`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedemptionDecision {
    /// The digest matches a live, unexpired, unexhausted `Pending` record.
    Accept { id: String },
    /// No live `Pending` record's digest matches, but at least one live
    /// `Pending` record remains that could still match a different code.
    WrongCode,
    /// The digest matches a `Pending` record whose window has closed, or
    /// every remaining live record has expired.
    Expired,
    /// The digest matches a record already `Redeemed` or `Pinned`.
    AlreadyRedeemed,
    /// The digest matches a `Pending` record whose attempt budget is
    /// spent, OR every remaining live `Pending` record's budget is spent —
    /// this wins over `Accept` even for the objectively correct code
    /// (T-18-12): once the budget is spent the invite is dead, and
    /// honoring a correct code afterwards would let an attacker who
    /// guesses correctly on attempt six still pair.
    AttemptsExhausted,
    /// Every record is `Revoked` (or the store is empty) — a `Revoked`
    /// record never matches any presented code regardless of digest
    /// equality (T-18-13/PAIR-03's kill-switch clause).
    NoPendingInvite,
}

/// The invite store's on-disk shape: `{"invites": [...]}`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InviteStore {
    pub invites: Vec<InviteRecord>,
}

impl InviteStore {
    /// Classify a presented code digest against the store's current
    /// state. Pure and read-only (`&self`) — mutates nothing. Callers
    /// (the gateway endpoint, `famp pair` internals) act on the returned
    /// [`RedemptionDecision`] via a SEPARATE `&mut self` mutator, after
    /// re-acquiring [`StoreLock`] and re-loading, so the classification a
    /// caller acted on is never stale relative to a concurrent mutation.
    ///
    /// Branch order is fixed and load-bearing — see each variant's own
    /// doc comment for why. In particular `AttemptsExhausted` is checked
    /// AHEAD of `Accept` for a matching record (T-18-12), and a `Revoked`
    /// record is filtered out before any digest comparison runs, so it
    /// can never match regardless of digest equality.
    #[must_use]
    pub fn decide(&self, digest: &[u8; 32], now: &str) -> RedemptionDecision {
        // 1. Filter out every Revoked record.
        let live: Vec<&InviteRecord> = self
            .invites
            .iter()
            .filter(|r| !matches!(r.state, InviteState::Revoked { .. }))
            .collect();
        if live.is_empty() {
            return RedemptionDecision::NoPendingInvite;
        }

        // 2. Find the matching record by scanning EVERY live record
        //    (never short-circuiting on first match) so the number of
        //    comparisons does not depend on which record matched.
        let mut matched: Option<&InviteRecord> = None;
        for record in &live {
            if let Some(stored) = crate::pairing::wordlist::digest_from_hex(&record.code_digest) {
                if crate::pairing::wordlist::digests_equal(digest, &stored) {
                    matched = Some(record);
                }
            }
        }

        if let Some(record) = matched {
            return match &record.state {
                InviteState::Redeemed { .. } | InviteState::Pinned { .. } => {
                    RedemptionDecision::AlreadyRedeemed
                }
                InviteState::Revoked { .. } => unreachable!("Revoked records are pre-filtered"),
                InviteState::Pending => {
                    if record.is_expired(now) {
                        RedemptionDecision::Expired
                    } else if record.attempts >= MAX_ATTEMPTS {
                        RedemptionDecision::AttemptsExhausted
                    } else {
                        RedemptionDecision::Accept {
                            id: record.id.clone(),
                        }
                    }
                }
            };
        }

        // 4. No match against any live record.
        let live_pending: Vec<&&InviteRecord> = live
            .iter()
            .filter(|r| matches!(r.state, InviteState::Pending))
            .collect();
        if live_pending.is_empty() {
            // Every remaining live record is Redeemed/Pinned; treat as
            // no outstanding invite to guess against.
            return RedemptionDecision::NoPendingInvite;
        }
        if live_pending.iter().all(|r| r.is_expired(now)) {
            return RedemptionDecision::Expired;
        }
        if live_pending
            .iter()
            .filter(|r| !r.is_expired(now))
            .all(|r| r.attempts >= MAX_ATTEMPTS)
        {
            return RedemptionDecision::AttemptsExhausted;
        }
        RedemptionDecision::WrongCode
    }

    /// Increment `attempts` on every live (non-expired) `Pending` record.
    /// Performs no validation of its own — `decide` already ran and
    /// determined this is the wrong-code path. A caller re-loads the
    /// store under [`StoreLock`] immediately before calling this, so a
    /// concurrent burn from another request is visible rather than lost.
    pub fn burn_attempt(&mut self, now: &str) {
        for record in &mut self.invites {
            if matches!(record.state, InviteState::Pending) && !record.is_expired(now) {
                record.attempts += 1;
            }
        }
    }

    /// Transition exactly the named record from `Pending` to `Redeemed`.
    /// Performs no validation of its own — `decide` already confirmed
    /// this is an `Accept`. Errors (and mutates nothing) if the record is
    /// missing or not `Pending`.
    pub fn consume(
        &mut self,
        id: &str,
        by: &str,
        key_id: &str,
        pubkey_b64url: &str,
        now: &str,
    ) -> Result<(), PairingError> {
        let record = self
            .invites
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| PairingError::UnknownInvite { id: id.to_string() })?;
        if !matches!(record.state, InviteState::Pending) {
            return Err(PairingError::AlreadyRedeemed);
        }
        record.state = InviteState::Redeemed {
            by: by.to_string(),
            key_id: key_id.to_string(),
            pubkey_b64url: pubkey_b64url.to_string(),
            at: now.to_string(),
        };
        Ok(())
    }

    /// Transition the named record to `Revoked`, regardless of its
    /// current state (including an already-`Redeemed`/`Pinned` one — a
    /// revoke after the fact still records operator intent). Errors (and
    /// mutates nothing) on an unknown id.
    pub fn revoke(&mut self, id: &str, now: &str) -> Result<(), PairingError> {
        let record = self
            .invites
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| PairingError::UnknownInvite { id: id.to_string() })?;
        record.state = InviteState::Revoked {
            at: now.to_string(),
        };
        Ok(())
    }

    /// Transition every currently-`Pending` record to `Revoked`, returning
    /// the count revoked. A store with zero `Pending` records revokes
    /// nothing and returns 0 — never an error.
    pub fn revoke_all_pending(&mut self, now: &str) -> usize {
        let mut count = 0;
        for record in &mut self.invites {
            if matches!(record.state, InviteState::Pending) {
                record.state = InviteState::Revoked {
                    at: now.to_string(),
                };
                count += 1;
            }
        }
        count
    }

    /// Load the store. A missing file is an EMPTY store, not an error —
    /// the gateway's redemption route treats "no file" and "file with
    /// zero Pending records" identically (both 404, T-18-06).
    pub fn load(path: &Path) -> Result<Self, PairingError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = std::fs::read(path).map_err(|e| PairingError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        if bytes.is_empty() {
            return Ok(Self::default());
        }
        serde_json::from_slice(&bytes).map_err(|e| PairingError::Wire {
            reason: format!("malformed pairing store at {}: {e}", path.display()),
        })
    }

    /// Persist the store: write a sibling temp file at mode 0600,
    /// `sync_all` it, `rename` it over the target, then `sync_all` the
    /// parent directory handle. Rename is atomic only within one
    /// filesystem — both writers (the gateway process and `famp pair
    /// status`) live under the same `$FAMP_HOME`.
    pub fn save_atomic(&self, path: &Path) -> Result<(), PairingError> {
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| PairingError::Wire {
            reason: format!("failed to serialize pairing store: {e}"),
        })?;
        write_atomic(path, &bytes)
    }
}

/// `~/.famp/gateway/pairing.json`.
#[must_use]
pub fn pairing_store_path(home: &Path) -> PathBuf {
    home.join("gateway").join("pairing.json")
}

fn temp_path_for(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".tmp");
    PathBuf::from(s)
}

#[cfg(unix)]
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), PairingError> {
    use std::os::unix::fs::OpenOptionsExt;

    let parent = path.parent().ok_or_else(|| PairingError::Wire {
        reason: format!(
            "pairing store path {} has no parent directory",
            path.display()
        ),
    })?;
    std::fs::create_dir_all(parent).map_err(|e| PairingError::Io {
        path: parent.to_path_buf(),
        source: e,
    })?;

    let tmp_path = temp_path_for(path);
    // A stale temp file from a previous crashed write would otherwise
    // wedge `create_new(true)` below forever — remove it first (best
    // effort; ENOENT is fine).
    let _ = std::fs::remove_file(&tmp_path);

    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp_path)
        .map_err(|e| PairingError::Io {
            path: tmp_path.clone(),
            source: e,
        })?;
    f.write_all(bytes).map_err(|e| PairingError::Io {
        path: tmp_path.clone(),
        source: e,
    })?;
    f.sync_all().map_err(|e| PairingError::Io {
        path: tmp_path.clone(),
        source: e,
    })?;
    drop(f);

    std::fs::rename(&tmp_path, path).map_err(|e| PairingError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    // Best effort: fsync the parent directory too, so the rename itself
    // is durable across a crash, not merely the file contents.
    if let Ok(dir) = std::fs::File::open(parent) {
        let _ = dir.sync_all();
    }

    Ok(())
}

#[cfg(not(unix))]
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), PairingError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| PairingError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    std::fs::write(path, bytes).map_err(|e| PairingError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

/// RAII lock guard for `pairing.json.lock`.
///
/// Every read-modify-write of [`InviteStore`] MUST hold one; a bare
/// [`InviteStore::load`] for display does not need it. Acquired via
/// `OpenOptions::create_new` (an `O_CREAT|O_EXCL`-equivalent TOCTOU-safe
/// exclusive create), retrying up to 50 times at 20ms on `AlreadyExists`
/// before returning [`PairingError::StoreBusy`]. Removes the lockfile on
/// `Drop`.
pub struct StoreLock {
    lock_path: PathBuf,
}

impl StoreLock {
    /// Acquire the lock for `store_path` (i.e. `store_path` with `.lock`
    /// appended).
    pub fn acquire(store_path: &Path) -> Result<Self, PairingError> {
        let lock_path = lock_path_for(store_path);
        if let Some(parent) = lock_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| PairingError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        for _ in 0..50 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(_) => return Ok(Self { lock_path }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => {
                    return Err(PairingError::Io {
                        path: lock_path,
                        source: e,
                    });
                }
            }
        }
        Err(PairingError::StoreBusy)
    }
}

impl Drop for StoreLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.lock_path);
    }
}

fn lock_path_for(store_path: &Path) -> PathBuf {
    let mut s = store_path.as_os_str().to_owned();
    s.push(".lock");
    PathBuf::from(s)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{
        pairing_store_path, InviteRecord, InviteState, InviteStore, PairingError,
        RedemptionDecision, StoreLock, MAX_ATTEMPTS,
    };

    fn sample_record(id: &str) -> InviteRecord {
        InviteRecord {
            id: id.to_string(),
            principal: "agent:inviter.test/gateway".to_string(),
            code_digest: "a".repeat(64),
            created_at: "2026-08-03T00:00:00Z".to_string(),
            expires_at: "2026-08-04T00:00:00Z".to_string(),
            attempts: 0,
            state: InviteState::Pending,
        }
    }

    /// The digest bytes matching `sample_record`'s `code_digest` (32
    /// bytes of `0xaa`, from `"a".repeat(64)` hex-decoded).
    const MATCHING_DIGEST: [u8; 32] = [0xaa; 32];
    /// A digest that matches no `sample_record` (32 bytes of `0xbb`).
    const NON_MATCHING_DIGEST: [u8; 32] = [0xbb; 32];

    #[test]
    fn max_attempts_is_five() {
        assert_eq!(MAX_ATTEMPTS, 5);
    }

    #[test]
    fn decide_accepts_matching_digest_under_budget() {
        let store = InviteStore {
            invites: vec![sample_record("inv-1")],
        };
        let decision = store.decide(&MATCHING_DIGEST, "2026-08-03T00:00:01Z");
        assert_eq!(
            decision,
            RedemptionDecision::Accept {
                id: "inv-1".to_string()
            }
        );
    }

    /// The falsification pair: one field flipped (`attempts`) changes the
    /// outcome from `Accept` to `AttemptsExhausted`.
    #[test]
    fn decide_attempts_exhausted_at_max_accept_one_below() {
        let mut exhausted = sample_record("inv-1");
        exhausted.attempts = MAX_ATTEMPTS;
        let store = InviteStore {
            invites: vec![exhausted],
        };
        assert_eq!(
            store.decide(&MATCHING_DIGEST, "2026-08-03T00:00:01Z"),
            RedemptionDecision::AttemptsExhausted,
            "attempts == MAX_ATTEMPTS must refuse even the correct code"
        );

        let mut one_below = sample_record("inv-1");
        one_below.attempts = MAX_ATTEMPTS - 1;
        let store = InviteStore {
            invites: vec![one_below],
        };
        assert_eq!(
            store.decide(&MATCHING_DIGEST, "2026-08-03T00:00:01Z"),
            RedemptionDecision::Accept {
                id: "inv-1".to_string()
            },
            "attempts == MAX_ATTEMPTS - 1 must still accept the correct code"
        );
    }

    #[test]
    fn decide_wrong_code_when_no_digest_matches_but_budget_remains() {
        let store = InviteStore {
            invites: vec![sample_record("inv-1")],
        };
        assert_eq!(
            store.decide(&NON_MATCHING_DIGEST, "2026-08-03T00:00:01Z"),
            RedemptionDecision::WrongCode
        );
    }

    #[test]
    fn decide_expired_at_now_equals_expires_at_accept_one_second_earlier() {
        let record = sample_record("inv-1");
        let store = InviteStore {
            invites: vec![record.clone()],
        };
        assert_eq!(
            store.decide(&MATCHING_DIGEST, "2026-08-04T00:00:00Z"),
            RedemptionDecision::Expired,
            "now == expires_at must be expired"
        );
        assert_eq!(
            store.decide(&MATCHING_DIGEST, "2026-08-03T23:59:59Z"),
            RedemptionDecision::Accept { id: record.id },
            "now one second before expires_at must not be expired"
        );
    }

    #[test]
    fn decide_already_redeemed_for_redeemed_or_pinned_match() {
        let mut redeemed = sample_record("inv-1");
        redeemed.state = InviteState::Redeemed {
            by: "agent:redeemer.test/gateway".to_string(),
            key_id: "key-1".to_string(),
            pubkey_b64url: "pk".to_string(),
            at: "2026-08-03T00:01:00Z".to_string(),
        };
        let store = InviteStore {
            invites: vec![redeemed],
        };
        assert_eq!(
            store.decide(&MATCHING_DIGEST, "2026-08-03T00:00:01Z"),
            RedemptionDecision::AlreadyRedeemed
        );

        let mut pinned = sample_record("inv-2");
        pinned.state = InviteState::Pinned {
            at: "2026-08-03T00:02:00Z".to_string(),
        };
        let store = InviteStore {
            invites: vec![pinned],
        };
        assert_eq!(
            store.decide(&MATCHING_DIGEST, "2026-08-03T00:00:01Z"),
            RedemptionDecision::AlreadyRedeemed
        );
    }

    #[test]
    fn decide_revoked_record_never_matches_its_own_digest() {
        let mut revoked = sample_record("inv-1");
        revoked.state = InviteState::Revoked {
            at: "2026-08-03T00:01:00Z".to_string(),
        };
        let store = InviteStore {
            invites: vec![revoked],
        };
        assert_eq!(
            store.decide(&MATCHING_DIGEST, "2026-08-03T00:00:01Z"),
            RedemptionDecision::NoPendingInvite,
            "a Revoked record must never match, even with a digest-equal presented code"
        );
    }

    #[test]
    fn decide_no_pending_invite_on_empty_store() {
        let store = InviteStore::default();
        assert_eq!(
            store.decide(&MATCHING_DIGEST, "2026-08-03T00:00:01Z"),
            RedemptionDecision::NoPendingInvite
        );
    }

    #[test]
    fn burn_attempt_increments_pending_only_leaves_redeemed_unchanged() {
        let mut redeemed = sample_record("inv-1");
        redeemed.state = InviteState::Redeemed {
            by: "agent:redeemer.test/gateway".to_string(),
            key_id: "key-1".to_string(),
            pubkey_b64url: "pk".to_string(),
            at: "2026-08-03T00:01:00Z".to_string(),
        };
        let pending = sample_record("inv-2");
        let mut store = InviteStore {
            invites: vec![redeemed, pending],
        };
        store.burn_attempt("2026-08-03T00:00:01Z");
        assert_eq!(
            store.invites[0].attempts, 0,
            "Redeemed record must not burn"
        );
        assert_eq!(store.invites[1].attempts, 1, "Pending record must burn");
    }

    #[test]
    fn consume_transitions_exactly_one_record_leaves_others_untouched() {
        let target = sample_record("inv-1");
        let other = sample_record("inv-2");
        let mut store = InviteStore {
            invites: vec![target, other.clone()],
        };
        store
            .consume(
                "inv-1",
                "agent:redeemer.test/gateway",
                "key-1",
                "pk",
                "2026-08-03T00:00:01Z",
            )
            .unwrap();
        assert!(matches!(
            store.invites[0].state,
            InviteState::Redeemed { .. }
        ));
        assert_eq!(
            store.invites[1], other,
            "the other record must be untouched"
        );
    }

    #[test]
    fn revoke_unknown_id_errors_and_mutates_nothing() {
        let mut store = InviteStore {
            invites: vec![sample_record("inv-1")],
        };
        let before = store.clone();
        let err = store
            .revoke("does-not-exist", "2026-08-03T00:00:01Z")
            .unwrap_err();
        assert!(matches!(err, PairingError::UnknownInvite { id } if id == "does-not-exist"));
        assert_eq!(store, before, "an unknown-id revoke must mutate nothing");
    }

    #[test]
    fn revoke_known_id_transitions_to_revoked() {
        let mut store = InviteStore {
            invites: vec![sample_record("inv-1")],
        };
        store.revoke("inv-1", "2026-08-03T00:00:01Z").unwrap();
        assert!(matches!(
            store.invites[0].state,
            InviteState::Revoked { .. }
        ));
    }

    #[test]
    fn pairing_store_path_is_gateway_pairing_json() {
        let home = std::path::Path::new("/home/x/.famp");
        assert_eq!(
            pairing_store_path(home),
            std::path::PathBuf::from("/home/x/.famp/gateway/pairing.json")
        );
    }

    #[test]
    fn load_missing_file_returns_empty_store() {
        let tmp = tempfile::tempdir().unwrap();
        let path = pairing_store_path(tmp.path());
        let store = InviteStore::load(&path).unwrap();
        assert!(store.invites.is_empty());
    }

    #[test]
    fn save_atomic_then_load_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let path = pairing_store_path(tmp.path());
        let store = InviteStore {
            invites: vec![sample_record("inv-1")],
        };
        store.save_atomic(&path).unwrap();

        let loaded = InviteStore::load(&path).unwrap();
        assert_eq!(loaded, store);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(&path).unwrap();
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn save_atomic_overwrites_existing_store() {
        let tmp = tempfile::tempdir().unwrap();
        let path = pairing_store_path(tmp.path());
        InviteStore {
            invites: vec![sample_record("inv-1")],
        }
        .save_atomic(&path)
        .unwrap();
        InviteStore {
            invites: vec![sample_record("inv-2")],
        }
        .save_atomic(&path)
        .unwrap();

        let loaded = InviteStore::load(&path).unwrap();
        assert_eq!(loaded.invites.len(), 1);
        assert_eq!(loaded.invites[0].id, "inv-2");
    }

    #[test]
    fn store_lock_acquire_then_drop_allows_reacquire() {
        let tmp = tempfile::tempdir().unwrap();
        let path = pairing_store_path(tmp.path());
        let lock_path = {
            let lock = StoreLock::acquire(&path).unwrap();
            lock.lock_path.clone()
        };
        assert!(
            !lock_path.exists(),
            "lock file must be removed on Drop: {}",
            lock_path.display()
        );
        // Second acquire must succeed now that the first guard dropped.
        let _second = StoreLock::acquire(&path).unwrap();
    }
}

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

/// The invite store's on-disk shape: `{"invites": [...]}`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InviteStore {
    pub invites: Vec<InviteRecord>,
}

impl InviteStore {
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
    use super::{pairing_store_path, InviteRecord, InviteState, InviteStore, StoreLock};

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

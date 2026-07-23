//! Gateway signing keypair persistence (TRUST-01 prerequisite, RESEARCH
//! Pitfall 3 — no live keygen/persistence path existed anywhere in the
//! codebase before this plan).
//!
//! **Scope note (RESEARCH Pitfall 4):** one signing key per remote
//! principal name. See `super` module docs for the full rationale.

use std::path::{Path, PathBuf};

use famp_crypto::FampSigningKey;

use crate::cli::error::CliError;

/// This gateway's own signing-key path: `~/.famp/gateway/identity.ed25519`.
///
/// Deliberately NOT the stale `IdentityLayout::key_ed25519` name (RESEARCH
/// Pitfall 3) — that layout is a dead-in-practice, pre-v0.9 concept with
/// no live writer. This is a fresh, gateway-owned identity file.
#[must_use]
pub fn gateway_identity_path(home: &Path) -> PathBuf {
    home.join("gateway").join("identity.ed25519")
}

/// Gateway peer keyring path: `~/.famp/gateway/peers.keyring` (D-06). The
/// same file the (Phase 9) `verify_inbound` ingress check reads —
/// `famp peer import` is the sole writer.
#[must_use]
pub fn gateway_peers_keyring_path(home: &Path) -> PathBuf {
    home.join("gateway").join("peers.keyring")
}

/// Load the gateway's persisted signing key, generating-and-persisting one
/// on first use.
///
/// Idempotent: a second call against the same `path`
/// returns the SAME key (T-08-12) — the gateway's signing identity must
/// never be silently regenerated once its public key has been distributed
/// to peers.
pub fn load_or_generate(path: &Path) -> Result<FampSigningKey, CliError> {
    if path.exists() {
        let bytes = std::fs::read(path).map_err(|e| CliError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let s = std::str::from_utf8(&bytes).map_err(|e| CliError::Io {
            path: path.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
        })?;
        return FampSigningKey::from_b64url(s.trim()).map_err(|e| {
            CliError::Generic(format!(
                "corrupt gateway signing key at {}: {e}",
                path.display()
            ))
        });
    }

    let key = FampSigningKey::generate();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CliError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    write_signing_key(path, key.to_b64url().as_bytes())?;
    Ok(key)
}

/// Persist the signing key at mode 0600 (unix). Reuses the project's
/// TOCTOU-safe `O_CREAT|O_EXCL` + `mode(0o600)` helper — the same
/// convention used for every other on-disk secret in this crate.
#[cfg(unix)]
fn write_signing_key(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    crate::cli::perms::write_secret(path, bytes).map_err(|e| CliError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

#[cfg(not(unix))]
fn write_signing_key(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    std::fs::write(path, bytes).map_err(|e| CliError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{gateway_identity_path, load_or_generate};

    #[test]
    fn load_or_generate_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let path = gateway_identity_path(tmp.path());
        let k1 = load_or_generate(&path).unwrap();
        let k2 = load_or_generate(&path).unwrap();
        assert_eq!(
            k1.verifying_key().to_b64url(),
            k2.verifying_key().to_b64url(),
            "second load_or_generate call must return the SAME key, not regenerate (T-08-12)"
        );
    }

    #[test]
    fn load_or_generate_persists_to_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let path = gateway_identity_path(tmp.path());
        assert!(!path.exists());
        let _ = load_or_generate(&path).unwrap();
        assert!(path.exists());
    }
}

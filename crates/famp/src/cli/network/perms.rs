//! Filesystem permission policy for the network home: directories 0700,
//! secret and trust files 0600. `problems` is read-only (doctor); `fix` is setup's.
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use super::state::error;
use crate::cli::error::CliError;
use crate::cli::peer::identity;

fn targets(home: &Path) -> Vec<(PathBuf, u32)> {
    vec![
        (home.to_path_buf(), 0o700),
        (home.join("gateway"), 0o700),
        (home.join("network-tls"), 0o700),
        (identity::gateway_identity_path(home), 0o600),
        (home.join("network-tls/key.pem"), 0o600),
        (home.join("gateway/peers.keyring"), 0o600),
        (home.join(super::allowlist::FILE), 0o600),
    ]
}

/// Paths whose mode has any group/other bit set (or is looser than policy).
pub fn problems(home: &Path) -> Vec<String> {
    targets(home)
        .into_iter()
        .filter_map(|(path, want)| {
            let mode = std::fs::metadata(&path).ok()?.permissions().mode() & 0o777;
            (mode & !want != 0).then(|| format!("{} is {mode:o}, must be {want:o}", path.display()))
        })
        .collect()
}

/// Tighten every existing target to policy. Never loosens, never creates.
pub fn fix(home: &Path) -> Result<(), CliError> {
    for (path, want) in targets(home) {
        if let Ok(meta) = std::fs::metadata(&path) {
            let mode = meta.permissions().mode() & 0o777;
            if mode & !want != 0 {
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode & want))
                    .map_err(error)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_and_fixes_drift_without_loosening() -> Result<(), CliError> {
        let dir = tempfile::tempdir().map_err(error)?;
        let home = dir.path();
        std::fs::create_dir_all(home.join("gateway")).map_err(error)?;
        std::fs::set_permissions(home, std::fs::Permissions::from_mode(0o775)).map_err(error)?;
        let ring = home.join("gateway/peers.keyring");
        std::fs::write(&ring, "").map_err(error)?;
        std::fs::set_permissions(&ring, std::fs::Permissions::from_mode(0o664)).map_err(error)?;
        assert_eq!(problems(home).len(), 3, "home, gateway dir, keyring");
        fix(home)?;
        assert!(problems(home).is_empty());
        std::fs::set_permissions(&ring, std::fs::Permissions::from_mode(0o400)).map_err(error)?;
        fix(home)?;
        assert_eq!(
            std::fs::metadata(&ring)
                .map_err(error)?
                .permissions()
                .mode()
                & 0o777,
            0o400
        );
        Ok(())
    }
}

//! Atomic directory replace for the `--force` path. Same-filesystem `TempDir`
//! + two-step rename with best-effort rollback (D-10).

use crate::cli::error::CliError;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Atomically replaces `target` via same-filesystem staging + rename.
///
/// Calls `writer` on a sibling `TempDir`, renames the old target aside,
/// renames the staging directory into place, then removes the backup. On
/// failure after the first rename, best-effort rolls back.
pub fn atomic_replace<F>(target: &Path, writer: F) -> Result<(), CliError>
where
    F: FnOnce(&Path) -> Result<(), CliError>,
{
    let parent = target.parent().ok_or_else(|| CliError::HomeHasNoParent {
        path: target.to_path_buf(),
    })?;

    let staging = TempDir::new_in(parent).map_err(|e| CliError::Io {
        path: parent.to_path_buf(),
        source: e,
    })?;
    writer(staging.path())?;

    let backup: PathBuf = parent.join(format!(".famp-old-{}", std::process::id()));
    if target.exists() {
        std::fs::rename(target, &backup).map_err(|e| CliError::Io {
            path: target.to_path_buf(),
            source: e,
        })?;
    }

    // Consume `TempDir` without dropping (drop would delete the staged dir).
    let staging_path = staging.keep();

    if let Err(e) = std::fs::rename(&staging_path, target) {
        // Best-effort rollback.
        if backup.exists() {
            let _ = std::fs::rename(&backup, target);
        }
        return Err(CliError::Io {
            path: target.to_path_buf(),
            source: e,
        });
    }

    if backup.exists() {
        let _ = std::fs::remove_dir_all(&backup);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn replaces_existing_directory_contents() {
        let parent = tempfile::TempDir::new().expect("parent tempdir");
        let target = parent.path().join("famphome");
        fs::create_dir(&target).expect("mkdir target");
        fs::write(target.join("old"), b"old content").expect("write old");

        atomic_replace(&target, |staging| {
            fs::write(staging.join("new"), b"new content").map_err(|e| CliError::Io {
                path: staging.join("new"),
                source: e,
            })
        })
        .expect("atomic_replace");

        assert!(target.join("new").exists());
        assert!(!target.join("old").exists());
    }
}

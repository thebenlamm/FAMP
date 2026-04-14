//! Atomic file write helper. Same-directory `NamedTempFile` + persist.
//!
//! MIRROR: `crates/famp-inbox/src/cursor.rs` keeps a near-identical helper
//! so the two crates stay independent. Keep them in sync if you touch the
//! fsync / permissions logic.

use std::io::Write as _;
use std::path::Path;

/// Write `bytes` to `path` atomically: temp file in the same dir → fsync →
/// rename → chmod 0600 (Unix). Returns `io::Error` on failure.
pub fn write_atomic_file(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent")
    })?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(bytes)?;
    tmp.as_file_mut().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

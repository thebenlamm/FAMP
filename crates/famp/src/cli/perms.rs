//! Secure file-write helpers for the FAMP identity layout.
//!
//! `write_secret` creates a file at mode 0600 atomically using
//! `O_CREAT|O_EXCL` + `mode(0o600)`, closing the TOCTOU window that a
//! separate `set_permissions` call would open (RESEARCH §Pattern 3).
//! A belt-and-braces `set_permissions(0o600)` follows the write to
//! neutralize any unusual umask (RESEARCH §Pitfall 7).

#[cfg(unix)]
use std::fs::{OpenOptions, Permissions};
#[cfg(unix)]
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
#[cfg(unix)]
use std::path::Path;

#[cfg(unix)]
fn write_with_mode(path: &Path, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    // Belt and braces: umask could in theory clip mode bits; force-set.
    std::fs::set_permissions(path, Permissions::from_mode(mode))?;
    Ok(())
}

/// Create `path` and write `bytes` at mode 0600.
///
/// Fails with `ErrorKind::AlreadyExists` if `path` already exists.
#[cfg(unix)]
pub fn write_secret(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    write_with_mode(path, bytes, 0o600)
}

/// Create `path` and write `bytes` at mode 0644.
///
/// Fails with `ErrorKind::AlreadyExists` if `path` already exists.
#[cfg(unix)]
pub fn write_public(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    write_with_mode(path, bytes, 0o644)
}

#[cfg(all(test, unix))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn write_secret_is_0600() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("secret.bin");
        let data = b"super secret 32 bytes of material";
        write_secret(&path, data).unwrap();
        let meta = std::fs::metadata(&path).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::read(&path).unwrap(), data);
    }

    #[test]
    fn write_public_is_0644() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("public.bin");
        let data = b"public bytes";
        write_public(&path, data).unwrap();
        let meta = std::fs::metadata(&path).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o644);
        assert_eq!(std::fs::read(&path).unwrap(), data);
    }

    #[test]
    fn write_secret_refuses_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("dup.bin");
        write_secret(&path, b"first").unwrap();
        let err = write_secret(&path, b"second").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
    }
}

//! Platform-conditional NFS detector — Phase 02 (BROKER-05).
//!
//! Returns `true` when `path` lives on an NFS-mounted filesystem.
//! Backed by `nix::sys::statfs` (which wraps `statfs(2)` /
//! `getfsstat(2)`):
//!
//! - **Linux**: compare `Statfs::filesystem_type()` against the
//!   `NFS_SUPER_MAGIC` constant nix re-exports from libc.
//! - **macOS**: read `Statfs::filesystem_type_name()` (which surfaces
//!   the BSD `f_fstypename` byte string) and check it starts with
//!   `"nfs"` — this matches every macOS NFS variant
//!   (`nfs`, `nfs3`, `nfs4`).
//!
//! Failure to `statfs` (e.g. permission denied, ENOENT) is treated as
//! "not NFS" rather than propagated — the warning is best-effort and
//! must not block broker startup.

use std::path::Path;

#[cfg(target_os = "linux")]
pub fn is_nfs(path: &Path) -> bool {
    use nix::sys::statfs::{statfs, NFS_SUPER_MAGIC};
    statfs(path)
        .map(|s| s.filesystem_type() == NFS_SUPER_MAGIC)
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
pub fn is_nfs(path: &Path) -> bool {
    use nix::sys::statfs::statfs;
    // `filesystem_type_name()` returns `&str` (nix 0.31 surfacing of
    // `f_fstypename`); compare its byte view against the magic prefix
    // `b"nfs"` so the matcher captures every macOS NFS variant
    // (`nfs`, `nfs3`, `nfs4`).
    statfs(path)
        .map(|s| s.filesystem_type_name().as_bytes().starts_with(b"nfs"))
        .unwrap_or(false)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn is_nfs(_path: &Path) -> bool {
    // No detection for unsupported platforms; default to "not NFS"
    // so the broker still starts.
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `/tmp` is never NFS on a developer laptop or CI runner. The
    /// detector must not falsely flag it.
    ///
    /// This unit test populates the BROKER-05 stub from VALIDATION.md
    /// (`test_nfs_warning`); a future integration test in
    /// `crates/famp/tests/broker_lifecycle.rs` (owned by plan 02-11)
    /// will exercise the broker startup path with a mocked NFS path.
    #[test]
    fn test_nfs_warning() {
        // Use the system tmp dir directly (e.g. `/tmp` on Linux,
        // `/var/folders/...` on macOS).
        let tmp = std::env::temp_dir();
        assert!(
            !is_nfs(&tmp),
            "system tmp dir {tmp:?} is unexpectedly classified as NFS"
        );
    }

    #[test]
    fn nonexistent_path_returns_false() {
        let p = Path::new("/nonexistent/famp/path/that/should/not/exist");
        assert!(
            !is_nfs(p),
            "nonexistent path must default to false (best-effort detector)"
        );
    }
}

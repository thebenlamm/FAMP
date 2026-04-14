//! Advisory file-based inbox lock. Best-effort, RAII.
//!
//! Held by `famp await` (and any future single-consumer reader) to prevent
//! two processes from double-consuming inbox entries. The lock file
//! contains the holder's PID as ASCII decimal; stale PIDs (dead processes)
//! are reaped on next acquire.
//!
//! Semantics are deliberately fail-fast: if a live PID already holds the
//! lock, [`InboxLock::acquire`] returns [`InboxError::LockHeld`] WITHOUT
//! waiting. Phase 3's CLI is a single-developer surface, so concurrent
//! awaits are user error and silent waiting would mask the mistake.

use std::fs::OpenOptions;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use crate::InboxError;

/// RAII advisory lock at `<home>/inbox.lock`.
///
/// Construct via [`InboxLock::acquire`]. Dropping the value best-effort
/// removes the lock file; a crashed holder is reaped by the next acquirer
/// via a PID liveness check.
pub struct InboxLock {
    path: PathBuf,
    _file: std::fs::File,
}

impl InboxLock {
    /// Acquire the advisory lock at `<home>/inbox.lock`.
    ///
    /// - If the lock file already exists and its PID is a live process,
    ///   returns [`InboxError::LockHeld`].
    /// - If the lock file exists but the PID is dead or unparseable, the
    ///   file is treated as stale: removed and reacquired.
    /// - On success the file is created mode 0600 (Unix) containing the
    ///   current PID as ASCII decimal followed by `\n`.
    pub fn acquire(home: &Path) -> Result<Self, InboxError> {
        let path = home.join("inbox.lock");

        // Check existing lock for liveness.
        if path.exists() {
            if let Ok(mut f) = std::fs::File::open(&path) {
                let mut s = String::new();
                if f.read_to_string(&mut s).is_ok() {
                    if let Ok(pid) = s.trim().parse::<u32>() {
                        if is_alive(pid) {
                            return Err(InboxError::LockHeld { path, pid });
                        }
                    }
                }
            }
            // Stale or unparseable — reap.
            let _ = std::fs::remove_file(&path);
        }

        // Create exclusively. A racing creator is reported as LockHeld.
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = match opts.open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let pid = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|s| s.trim().parse::<u32>().ok())
                    .unwrap_or(0);
                return Err(InboxError::LockHeld { path, pid });
            }
            Err(source) => {
                return Err(InboxError::Io { path, source });
            }
        };

        let pid = std::process::id();
        file.write_all(format!("{pid}\n").as_bytes())
            .map_err(|source| InboxError::Io {
                path: path.clone(),
                source,
            })?;
        file.sync_all().map_err(|source| InboxError::Io {
            path: path.clone(),
            source,
        })?;

        Ok(Self { path, _file: file })
    }

    /// Path of the lock file on disk.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for InboxLock {
    fn drop(&mut self) {
        // Best-effort: remove the lock file. If another process has
        // already re-created it (impossible under fail-fast semantics, but
        // defensive), the rm quietly fails and we do nothing.
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(unix)]
fn is_alive(pid: u32) -> bool {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;
    // signal=None → just check existence.
    // - Ok(()) → process exists and we can signal it.
    // - EPERM → process exists but is owned by another user: still alive.
    // - ESRCH (or anything else) → dead.
    let Ok(pid_i32) = i32::try_from(pid) else {
        return false;
    };
    matches!(
        kill(Pid::from_raw(pid_i32), None),
        Ok(()) | Err(nix::errno::Errno::EPERM)
    )
}

#[cfg(not(unix))]
fn is_alive(_pid: u32) -> bool {
    // Phase 3 does not target Windows. Conservative: assume alive so we
    // never accidentally reap a live holder on a platform we have not
    // validated.
    true
}

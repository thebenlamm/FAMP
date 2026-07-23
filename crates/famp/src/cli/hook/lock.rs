//! Per-identity Stop-await singleton lock (B2 dual-hook guard).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::log::log;

pub struct StopAwaitLock {
    path: PathBuf,
}

impl StopAwaitLock {
    /// Acquire the exclusive await lock for `identity`. Returns `None` if
    /// another live process already holds it (caller should no-op).
    pub fn try_acquire(identity: &str) -> Option<Self> {
        let root = state_dir().join("stop-await-locks");
        Self::try_acquire_in(&root, identity)
    }

    /// Root-explicit variant (unit tests use an isolated tempdir root so
    /// they never touch the real `$XDG_STATE_HOME`/`$HOME` lock directory).
    fn try_acquire_in(root: &Path, identity: &str) -> Option<Self> {
        let _ = fs::create_dir_all(root);
        let path = root.join(format!("{identity}.lock"));

        // Atomic acquire: a single lock FILE opened with O_EXCL
        // (`create_new`) so a second concurrent hook firing cannot observe
        // "no pid file yet" and conclude the lock is stale before the first
        // holder has written its pid (the prior create_dir-then-write-pid
        // sequence had exactly that TOCTOU window).
        if Self::create_and_claim(&path).is_ok() {
            return Some(Self { path });
        }

        // The lock file exists; classify the holder by its contents. A
        // parseable pid tells us whether the holder is alive. An EMPTY or
        // unparseable file means the holder created it (O_EXCL) but has not
        // yet written its pid — `create_and_claim` does `create_new` and
        // `writeln!(pid)` as two syscalls, and that nanosecond window is a
        // JUST-BORN lock, not a stale one. Stealing it would double-park
        // (the exact race this singleton exists to prevent). Distinguish by
        // mtime: a file that crashed between the two syscalls ages out of the
        // grace window and becomes reclaimable, so listen mode self-heals.
        match fs::read_to_string(&path).ok().map(|s| s.trim().to_owned()) {
            Some(s) if !s.is_empty() => match s.parse::<u32>() {
                Ok(pid) if pid_alive(pid) => {
                    log(&format!(
                        "stop-await singleton: {identity} already parked by pid={pid}; exiting no-op"
                    ));
                    return None;
                }
                // Parseable but dead pid, or garbage contents: genuinely
                // stale — fall through to reclaim.
                _ => {}
            },
            // Empty file (or unreadable): a just-born holder within the grace
            // window must not be stolen. Aged-out empties are orphans → reclaim.
            _ => {
                if lock_is_within_grace(&path) {
                    log(&format!(
                        "stop-await singleton: {identity} lock is just-born (empty, fresh); exiting no-op"
                    ));
                    return None;
                }
            }
        }

        // Stale lock: reclaim by removing then retrying `create_new` once.
        // Residual window: another process could recreate the file between
        // this `remove_file` and the retry below; accepted as a tight but
        // non-zero race, same as any stale-lockfile reclaim strategy without
        // a kernel-level cross-process mutex.
        let _ = fs::remove_file(&path);
        if Self::create_and_claim(&path).is_ok() {
            return Some(Self { path });
        }

        log(&format!(
            "stop-await singleton: could not reclaim lock for {identity}; exiting no-op"
        ));
        None
    }

    /// Atomically create the lock file (`O_EXCL`) and write this process's
    /// pid on the same handle. There is a nanosecond window between the
    /// `create_new` and the `writeln!` where the file exists but is empty;
    /// `try_acquire_in` treats such an empty-but-fresh lock as a live
    /// just-born holder (mtime grace), not a stale one, so it is never stolen.
    fn create_and_claim(path: &Path) -> std::io::Result<()> {
        let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
        writeln!(f, "{}", std::process::id())?;
        Ok(())
    }
}

/// Grace window during which an empty/unparseable lock file is presumed to be
/// a live just-born holder (created via `O_EXCL` but pid not yet written)
/// rather than a stale orphan. Far larger than the real inter-syscall gap
/// (nanoseconds) so a live holder is never stolen, yet bounded so a holder
/// that crashed in that window self-heals on a later hook firing.
const LOCK_JUST_BORN_GRACE: Duration = Duration::from_secs(30);

fn lock_is_within_grace(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    let Ok(mtime) = meta.modified() else {
        return false;
    };
    match SystemTime::now().duration_since(mtime) {
        Ok(age) => age < LOCK_JUST_BORN_GRACE,
        // mtime in the future (clock skew): treat as fresh, do not steal.
        Err(_) => true,
    }
}

impl Drop for StopAwaitLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn state_dir() -> PathBuf {
    if let Ok(p) = std::env::var("XDG_STATE_HOME") {
        return PathBuf::from(p).join("famp");
    }
    if let Ok(h) = std::env::var("HOME") {
        return PathBuf::from(h).join(".local").join("state").join("famp");
    }
    PathBuf::from("/tmp/famp")
}

#[cfg(target_os = "linux")]
fn pid_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(not(target_os = "linux"))]
fn pid_alive(pid: u32) -> bool {
    // Existence probe via kill -0; absolute path so minimal PATH still works.
    std::process::Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(true)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_while_first_alive_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("stop-await-locks");
        let guard = StopAwaitLock::try_acquire_in(&root, "dk");
        assert!(guard.is_some(), "first acquire should succeed");

        let second = StopAwaitLock::try_acquire_in(&root, "dk");
        assert!(
            second.is_none(),
            "second acquire while first guard is alive must return None"
        );
    }

    #[test]
    fn acquire_succeeds_again_after_drop() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("stop-await-locks");
        let guard = StopAwaitLock::try_acquire_in(&root, "dk");
        assert!(guard.is_some());
        drop(guard);

        let second = StopAwaitLock::try_acquire_in(&root, "dk");
        assert!(
            second.is_some(),
            "acquire must succeed again once the first guard is dropped"
        );
    }

    #[test]
    fn empty_fresh_lock_is_not_stolen() {
        // Regression for the just-born double-park: an existing lock file that
        // is EMPTY (holder created it via O_EXCL but has not written its pid
        // yet) must be treated as a live holder, not stolen.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("stop-await-locks");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("dk.lock");
        std::fs::write(&path, "").unwrap(); // empty, fresh mtime
        let guard = StopAwaitLock::try_acquire_in(&root, "dk");
        assert!(
            guard.is_none(),
            "an empty, freshly-created lock must be treated as a live just-born holder"
        );
    }
}

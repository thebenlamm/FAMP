//! Per-identity Stop-await singleton lock (B2 dual-hook guard).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

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

        let old_pid = fs::read_to_string(&path)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok());
        if let Some(pid) = old_pid {
            if pid_alive(pid) {
                log(&format!(
                    "stop-await singleton: {identity} already parked by pid={pid}; exiting no-op"
                ));
                return None;
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
    /// pid into it on the same handle, so the pid is visible the instant the
    /// file exists — no window where the file exists but is unclaimed.
    fn create_and_claim(path: &Path) -> std::io::Result<()> {
        let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
        writeln!(f, "{}", std::process::id())?;
        Ok(())
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
}

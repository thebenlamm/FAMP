//! Phase 3 Plan 03-04 Task 1 — `InboxLock` advisory lock behavior.
//!
//! Locks the six expected behaviors:
//! 1. `acquire` creates the lock file containing the caller PID.
//! 2. Drop removes the lock file.
//! 3. A second in-process acquire while the first is still held returns
//!    `LockHeld`.
//! 4. A second acquire after the first is dropped succeeds.
//! 5. A stale-PID lock (content = a PID that is no longer a process) is
//!    reaped and the new acquire succeeds.
//! 6. The lock file is mode 0600 on Unix.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use famp_inbox::{InboxError, InboxLock};
use tempfile::TempDir;

#[test]
fn acquire_creates_lock_file_with_pid() {
    let tmp = TempDir::new().unwrap();
    let lock = InboxLock::acquire(tmp.path()).expect("acquire");
    let path = tmp.path().join("inbox.lock");
    assert!(path.exists(), "lock file must exist after acquire");
    let contents = std::fs::read_to_string(&path).unwrap();
    let pid: u32 = contents.trim().parse().expect("pid parses");
    assert_eq!(pid, std::process::id(), "lock file holds this process's pid");
    drop(lock);
}

#[test]
fn drop_removes_lock_file() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("inbox.lock");
    {
        let _lock = InboxLock::acquire(tmp.path()).expect("acquire");
        assert!(path.exists());
    }
    assert!(!path.exists(), "drop must remove the lock file");
}

#[test]
fn second_acquire_while_first_held_returns_lock_held() {
    let tmp = TempDir::new().unwrap();
    let first = InboxLock::acquire(tmp.path()).expect("acquire first");
    let result = InboxLock::acquire(tmp.path());
    match result {
        Err(InboxError::LockHeld { pid, .. }) => {
            assert_eq!(pid, std::process::id());
        }
        Err(other) => panic!("expected LockHeld, got {other:?}"),
        Ok(_) => panic!("expected LockHeld, got Ok(InboxLock)"),
    }
    drop(first);
}

#[test]
fn second_acquire_after_drop_succeeds() {
    let tmp = TempDir::new().unwrap();
    let first = InboxLock::acquire(tmp.path()).expect("acquire first");
    drop(first);
    let _second = InboxLock::acquire(tmp.path()).expect("acquire second");
}

#[test]
fn stale_pid_lock_is_reaped() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("inbox.lock");
    // Write a PID that is almost certainly not a running process. The
    // Linux kernel default pid_max is 2^22 but many systems cap earlier;
    // 0x7fff_fff0 is a guaranteed-out-of-range sentinel.
    std::fs::write(&path, "2147483632\n").unwrap();
    assert!(path.exists());
    let _lock = InboxLock::acquire(tmp.path()).expect("stale lock reaped");
    // New file contains our pid, not the garbage.
    let contents = std::fs::read_to_string(&path).unwrap();
    let pid: u32 = contents.trim().parse().unwrap();
    assert_eq!(pid, std::process::id());
}

#[test]
fn unparseable_lock_is_reaped() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("inbox.lock");
    std::fs::write(&path, "not-a-number\n").unwrap();
    let _lock = InboxLock::acquire(tmp.path()).expect("unparseable reaped");
    let contents = std::fs::read_to_string(&path).unwrap();
    let pid: u32 = contents.trim().parse().unwrap();
    assert_eq!(pid, std::process::id());
}

#[test]
fn lock_file_is_mode_0600_on_unix() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = TempDir::new().unwrap();
    let _lock = InboxLock::acquire(tmp.path()).expect("acquire");
    let path = tmp.path().join("inbox.lock");
    let meta = std::fs::metadata(&path).unwrap();
    let mode = meta.permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "lock file must be 0600, got {mode:o}");
}

// Silencers for transitive deps the binary pulls but this test does not
// reference directly.
use serde_json as _;
use thiserror as _;
use tokio as _;

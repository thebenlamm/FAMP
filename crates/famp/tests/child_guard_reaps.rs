//! Regression test for the test-harness leak fix: ChildGuard must kill + reap
//! its child when dropped (the RAII path that fires on panic unwind).
#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[path = "common/child_guard.rs"]
mod child_guard;
use child_guard::ChildGuard;

use nix::sys::signal::kill;
use nix::unistd::Pid;
use std::process::{Command, Stdio};
use std::time::Duration;

#[test]
fn child_guard_kills_and_reaps_on_drop() {
    let child = Command::new("sleep")
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    // PID is positive; cast is safe on every supported target.
    #[allow(clippy::cast_possible_wrap)]
    let pid = child.id() as i32;
    let nix_pid = Pid::from_raw(pid);

    let mut guard = ChildGuard::new(child);
    // Sanity: the child is alive before we drop the guard.
    assert!(
        kill(nix_pid, None).is_ok(),
        "sleep child {pid} should be alive before drop"
    );
    // try_wait must report still-running.
    assert!(
        guard.as_mut().unwrap().try_wait().unwrap().is_none(),
        "child {pid} unexpectedly exited before drop"
    );

    // Drop fires kill() + wait() -> process is signalled and reaped.
    drop(guard);

    // Give the OS a moment to tear the process down, then prove ESRCH.
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        kill(nix_pid, None).is_err(),
        "child {pid} still reachable after ChildGuard drop - not reaped"
    );
}

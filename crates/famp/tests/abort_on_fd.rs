#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

//! Issue #21 — `famp await --abort-on-fd <n>` cancellation seam.
//!
//! These tests drive the REAL `famp` binary (via `assert_cmd`) against an
//! isolated per-test broker, arming the cancellation fd with a real inherited
//! pipe. They assert the exit-code contract (0 = message/timeout, 1 = error,
//! 3 = aborted) and the in-flight-reply tie-break.
//!
//! Every spawned `famp register`/broker child is held in a [`ChildGuard`] so a
//! panic mid-test still kills + reaps it (project rule — else tmp-socket
//! brokers leak).

use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;
use nix::fcntl::{fcntl, FcntlArg, FdFlag};

#[path = "common/child_guard.rs"]
mod child_guard;
use child_guard::ChildGuard;

/// Per-test isolation: a unique tempdir holding the bus socket. Each test
/// gets its own broker via `famp register`-driven lazy spawn.
struct Bus {
    tmp: tempfile::TempDir,
    sock: std::path::PathBuf,
}

impl Bus {
    fn new() -> Self {
        let tmp = tempfile::TempDir::new().unwrap();
        let sock = tmp.path().join("bus.sock");
        Self { tmp, sock }
    }

    fn sock(&self) -> &Path {
        self.sock.as_path()
    }

    fn famp_cmd(&self, args: &[&str]) -> std::process::Output {
        Command::cargo_bin("famp")
            .unwrap()
            .env("FAMP_BUS_SOCKET", self.sock())
            .env("HOME", self.tmp.path())
            .args(args)
            .output()
            .unwrap()
    }

    /// Spawn a long-lived `famp register <name>` holder, wrapped in a
    /// ChildGuard so it is always reaped.
    fn register(&self, name: &str) -> ChildGuard {
        let child = Command::cargo_bin("famp")
            .unwrap()
            .env("FAMP_BUS_SOCKET", self.sock())
            .env("HOME", self.tmp.path())
            .args(["register", name])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        ChildGuard::new(child)
    }

    /// Poll until `name` is a live registered holder (proxy `whoami`
    /// succeeds) or a deadline elapses.
    fn wait_for_register(&self, name: &str) {
        for _ in 0..50 {
            if self.famp_cmd(&["whoami", "--as", name]).status.success() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("{name} did not register within 5s");
    }

    /// Spawn `famp await --as <name> --timeout <t> --abort-on-fd <fd>`,
    /// returning the child. `read_fd` is inherited by the child at the same
    /// fd number (it must be non-CLOEXEC — see [`make_abort_pipe`]).
    fn spawn_await_abortable(
        &self,
        name: &str,
        timeout: &str,
        read_fd: i32,
    ) -> std::process::Child {
        let fd_str = read_fd.to_string();
        Command::cargo_bin("famp")
            .unwrap()
            .env("FAMP_BUS_SOCKET", self.sock())
            .env("HOME", self.tmp.path())
            .args([
                "await",
                "--as",
                name,
                "--timeout",
                timeout,
                "--abort-on-fd",
                &fd_str,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    }
}

/// Create a pipe whose READ end is inheritable (non-CLOEXEC, so a spawned
/// child sees it at the same fd number) and whose WRITE end is CLOEXEC (so
/// the child never inherits a writer — required for the EOF test to reach
/// zero writers when the parent drops its write end).
fn make_abort_pipe() -> (OwnedFd, OwnedFd) {
    let (read_end, write_end) = nix::unistd::pipe().unwrap();
    fcntl(read_end.as_fd(), FcntlArg::F_SETFD(FdFlag::empty())).unwrap();
    fcntl(write_end.as_fd(), FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC)).unwrap();
    (read_end, write_end)
}

/// TEST 1 — a byte written to the abort fd cancels a parked await, exit 3.
#[test]
fn abort_fd_write_cancels_parked_await_with_exit_3() {
    let bus = Bus::new();
    let _dk = bus.register("dk");
    bus.wait_for_register("dk");

    let (read_end, write_end) = make_abort_pipe();
    // Long timeout: only the abort should end the await. If abort is broken,
    // the await times out here (exit 0) and the exit-3 assertion fails.
    let child = bus.spawn_await_abortable("dk", "8s", read_end.as_raw_fd());

    // Let the await park on the broker before we fire the abort.
    std::thread::sleep(Duration::from_millis(500));
    nix::unistd::write(write_end.as_fd(), b"x").unwrap();

    let out = child.wait_with_output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(3),
        "abort must exit 3; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\"aborted\":true"),
        "abort must print the aborted sentinel; got: {stdout:?}"
    );
    drop((read_end, write_end));
}

/// TEST 2 — EOF (all writers gone), not just bytes, cancels the await.
#[test]
fn abort_fd_close_eof_also_cancels() {
    let bus = Bus::new();
    let _dk = bus.register("dk");
    bus.wait_for_register("dk");

    let (read_end, write_end) = make_abort_pipe();
    let child = bus.spawn_await_abortable("dk", "8s", read_end.as_raw_fd());

    std::thread::sleep(Duration::from_millis(500));
    // The child does NOT hold the write end (CLOEXEC); dropping the parent's
    // write end leaves zero writers → the child's read end sees EOF, which
    // AsyncFd reports as readable → abort.
    drop(write_end);

    let out = child.wait_with_output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(3),
        "EOF must abort with exit 3; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("\"aborted\":true"),
        "EOF abort must print the aborted sentinel"
    );
    drop(read_end);
}

/// TEST 3 — the tie-break that matters: an in-flight `AwaitOk` beats a
/// simultaneous abort. We fire the abort byte AND a real message; the
/// grace-window re-poll of the pinned read future must return the envelope
/// with exit 0.
#[test]
fn inflight_awaitok_beats_a_simultaneous_abort() {
    let bus = Bus::new();
    let _alice = bus.register("alice");
    let _bob = bus.register("bob");
    bus.wait_for_register("alice");
    bus.wait_for_register("bob");

    let (read_end, write_end) = make_abort_pipe();
    let child = bus.spawn_await_abortable("bob", "8s", read_end.as_raw_fd());

    // Let bob's await park.
    std::thread::sleep(Duration::from_millis(500));

    // Fire the abort FIRST (worst case for the message: abort becomes
    // readable before the reply arrives), then immediately send. The grace
    // window must still let the reply win.
    nix::unistd::write(write_end.as_fd(), b"x").unwrap();
    let send = bus.famp_cmd(&["send", "--as", "alice", "--to", "bob", "--new-task", "ping"]);
    assert!(
        send.status.success(),
        "send failed: {}",
        String::from_utf8_lossy(&send.stderr)
    );

    let out = child.wait_with_output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "in-flight message must beat the abort (exit 0); stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\"ping\""),
        "the delivered envelope must be returned, not swallowed by the abort; got: {stdout:?}"
    );
    assert!(
        !stdout.contains("\"aborted\":true"),
        "must not print the aborted sentinel when the message wins; got: {stdout:?}"
    );
    drop((read_end, write_end));
}

/// TEST 4 — an invalid fd is a clean hard error (exit 1), never UB, never
/// exit 3. Validation happens before the broker round-trip, so no holder is
/// needed.
#[test]
fn invalid_abort_fd_is_a_hard_error_not_ub() {
    let bus = Bus::new();
    // fd 999 is not open in the child; F_GETFD → EBADF → CliError → exit 1.
    let out = bus.famp_cmd(&["await", "--as", "dk", "--abort-on-fd", "999"]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "invalid fd must be a hard error (exit 1), not 0/3; stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not a valid open file descriptor"),
        "error must name the invalid fd cause; got: {stderr:?}"
    );
}

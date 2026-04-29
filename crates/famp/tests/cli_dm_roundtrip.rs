#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Phase 02 plan 02-12 — `famp` CLI integration round-trip tests.
//!
//! Exercises TEST-01 (DM round-trip) and the per-command CLI assertions
//! CLI-01 (`register`), CLI-02 (`send`), CLI-03 (`inbox list`), CLI-05
//! (`await`), CLI-08 (`whoami`). All five tests shell `famp` as a real
//! subprocess via `assert_cmd::Command::cargo_bin` and run against an
//! isolated `FAMP_BUS_SOCKET` per test (each test has its own tempdir
//! and broker process). Identity binding follows D-10 (proxy via
//! `Hello.bind_as`), surfaced via `--as <name>`.

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;

/// Per-test isolation: a unique tempdir holding the bus socket. Each
/// test gets its own broker process via the `famp register`-driven lazy
/// spawn (`spawn_broker_if_absent`).
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

    /// Run a one-shot `famp <args>` against this bus and return the
    /// captured `Output`. Adds `HOME` pointed at the bus tempdir so any
    /// D-01 identity-resolution side effects cannot leak into the
    /// developer's real `~/.famp-local`.
    fn famp_cmd(&self, args: &[&str]) -> std::process::Output {
        Command::cargo_bin("famp")
            .unwrap()
            .env("FAMP_BUS_SOCKET", self.sock())
            .env("HOME", self.tmp.path())
            .args(args)
            .output()
            .unwrap()
    }

    /// Spawn `famp <args>` in the background (suppress output) and
    /// return the `Child` handle. Used for the long-lived `famp
    /// register <name>` canonical-holder processes.
    fn famp_spawn(&self, args: &[&str]) -> Child {
        Command::cargo_bin("famp")
            .unwrap()
            .env("FAMP_BUS_SOCKET", self.sock())
            .env("HOME", self.tmp.path())
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    }

    /// Poll until a one-shot proxy connect with `--as <name>` succeeds
    /// OR a deadline elapses. Proves the broker is up AND `name`'s
    /// holder has completed its Register handshake. The lazy
    /// broker-spawn-then-Register path can take longer than a fixed
    /// sleep on slow CI.
    fn wait_for_register(&self, name: &str) {
        for _ in 0..50 {
            // `whoami --as <name>` exits 0 only if Hello.bind_as proxy
            // validation succeeded, which requires `name` to be held by
            // a live registered holder.
            let out = self.famp_cmd(&["whoami", "--as", name]);
            if out.status.success() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("{name} did not register within 5s");
    }
}

/// Best-effort cleanup so the broker observes Disconnect promptly and
/// the next test starts with a fresh tempdir.
fn kill_and_wait(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// CLI-01 (replaces the plan 02-03 wire-level test): `famp register
/// alice --no-reconnect` blocks. With `--no-reconnect` the holder
/// process stays alive until killed.
#[test]
fn test_register_blocks() {
    let bus = Bus::new();
    let mut alice = bus.famp_spawn(&["register", "alice", "--no-reconnect"]);
    bus.wait_for_register("alice");
    // alice should still be running (blocking) once registered.
    assert!(
        alice.try_wait().unwrap().is_none(),
        "alice should still be blocking"
    );
    kill_and_wait(&mut alice);
}

/// TEST-01: full DM round-trip via shelled CLI. Covers CLI-01, CLI-02,
/// and CLI-03 in one test:
///
/// 1. alice + bob register (CLI-01).
/// 2. alice `famp send --as alice --to bob --new-task "hi"` (CLI-02).
/// 3. bob `famp inbox list --as bob` shows "hi" plus the
///    `{"next_offset":N}` footer (CLI-03).
#[test]
fn test_dm_roundtrip() {
    let bus = Bus::new();
    let mut alice = bus.famp_spawn(&["register", "alice"]);
    let mut bob = bus.famp_spawn(&["register", "bob"]);
    bus.wait_for_register("alice");
    bus.wait_for_register("bob");

    let send = bus.famp_cmd(&["send", "--as", "alice", "--to", "bob", "--new-task", "hi"]);
    assert!(
        send.status.success(),
        "send failed: stderr={}",
        String::from_utf8_lossy(&send.stderr)
    );
    let send_stdout = String::from_utf8_lossy(&send.stdout);
    assert!(
        send_stdout.contains("task_id"),
        "send stdout should include task_id: {send_stdout}"
    );

    // bob's inbox sees the message body. The envelope on the wire is
    // `{"mode":"new_task","summary":"hi"}` — the summary string is the
    // body the user typed, so a substring match for "hi" is faithful.
    let inbox = bus.famp_cmd(&["inbox", "list", "--as", "bob"]);
    assert!(
        inbox.status.success(),
        "inbox list failed: stderr={}",
        String::from_utf8_lossy(&inbox.stderr)
    );
    let inbox_stdout = String::from_utf8_lossy(&inbox.stdout);
    assert!(
        inbox_stdout.contains("\"hi\""),
        "inbox should contain message body 'hi': {inbox_stdout}"
    );
    assert!(
        inbox_stdout.contains("next_offset"),
        "inbox output should include next_offset footer: {inbox_stdout}"
    );

    kill_and_wait(&mut alice);
    kill_and_wait(&mut bob);
}

/// CLI-03 — direct coverage of empty-inbox case. `inbox list` against
/// a freshly-registered identity emits ZERO envelope lines and one
/// `{"next_offset":0}` footer.
#[test]
fn test_inbox_list() {
    let bus = Bus::new();
    let mut alice = bus.famp_spawn(&["register", "alice"]);
    bus.wait_for_register("alice");

    let out = bus.famp_cmd(&["inbox", "list", "--as", "alice"]);
    assert!(
        out.status.success(),
        "inbox list failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("next_offset"),
        "empty inbox MUST still emit next_offset footer: {stdout}"
    );

    kill_and_wait(&mut alice);
}

/// CLI-05: `famp await --as bob` blocks until alice sends; on receipt
/// bob's await emits the typed envelope JSONL on stdout and exits 0.
#[test]
fn test_await_unblocks() {
    let bus = Bus::new();
    let mut alice = bus.famp_spawn(&["register", "alice"]);
    let mut bob = bus.famp_spawn(&["register", "bob"]);
    bus.wait_for_register("alice");
    bus.wait_for_register("bob");

    // Spawn bob's `famp await --as bob --timeout 10s` in background;
    // capture stdout for the envelope JSONL on AwaitOk.
    let bob_await = Command::cargo_bin("famp")
        .unwrap()
        .env("FAMP_BUS_SOCKET", bus.sock())
        .env("HOME", bus.tmp.path())
        .args(["await", "--as", "bob", "--timeout", "10s"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    // Give bob's await time to register its parked-await on the broker
    // BEFORE alice sends. If alice sends first the broker writes to
    // bob's mailbox and the await still works (per send_agent's
    // waiting-client lookup), but the test reads more cleanly when
    // await is parked first.
    std::thread::sleep(Duration::from_millis(500));

    let send = bus.famp_cmd(&["send", "--as", "alice", "--to", "bob", "--new-task", "ping"]);
    assert!(
        send.status.success(),
        "send failed: stderr={}",
        String::from_utf8_lossy(&send.stderr)
    );

    let await_out = bob_await.wait_with_output().unwrap();
    assert!(
        await_out.status.success(),
        "await failed: stderr={}",
        String::from_utf8_lossy(&await_out.stderr)
    );
    let stdout = String::from_utf8_lossy(&await_out.stdout);
    assert!(
        stdout.contains("\"ping\""),
        "await stdout should contain 'ping': {stdout}"
    );

    kill_and_wait(&mut alice);
    kill_and_wait(&mut bob);
}

/// CLI-08: `famp whoami --as alice` returns `{"active":"alice", ...}`.
/// Per D-10, the proxy connection's effective identity IS the bound
/// name, so the broker's `WhoamiOk` reply carries `active:
/// Some("alice")`.
#[test]
fn test_whoami() {
    let bus = Bus::new();
    let mut alice = bus.famp_spawn(&["register", "alice"]);
    bus.wait_for_register("alice");

    let out = bus.famp_cmd(&["whoami", "--as", "alice"]);
    assert!(
        out.status.success(),
        "whoami failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("\"active\""),
        "whoami stdout should include active key: {stdout}"
    );
    assert!(
        stdout.contains("\"alice\""),
        "whoami active value should be alice (D-10 proxy): {stdout}"
    );

    kill_and_wait(&mut alice);
}

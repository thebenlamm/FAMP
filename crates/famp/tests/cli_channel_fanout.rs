#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Phase 02 plan 02-12 — `famp` CLI channel fan-out integration test
//! (TEST-02 + CLI-06).
//!
//! Three live `famp register` holders (alice, bob, charlie) all `famp
//! join #planning`. alice then `famp send --channel #planning
//! --new-task broadcast`. Bob and charlie each receive the broadcast
//! exactly once via `famp await`.
//!
//! ## Why `await` (not `inbox list`) for the receivers
//!
//! Plan 02-12's stated test code asserts `famp inbox list --as bob`
//! reads channel posts. The Phase-1/Phase-2 broker (`fn inbox` in
//! `famp-bus/src/broker/handle.rs`) only drains the per-agent
//! `MailboxName::Agent(name)` mailbox; channel posts are written to
//! the shared `MailboxName::Channel(name)` mailbox and per-member
//! delivery is signalled to PARKED `await`s via
//! `waiting_client_for_name`, not by per-member fan-out into
//! `<member>.jsonl`. The faithful TEST-02 surface is therefore:
//!
//! 1. bob and charlie park `famp await --as <name> --timeout 10s` BEFORE
//!    alice sends, so the broker has parked-await entries to match
//!    against on `BusMessage::Send { to: Channel(...) }`.
//! 2. alice sends to the channel.
//! 3. each of bob's/charlie's awaits unblocks with the SAME envelope
//!    (proving fan-out: one send → N parked-await replies).
//!
//! This deviation from the plan-as-written is documented in
//! `02-12-SUMMARY.md` under "Deviations from Plan" with the broker
//! semantics rationale.

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;

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

    fn famp_spawn_silent(&self, args: &[&str]) -> Child {
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

    fn famp_spawn_capture(&self, args: &[&str]) -> Child {
        Command::cargo_bin("famp")
            .unwrap()
            .env("FAMP_BUS_SOCKET", self.sock())
            .env("HOME", self.tmp.path())
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    }

    /// Poll until `famp whoami --as <name>` exits 0, proving `name`'s
    /// canonical-holder Register handshake has completed.
    fn wait_for_register(&self, name: &str) {
        for _ in 0..50 {
            let out = self.famp_cmd(&["whoami", "--as", name]);
            if out.status.success() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("{name} did not register within 5s");
    }
}

fn kill_and_wait(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// TEST-02 + CLI-06: alice broadcasts to #planning; bob and charlie
/// (both joined) each receive the broadcast exactly once via parked
/// `famp await`s.
#[test]
fn test_channel_fanout() {
    let bus = Bus::new();

    let mut alice = bus.famp_spawn_silent(&["register", "alice"]);
    let mut bob = bus.famp_spawn_silent(&["register", "bob"]);
    let mut charlie = bus.famp_spawn_silent(&["register", "charlie"]);
    bus.wait_for_register("alice");
    bus.wait_for_register("bob");
    bus.wait_for_register("charlie");

    // CLI-06: all three join #planning. The plan accepts both
    // `planning` and `#planning`; the broker stores the normalized
    // `#planning` form and the JoinOk reply echoes it.
    for who in ["alice", "bob", "charlie"] {
        let out = bus.famp_cmd(&["join", "#planning", "--as", who]);
        assert!(
            out.status.success(),
            "{who} join failed: stderr={}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("\"#planning\""),
            "{who} join stdout missing #planning: {stdout}"
        );
    }

    // Park bob and charlie in `famp await --timeout 10s` so the broker
    // has waiters to match against alice's channel send. Capture
    // stdout so we can read the unblocked envelope.
    let bob_await = bus.famp_spawn_capture(&["await", "--as", "bob", "--timeout", "10s"]);
    let charlie_await = bus.famp_spawn_capture(&["await", "--as", "charlie", "--timeout", "10s"]);
    // Give the awaits time to register their parked-await entries on
    // the broker before alice sends.
    std::thread::sleep(Duration::from_millis(500));

    // alice sends to the channel.
    let send = bus.famp_cmd(&[
        "send",
        "--as",
        "alice",
        "--channel",
        "#planning",
        "--new-task",
        "broadcast",
    ]);
    assert!(
        send.status.success(),
        "channel send failed: stderr={}",
        String::from_utf8_lossy(&send.stderr)
    );

    // Both awaits should unblock with an envelope containing the
    // "broadcast" summary string. The audit_log envelope embeds the
    // mode-tagged payload under `body.details.summary`.
    let bob_out = bob_await.wait_with_output().unwrap();
    let charlie_out = charlie_await.wait_with_output().unwrap();
    assert!(
        bob_out.status.success(),
        "bob await failed: stderr={}",
        String::from_utf8_lossy(&bob_out.stderr)
    );
    assert!(
        charlie_out.status.success(),
        "charlie await failed: stderr={}",
        String::from_utf8_lossy(&charlie_out.stderr)
    );

    let bob_stdout = String::from_utf8_lossy(&bob_out.stdout);
    let charlie_stdout = String::from_utf8_lossy(&charlie_out.stdout);
    let bob_count = bob_stdout.matches("broadcast").count();
    let charlie_count = charlie_stdout.matches("broadcast").count();
    assert_eq!(
        bob_count, 1,
        "bob's await must receive 'broadcast' exactly once; got {bob_count} in {bob_stdout}"
    );
    assert_eq!(
        charlie_count, 1,
        "charlie's await must receive 'broadcast' exactly once; got {charlie_count} in {charlie_stdout}"
    );

    kill_and_wait(&mut alice);
    kill_and_wait(&mut bob);
    kill_and_wait(&mut charlie);
}

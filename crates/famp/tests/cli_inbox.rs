//! Phase 02 plan 02-05 — `famp inbox ack` GREEN integration test.
//!
//! Scope: exercise the *local* atomic cursor-write path. `inbox ack` is
//! intentionally a no-broker-round-trip command (RESEARCH §6 — the client
//! is authoritative on its per-session cursor), so this test does NOT
//! spawn a broker, register an identity, or open a UDS socket. It just
//! shells `famp inbox ack --offset N --as alice` against an isolated
//! `FAMP_BUS_SOCKET` path and asserts the resulting cursor file.
//!
//! Closes CLI-04 (atomic ack) + CLI-10 (atomic write — single-shot
//! crash-safety on this path; full kill-9 recovery proof is owned by
//! plan 02-11).

#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;

#[test]
fn test_inbox_ack_cursor() {
    let tmp = tempfile::TempDir::new().unwrap();
    let bus_dir = tmp.path();
    let sock = bus_dir.join("test-bus.sock");

    // Pre-create the mailbox tree as the broker would, with three
    // fake JSONL lines for alice. `inbox ack` does not actually read
    // this file; the lines are present only to make the fixture
    // realistic.
    let mailboxes = bus_dir.join("mailboxes");
    std::fs::create_dir_all(&mailboxes).unwrap();
    let mailbox = mailboxes.join("alice.jsonl");
    let body = b"{\"id\":\"01913000-0000-7000-8000-000000000001\"}\n\
                 {\"id\":\"01913000-0000-7000-8000-000000000002\"}\n\
                 {\"id\":\"01913000-0000-7000-8000-000000000003\"}\n";
    std::fs::write(&mailbox, body).unwrap();

    // Run `famp inbox ack --offset 99 --as alice`. Pure local — no
    // broker, no Hello, no BusMessage. Stdout MUST be the ack JSON.
    let out = Command::new(env!("CARGO_BIN_EXE_famp"))
        .args(["inbox", "ack", "--offset", "99", "--as", "alice"])
        .env("FAMP_BUS_SOCKET", &sock)
        .output()
        .expect("famp inbox ack");
    assert!(
        out.status.success(),
        "famp inbox ack failed: status={:?} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stderr),
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    let trimmed = stdout.trim_end();
    assert_eq!(
        trimmed, "{\"acked\":true,\"offset\":99}",
        "stdout shape mismatch: {trimmed:?}",
    );

    // Cursor file MUST exist with mode 0o600 and contain `99\n`.
    let cursor = mailboxes.join(".alice.cursor");
    assert!(cursor.exists(), "cursor file not created at {cursor:?}");
    let mode = std::fs::metadata(&cursor).unwrap().permissions().mode() & 0o777;
    assert_eq!(
        mode, 0o600,
        "cursor file mode: expected 0o600, got {mode:o}"
    );
    let cursor_body = std::fs::read_to_string(&cursor).unwrap();
    assert_eq!(cursor_body, "99\n", "cursor body: {cursor_body:?}");
}

/// Broker-driven fixture mirroring `cli_channel_fanout::Bus`. Spawned for
/// the Scope B integration test below.
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

/// Scope B (260619): `famp inbox list --as <identity>` must include
/// envelopes from channels the identity has joined, alongside its
/// agent-mailbox envelopes.
///
/// Before B: `BusMessage::Inbox` reads only `MailboxName::Agent(name)`.
/// Channel posts to `#planning` that alice joined are written to
/// `mailboxes/#planning.jsonl` but never surface in `inbox list --as alice`.
///
/// After B: the broker merges joined-channel mailboxes into the response,
/// so bob's channel post is present in alice's JSONL output.
#[test]
fn inbox_list_includes_joined_channel_posts() {
    let bus = Bus::new();

    let mut alice = bus.famp_spawn_silent(&["register", "alice"]);
    let mut bob = bus.famp_spawn_silent(&["register", "bob"]);
    bus.wait_for_register("alice");
    bus.wait_for_register("bob");

    for who in ["alice", "bob"] {
        let out = bus.famp_cmd(&["join", "#planning", "--as", who]);
        assert!(
            out.status.success(),
            "{who} join #planning failed: stderr={}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    // bob posts a channel envelope.
    let send = bus.famp_cmd(&[
        "send",
        "--as",
        "bob",
        "--channel",
        "#planning",
        "--new-task",
        "channel-list-marker",
    ]);
    assert!(
        send.status.success(),
        "bob channel send failed: stderr={}",
        String::from_utf8_lossy(&send.stderr)
    );

    // alice runs `inbox list` — should include bob's channel post.
    let list = bus.famp_cmd(&["inbox", "list", "--as", "alice"]);
    assert!(
        list.status.success(),
        "alice inbox list failed: stderr={}",
        String::from_utf8_lossy(&list.stderr)
    );
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(
        stdout.contains("channel-list-marker"),
        "alice's `inbox list` must include bob's channel post; got: {stdout}"
    );

    kill_and_wait(&mut alice);
    kill_and_wait(&mut bob);
}

/// Scope B (260619): `inbox list --as '#channel'` must NOT return the
/// misleading `NotRegistered` error. It should fail with a typed error
/// directing the user to `inbox list --as <member-identity>` or
/// `inspect messages --to '#channel'`.
#[test]
fn inbox_list_rejects_channel_name_with_typed_error() {
    let bus = Bus::new();

    // No registration; the call is meant to fail fast at identity resolution.
    let out = bus.famp_cmd(&["inbox", "list", "--as", "#planning"]);
    assert!(
        !out.status.success(),
        "inbox list --as '#planning' must fail; got stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("not registered"),
        "stderr should NOT use misleading 'not registered' wording; got: {stderr}"
    );
    assert!(
        stderr.contains("#planning") || stderr.contains("channel"),
        "stderr should reference the channel-name input; got: {stderr}"
    );
    assert!(
        stderr.contains("inspect messages") || stderr.contains("--as <"),
        "stderr should direct the user to the correct command; got: {stderr}"
    );
}

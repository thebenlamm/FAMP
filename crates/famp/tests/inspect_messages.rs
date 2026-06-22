//! Integration tests for `famp inspect messages`.
#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use assert_cmd::prelude::*;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[path = "common/child_guard.rs"]
mod child_guard;
use child_guard::ChildGuard;

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

    fn famp_cmd_in(&self, cwd: &Path, args: &[&str]) -> std::process::Output {
        Command::cargo_bin("famp")
            .unwrap()
            .env("FAMP_BUS_SOCKET", self.sock())
            .env("HOME", self.tmp.path())
            .current_dir(cwd)
            .args(args)
            .output()
            .unwrap()
    }

    fn famp_spawn_in(&self, cwd: &Path, args: &[&str]) -> ChildGuard {
        ChildGuard::new(
            Command::cargo_bin("famp")
                .unwrap()
                .env("FAMP_BUS_SOCKET", self.sock())
                .env("HOME", self.tmp.path())
                .current_dir(cwd)
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        )
    }

    fn famp_spawn_broker(&self) -> Child {
        let mut child = Command::cargo_bin("famp")
            .unwrap()
            .env("FAMP_BUS_SOCKET", self.sock())
            .env("HOME", self.tmp.path())
            .args(["broker"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        for _ in 0..50 {
            if let Ok(Some(status)) = child.try_wait() {
                panic!("broker exited before becoming ready: {status}");
            }
            if self.sock.exists() {
                return child;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("broker did not create socket within 5s");
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

fn cwd_from(bus: &Bus, sub: &str) -> PathBuf {
    let cwd = bus.tmp.path().join(sub);
    std::fs::create_dir_all(&cwd).unwrap();
    cwd
}

fn poll_for_message_count(
    bus: &Bus,
    recipient: &str,
    expected: usize,
    max_wait: Duration,
) -> usize {
    let start = Instant::now();
    loop {
        let out = bus.famp_cmd(&["inspect", "messages", "--to", recipient, "--json"]);
        let count = if out.status.success() {
            let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
            value["rows"].as_array().map_or(0, Vec::len)
        } else {
            0
        };
        if count >= expected || start.elapsed() >= max_wait {
            return count;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[test]
fn metadata_only_no_body() {
    let bus = Bus::new();
    let sender_cwd = cwd_from(&bus, "sender");
    let receiver_cwd = cwd_from(&bus, "receiver");
    let mut sender = bus.famp_spawn_in(&sender_cwd, &["register", "sender"]);
    let mut receiver = bus.famp_spawn_in(&receiver_cwd, &["register", "receiver"]);
    bus.wait_for_register("sender");
    bus.wait_for_register("receiver");

    let send = bus.famp_cmd_in(
        &sender_cwd,
        &[
            "send",
            "--as",
            "sender",
            "--to",
            "receiver",
            "--new-task",
            "t1",
            "--body",
            "secret-payload",
        ],
    );
    assert!(
        send.status.success(),
        "send failed: stderr={}",
        String::from_utf8_lossy(&send.stderr)
    );
    let _ = poll_for_message_count(&bus, "receiver", 1, Duration::from_secs(2));

    let out = bus.famp_cmd(&["inspect", "messages", "--to", "receiver"]);
    assert!(
        out.status.success(),
        "inspect messages failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("BODY_BYTES"), "missing BODY_BYTES header");
    assert!(
        stdout.contains("SHA256_PREFIX"),
        "missing SHA256_PREFIX header"
    );
    assert!(
        !stdout.contains("secret-payload"),
        "body content leaked into messages output: {stdout}"
    );

    kill_and_wait(sender.as_mut().unwrap());
    kill_and_wait(receiver.as_mut().unwrap());
}

/// Regression for beta UX bug (2026-06-19): `famp inspect messages
/// --to '#channel'` returned an empty list even when the channel
/// mailbox file `~/.famp/mailboxes/#channel.jsonl` contained
/// envelopes. Root cause: `read_message_snapshot` iterated registered
/// identities only and never enumerated channel mailbox files. Fix:
/// scan `mailboxes/#*.jsonl` from disk.
#[test]
fn channel_messages_are_visible_via_inspect() {
    let bus = Bus::new();
    let mut broker = bus.famp_spawn_broker();
    let sender_cwd = cwd_from(&bus, "sender");
    let receiver_cwd = cwd_from(&bus, "receiver");
    let mut sender = bus.famp_spawn_in(&sender_cwd, &["register", "sender"]);
    let mut receiver = bus.famp_spawn_in(&receiver_cwd, &["register", "receiver"]);
    bus.wait_for_register("sender");
    bus.wait_for_register("receiver");

    // Both join the channel so the broker has known members; sender
    // joins to mirror Ben's repro (a participant lead-posting), receiver
    // joins to validate the post-join "I'm a member but can't see msgs"
    // failure mode.
    for who in ["sender", "receiver"] {
        let out = bus.famp_cmd(&["join", "#brain-test", "--as", who]);
        assert!(
            out.status.success(),
            "{who} join failed: stderr={}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let send = bus.famp_cmd_in(
        &sender_cwd,
        &[
            "send",
            "--as",
            "sender",
            "--channel",
            "#brain-test",
            "--new-task",
            "channel-close",
            "--body",
            "yield",
        ],
    );
    assert!(
        send.status.success(),
        "channel send failed: stderr={}",
        String::from_utf8_lossy(&send.stderr)
    );

    // Ground-truth: the channel mailbox file is on disk before we
    // poll the inspector. Without the fix, the file is there but the
    // inspector returns []; with the fix, the inspector matches the
    // file's row count. bus_dir is the parent of bus.sock, so
    // mailboxes live at {tmp}/mailboxes/, not {tmp}/.famp/mailboxes/.
    let mailbox_path = bus.tmp.path().join("mailboxes").join("#brain-test.jsonl");
    let start = Instant::now();
    while !mailbox_path.exists() && start.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        mailbox_path.exists(),
        "channel mailbox file never written: {mailbox_path:?}"
    );

    let observed = poll_for_message_count(&bus, "#brain-test", 1, Duration::from_secs(3));
    assert_eq!(
        observed, 1,
        "channel envelope present on disk but invisible to inspect messages --to '#brain-test' \
         (regression: read_message_snapshot must scan #*.jsonl from disk)"
    );

    kill_and_wait(sender.as_mut().unwrap());
    kill_and_wait(receiver.as_mut().unwrap());
    kill_and_wait(&mut broker);
}

#[test]
fn tail_default_is_50() {
    let bus = Bus::new();
    let mut broker = bus.famp_spawn_broker();
    let out = bus.famp_cmd(&["inspect", "messages", "--to", "nobody", "--json"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["kind"], "list");
    assert!(value["rows"].is_array());
    kill_and_wait(&mut broker);
}

#[test]
fn tail_3_returns_only_three_rows() {
    let bus = Bus::new();
    // Explicitly start the broker before register calls so that
    // `spawn_broker_if_absent` inside `famp register` is a no-op.
    // This ensures no orphan broker process survives after the test.
    let mut broker = bus.famp_spawn_broker();
    let sender_cwd = cwd_from(&bus, "sender");
    let receiver_cwd = cwd_from(&bus, "receiver");
    let mut sender = bus.famp_spawn_in(&sender_cwd, &["register", "sender"]);
    let mut receiver = bus.famp_spawn_in(&receiver_cwd, &["register", "receiver"]);
    bus.wait_for_register("sender");
    bus.wait_for_register("receiver");

    for i in 0..5 {
        let body = format!("msg-{i}");
        let task = format!("t{i}");
        let send = bus.famp_cmd_in(
            &sender_cwd,
            &[
                "send",
                "--as",
                "sender",
                "--to",
                "receiver",
                "--new-task",
                &task,
                "--body",
                &body,
            ],
        );
        assert!(send.status.success());
    }

    let observed = poll_for_message_count(&bus, "receiver", 5, Duration::from_secs(3));
    assert_eq!(observed, 5, "expected all 5 sends to land; got {observed}");

    let out = bus.famp_cmd(&[
        "inspect", "messages", "--to", "receiver", "--tail", "3", "--json",
    ]);
    assert!(out.status.success());
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let rows = value["rows"].as_array().expect("rows is array");
    assert_eq!(
        rows.len(),
        3,
        "expected exactly 3 rows (tail=3 against 5 sends); got {} -- output: {value}",
        rows.len()
    );

    kill_and_wait(sender.as_mut().unwrap());
    kill_and_wait(receiver.as_mut().unwrap());
    kill_and_wait(&mut broker);
}

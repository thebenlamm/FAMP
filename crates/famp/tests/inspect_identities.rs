//! Integration tests for `famp inspect identities`.
#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use assert_cmd::prelude::*;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

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

    fn famp_spawn_in(&self, cwd: &Path, args: &[&str]) -> Child {
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

fn data_lines(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect()
}

#[test]
fn inspect_identities_help_lists_json_flag() {
    let out = Command::cargo_bin("famp")
        .unwrap()
        .args(["inspect", "identities", "--help"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("--json"),
        "stdout did not contain --json: {stdout}"
    );
}

#[test]
fn inspect_identities_two_registered_renders_two_data_rows() {
    let bus = Bus::new();
    let cwd_a = bus.tmp.path().join("cwd_a");
    let cwd_b = bus.tmp.path().join("cwd_b");
    std::fs::create_dir_all(&cwd_a).unwrap();
    std::fs::create_dir_all(&cwd_b).unwrap();
    let mut alice = bus.famp_spawn_in(&cwd_a, &["register", "alice", "--tail"]);
    let mut bob = bus.famp_spawn_in(&cwd_b, &["register", "bob"]);
    bus.wait_for_register("alice");
    bus.wait_for_register("bob");

    let out = bus.famp_cmd(&["inspect", "identities"]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines = data_lines(&stdout);
    assert!(lines[0].contains("NAME"), "missing NAME header: {stdout}");
    assert!(
        lines[0].contains("LISTEN"),
        "missing LISTEN header: {stdout}"
    );
    assert!(lines[0].contains("CWD"), "missing CWD header: {stdout}");
    assert!(
        lines[0].contains("REGISTERED"),
        "missing REGISTERED header: {stdout}"
    );
    assert!(
        lines[0].contains("UNREAD"),
        "missing UNREAD header: {stdout}"
    );
    assert!(lines[0].contains("TOTAL"), "missing TOTAL header: {stdout}");
    assert!(
        lines[0].contains("LAST_SENDER"),
        "missing LAST_SENDER header: {stdout}"
    );
    assert!(
        lines[0].contains("LAST_RECEIVED"),
        "missing LAST_RECEIVED header: {stdout}"
    );
    assert_eq!(lines.len(), 3, "expected header + 2 rows: {stdout}");
    assert!(stdout.contains("alice"), "missing alice row: {stdout}");
    assert!(stdout.contains("bob"), "missing bob row: {stdout}");
    assert!(stdout.contains("true"), "missing listen=true row: {stdout}");
    assert!(
        stdout.contains("false"),
        "missing listen=false row: {stdout}"
    );
    assert!(
        stdout.contains(&cwd_a.display().to_string()),
        "missing alice cwd: {stdout}"
    );
    assert!(
        stdout.contains(&cwd_b.display().to_string()),
        "missing bob cwd: {stdout}"
    );

    kill_and_wait(&mut alice);
    kill_and_wait(&mut bob);
}

#[test]
fn inspect_identities_dead_broker_fast_fail() {
    let bus = Bus::new();
    let start = Instant::now();
    let out = bus.famp_cmd(&["inspect", "identities"]);
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(1),
        "dead broker check took {elapsed:?}"
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty(), "stdout should be empty");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("error: broker not running at"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn inspect_identities_json_emits_documented_schema() {
    let bus = Bus::new();
    let cwd = bus.tmp.path().join("json_cwd");
    std::fs::create_dir_all(&cwd).unwrap();
    let mut alice = bus.famp_spawn_in(&cwd, &["register", "alice"]);
    bus.wait_for_register("alice");

    let out = bus.famp_cmd(&["inspect", "identities", "--json"]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let rows = value["rows"].as_array().expect("rows must be an array");
    assert_eq!(rows.len(), 1, "{value}");
    let row = rows[0].as_object().unwrap();
    for key in [
        "name",
        "listen_mode",
        "cwd",
        "registered_at_unix_seconds",
        "last_activity_unix_seconds",
        "mailbox_unread",
        "mailbox_total",
        "last_sender",
        "last_received_at_unix_seconds",
    ] {
        assert!(row.contains_key(key), "missing key {key}: {value}");
    }
    for key in row.keys() {
        let lower = key.to_lowercase();
        assert!(!lower.contains("surfaced"), "forbidden key {key}");
        assert!(!lower.contains("double"), "forbidden key {key}");
        assert!(
            key == "last_received_at_unix_seconds" || !lower.contains("received"),
            "forbidden received-like key {key}"
        );
    }
    let registered = row["registered_at_unix_seconds"].as_u64().unwrap();
    let initial_activity = row["last_activity_unix_seconds"].as_u64().unwrap();
    assert!(
        initial_activity >= registered,
        "initial activity should be at or after registration: {value}"
    );

    std::thread::sleep(Duration::from_millis(1100));
    let whoami = bus.famp_cmd(&["whoami", "--as", "alice"]);
    assert!(
        whoami.status.success(),
        "whoami failed: stderr={}",
        String::from_utf8_lossy(&whoami.stderr)
    );
    let out = bus.famp_cmd(&["inspect", "identities", "--json"]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let row = value["rows"].as_array().unwrap()[0].as_object().unwrap();
    let later_activity = row["last_activity_unix_seconds"].as_u64().unwrap();
    assert!(
        later_activity > registered,
        "activity should advance after authenticated operation: {value}"
    );

    kill_and_wait(&mut alice);
}

#[test]
fn inspect_identities_no_debug_format_in_default_output() {
    let bus = Bus::new();
    let cwd = bus.tmp.path().join("plain_cwd");
    std::fs::create_dir_all(&cwd).unwrap();
    let mut alice = bus.famp_spawn_in(&cwd, &["register", "alice"]);
    bus.wait_for_register("alice");

    let out = bus.famp_cmd(&["inspect", "identities"]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains('{'), "debug map marker found: {stdout}");
    assert!(!stdout.contains('}'), "debug map marker found: {stdout}");
    assert!(!stdout.contains('['), "debug vec marker found: {stdout}");
    assert!(!stdout.contains(']'), "debug vec marker found: {stdout}");

    kill_and_wait(&mut alice);
}

#[test]
fn inspect_identities_mailbox_metadata_unread_total() {
    let bus = Bus::new();
    let recv_cwd = bus.tmp.path().join("recv_cwd");
    let send_cwd = bus.tmp.path().join("send_cwd");
    std::fs::create_dir_all(&recv_cwd).unwrap();
    std::fs::create_dir_all(&send_cwd).unwrap();
    let mut receiver = bus.famp_spawn_in(&recv_cwd, &["register", "receiver"]);
    let mut sender = bus.famp_spawn_in(&send_cwd, &["register", "sender"]);
    bus.wait_for_register("receiver");
    bus.wait_for_register("sender");

    for body in ["one", "two"] {
        let out = bus.famp_cmd_in(
            &send_cwd,
            &[
                "send",
                "--as",
                "sender",
                "--to",
                "receiver",
                "--new-task",
                body,
                "--body",
                body,
            ],
        );
        assert!(
            out.status.success(),
            "send failed: stderr={}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let out = bus.famp_cmd(&["inspect", "identities"]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let receiver_row = data_lines(&stdout)
        .into_iter()
        .find(|line| line.contains("receiver"))
        .unwrap_or_else(|| panic!("missing receiver row: {stdout}"));
    let cells: Vec<&str> = receiver_row.split_whitespace().collect();
    assert!(
        cells.contains(&"2"),
        "receiver row should contain unread/total counts: {receiver_row}"
    );
    assert!(
        receiver_row.contains("sender"),
        "receiver row should contain LAST_SENDER=sender: {receiver_row}"
    );
    assert!(
        !receiver_row.ends_with(" -"),
        "receiver row should contain LAST_RECEIVED timestamp: {receiver_row}"
    );

    let out = bus.famp_cmd(&["inspect", "identities", "--json"]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let rows = value["rows"].as_array().unwrap();
    let receiver_json = rows
        .iter()
        .find(|row| row["name"] == "receiver")
        .unwrap_or_else(|| panic!("missing receiver row: {value}"));
    assert!(
        receiver_json["last_received_at_unix_seconds"]
            .as_u64()
            .is_some(),
        "last_received_at_unix_seconds should be populated after messages: {value}"
    );

    kill_and_wait(&mut receiver);
    kill_and_wait(&mut sender);
}

//! Integration tests for `famp inspect tasks`.
#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use assert_cmd::prelude::*;
use std::path::{Path, PathBuf};
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

fn data_lines(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect()
}

fn cwd_from(bus: &Bus, sub: &str) -> PathBuf {
    let cwd = bus.tmp.path().join(sub);
    std::fs::create_dir_all(&cwd).unwrap();
    cwd
}

fn poll_for_task_id(bus: &Bus, max_wait: Duration) -> String {
    let start = Instant::now();
    loop {
        let out = bus.famp_cmd(&["inspect", "tasks", "--json"]);
        if out.status.success() {
            let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
            if let Some(task_id) = value["rows"]
                .as_array()
                .and_then(|rows| rows.first())
                .and_then(|row| row["task_id"].as_str())
                .filter(|task_id| !task_id.is_empty())
            {
                return task_id.to_string();
            }
        }
        assert!(
            start.elapsed() < max_wait,
            "poll_for_task_id timed out after {max_wait:?}; taskdir never settled with >=1 row"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[test]
fn list_groups_by_task_id_with_state_and_envelope_count() {
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
            "hello",
        ],
    );
    assert!(
        send.status.success(),
        "send failed: stderr={}",
        String::from_utf8_lossy(&send.stderr)
    );
    let _task_id = poll_for_task_id(&bus, Duration::from_secs(2));

    let out = bus.famp_cmd(&["inspect", "tasks"]);
    assert!(
        out.status.success(),
        "inspect tasks failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("TASK_ID"),
        "missing TASK_ID header: {stdout}"
    );
    assert!(stdout.contains("STATE"), "missing STATE header: {stdout}");
    assert!(
        stdout.contains("ENVELOPES"),
        "missing ENVELOPES header: {stdout}"
    );
    assert!(
        stdout.contains("LAST_TRANSITION_AGE"),
        "missing transition-age column: {stdout}"
    );
    assert!(stdout.contains("ORPHAN"), "missing ORPHAN column: {stdout}");
    assert!(
        data_lines(&stdout).len() >= 2,
        "expected header plus at least one data row: {stdout}"
    );

    kill_and_wait(&mut sender);
    kill_and_wait(&mut receiver);
}

#[test]
fn json_emits_kind_list_for_no_id_request() {
    let bus = Bus::new();
    let mut broker = bus.famp_spawn_broker();
    let out = bus.famp_cmd(&["inspect", "tasks", "--json"]);
    assert!(
        out.status.success(),
        "inspect tasks --json failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("valid JSON in --json output");
    assert_eq!(value["kind"], "list");
    assert!(value["rows"].is_array(), "rows must be array: {stdout}");
    kill_and_wait(&mut broker);
}

#[test]
fn id_full_jcs_pipes_through_jq() {
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
            "hi",
        ],
    );
    assert!(send.status.success());
    let task_id = poll_for_task_id(&bus, Duration::from_secs(2));

    let out = bus.famp_cmd(&["inspect", "tasks", "--id", &task_id, "--full"]);
    assert!(
        out.status.success(),
        "inspect tasks --id <uuid> --full failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("--full output must be valid JSON for jq piping");
    assert_eq!(value["task_id"], task_id);
    assert!(value["envelopes"].is_array(), "envelopes must be array");

    kill_and_wait(&mut sender);
    kill_and_wait(&mut receiver);
}

#[test]
fn broker_not_running_exits_one_with_stderr_message() {
    let tmp = tempfile::tempdir().unwrap();
    let bogus_sock = tmp.path().join("nope.sock");
    let out = Command::cargo_bin("famp")
        .unwrap()
        .env("FAMP_BUS_SOCKET", &bogus_sock)
        .env("HOME", tmp.path())
        .args(["inspect", "tasks"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("error: broker not running at"),
        "expected broker-down message in stderr: {stderr}"
    );
}

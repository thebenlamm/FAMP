//! Integration tests for `famp inspect broker`.
#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use assert_cmd::prelude::*;
use regex::Regex;
use std::io::Write as _;
use std::os::unix::net::UnixListener as StdUnixListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

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
        self.wait_for_broker(&mut child);
        child
    }

    fn wait_for_broker(&self, child: &mut Child) {
        for _ in 0..50 {
            if let Ok(Some(status)) = child.try_wait() {
                panic!("broker exited before becoming ready: {status}");
            }
            if self.sock.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        panic!("broker did not create socket within 5s");
    }
}

fn kill_and_wait(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn inspect_broker_help_lists_json_flag() {
    let out = Command::cargo_bin("famp")
        .unwrap()
        .args(["inspect", "broker", "--help"])
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
fn inspect_help_lists_all_inspect_subcommands() {
    let out = Command::cargo_bin("famp")
        .unwrap()
        .args(["inspect", "--help"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("broker"),
        "missing broker subcommand: {stdout}"
    );
    assert!(
        stdout.contains("identities"),
        "missing identities subcommand: {stdout}"
    );
    assert!(
        stdout.contains("tasks"),
        "missing tasks subcommand: {stdout}"
    );
    assert!(
        stdout.contains("messages"),
        "missing messages subcommand: {stdout}"
    );
}

#[test]
fn inspect_broker_healthy_exit_0() {
    let bus = Bus::new();
    let mut broker = bus.famp_spawn_broker();

    let out = bus.famp_cmd(&["inspect", "broker"]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let re = Regex::new(r"^state: HEALTHY pid=\d+ socket=.+ started_at=.+ build=.+\n?$").unwrap();
    assert!(re.is_match(&stdout), "unexpected stdout: {stdout}");
    assert!(
        out.stderr.is_empty(),
        "stderr should be empty: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    kill_and_wait(&mut broker);
}

#[test]
fn inspect_broker_down_clean_exit_1() {
    let bus = Bus::new();
    let out = bus.famp_cmd(&["inspect", "broker"]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.starts_with("state: DOWN_CLEAN "),
        "unexpected stdout: {stdout}"
    );
}

#[test]
fn inspect_broker_stale_socket_exit_1() {
    let bus = Bus::new();
    let stale = StdUnixListener::bind(bus.sock()).unwrap();
    drop(stale);

    let out = bus.famp_cmd(&["inspect", "broker"]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.starts_with("state: STALE_SOCKET "),
        "unexpected stdout: {stdout}"
    );
}

#[test]
fn inspect_broker_orphan_holder_exit_1() {
    let bus = Bus::new();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);
    let listener = StdUnixListener::bind(bus.sock()).unwrap();
    listener.set_nonblocking(true).unwrap();
    let handle = std::thread::spawn(move || {
        while !stop_thread.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.write_all(b"garbage");
                    let _ = stream.flush();
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    let out = bus.famp_cmd(&["inspect", "broker"]);
    stop.store(true, Ordering::SeqCst);
    let _ = std::os::unix::net::UnixStream::connect(bus.sock());
    handle.join().unwrap();

    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.starts_with("state: ORPHAN_HOLDER "),
        "unexpected stdout: {stdout}"
    );
    assert!(
        stdout.contains("holder_pid="),
        "missing holder_pid: {stdout}"
    );
    assert!(
        stdout.contains("pid_source="),
        "missing pid_source: {stdout}"
    );
}

#[test]
fn inspect_broker_json_healthy_emits_documented_schema() {
    let bus = Bus::new();
    let mut broker = bus.famp_spawn_broker();

    let out = bus.famp_cmd(&["inspect", "broker", "--json"]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["state"], "HEALTHY");
    assert!(value["pid"].as_u64().is_some(), "{value}");
    assert!(value["socket_path"].as_str().is_some(), "{value}");
    assert!(
        value["started_at_unix_seconds"].as_u64().is_some(),
        "{value}"
    );
    assert!(value["build_version"].as_str().is_some(), "{value}");

    kill_and_wait(&mut broker);
}

#[test]
fn inspect_broker_diagnosis_on_stdout_not_stderr() {
    let bus = Bus::new();
    let out = bus.famp_cmd(&["inspect", "broker"]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("state: DOWN_CLEAN"),
        "diagnosis should be on stdout: {stdout}"
    );
    assert!(
        out.stderr.is_empty(),
        "stderr should be empty: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

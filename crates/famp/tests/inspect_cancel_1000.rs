//! INSP-RPC-04 integration test for cancellation pressure on inspect calls.
#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use assert_cmd::prelude::*;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
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

fn count_broker_fds(broker_pid: u32) -> usize {
    let proc_fd = PathBuf::from(format!("/proc/{broker_pid}/fd"));
    if proc_fd.is_dir() {
        return std::fs::read_dir(&proc_fd)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", proc_fd.display()))
            .count();
    }

    let out = Command::new("lsof")
        .args(["-p", &broker_pid.to_string()])
        .output()
        .expect("lsof must be available on Unix platforms without /proc/<pid>/fd");
    assert!(
        out.status.success(),
        "lsof failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).lines().skip(1).count()
}

#[test]
#[allow(clippy::cast_possible_truncation)]
fn one_thousand_cancel_no_leak() {
    let bus = Bus::new();
    let mut broker = bus.famp_spawn_broker();
    let broker_json = bus.famp_cmd(&["inspect", "broker", "--json"]);
    assert!(broker_json.status.success(), "broker not healthy");
    let value: serde_json::Value = serde_json::from_slice(&broker_json.stdout).unwrap();
    let broker_pid = value["pid"].as_u64().expect("broker pid present") as u32;
    let baseline_fds = count_broker_fds(broker_pid);

    let completed = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(1000);
    for i in 0..1000usize {
        let sock = bus.sock().to_path_buf();
        let home = bus.tmp.path().to_path_buf();
        let completed = Arc::clone(&completed);
        handles.push(std::thread::spawn(move || {
            let mut child = Command::cargo_bin("famp")
                .unwrap()
                .env("FAMP_BUS_SOCKET", &sock)
                .env("HOME", &home)
                .args(["inspect", "tasks"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("spawn famp");
            if i % 2 == 0 {
                std::thread::sleep(Duration::from_millis(5));
                let _ = child.kill();
            }
            let _ = child.wait();
            completed.fetch_add(1, Ordering::SeqCst);
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
    std::thread::sleep(Duration::from_secs(2));

    let after_fds = count_broker_fds(broker_pid);
    let delta = after_fds.saturating_sub(baseline_fds);
    assert!(
        delta < 10,
        "FD leak detected: baseline={baseline_fds}, after={after_fds}, delta={delta}"
    );
    assert_eq!(completed.load(Ordering::SeqCst), 1000);
    kill_and_wait(&mut broker);
}

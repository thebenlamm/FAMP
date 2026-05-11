//! INSP-RPC-05 integration test: famp.inspect.* RPC pressure must not starve
//! concurrent bus message throughput. Validated against Phase 2's spawn_blocking
//! + 500ms timeout inspect dispatch (Out::InspectRequest), which is designed to
//! be starvation-resistant by construction; this test commits the public 80%
//! threshold (STARVATION_THRESHOLD = 0.80, i.e. tolerate up to 20% degradation).
#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use assert_cmd::prelude::*;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
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

/// Per-phase measurement window. Baseline + loaded phases each run for WINDOW.
const WINDOW: Duration = Duration::from_secs(5);
/// Concurrent `famp send --as sender` worker threads driving bus traffic.
const SENDER_THREADS: usize = 4;
/// Concurrent `famp inspect tasks` worker threads applying inspect-side load.
const INSPECTOR_THREADS: usize = 8;
/// Minimum acceptable ratio of loaded-throughput / baseline-throughput.
/// 0.80 = inspect calls may reduce bus throughput by at most 20%.
const STARVATION_THRESHOLD: f64 = 0.80;

/// Run SENDER_THREADS workers each looping `famp send --as sender --to receiver`
/// for the duration of WINDOW, while INSPECTOR_THREADS workers concurrently loop
/// `famp inspect tasks`. Returns the count of successful `famp send` invocations
/// observed during WINDOW.
///
/// `sender_cwd` is passed so the send subprocesses inherit the same cwd as the
/// `famp register sender` long-lived process (mirrors inspect_tasks.rs line 144).
fn measure_send_throughput(bus: &Bus, sender_cwd: &Path, inspector_threads: usize) -> u64 {
    let delivered = Arc::new(AtomicU64::new(0));
    let deadline = Instant::now() + WINDOW;
    let mut handles = Vec::with_capacity(SENDER_THREADS + inspector_threads);

    // Sender workers: drive `famp send --as sender --to receiver --new-task <uniq> --body <uniq>`.
    for worker_id in 0..SENDER_THREADS {
        let sock = bus.sock().to_path_buf();
        let home = bus.tmp.path().to_path_buf();
        let cwd = sender_cwd.to_path_buf();
        let delivered = Arc::clone(&delivered);
        handles.push(std::thread::spawn(move || {
            let mut iter: u64 = 0;
            while Instant::now() < deadline {
                let task = format!("t-{worker_id}-{iter}");
                let body = format!("load-{worker_id}-{iter}");
                let out = Command::cargo_bin("famp")
                    .unwrap()
                    .env("FAMP_BUS_SOCKET", &sock)
                    .env("HOME", &home)
                    .current_dir(&cwd)
                    .args([
                        "send",
                        "--as",
                        "sender",
                        "--to",
                        "receiver",
                        "--new-task",
                        &task,
                        "--body",
                        &body,
                    ])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .output()
                    .expect("spawn famp send");
                if out.status.success() {
                    delivered.fetch_add(1, Ordering::SeqCst);
                }
                iter += 1;
            }
        }));
    }

    // Inspector workers: drive `famp inspect tasks` to saturate the inspect path.
    for _ in 0..inspector_threads {
        let sock = bus.sock().to_path_buf();
        let home = bus.tmp.path().to_path_buf();
        handles.push(std::thread::spawn(move || {
            while Instant::now() < deadline {
                let _ = Command::cargo_bin("famp")
                    .unwrap()
                    .env("FAMP_BUS_SOCKET", &sock)
                    .env("HOME", &home)
                    .args(["inspect", "tasks"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .output();
                std::thread::sleep(Duration::from_millis(1500));
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
    delivered.load(Ordering::SeqCst)
}

fn measure_scenario(inspector_threads: usize) -> u64 {
    let bus = Bus::new();
    let mut broker = bus.famp_spawn_broker();

    let sender_cwd = cwd_from(&bus, "sender");
    let receiver_cwd = cwd_from(&bus, "receiver");
    let mut sender = bus.famp_spawn_in(&sender_cwd, &["register", "sender"]);
    let mut receiver = bus.famp_spawn_in(&receiver_cwd, &["register", "receiver"]);
    bus.wait_for_register("sender");
    bus.wait_for_register("receiver");

    let delivered = measure_send_throughput(&bus, &sender_cwd, inspector_threads);

    kill_and_wait(&mut sender);
    kill_and_wait(&mut receiver);
    kill_and_wait(&mut broker);

    delivered
}

#[test]
fn inspect_load_does_not_starve_bus_messages() {
    // Phase A: baseline throughput (no concurrent inspect pressure).
    let baseline = measure_scenario(0);
    assert!(
        baseline > 0,
        "baseline throughput must be non-zero; got {baseline}"
    );

    // Phase B: throughput under saturating inspect-call load.
    let loaded = measure_scenario(INSPECTOR_THREADS);

    let ratio = loaded as f64 / baseline as f64;
    println!("inspect_load_test baseline={baseline} loaded={loaded} ratio={ratio:.2}");
    assert!(
        ratio >= STARVATION_THRESHOLD,
        "Bus message throughput under inspect load ({loaded}) degraded below \
         {:.0}% of baseline ({baseline}): ratio={ratio:.2}",
        STARVATION_THRESHOLD * 100.0
    );
}

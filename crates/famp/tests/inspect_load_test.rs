//! INSP-RPC-05 integration test: famp.inspect.* RPC pressure must not starve
//! concurrent bus message throughput.
//!
//! GAP-03-01 closure: this test applies SATURATED DIRECT inspect RPC pressure
//! via `famp_inspect_client::connect_and_call(InspectKind::Tasks(_))` from
//! a tight per-thread loop (no per-call sleep), NOT paced `famp inspect`
//! subprocess invocations. The direct-RPC variant exercises Phase 03 Plan 03's
//! non-blocking bounded inspect dispatch (`Out::InspectRequest` now runs in a
//! `tokio::spawn`'d task that owns a bounded
//! `MAX_CONCURRENT_INSPECT_REQUESTS` semaphore permit; permit-exhausted
//! requests are shed immediately with the existing budget_exceeded payload —
//! see `crates/famp/src/cli/broker/mod.rs`). The public threshold
//! `STARVATION_THRESHOLD = 0.80` is committed: bus send throughput under
//! saturated inspect RPC pressure must remain at >= 80% of the inspector-free
//! baseline.
#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use assert_cmd::prelude::*;
use famp_inspect_client::connect_and_call;
use famp_inspect_proto::{InspectKind, InspectTasksRequest};
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
///
/// 8 seconds, up from the pre-GAP-03-01 5s window: under SATURATED DIRECT
/// inspect RPC pressure the inspector and sender workers contend with the
/// broker for CPU/IO; a longer window amortizes per-second variance so the
/// observed loaded/baseline ratio is dominated by steady-state behavior
/// rather than transient scheduling noise near the 0.80 threshold.
const WINDOW: Duration = Duration::from_secs(8);
/// Concurrent `famp send --as sender` worker threads driving bus traffic.
const SENDER_THREADS: usize = 4;
/// Concurrent direct `InspectKind::Tasks` RPC worker threads applying
/// saturated inspect pressure (GAP-03-01). Each worker runs a current-thread
/// tokio runtime and tight-loops `famp_inspect_client::connect_and_call`.
const INSPECTOR_THREADS: usize = 8;
/// Minimum acceptable ratio of loaded-throughput / baseline-throughput.
/// 0.80 = inspect calls may reduce bus throughput by at most 20%. Locked
/// public commitment — do not relax (see VERIFICATION.md GAP-03-01 and
/// the testing note "STARVATION_THRESHOLD = 0.80 for INSP-RPC-05").
const STARVATION_THRESHOLD: f64 = 0.80;

/// Run SENDER_THREADS workers each looping `famp send --as sender --to receiver`
/// for the duration of WINDOW, while INSPECTOR_THREADS workers concurrently
/// drive saturated direct `InspectKind::Tasks` RPC calls (no pacing). Returns
/// the count of successful `famp send` invocations observed during WINDOW.
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

    // Inspector workers: GAP-03-01 — drive saturated direct
    // `InspectKind::Tasks` RPC pressure via `famp_inspect_client::
    // connect_and_call`. NO per-call sleep: each worker tight-loops
    // a new UDS connection + Hello + Inspect frame for the duration
    // of WINDOW so the broker's non-blocking inspect dispatch path is
    // exercised at saturating rate.
    for _ in 0..inspector_threads {
        let sock = bus.sock().to_path_buf();
        handles.push(std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .enable_time()
                .build()
                .expect("build current-thread tokio runtime");
            rt.block_on(async move {
                while Instant::now() < deadline {
                    // Drop the result; we measure SEND throughput, not
                    // inspect outcome. Each call independently
                    // connects, performs Hello, sends the Inspect
                    // frame, and decodes the reply.
                    let _ =
                        connect_and_call(&sock, InspectKind::Tasks(InspectTasksRequest::default()))
                            .await;
                }
            });
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

    // Phase B: throughput under SATURATED DIRECT inspect RPC pressure.
    // INSPECTOR_THREADS workers tight-loop
    // `famp_inspect_client::connect_and_call(InspectKind::Tasks(_))` with
    // no pacing for the full WINDOW. This is the stronger no-starvation
    // property required by INSP-RPC-05 / GAP-03-01.
    let loaded = measure_scenario(INSPECTOR_THREADS);

    let ratio = loaded as f64 / baseline as f64;
    println!("inspect_load_test baseline={baseline} loaded={loaded} ratio={ratio:.2}");
    assert!(
        ratio >= STARVATION_THRESHOLD,
        "Bus message throughput under saturated direct inspect RPC load ({loaded}) \
         degraded below {:.0}% of baseline ({baseline}): ratio={ratio:.2}",
        STARVATION_THRESHOLD * 100.0
    );
}

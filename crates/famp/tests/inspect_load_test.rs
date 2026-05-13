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
//! requests are shed immediately with the existing `budget_exceeded` payload —
//! see `crates/famp/src/cli/broker/mod.rs`). The public threshold
//! `STARVATION_THRESHOLD = 0.80` is committed: bus send throughput under
//! saturated inspect RPC pressure must remain at >= 80% of the inspector-free
//! baseline.
#![cfg(unix)]
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use assert_cmd::prelude::*;
use famp::bus_client::BusClient;
use famp_bus::{BusMessage, BusReply, Target};
use famp_inspect_client::connect_and_call;
use famp_inspect_proto::{InspectKind, InspectTasksRequest};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
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
/// Concurrent direct bus-send worker threads driving bus traffic.
const SENDER_THREADS: usize = 4;
/// Concurrent direct `InspectKind::Tasks` RPC worker threads applying
/// saturated inspect pressure (GAP-03-01). Each worker runs a current-thread
/// tokio runtime and tight-loops `famp_inspect_client::connect_and_call`.
const INSPECTOR_THREADS: usize = 8;
/// Minimum acceptable ratio of loaded-throughput / baseline-throughput.
/// 0.80 = inspect calls may reduce bus throughput by at most 20%. Locked
/// public commitment — do not relax (see VERIFICATION.md GAP-03-01 and
/// the testing note `STARVATION_THRESHOLD` = 0.80 for INSP-RPC-05).
const STARVATION_THRESHOLD: f64 = 0.80;

#[derive(Debug, Default)]
struct Measurement {
    delivered: u64,
    inspect_attempts: u64,
    inspect_ok: u64,
    inspect_budget_exceeded: u64,
    inspect_errors: u64,
}

/// Build a minimal valid `audit_log` envelope JSON value. This test
/// measures broker send throughput, not CLI envelope construction, so it
/// drives the same `BusMessage::Send` wire surface directly.
fn audit_log_envelope(worker_id: usize, iter: u64) -> serde_json::Value {
    let value = serde_json::json!({
        "famp": "0.5.2",
        "class": "audit_log",
        "scope": "standalone",
        "id": uuid::Uuid::now_v7().to_string(),
        "from": "agent:local.bus/sender",
        "to": "agent:local.bus/receiver",
        "authority": "advisory",
        "ts": "2026-05-10T18:00:00Z",
        "body": {
            "event": "inspect_load_test.send",
            "details": {
                "worker": worker_id,
                "iter": iter
            }
        }
    });
    let bytes = famp_canonical::canonicalize(&value).expect("audit_log envelope canonicalizes");
    famp_envelope::AnyBusEnvelope::decode(&bytes).expect("audit_log envelope decodes");
    value
}

fn current_thread_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .expect("build current-thread tokio runtime")
}

fn delivered_to_receiver(reply: Result<BusReply, famp::bus_client::BusClientError>) -> bool {
    let Ok(BusReply::SendOk {
        delivered: rows, ..
    }) = reply
    else {
        return false;
    };
    rows.iter().any(|row| {
        row.ok
            && matches!(
                &row.to,
                Target::Agent { name } if name == "receiver"
            )
    })
}

fn spawn_sender_worker(
    handles: &mut Vec<std::thread::JoinHandle<()>>,
    sock: PathBuf,
    start_gate: Arc<Barrier>,
    delivered: Arc<AtomicU64>,
    worker_id: usize,
) {
    handles.push(std::thread::spawn(move || {
        start_gate.wait();
        let deadline = Instant::now() + WINDOW;
        current_thread_runtime().block_on(async move {
            let mut iter: u64 = 0;
            while Instant::now() < deadline {
                if let Ok(mut bus) = BusClient::connect(&sock, Some("sender".to_string())).await {
                    let reply = bus
                        .send_recv(BusMessage::Send {
                            to: Target::Agent {
                                name: "receiver".to_string(),
                            },
                            envelope: audit_log_envelope(worker_id, iter),
                        })
                        .await;
                    if delivered_to_receiver(reply) {
                        delivered.fetch_add(1, Ordering::SeqCst);
                    }
                    bus.shutdown().await;
                }
                iter += 1;
            }
        });
    }));
}

fn spawn_inspector_worker(
    handles: &mut Vec<std::thread::JoinHandle<()>>,
    sock: PathBuf,
    start_gate: Arc<Barrier>,
    attempts: Arc<AtomicU64>,
    ok: Arc<AtomicU64>,
    budget_exceeded: Arc<AtomicU64>,
    errors: Arc<AtomicU64>,
) {
    handles.push(std::thread::spawn(move || {
        start_gate.wait();
        let deadline = Instant::now() + WINDOW;
        current_thread_runtime().block_on(async move {
            while Instant::now() < deadline {
                attempts.fetch_add(1, Ordering::SeqCst);
                match connect_and_call(&sock, InspectKind::Tasks(InspectTasksRequest::default()))
                    .await
                {
                    Ok(payload) => {
                        ok.fetch_add(1, Ordering::SeqCst);
                        if payload.get("kind").and_then(serde_json::Value::as_str)
                            == Some("budget_exceeded")
                        {
                            budget_exceeded.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    Err(_) => {
                        errors.fetch_add(1, Ordering::SeqCst);
                    }
                }
            }
        });
    }));
}

/// Run `SENDER_THREADS` workers each looping direct `BusMessage::Send`
/// for the duration of WINDOW, while `INSPECTOR_THREADS` workers concurrently
/// drive saturated direct `InspectKind::Tasks` RPC calls (no pacing). Returns
/// the count of successful sends observed during WINDOW.
///
/// This intentionally bypasses the `famp send` subprocess path. The property
/// under test is broker send throughput under inspect RPC pressure; including
/// thousands of CLI process launches made the ratio sensitive to CI process
/// scheduling instead of broker starvation.
fn measure_send_throughput(bus: &Bus, inspector_threads: usize) -> Measurement {
    let delivered = Arc::new(AtomicU64::new(0));
    let inspect_attempts = Arc::new(AtomicU64::new(0));
    let inspect_ok = Arc::new(AtomicU64::new(0));
    let inspect_budget_exceeded = Arc::new(AtomicU64::new(0));
    let inspect_errors = Arc::new(AtomicU64::new(0));
    let start_gate = Arc::new(Barrier::new(SENDER_THREADS + inspector_threads));
    let mut handles = Vec::with_capacity(SENDER_THREADS + inspector_threads);

    // Sender workers: direct one-shot bus RPC. Each measured send opens
    // its own proxy connection, sends once, and shuts down. This preserves
    // accept + Hello pressure without the unrelated `famp` subprocess
    // launch cost.
    for worker_id in 0..SENDER_THREADS {
        spawn_sender_worker(
            &mut handles,
            bus.sock().to_path_buf(),
            Arc::clone(&start_gate),
            Arc::clone(&delivered),
            worker_id,
        );
    }

    // Inspector workers: GAP-03-01 — drive saturated direct
    // `InspectKind::Tasks` RPC pressure via `famp_inspect_client::
    // connect_and_call`. NO per-call sleep: each worker tight-loops
    // a new UDS connection + Hello + Inspect frame for the duration
    // of WINDOW so the broker's non-blocking inspect dispatch path is
    // exercised at saturating rate.
    for _ in 0..inspector_threads {
        spawn_inspector_worker(
            &mut handles,
            bus.sock().to_path_buf(),
            Arc::clone(&start_gate),
            Arc::clone(&inspect_attempts),
            Arc::clone(&inspect_ok),
            Arc::clone(&inspect_budget_exceeded),
            Arc::clone(&inspect_errors),
        );
    }

    for h in handles {
        h.join().unwrap();
    }
    Measurement {
        delivered: delivered.load(Ordering::SeqCst),
        inspect_attempts: inspect_attempts.load(Ordering::SeqCst),
        inspect_ok: inspect_ok.load(Ordering::SeqCst),
        inspect_budget_exceeded: inspect_budget_exceeded.load(Ordering::SeqCst),
        inspect_errors: inspect_errors.load(Ordering::SeqCst),
    }
}

fn measure_scenario(inspector_threads: usize) -> Measurement {
    let bus = Bus::new();
    let mut broker = bus.famp_spawn_broker();

    let sender_cwd = cwd_from(&bus, "sender");
    let receiver_cwd = cwd_from(&bus, "receiver");
    let mut sender = bus.famp_spawn_in(&sender_cwd, &["register", "sender"]);
    let mut receiver = bus.famp_spawn_in(&receiver_cwd, &["register", "receiver"]);
    bus.wait_for_register("sender");
    bus.wait_for_register("receiver");

    let delivered = measure_send_throughput(&bus, inspector_threads);

    kill_and_wait(&mut sender);
    kill_and_wait(&mut receiver);
    kill_and_wait(&mut broker);

    delivered
}

#[test]
#[ignore = "CI runners too constrained; run locally with -- --ignored"]
#[allow(clippy::cast_precision_loss)]
fn inspect_load_does_not_starve_bus_messages() {
    // Collect SAMPLES independent (baseline, loaded) pairs and require every
    // ratio to satisfy the public threshold. Multiple samples make a timing
    // anomaly visible without weakening the INSP-RPC-05 guarantee.
    const SAMPLES: usize = 3;

    let mut ratios: Vec<f64> = Vec::with_capacity(SAMPLES);
    for i in 0..SAMPLES {
        let baseline = measure_scenario(0);
        assert!(
            baseline.delivered > 0,
            "baseline throughput must be non-zero on sample {i}; got {baseline:?}"
        );
        let loaded = measure_scenario(INSPECTOR_THREADS);
        assert!(
            loaded.inspect_attempts >= 100,
            "sample {i} did not apply enough inspect pressure: {loaded:?}"
        );
        assert!(
            loaded.inspect_ok > 0,
            "sample {i} had no successful inspect replies: {loaded:?}"
        );
        assert!(
            loaded.inspect_budget_exceeded > 0,
            "sample {i} did not saturate inspect budget: {loaded:?}"
        );
        let ratio = loaded.delivered as f64 / baseline.delivered as f64;
        println!(
            "inspect_load_test sample={i} baseline={} loaded={} ratio={ratio:.2} \
             inspect_attempts={} inspect_ok={} inspect_budget_exceeded={} inspect_errors={}",
            baseline.delivered,
            loaded.delivered,
            loaded.inspect_attempts,
            loaded.inspect_ok,
            loaded.inspect_budget_exceeded,
            loaded.inspect_errors
        );
        ratios.push(ratio);
    }

    for ratio in ratios {
        assert!(
            ratio >= STARVATION_THRESHOLD,
            "ratio {ratio:.2} below {:.0}% threshold — bus throughput under saturated inspect \
             RPC load degraded too far",
            STARVATION_THRESHOLD * 100.0
        );
    }
}

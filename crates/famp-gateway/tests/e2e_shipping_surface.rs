//! 11-04 Task 3 (D-09) — shipping-surface regression net.
//!
//! Replaces the throwaway `wire_proof_inject.rs`/`probe_tls.rs` scaffolding
//! (confirmed absent from `git ls-files` — never committed, nothing to
//! delete) with a real, additive, `just ci`-gated integration test that
//! drives the FIXED `famp send` CLI cross-host (plan 03's mode-branched
//! remote class) — never `send_bus_envelope`/the raw bus client, never a
//! hand-built injector.
//!
//! Reuses the two-host harness extracted in Task 2
//! (`common/gateway_harness.rs`) for broker/register/TOFU-trust/gateway
//! setup, then drives every leg through real `famp send` subprocesses:
//!
//! 1. **Happy path** — `famp send --to agent:<bob-domain>/bob --new-task`
//!    from A delivers a typed `request` into B's real `bob` mailbox with a
//!    domain-qualified `from`.
//! 2. **Full-cycle terminal** — request (A `--new-task`) -> commit (B
//!    `--task <id>`) -> deliver-terminal (B `--task <id> --terminal`), each
//!    via the shipping client, and BOTH sides converge on the same
//!    terminal FSM state via `poll_terminal_state` (mirrors
//!    `e2e_cross_host_delivery.rs:761-774`). Note: unlike that test's
//!    hand-built four-leg cycle (...+ack), the shipping `famp send` CLI
//!    (plan 03) only mode-branches three shapes — `--new-task` (request),
//!    `--task` (commit), `--task --terminal` (deliver+terminal_status) —
//!    there is no shipping "ack" mode. Per `famp-fsm::TaskFsm::step` and
//!    the 11-03/11-07 SUMMARYs, the terminal state is reached from the
//!    deliver envelope's own `terminal_status` header, not from a
//!    trailing ack/control envelope, so three legs is sufficient to prove
//!    UAT-01's terminal-FSM-reachable-via-shipping-surface claim.
//! 3. **Negative (observable)** — a `famp send --domain local.bus` remote
//!    send stamps `from = agent:local.bus/<identity>` (mismatched
//!    authority vs. the pinned `agent:<domain>/alice` label) while still
//!    targeting a real remote principal, so it takes the federated path.
//!    Gateway A's own-domain is UNSET in this harness by default (mirrors
//!    `e2e_cross_host_delivery.rs`'s `spawn_gateway`, which passes no
//!    gateway-side `--domain`), so plan 07's egress `FromDomainMismatch`
//!    check is skipped and the envelope reaches gateway B's ingress,
//!    which rejects it typed (`unpinned_key`, HTTP 403). Asserted by
//!    capturing gateway A's stderr (the drain-loop's `{e:?}` relay-failure
//!    line) for the `unpinned_key`/`403` substrings AND confirming
//!    non-delivery on B — never a cross-process Rust-type match.

#![allow(unused_crate_dependencies)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::similar_names
)]

use std::io::Read as _;
use std::net::SocketAddr;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;

#[path = "common/gateway_harness.rs"]
mod gateway_harness;
use gateway_harness::{
    cross_machine_fixture_dir, ensure_famp_bin_built, famp_cmd, peer_export, peer_import,
    pick_free_port, poll_inbox_contains, poll_terminal_state, spawn_broker, spawn_register,
    wait_for_broker_socket, wait_for_tcp, wait_until_live, ChildGuard, Side, ALICE, ALICE_DOMAIN,
    BOB, BOB_DOMAIN, POLL_DEADLINE, STARTUP_DEADLINE,
};

/// A shorter bounded window for confirming NON-delivery (D-09 negative
/// test). Generous enough to absorb one egress `Await` drain cycle
/// (~1s cadence) plus the HTTPS round-trip, short enough that the test
/// doesn't waste `just ci` time waiting out a full `POLL_DEADLINE` for an
/// event that structurally cannot arrive.
const NON_DELIVERY_WINDOW: Duration = Duration::from_secs(8);

/// Spawn `famp-gateway` exactly like `gateway_harness::spawn_gateway`, but
/// with `stderr` piped instead of nulled, and a background reader thread
/// draining it into a shared buffer so the D-09 negative test can observe
/// the drain-loop's relay-failure log line without risking a filled pipe
/// buffer stalling the child.
fn spawn_gateway_capturing_stderr(
    side: &Side,
    backed_name: &str,
    listen_port: u16,
    own_cert_stub: &str,
    peer_domain: &str,
    peer_port: u16,
    trust_cert_stub: &str,
) -> (ChildGuard, Arc<Mutex<String>>) {
    let fixtures = cross_machine_fixture_dir();
    let mut child = Command::cargo_bin("famp-gateway")
        .unwrap()
        .arg("--socket")
        .arg(side.sock())
        .arg("--listen")
        .arg(format!("127.0.0.1:{listen_port}"))
        .arg("--tls-cert")
        .arg(fixtures.join(format!("{own_cert_stub}.crt")))
        .arg("--tls-key")
        .arg(fixtures.join(format!("{own_cert_stub}.key")))
        .arg("--peer")
        .arg(format!("{peer_domain}=https://127.0.0.1:{peer_port}"))
        .arg("--trust-cert")
        .arg(fixtures.join(format!("{trust_cert_stub}.crt")))
        .env("FAMP_HOME", side.home())
        .arg(backed_name)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let stderr_pipe = child.stderr.take().expect("piped stderr");
    let buf = Arc::new(Mutex::new(String::new()));
    let buf_writer = Arc::clone(&buf);
    std::thread::spawn(move || {
        let mut pipe = stderr_pipe;
        let mut chunk = [0u8; 4096];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if let Ok(mut guard) = buf_writer.lock() {
                        guard.push_str(&String::from_utf8_lossy(&chunk[..n]));
                    }
                }
            }
        }
    });

    (ChildGuard::new(child), buf)
}

/// Poll (bounded) the shared stderr buffer for BOTH `needle_a` and
/// `needle_b` to appear (order-independent) — never a fixed sleep.
fn wait_for_stderr_contains(
    buf: &Arc<Mutex<String>>,
    needle_a: &str,
    needle_b: &str,
    deadline: Duration,
) {
    let start = Instant::now();
    loop {
        {
            let snapshot = buf.lock().unwrap();
            if snapshot.contains(needle_a) && snapshot.contains(needle_b) {
                return;
            }
        }
        assert!(
            start.elapsed() <= deadline,
            "timed out after {deadline:?} waiting for gateway stderr to contain \
             '{needle_a}' and '{needle_b}'; captured so far: {}",
            buf.lock().unwrap()
        );
        std::thread::sleep(Duration::from_millis(150));
    }
}

/// Run `famp send <args>` via the FIXED shipping CLI (through the shared
/// `famp_cmd` subprocess helper), asserting the process exits 0, and
/// returning the parsed `task_id` field from its JSON-Line stdout.
fn famp_send_ok(sock: &Path, home: &Path, args: &[&str]) -> String {
    let mut full_args = vec!["send"];
    full_args.extend_from_slice(args);
    let out = famp_cmd(sock, home, &full_args);
    assert!(
        out.status.success(),
        "famp send {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
        panic!(
            "famp send {args:?} stdout not JSON: {e}; stdout={}",
            String::from_utf8_lossy(&out.stdout)
        )
    });
    v["task_id"]
        .as_str()
        .unwrap_or_else(|| panic!("famp send {args:?} stdout missing task_id: {v}"))
        .to_string()
}

/// `famp inbox list --as <as_name>` (single snapshot, no polling — callers
/// must already know the line is present via `poll_inbox_contains`), then
/// return the matching line's `from` field. Used by the happy-path
/// assertion that the shipping client stamped a domain-qualified `from`.
fn inbox_line_from(
    sock: &Path,
    home: &Path,
    as_name: &str,
    task_id: &str,
    class_wanted: &str,
) -> String {
    let out = famp_cmd(sock, home, &["inbox", "list", "--as", as_name]);
    assert!(
        out.status.success(),
        "famp inbox list --as {as_name} failed"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        // Phase 14 (D-04/D-05): `famp inbox list` JSONL lines are now
        // `{"origin": ..., "envelope": {...}}` rather than the bare
        // envelope.
        let env = v.get("envelope").unwrap_or(&v);
        let matches_id = env.get("id").and_then(serde_json::Value::as_str) == Some(task_id)
            || env
                .get("causality")
                .and_then(|c| c.get("ref"))
                .and_then(serde_json::Value::as_str)
                == Some(task_id);
        let matches_class =
            env.get("class").and_then(serde_json::Value::as_str) == Some(class_wanted);
        if matches_id && matches_class {
            return env["from"]
                .as_str()
                .unwrap_or_else(|| panic!("matching line has no 'from': {env}"))
                .to_string();
        }
    }
    panic!("no {class_wanted} line for task {task_id} found in {as_name}'s inbox: {stdout}");
}

/// Assert `task_id`/`class_wanted` NEVER appears in `as_name`'s inbox on
/// `sock` within `window` — the D-09 negative test's non-delivery pole.
/// Polls repeatedly rather than sleeping once, so a late-but-still-wrong
/// delivery inside the window is caught, not raced past.
fn assert_never_delivered(
    sock: &Path,
    home: &Path,
    as_name: &str,
    task_id: &str,
    class_wanted: &str,
    window: Duration,
) {
    let start = Instant::now();
    loop {
        let out = famp_cmd(sock, home, &["inbox", "list", "--as", as_name]);
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                    continue;
                };
                // Phase 14 (D-04/D-05): see `inbox_line_from` above.
                let env = v.get("envelope").unwrap_or(&v);
                let matches_id = env.get("id").and_then(serde_json::Value::as_str) == Some(task_id)
                    || env
                        .get("causality")
                        .and_then(|c| c.get("ref"))
                        .and_then(serde_json::Value::as_str)
                        == Some(task_id);
                let matches_class =
                    env.get("class").and_then(serde_json::Value::as_str) == Some(class_wanted);
                assert!(
                    !(matches_id && matches_class),
                    "task {task_id} (class={class_wanted}) WAS delivered into {as_name}'s inbox \
                     despite the mismatched-authority `from` — the federated path failed to \
                     reject it: {env}"
                );
            }
        }
        if start.elapsed() >= window {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[test]
fn shipping_send_happy_path_full_cycle_and_observable_negative() {
    ensure_famp_bin_built();

    // --- Harness setup: two isolated broker+gateway pairs with mutual
    // --- TOFU trust over loopback HTTPS (identical topology to
    // --- e2e_cross_host_delivery.rs, Task 2's extraction target).
    let side_a = Side::new();
    let side_b = Side::new();

    let _broker_a = spawn_broker(&side_a);
    let _broker_b = spawn_broker(&side_b);
    wait_for_broker_socket(&side_a.sock(), STARTUP_DEADLINE);
    wait_for_broker_socket(&side_b.sock(), STARTUP_DEADLINE);

    let _alice_register = spawn_register(&side_a, "alice");
    let _bob_register = spawn_register(&side_b, "bob");
    wait_until_live(&side_a.sock(), side_a.home(), "alice", STARTUP_DEADLINE);
    wait_until_live(&side_b.sock(), side_b.home(), "bob", STARTUP_DEADLINE);

    let alice_blob = peer_export(side_a.home(), ALICE);
    peer_import(side_b.home(), &alice_blob);
    let bob_blob = peer_export(side_b.home(), BOB);
    peer_import(side_a.home(), &bob_blob);

    let port_a = pick_free_port();
    let port_b = pick_free_port();

    // Gateway A: stderr captured (D-09 negative test observable). Gateway
    // A's own-domain is left UNSET (no --domain flag here, matching
    // e2e_cross_host_delivery.rs's spawn_gateway) so plan 07's egress
    // FromDomainMismatch check is skipped and the negative send reaches
    // gateway B's ingress deterministically.
    let (_gateway_a, gateway_a_stderr) =
        spawn_gateway_capturing_stderr(&side_a, "bob", port_a, "alice", BOB_DOMAIN, port_b, "bob");
    let _gateway_b = gateway_harness::spawn_gateway(
        &side_b,
        "alice",
        port_b,
        "bob",
        ALICE_DOMAIN,
        port_a,
        "alice",
    );

    wait_until_live(&side_a.sock(), side_a.home(), "bob", STARTUP_DEADLINE);
    wait_until_live(&side_b.sock(), side_b.home(), "alice", STARTUP_DEADLINE);
    wait_for_tcp(SocketAddr::from(([127, 0, 0, 1], port_a)), STARTUP_DEADLINE);
    wait_for_tcp(SocketAddr::from(([127, 0, 0, 1], port_b)), STARTUP_DEADLINE);

    // ===================================================================
    // 1. HAPPY PATH — `famp send --to agent:<bob-domain>/bob --new-task`
    //    from A delivers into B's real `bob` mailbox with a
    //    domain-qualified `from`.
    // ===================================================================
    let task_id = famp_send_ok(
        &side_a.sock(),
        side_a.home(),
        &[
            "--to",
            &format!("agent:{BOB_DOMAIN}/bob"),
            "--as",
            "alice",
            "--domain",
            ALICE_DOMAIN,
            "--new-task",
            "shipping-surface ping",
        ],
    );

    poll_inbox_contains(
        &side_b.sock(),
        side_b.home(),
        "bob",
        &task_id,
        "request",
        POLL_DEADLINE,
    );
    let request_from = inbox_line_from(&side_b.sock(), side_b.home(), "bob", &task_id, "request");
    assert_eq!(
        request_from, ALICE,
        "the shipping client must stamp a domain-qualified `from` (D-01/D-02), got {request_from}"
    );

    // ===================================================================
    // 2. FULL-CYCLE TERMINAL — commit (B) -> deliver-terminal (B), both
    //    via the shipping client; both sides converge on the same
    //    terminal FSM state (COMPLETED).
    // ===================================================================
    let commit_task_id = famp_send_ok(
        &side_b.sock(),
        side_b.home(),
        &[
            "--to",
            &format!("agent:{ALICE_DOMAIN}/alice"),
            "--as",
            "bob",
            "--domain",
            BOB_DOMAIN,
            "--task",
            &task_id,
        ],
    );
    // The commit is a REPLY: `famp send`'s own envelope id is fresh, but
    // it must causally reference the original task — assert via the
    // commit line landing on A tagged with `causality.ref == task_id`
    // (poll_inbox_contains already matches on id-OR-causality.ref).
    let _ = commit_task_id;
    poll_inbox_contains(
        &side_a.sock(),
        side_a.home(),
        "alice",
        &task_id,
        "commit",
        POLL_DEADLINE,
    );

    famp_send_ok(
        &side_b.sock(),
        side_b.home(),
        &[
            "--to",
            &format!("agent:{ALICE_DOMAIN}/alice"),
            "--as",
            "bob",
            "--domain",
            BOB_DOMAIN,
            "--task",
            &task_id,
            "--terminal",
        ],
    );
    poll_inbox_contains(
        &side_a.sock(),
        side_a.home(),
        "alice",
        &task_id,
        "deliver",
        POLL_DEADLINE,
    );

    let state_a = poll_terminal_state(&side_a.sock(), side_a.home(), &task_id, POLL_DEADLINE);
    let state_b = poll_terminal_state(&side_b.sock(), side_b.home(), &task_id, POLL_DEADLINE);
    assert_eq!(
        state_a, state_b,
        "both sides must converge on the same terminal FSM state"
    );
    assert_eq!(state_a, "COMPLETED");

    // ===================================================================
    // 3. NEGATIVE (OBSERVABLE) — a remote send with `--domain local.bus`
    //    stamps `from = agent:local.bus/alice`, mismatched against the
    //    pinned `agent:hosta.test/alice` label B trusts. Assert a TYPED,
    //    OBSERVABLE rejection (gateway A's stderr) AND non-delivery on B.
    // ===================================================================
    let negative_task_id = famp_send_ok(
        &side_a.sock(),
        side_a.home(),
        &[
            "--to",
            &format!("agent:{BOB_DOMAIN}/bob"),
            "--as",
            "alice",
            "--domain",
            "local.bus",
            "--new-task",
            "should be rejected at gateway B ingress",
        ],
    );

    wait_for_stderr_contains(&gateway_a_stderr, "unpinned_key", "403", POLL_DEADLINE);
    assert_never_delivered(
        &side_b.sock(),
        side_b.home(),
        "bob",
        &negative_task_id,
        "request",
        NON_DELIVERY_WINDOW,
    );
}

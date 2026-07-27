//! Phase 9 Plan 05 — the D-07 phase gate: a two-process loopback E2E that
//! proves GW-01/GW-02/GW-03 by standing up two brokers on distinct sockets,
//! two real `famp-gateway` subprocesses over loopback HTTPS, establishing
//! mutual TOFU trust via `famp peer export`/`import`, and driving a full
//! request -> commit -> deliver -> ack task cycle between an agent on each
//! side.
//!
//! ## Topology (09-RESEARCH.md D-01/D-05, 09-05-PLAN.md)
//!
//! Side A runs broker A, the REAL `alice` identity (`famp register alice`),
//! and gateway A, which backs the bare name `bob` as a local stand-in/proxy
//! for the remote `bob`. Side B is symmetric: broker B, the REAL `bob`
//! identity, and gateway B backing `alice`. A message `alice` addresses "to
//! bob" lands in gateway A's own `bob`-proxy mailbox on bus A; gateway A's
//! egress loop drains, federation-signs, and POSTs it to gateway B; gateway
//! B's ingress verifies it and delivers it via its own `alice`-proxy
//! connection onto bus B, landing in the REAL `bob`'s mailbox. The reverse
//! path is symmetric (bob to alice).
//!
//! Principals are domain-qualified (`agent:hosta.test/alice`,
//! `agent:hostb.test/bob`) so `--peer <domain>=<url>` (keyed by the
//! envelope's own `to`/`from` domain, not the bus's bare local names) can
//! route correctly — see `crates/famp-gateway/src/egress.rs`'s
//! `parse_principal_field`/`relay_one` and `main.rs`'s peer-map cross
//! product.
//!
//! ## Envelope construction — why this bypasses `famp send`
//!
//! `famp send`'s CLI always emits `class: "audit_log"` (mode/terminal
//! encoded under `body.details`); `famp-inspect-server::derive_fsm_state`
//! only recognizes the literal `class` values `request`/`commit`/
//! `deliver`/`control` (confirmed empirically: a `famp send`-driven
//! conversation always shows `fsm_transition: "UNKNOWN"`). This test
//! instead builds real typed envelopes (`RequestBody`/`CommitBody`/
//! `DeliverBody`/`AckBody`/`ControlBody`) directly via
//! `UnsignedEnvelope<B>`, sent unsigned onto the local bus (BUS-11) via
//! the sanctioned "sign-then-strip" pattern already used by
//! `famp-gateway`'s own `egress.rs` unit tests and
//! `famp/tests/common/cycle_driver.rs`. These are federation-crossable
//! (their bodies satisfy each class's strict `deny_unknown_fields` schema,
//! so `verify_inbound_any`'s typed decode at gateway ingress accepts
//! them) — see the SUMMARY's Deviations section for the full rationale,
//! including why a closing `control`/`cancel` envelope (not `deliver` or
//! `ack`) is the terminal signal GW-03 asserts on.
//!
//! ## Harness patterns reused
//!
//! Pattern A (`crates/famp-gateway/tests/liveness.rs`, 07-03):
//! `ensure_famp_bin_built()`, `ChildGuard`, poll-with-deadline (never a
//! fixed `sleep()`-then-assert). Fixture certs from
//! `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}`
//! (09-PATTERNS.md). Two isolation axes per side (09-RESEARCH.md §7):
//! `--socket` for bus/mailbox isolation, `FAMP_HOME` for gateway
//! identity/peers-keyring isolation — one `tempfile::TempDir` per side
//! serves both.

#![allow(unused_crate_dependencies)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::too_many_arguments
)]

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;
use famp::{AuthorityScope, FampSigningKey, MessageId, Principal, TerminalStatus, Timestamp};
use famp_bus::{BusMessage, BusReply, Target};
use famp_envelope::body::{
    AckBody, AckDisposition, Bounds, Budget, CommitBody, ControlAction, ControlBody, ControlTarget,
    DeliverBody, RequestBody,
};
use famp_envelope::{BodySchema, Causality, Relation, UnsignedEnvelope};
use famp_inspect_proto::IdentityListReply;

#[path = "common/child_guard.rs"]
mod child_guard;
use child_guard::ChildGuard;

/// Domain-qualified principal for the REAL identity registered on side A.
const ALICE: &str = "agent:hosta.test/alice";
/// Domain-qualified principal for the REAL identity registered on side B.
const BOB: &str = "agent:hostb.test/bob";
/// Bare domain segments (must match the `--peer` map + the envelope's own
/// `from_domain`/`to_domain` derivation via `Principal::authority()`).
const ALICE_DOMAIN: &str = "hosta.test";
const BOB_DOMAIN: &str = "hostb.test";

/// Bounded poll deadline shared by every convergence assertion in this
/// test — generous enough to absorb subprocess startup + egress `Await`
/// polling (~1s cadence, `egress.rs::AWAIT_TIMEOUT_MS`) without being
/// unbounded.
const POLL_DEADLINE: Duration = Duration::from_secs(20);

// ---------------------------------------------------------------------
// Subprocess + harness plumbing (Pattern A, liveness.rs)
// ---------------------------------------------------------------------

/// `Command::cargo_bin("famp")` resolves the sibling `famp` binary via
/// assert_cmd's `legacy_cargo_bin` fallback rather than the
/// `CARGO_BIN_EXE_famp` env var, which Cargo does not propagate across
/// package boundaries. Build famp's bin explicitly first so this test is
/// hermetic regardless of invocation order (identical rationale to
/// `liveness.rs::ensure_famp_bin_built`). `famp-gateway`'s own `[[bin]]`
/// is built automatically as a side effect of `cargo test -p famp-gateway`
/// building this same package's test target.
fn ensure_famp_bin_built() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let status = Command::new(cargo)
        .args(["build", "--quiet", "-p", "famp", "--bin", "famp"])
        .status()
        .expect("failed to invoke cargo to build the famp binary");
    assert!(status.success(), "cargo build -p famp --bin famp failed");
}

/// `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}` — the
/// shared TLS fixture cert pair every cross-host gateway test reuses
/// (09-PATTERNS.md). TLS here is channel encryption only (D-08); the
/// Ed25519 envelope signature is the real trust boundary.
fn cross_machine_fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("famp")
        .join("tests")
        .join("fixtures")
        .join("cross_machine")
}

/// OS-assigned free loopback port. Bind-then-drop before the port is
/// handed to a spawned `famp-gateway` subprocess's own `--listen`
/// bind — a small, standard TOCTOU window this codebase already accepts
/// elsewhere for ephemeral-port test setups.
fn pick_free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local_addr")
        .port()
}

/// One side's isolated broker + `FAMP_HOME`. A single `TempDir` serves
/// BOTH isolation axes (09-RESEARCH.md §7): `--socket` for bus/mailbox
/// isolation, `FAMP_HOME` for the gateway's identity/peers-keyring
/// isolation.
struct Side {
    tmp: tempfile::TempDir,
}

impl Side {
    fn new() -> Self {
        Self {
            tmp: tempfile::TempDir::new().expect("tempdir"),
        }
    }

    fn sock(&self) -> PathBuf {
        self.tmp.path().join("bus.sock")
    }

    fn home(&self) -> &Path {
        self.tmp.path()
    }
}

/// Run a one-shot `famp <args>` subprocess against `sock`/`home`,
/// returning its captured output. Sets `FAMP_BUS_SOCKET`, `FAMP_HOME`,
/// and `HOME` so every identity-resolution / mailbox / peer-keyring path
/// stays inside this side's isolated tempdir.
fn famp_cmd(sock: &Path, home: &Path, args: &[&str]) -> std::process::Output {
    Command::cargo_bin("famp")
        .unwrap()
        .env("FAMP_BUS_SOCKET", sock)
        .env("FAMP_HOME", home)
        .env("HOME", home)
        .args(args)
        .output()
        .unwrap()
}

/// Spawn a long-lived `famp <args>` subprocess (e.g. `broker`,
/// `register`), ChildGuard-wrapped so a panicking test still reaps it
/// (project ChildGuard convention — leaked tmp-socket brokers/holders
/// respawn).
fn famp_spawn(sock: &Path, home: &Path, args: &[&str]) -> ChildGuard {
    ChildGuard::new(
        Command::cargo_bin("famp")
            .unwrap()
            .env("FAMP_BUS_SOCKET", sock)
            .env("FAMP_HOME", home)
            .env("HOME", home)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

fn spawn_broker(side: &Side) -> ChildGuard {
    ChildGuard::new(
        Command::cargo_bin("famp")
            .unwrap()
            .arg("broker")
            .arg("--socket")
            .arg(side.sock())
            .env("FAMP_HOME", side.home())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

/// Poll (bounded) until the broker's UDS socket accepts a raw connection —
/// never a fixed `sleep()`-then-assert (07-RESEARCH.md Pitfall 4).
fn wait_for_broker_socket(sock: &Path, deadline: Duration) {
    let start = Instant::now();
    loop {
        if std::os::unix::net::UnixStream::connect(sock).is_ok() {
            return;
        }
        assert!(
            start.elapsed() <= deadline,
            "broker socket at {} never came up within {deadline:?}",
            sock.display()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn spawn_register(side: &Side, name: &str) -> ChildGuard {
    famp_spawn(&side.sock(), side.home(), &["register", name])
}

/// `famp inspect identities --json` against `sock`, or `None` if the call
/// failed (broker not yet up / transient). Callers poll on `None`.
fn live_identity_names(sock: &Path, home: &Path) -> Option<Vec<String>> {
    let out = famp_cmd(sock, home, &["inspect", "identities", "--json"]);
    if !out.status.success() {
        return None;
    }
    let list: IdentityListReply = serde_json::from_slice(&out.stdout).ok()?;
    Some(list.rows.into_iter().map(|r| r.name).collect())
}

/// Poll (bounded) until `name` appears among the live identities on
/// `sock` — used for both real `famp register` holders and gateway-backed
/// proxy names, since both are plain live registrations from the
/// broker's point of view.
fn wait_until_live(sock: &Path, home: &Path, name: &str, deadline: Duration) {
    let start = Instant::now();
    loop {
        if let Some(live) = live_identity_names(sock, home) {
            if live.iter().any(|l| l == name) {
                return;
            }
        }
        assert!(
            start.elapsed() <= deadline,
            "timed out after {deadline:?} waiting for '{name}' to appear live on {}",
            sock.display()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// `famp peer export --as <principal>` against `home`, returning the
/// captured one-line export blob.
fn peer_export(home: &Path, principal: &str) -> String {
    let out = Command::cargo_bin("famp")
        .unwrap()
        .env("FAMP_HOME", home)
        .args(["peer", "export", "--as", principal])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "famp peer export --as {principal} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

/// `famp peer import <file>` against `home`, TOFU-pinning `blob`'s
/// principal + pubkey into this side's gateway peers keyring. Written to
/// a temp file rather than piped over stdin — simpler subprocess
/// plumbing, same on-disk effect.
fn peer_import(home: &Path, blob: &str) {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), blob).unwrap();
    let out = Command::cargo_bin("famp")
        .unwrap()
        .env("FAMP_HOME", home)
        .args(["peer", "import"])
        .arg(file.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "famp peer import failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Spawn a `famp-gateway` subprocess on `side`, backing the bare local
/// name `backed_name` (the REMOTE principal this gateway fronts on its
/// own bus, D-01), listening on `127.0.0.1:<listen_port>` with the
/// `{own_cert_stub}.{crt,key}` fixture pair, relaying to
/// `{peer_domain}=https://127.0.0.1:<peer_port>`, and trusting the
/// `{trust_cert_stub}.crt` fixture cert for its outbound TLS client.
fn spawn_gateway(
    side: &Side,
    backed_name: &str,
    listen_port: u16,
    own_cert_stub: &str,
    peer_domain: &str,
    peer_port: u16,
    trust_cert_stub: &str,
) -> ChildGuard {
    let fixtures = cross_machine_fixture_dir();
    ChildGuard::new(
        Command::cargo_bin("famp-gateway")
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
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

/// Poll (bounded) until `addr` accepts a raw TCP connection — confirms
/// the gateway's `run_ingress` HTTPS listener is actually bound before
/// this test triggers any egress relay that depends on it. Load-bearing:
/// `run_egress`'s `Await` drain ADVANCES the mailbox read cursor even on
/// a failed relay POST (no re-queue on error) — a message drained before
/// the peer listener is up would be silently lost, not retried.
fn wait_for_tcp(addr: SocketAddr, deadline: Duration) {
    let start = Instant::now();
    loop {
        if TcpStream::connect(addr).is_ok() {
            return;
        }
        assert!(
            start.elapsed() <= deadline,
            "gateway listener at {addr} never came up within {deadline:?}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

// ---------------------------------------------------------------------
// Envelope construction — real typed classes, unsigned (BUS-11), sent
// directly onto the local bus (bypasses `famp send`'s audit_log-only
// CLI surface — see module doc for why).
// ---------------------------------------------------------------------

fn two_key_bounds() -> Bounds {
    Bounds {
        deadline: Some("2026-12-31T00:00:00Z".to_string()),
        budget: Some(Budget {
            amount: "10".to_string(),
            unit: "usd".to_string(),
        }),
        hop_limit: None,
        policy_domain: None,
        authority_scope: None,
        max_artifact_size: None,
        confidence_floor: None,
        recursion_depth: None,
    }
}

fn ts() -> Timestamp {
    Timestamp("2026-07-27T00:00:00Z".to_string())
}

/// Sign an `UnsignedEnvelope<B>` with a throwaway key then strip the
/// `signature` field — the sanctioned pattern for producing a
/// BUS-11-compliant unsigned `Value` from the typed builder API without
/// a separate "encode unsigned" accessor (there isn't one; the only path
/// to wire bytes is `sign()` -> `encode()`). Identical technique to
/// `famp-gateway/src/egress.rs`'s own `plain_request_value` unit-test
/// helper and `famp/tests/common/cycle_driver.rs`.
fn unsigned_value<B: BodySchema>(env: UnsignedEnvelope<B>) -> serde_json::Value {
    let dummy_sk = FampSigningKey::from_bytes([42u8; 32]);
    let bytes = env
        .sign(&dummy_sk)
        .expect("sign for value-strip")
        .encode()
        .expect("encode");
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).expect("parse signed bytes");
    value
        .as_object_mut()
        .expect("envelope root is an object")
        .remove("signature");
    value
}

fn build_request(task_id: MessageId, from: &Principal, to: &Principal) -> serde_json::Value {
    let body = RequestBody {
        scope: serde_json::json!({"task": "cross-host-e2e"}),
        bounds: two_key_bounds(),
        natural_language_summary: Some("ping".to_string()),
    };
    let env = UnsignedEnvelope::<RequestBody>::new(
        task_id,
        from.clone(),
        to.clone(),
        AuthorityScope::Advisory,
        ts(),
        body,
    );
    unsigned_value(env)
}

fn build_commit(task_id: MessageId, from: &Principal, to: &Principal) -> serde_json::Value {
    let body = CommitBody {
        scope: serde_json::json!({"task": "cross-host-e2e"}),
        scope_subset: None,
        bounds: two_key_bounds(),
        accepted_policies: vec!["policy://famp/v0.7/personal".to_string()],
        delegation_permissions: None,
        reporting_obligations: None,
        terminal_condition: serde_json::json!({"type": "final_delivery"}),
        conditions: None,
        natural_language_summary: None,
    };
    let env = UnsignedEnvelope::<CommitBody>::new(
        MessageId::new_v7(),
        from.clone(),
        to.clone(),
        AuthorityScope::CommitLocal,
        ts(),
        body,
    )
    .with_causality(Causality {
        rel: Relation::Commits,
        referenced: task_id,
    });
    unsigned_value(env)
}

fn build_deliver(task_id: MessageId, from: &Principal, to: &Principal) -> serde_json::Value {
    let body = DeliverBody {
        interim: false,
        artifacts: None,
        result: Some(serde_json::json!({"text": "pong"})),
        usage_metrics: None,
        error_detail: None,
        provenance: Some(serde_json::json!({"signer": from.to_string()})),
        natural_language_summary: None,
    };
    let env = UnsignedEnvelope::<DeliverBody>::new(
        MessageId::new_v7(),
        from.clone(),
        to.clone(),
        AuthorityScope::Advisory,
        ts(),
        body,
    )
    .with_causality(Causality {
        rel: Relation::Delivers,
        referenced: task_id,
    })
    .with_terminal_status(TerminalStatus::Completed);
    unsigned_value(env)
}

fn build_ack(task_id: MessageId, from: &Principal, to: &Principal) -> serde_json::Value {
    let body = AckBody {
        disposition: AckDisposition::Completed,
        reason: None,
    };
    let env = UnsignedEnvelope::<AckBody>::new(
        MessageId::new_v7(),
        from.clone(),
        to.clone(),
        AuthorityScope::Advisory,
        ts(),
        body,
    )
    .with_causality(Causality {
        rel: Relation::Acknowledges,
        referenced: task_id,
    });
    unsigned_value(env)
}

/// `control`/`cancel` closing envelope. Not part of the plan's literal
/// request/commit/deliver/ack wording — see module doc + SUMMARY
/// Deviations for why this is the envelope GW-03 asserts a terminal
/// `fsm_transition` against (`derive_fsm_state`'s unconditional
/// `("control", _, _, _) => "CANCELLED"` catch-all is the only class
/// this CLI view maps to a genuine terminal state for a federation-
/// crossable, strictly-typed body).
fn build_cancel(task_id: MessageId, from: &Principal, to: &Principal) -> serde_json::Value {
    let body = ControlBody {
        target: ControlTarget::Task,
        action: ControlAction::Cancel,
        disposition: None,
        reason: Some("e2e cycle complete".to_string()),
        affected_ids: None,
    };
    let env = UnsignedEnvelope::<ControlBody>::new(
        MessageId::new_v7(),
        from.clone(),
        to.clone(),
        AuthorityScope::Advisory,
        ts(),
        body,
    )
    .with_causality(Causality {
        rel: Relation::Cancels,
        referenced: task_id,
    });
    unsigned_value(env)
}

/// Send `envelope` onto the local bus at `sock`, proxying through the
/// `bind_as` canonical holder's connection (D-10 — matches exactly what
/// `famp send`'s CLI does, just with a real typed envelope instead of an
/// `audit_log`-wrapped one) to `Target::Agent { name: to_name }`. Builds
/// a throwaway current-thread runtime so the rest of this test stays
/// synchronous (mirrors `send/mod.rs`'s own
/// `more_coming_without_new_task_errors_in_run_at_structured` unit-test
/// pattern of `Builder::new_current_thread().block_on(..)`).
fn send_bus_envelope(sock: &Path, bind_as: &str, to_name: &str, envelope: serde_json::Value) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let mut client =
            famp::bus_client::BusClient::connect_no_spawn(sock, Some(bind_as.to_string()))
                .await
                .unwrap_or_else(|e| panic!("connect_no_spawn as {bind_as} failed: {e:?}"));
        let reply = client
            .send_recv(BusMessage::Send {
                to: Target::Agent {
                    name: to_name.to_string(),
                },
                envelope,
            })
            .await
            .unwrap_or_else(|e| panic!("send_recv failed: {e:?}"));
        match reply {
            BusReply::SendOk { .. } => {}
            other => panic!("unexpected reply to Send: {other:?}"),
        }
    });
}

// ---------------------------------------------------------------------
// Assertions — poll-with-deadline, never a fixed sleep.
// ---------------------------------------------------------------------

/// Poll `famp inbox list --as <as_name>` (bounded) until a JSONL line
/// with `class == class_wanted` and (`id` OR `causality.ref`) ==
/// `task_id` appears. Proves GW-01/GW-02: the envelope actually reached
/// this side's real local mailbox, not just "the gateway said 202".
fn poll_inbox_contains(
    sock: &Path,
    home: &Path,
    as_name: &str,
    task_id: &str,
    class_wanted: &str,
    deadline: Duration,
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
                let matches_id = v.get("id").and_then(serde_json::Value::as_str) == Some(task_id)
                    || v.get("causality")
                        .and_then(|c| c.get("ref"))
                        .and_then(serde_json::Value::as_str)
                        == Some(task_id);
                let matches_class =
                    v.get("class").and_then(serde_json::Value::as_str) == Some(class_wanted);
                if matches_id && matches_class {
                    return;
                }
            }
        }
        assert!(
            start.elapsed() <= deadline,
            "timed out after {deadline:?} waiting for class={class_wanted} task={task_id} \
             in {as_name}'s inbox on {}; last stdout: {}",
            sock.display(),
            String::from_utf8_lossy(&out.stdout)
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Poll `famp inspect tasks --id <task_id> --json` (bounded) until the
/// LAST envelope in the returned chain shows a terminal `fsm_transition`
/// (`COMPLETED`/`FAILED`/`CANCELLED`), returning it. GW-03's proof point.
fn poll_terminal_state(sock: &Path, home: &Path, task_id: &str, deadline: Duration) -> String {
    let start = Instant::now();
    loop {
        let out = famp_cmd(sock, home, &["inspect", "tasks", "--id", task_id, "--json"]);
        if out.status.success() {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                if let Some(state) = v
                    .get("envelopes")
                    .and_then(serde_json::Value::as_array)
                    .and_then(|envs| envs.last())
                    .and_then(|last| last.get("fsm_transition"))
                    .and_then(serde_json::Value::as_str)
                {
                    if matches!(state, "COMPLETED" | "FAILED" | "CANCELLED") {
                        return state.to_string();
                    }
                }
            }
        }
        assert!(
            start.elapsed() <= deadline,
            "timed out after {deadline:?} waiting for a terminal fsm_transition on task \
             {task_id} via {}; last stdout: {}",
            sock.display(),
            String::from_utf8_lossy(&out.stdout)
        );
        std::thread::sleep(Duration::from_millis(300));
    }
}

// ---------------------------------------------------------------------
// The gate.
// ---------------------------------------------------------------------

#[test]
fn gw01_gw02_gw03_two_process_cross_host_delivery() {
    ensure_famp_bin_built();

    let side_a = Side::new();
    let side_b = Side::new();

    // Two isolated brokers.
    let _broker_a = spawn_broker(&side_a);
    let _broker_b = spawn_broker(&side_b);
    wait_for_broker_socket(&side_a.sock(), Duration::from_secs(5));
    wait_for_broker_socket(&side_b.sock(), Duration::from_secs(5));

    // Real identities: alice on A, bob on B.
    let _alice_register = spawn_register(&side_a, "alice");
    let _bob_register = spawn_register(&side_b, "bob");
    wait_until_live(
        &side_a.sock(),
        side_a.home(),
        "alice",
        Duration::from_secs(5),
    );
    wait_until_live(&side_b.sock(), side_b.home(), "bob", Duration::from_secs(5));

    // Mutual TOFU trust bootstrap — MUST happen before gateway startup;
    // the gateway loads its peers keyring once at startup, no hot-reload
    // (09-RESEARCH.md §5 Assumption A3).
    let alice_blob = peer_export(side_a.home(), ALICE);
    peer_import(side_b.home(), &alice_blob);
    let bob_blob = peer_export(side_b.home(), BOB);
    peer_import(side_a.home(), &bob_blob);

    // Two gateways, each backing the OTHER side's principal as a local
    // stand-in (D-01).
    let port_a = pick_free_port();
    let port_b = pick_free_port();

    let _gateway_a = spawn_gateway(&side_a, "bob", port_a, "alice", BOB_DOMAIN, port_b, "bob");
    let _gateway_b = spawn_gateway(
        &side_b,
        "alice",
        port_b,
        "bob",
        ALICE_DOMAIN,
        port_a,
        "alice",
    );

    wait_until_live(
        &side_a.sock(),
        side_a.home(),
        "bob",
        Duration::from_secs(10),
    );
    wait_until_live(
        &side_b.sock(),
        side_b.home(),
        "alice",
        Duration::from_secs(10),
    );
    wait_for_tcp(
        SocketAddr::from(([127, 0, 0, 1], port_a)),
        Duration::from_secs(10),
    );
    wait_for_tcp(
        SocketAddr::from(([127, 0, 0, 1], port_b)),
        Duration::from_secs(10),
    );

    // --- Harness ready: two isolated broker+gateway pairs with mutual
    // --- TOFU trust over loopback HTTPS (Task 1's <done> criterion).

    let alice: Principal = ALICE.parse().unwrap();
    let bob: Principal = BOB.parse().unwrap();
    let task_id = MessageId::new_v7();
    let task_id_str = task_id.to_string();

    // 1. alice -> bob: REQUEST.
    let request = build_request(task_id, &alice, &bob);
    send_bus_envelope(&side_a.sock(), "alice", "bob", request);

    // GW-01: the request reaches B's real bob mailbox through both gateways.
    poll_inbox_contains(
        &side_b.sock(),
        side_b.home(),
        "bob",
        &task_id_str,
        "request",
        POLL_DEADLINE,
    );

    // 2. bob -> alice: COMMIT (reply within the same task/conversation).
    let commit = build_commit(task_id, &bob, &alice);
    send_bus_envelope(&side_b.sock(), "bob", "alice", commit);

    // GW-02: bob's reply reaches A's real alice mailbox.
    poll_inbox_contains(
        &side_a.sock(),
        side_a.home(),
        "alice",
        &task_id_str,
        "commit",
        POLL_DEADLINE,
    );

    // 3. bob -> alice: DELIVER (terminal_status=Completed; content-
    //    transparent relay — task_id/class/body never rewritten).
    let deliver = build_deliver(task_id, &bob, &alice);
    send_bus_envelope(&side_b.sock(), "bob", "alice", deliver);
    poll_inbox_contains(
        &side_a.sock(),
        side_a.home(),
        "alice",
        &task_id_str,
        "deliver",
        POLL_DEADLINE,
    );

    // 4. alice -> bob: ACK, closing the literal request/commit/deliver/ack
    //    cycle.
    let ack = build_ack(task_id, &alice, &bob);
    send_bus_envelope(&side_a.sock(), "alice", "bob", ack);
    poll_inbox_contains(
        &side_b.sock(),
        side_b.home(),
        "bob",
        &task_id_str,
        "ack",
        POLL_DEADLINE,
    );

    // 5/6. Close the task in BOTH directions so each side's real mailbox
    //      carries an envelope `famp inspect tasks --id --json` maps to a
    //      genuine terminal state (see build_cancel's doc comment).
    let cancel_to_bob = build_cancel(task_id, &alice, &bob);
    send_bus_envelope(&side_a.sock(), "alice", "bob", cancel_to_bob);
    let cancel_to_alice = build_cancel(task_id, &bob, &alice);
    send_bus_envelope(&side_b.sock(), "bob", "alice", cancel_to_alice);

    // GW-03: a full cycle completed across both machines and BOTH sides
    // converge on the SAME terminal FSM state via `famp inspect tasks
    // --id <task_id> --json`.
    let state_b = poll_terminal_state(&side_b.sock(), side_b.home(), &task_id_str, POLL_DEADLINE);
    let state_a = poll_terminal_state(&side_a.sock(), side_a.home(), &task_id_str, POLL_DEADLINE);
    assert_eq!(
        state_a, state_b,
        "both sides must converge on the same terminal FSM state"
    );
    assert_eq!(state_a, "CANCELLED");
}

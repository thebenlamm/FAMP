#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

/// Startup deadline for broker socket, relaxed from 5s to accommodate parallel
/// test execution where multiple harnesses spawn brokers simultaneously.
/// See crates/famp-gateway/tests/common/gateway_harness.rs::STARTUP_DEADLINE.
const STARTUP_DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);

// Phase 09 Plan 01 Task 1: ProxiedPrincipal::send_recv and
// GatewayRegistry::get_mut — the shared drain/send plumbing every
// downstream Phase 9 plan (egress/ingress) hangs off.
//
// Shape mirrors `crates/famp-gateway/tests/liveness.rs` /
// `no_cross_talk.rs` (07-PATTERNS.md): a real broker subprocess,
// ChildGuard-wrapped, poll-with-deadline never a fixed sleep. Unlike
// those tests, this one constructs `ProxiedPrincipal` directly (the
// object under test) rather than going through a spawned
// `famp-gateway` process — 09-PATTERNS.md's `register()` analog is a
// `pub async fn` reachable straight from an integration test.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use assert_cmd::cargo::CommandCargoExt;
use famp::bus_client::BusClient;
use famp_bus::{BusMessage, BusReply, Target};
use famp_gateway::{GatewayRegistry, ProxiedPrincipal};

#[path = "common/child_guard.rs"]
mod child_guard;
use child_guard::ChildGuard;

/// See `tests/liveness.rs` for the full rationale on why `famp`'s bin
/// must be built explicitly before `Command::cargo_bin("famp")` works
/// from a `famp-gateway` test binary.
fn ensure_famp_bin_built() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let status = Command::new(cargo)
        .args(["build", "--quiet", "-p", "famp", "--bin", "famp"])
        .status()
        .expect("failed to invoke cargo to build the famp binary");
    assert!(status.success(), "cargo build -p famp --bin famp failed");
}

fn spawn_broker_subprocess(sock: &Path) -> ChildGuard {
    ChildGuard::new(
        Command::cargo_bin("famp")
            .unwrap()
            .args(["broker", "--socket", sock.to_str().unwrap()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

/// A minimal, fully-decodable `audit_log`/`standalone` envelope (the one
/// class that never fires the task FSM — CLAUDE.md). `AnyBusEnvelope::decode`
/// (`decode_line`, the broker's per-line drain gate) rejects anything
/// missing a required `WireEnvelope` field as an undecodable line and
/// silently skips it (head-of-line resilience) — so the test's send/await
/// round-trip needs a real envelope shape, not just `{"id", "body"}`.
fn audit_log_envelope(id: uuid::Uuid, from: &str, to: &str) -> serde_json::Value {
    serde_json::json!({
        "famp": famp_envelope::FAMP_SPEC_VERSION,
        "class": "audit_log",
        "scope": "standalone",
        "id": id.to_string(),
        "from": format!("agent:example.test/{from}"),
        "to": format!("agent:example.test/{to}"),
        "authority": "advisory",
        "ts": "2026-07-27T00:00:00Z",
        "body": { "event": "user_login" },
    })
}

fn wait_for_broker_socket(sock: &Path, deadline: Duration) {
    let start = std::time::Instant::now();
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

#[tokio::test]
async fn send_recv_round_trips_await_timeout_and_send() {
    ensure_famp_bin_built();

    let tmp = tempfile::TempDir::new().unwrap();
    let sock = tmp.path().join("bus.sock");
    let _broker = spawn_broker_subprocess(&sock);
    wait_for_broker_socket(&sock, STARTUP_DEADLINE);

    // "bob" is the gateway-backed remote principal stand-in under test.
    let mut bob = ProxiedPrincipal::register(&sock, "bob".into())
        .await
        .expect("bob register");

    // Empty mailbox -> AwaitTimeout on a short timeout.
    let reply = bob
        .send_recv(BusMessage::Await {
            timeout_ms: 200,
            task: None,
        })
        .await
        .expect("send_recv await (empty mailbox)");
    assert!(
        matches!(reply, BusReply::AwaitTimeout {}),
        "expected AwaitTimeout on an empty mailbox, got {reply:?}"
    );

    // A local agent ("alice") registers as its own canonical holder and
    // sends to "bob" — this is the traffic a real local sender would
    // produce, landing in the gateway-backed mailbox for bob (D-01/D-03).
    let mut alice = BusClient::connect(&sock, None)
        .await
        .expect("alice connect");
    alice
        .send_recv(BusMessage::Register {
            name: "alice".into(),
            pid: std::process::id(),
            cwd: None,
            listen: false,
        })
        .await
        .expect("alice register");

    let tag = uuid::Uuid::now_v7();
    let envelope = audit_log_envelope(tag, "alice", "bob");
    let send_reply = alice
        .send_recv(BusMessage::Send {
            to: Target::Agent { name: "bob".into() },
            envelope,
        })
        .await
        .expect("alice send to bob");
    let task_id = match send_reply {
        BusReply::SendOk { task_id, .. } => task_id,
        other => panic!("expected SendOk, got {other:?}"),
    };
    assert_eq!(task_id.to_string(), tag.to_string());

    // bob's own send_recv connection drains the mailbox via Await and
    // sees the envelope alice just sent.
    let reply = bob
        .send_recv(BusMessage::Await {
            timeout_ms: 5_000,
            task: None,
        })
        .await
        .expect("send_recv await (non-empty mailbox)");
    match reply {
        BusReply::AwaitOk { envelopes, .. } => {
            assert!(
                envelopes
                    .iter()
                    .any(|e| e.get("id").and_then(|v| v.as_str()) == Some(tag.to_string().as_str())),
                "drained envelopes did not contain the tagged send: {envelopes:?}"
            );
        }
        other => panic!("expected AwaitOk, got {other:?}"),
    }

    // send_recv can also SEND on bob's own connection to a live local
    // recipient (the outbound-drain-then-relay shape D-03/D-05 need):
    // returns SendOk carrying a task_id.
    let tag2 = uuid::Uuid::now_v7();
    let envelope2 = audit_log_envelope(tag2, "bob", "alice");
    let reply = bob
        .send_recv(BusMessage::Send {
            to: Target::Agent {
                name: "alice".into(),
            },
            envelope: envelope2,
        })
        .await
        .expect("bob send to alice");
    match reply {
        BusReply::SendOk { task_id, .. } => assert_eq!(task_id.to_string(), tag2.to_string()),
        other => panic!("expected SendOk, got {other:?}"),
    }
}

#[tokio::test]
async fn registry_get_mut_returns_backed_and_none_for_unbacked() {
    ensure_famp_bin_built();

    let tmp = tempfile::TempDir::new().unwrap();
    let sock = tmp.path().join("bus.sock");
    let _broker = spawn_broker_subprocess(&sock);
    wait_for_broker_socket(&sock, STARTUP_DEADLINE);

    let mut registry = GatewayRegistry::default();
    registry
        .back(&sock, "dave".into())
        .await
        .expect("back dave");

    assert!(registry.get_mut("dave").is_some());
    assert!(registry.get_mut("nobody").is_none());

    // Mutable access is actually usable — issue a send_recv through it.
    let dave = registry.get_mut("dave").expect("dave backed");
    let reply = dave
        .send_recv(BusMessage::Await {
            timeout_ms: 200,
            task: None,
        })
        .await
        .expect("send_recv via get_mut");
    assert!(matches!(reply, BusReply::AwaitTimeout {}));
}

#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

// Phase 07 Plan 03 Task 2: GW-04 — a single `famp-gateway` process
// backing two proxied principals (alice, bob) never lets a message
// addressed to alice appear in bob's mailbox.
//
// 07-RESEARCH.md: routing isolation falls out structurally — each
// proxied principal is its own UDS connection/`ClientId`, and the
// broker's existing per-name mailbox file model (`mailboxes/<name>.jsonl`)
// is what `famp inspect messages --to <name>` reads. The gateway itself
// does no demuxing of INBOUND wire traffic in this skeleton phase; this
// test pins the isolation the broker already provides for two names
// backed by the SAME gateway PID.
//
// Sender: a `bind_as = Some("bob")` proxy connection (D-10) piggybacking
// on the gateway's own live "bob" registration — exactly the mechanism
// `famp send --as bob` would use. No extra spawned process needed; bob
// sends directly to alice over the wire (`BusClient`/`BusMessage`, same
// pattern as `crates/famp/tests/broker_proxy_semantics.rs`).
//
// Correlation: the envelope carries its own `id` (a fresh UUIDv7) plus
// `body.event == "famp.send.new_task"`, so both the broker's `SendOk`
// reply (`task_id_from`, `handle.rs:1157`) and the inspector's row
// projection (`EnvelopeView::task_id`, third resolution branch) resolve
// to the SAME tag — a reliable way to correlate "this exact send" without
// reading message bodies (`famp inspect messages` never exposes body
// content, INSP-MSG-01).

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use assert_cmd::cargo::CommandCargoExt;
use famp::bus_client::BusClient;
use famp_bus::{BusMessage, BusReply, Target};
use famp_inspect_proto::{IdentityListReply, InspectMessagesReply};

#[path = "common/child_guard.rs"]
mod child_guard;
use child_guard::ChildGuard;

/// See `tests/liveness.rs` for the full rationale: `Command::cargo_bin`
/// resolves the sibling `famp` binary via the shared-workspace
/// `target/debug/` fallback, not `CARGO_BIN_EXE_famp` (Cargo does not
/// propagate that var across the `famp-gateway` -> `famp` package
/// boundary). Build famp's bin explicitly so this test is hermetic on a
/// clean checkout.
fn ensure_famp_bin_built() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let status = Command::new(cargo)
        .args(["build", "--quiet", "-p", "famp", "--bin", "famp"])
        .status()
        .expect("failed to invoke cargo to build the famp binary");
    assert!(status.success(), "cargo build -p famp --bin famp failed");
}

/// Spawn a bare `famp broker --socket <path>` daemon subprocess,
/// ChildGuard-wrapped so a panicking test still reaps it.
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

/// Spawn ONE `famp-gateway --socket <path> alice bob` subprocess backing
/// BOTH principals (GW-04 requires a single gateway process backing 2+
/// principals), ChildGuard-wrapped.
fn spawn_gateway_subprocess(sock: &Path, names: &[&str]) -> ChildGuard {
    ChildGuard::new(
        Command::cargo_bin("famp-gateway")
            .unwrap()
            .arg("--socket")
            .arg(sock)
            .args(names)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

/// Poll (bounded by `deadline`) until the broker's UDS socket accepts a
/// raw connection — see `tests/liveness.rs` for why this matters
/// (`connect_no_spawn` makes exactly one attempt, no retry).
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

fn live_identity_names(sock: &Path) -> Option<Vec<String>> {
    let output = Command::cargo_bin("famp")
        .ok()?
        .env("FAMP_BUS_SOCKET", sock)
        .args(["inspect", "identities", "--json"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let list: IdentityListReply = serde_json::from_slice(&output.stdout).ok()?;
    Some(list.rows.into_iter().map(|r| r.name).collect())
}

/// Poll until every name in `expect_live` is present among live
/// identities. Bounded deadline, never a fixed sleep-then-assert.
fn poll_until_all_live(sock: &Path, expect_live: &[&str], deadline: Duration) {
    let start = Instant::now();
    loop {
        if let Some(live) = live_identity_names(sock) {
            if expect_live.iter().all(|n| live.iter().any(|l| l == n)) {
                return;
            }
        }
        assert!(
            start.elapsed() <= deadline,
            "timed out after {deadline:?} waiting for {expect_live:?} to appear live"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Every `task_id` currently visible in `to`'s mailbox via
/// `famp inspect messages --to <name> --json`, or `None` on a transient
/// inspect failure (broker not up / budget exceeded) so callers can poll.
fn messages_task_ids(sock: &Path, to: &str) -> Option<Vec<String>> {
    let output = Command::cargo_bin("famp")
        .ok()?
        .env("FAMP_BUS_SOCKET", sock)
        .args(["inspect", "messages", "--to", to, "--json"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let reply: InspectMessagesReply = serde_json::from_slice(&output.stdout).ok()?;
    match reply {
        InspectMessagesReply::List(list) => {
            Some(list.rows.into_iter().map(|r| r.task_id).collect())
        }
        InspectMessagesReply::BudgetExceeded { .. } => None,
    }
}

/// Poll (bounded by `deadline`) until `task_id` appears in `to`'s
/// mailbox — the positive side of the isolation assertion.
fn poll_until_task_id_present(sock: &Path, to: &str, task_id: &str, deadline: Duration) {
    let start = Instant::now();
    loop {
        if let Some(ids) = messages_task_ids(sock, to) {
            if ids.iter().any(|id| id == task_id) {
                return;
            }
        }
        assert!(
            start.elapsed() <= deadline,
            "timed out after {deadline:?} waiting for task_id {task_id} to appear in {to}'s mailbox"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[tokio::test]
async fn gw04_no_cross_talk_between_proxied_principals() {
    ensure_famp_bin_built();

    let tmp = tempfile::TempDir::new().unwrap();
    let sock = tmp.path().join("bus.sock");

    let _broker = spawn_broker_subprocess(&sock);
    wait_for_broker_socket(&sock, Duration::from_secs(5));
    // ONE gateway process backs BOTH alice and bob.
    let _gateway = spawn_gateway_subprocess(&sock, &["alice", "bob"]);
    poll_until_all_live(&sock, &["alice", "bob"], Duration::from_secs(5));

    // Sender: a D-10 `bind_as = Some("bob")` proxy connection onto the
    // gateway's own live "bob" registration -- the same mechanism
    // `famp send --as bob` uses. bob sends a uniquely-tagged message to
    // alice.
    let tag = uuid::Uuid::now_v7();
    let mut sender = BusClient::connect(&sock, Some("bob".into()))
        .await
        .expect("bob proxy connect");
    let envelope = serde_json::json!({
        "id": tag.to_string(),
        "body": { "event": "famp.send.new_task" },
    });
    let reply = sender
        .send_recv(BusMessage::Send {
            to: Target::Agent {
                name: "alice".into(),
            },
            envelope,
        })
        .await
        .expect("send_recv");
    match reply {
        BusReply::SendOk { task_id, .. } => {
            assert_eq!(
                task_id.to_string(),
                tag.to_string(),
                "SendOk task_id must echo the tagged envelope id"
            );
        }
        other => panic!("expected SendOk, got {other:?}"),
    }
    sender.shutdown().await;

    // Positive side (bounded-deadline poll): the tagged message must
    // land in alice's mailbox.
    poll_until_task_id_present(&sock, "alice", &tag.to_string(), Duration::from_secs(5));

    // Negative side: now that the positive side has landed, bob's
    // mailbox must NOT contain it -- checked directly, nothing left to
    // wait for.
    let bob_ids = messages_task_ids(&sock, "bob").unwrap_or_default();
    assert!(
        !bob_ids.contains(&tag.to_string()),
        "message addressed to alice MUST NOT appear in bob's mailbox (GW-04): bob_ids={bob_ids:?}"
    );
}

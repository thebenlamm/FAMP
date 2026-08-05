//! Phase 17 Plan 06 (REACH-05) — proves the failure surface `egress.rs`'s
//! `notify_relay_failure` adds: "a reachability failure (relay down, peer
//! offline, connection refused) surfaces at the SENDER as a distinct,
//! actionable error, never as a silent fire-and-forget success."
//!
//! The peer gateway is DELIBERATELY NEVER STARTED — `bob`'s `--peer`
//! points at a free loopback port nothing is listening on, so every
//! relay attempt hits a genuine connection-refused, not a simulated
//! error. The assertion is on `alice`'s own local mailbox contents
//! (`famp inbox list --as alice`), not on any gateway log line — a log
//! line the sender never reads is precisely the gap REACH-05 exists to
//! close.
//!
//! ## Negative control — the success path stays green
//!
//! This file does not stand up a second, successfully-relaying gateway
//! pair to prove "a message that relays successfully does NOT produce a
//! failure ack" — doing so here would duplicate the two-gateway,
//! mutual-TOFU-trust topology `e2e_cross_host_delivery.rs` already
//! stands up and exercises end-to-end. That file (its
//! `gw01_gw02_gw03_two_process_cross_host_delivery` test, asserted green
//! in this plan's own `<verification>`) IS the named success control: a
//! full request/commit/deliver/ack cycle completing and converging on
//! `COMPLETED` on both sides is only possible if no relay attempt in
//! that cycle ever failed and no failure-ack notification ever
//! interfered with it. A control that is named and located here is
//! sufficient; a control that is merely assumed is not.

#![allow(unused_crate_dependencies)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::similar_names
)]

use std::net::SocketAddr;
use std::path::Path;
use std::time::{Duration, Instant};

use famp::{AuthorityScope, FampSigningKey, MessageId, Principal, Timestamp, UnsignedEnvelope};
use famp_bus::{BusMessage, BusReply, Target};
use famp_envelope::body::{Bounds, Budget, RequestBody};
use famp_envelope::BodySchema;
use famp_gateway::egress::RELAY_FAILURE_REASON_PREFIX;
use famp_keyring::Keyring;

#[path = "common/gateway_harness.rs"]
mod gateway_harness;
use gateway_harness::{
    ensure_famp_bin_built, famp_cmd, pick_free_port, spawn_broker, spawn_gateway, spawn_register,
    wait_for_broker_socket, wait_for_https, wait_until_live, ChildGuard, Side, ALICE, ALICE_DOMAIN,
    BOB, BOB_DOMAIN, POLL_DEADLINE, STARTUP_DEADLINE,
};

// ---------------------------------------------------------------------
// Envelope construction — mirrors `e2e_cross_host_delivery.rs`'s own
// typed-envelope + sign-then-strip helpers (each e2e-style file in this
// package owns a small local copy rather than sharing one — there is no
// existing shared module for it).
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

/// Live, not fixed — this envelope flows through a real `famp-gateway`
/// subprocess's egress drain loop, and there is no ingress freshness
/// gate on THIS path (egress only reads the drained `ts` back out, it
/// does not enforce a skew window), but a live timestamp keeps this test
/// consistent with the rest of the package's envelope-construction
/// convention (`e2e_cross_host_delivery.rs::ts`).
fn ts() -> Timestamp {
    Timestamp(famp_gateway::now_canonical_utc())
}

/// Sign an `UnsignedEnvelope<B>` with a throwaway key then strip the
/// `signature` field — the sanctioned pattern for producing a
/// BUS-11-compliant unsigned `Value` from the typed builder API, used
/// identically by `egress.rs`'s own unit tests and
/// `e2e_cross_host_delivery.rs`.
fn unsigned_value<B: BodySchema>(env: UnsignedEnvelope<B>) -> serde_json::Value {
    let dummy_sk = FampSigningKey::from_bytes([77u8; 32]);
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
        scope: serde_json::json!({"task": "relay-failure-surface"}),
        bounds: two_key_bounds(),
        natural_language_summary: Some("this peer will never answer".to_string()),
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

/// Send `envelope` onto the local bus at `sock`, proxying through the
/// `bind_as` canonical holder's connection to `Target::Agent { name:
/// to_name }` — identical shape to
/// `e2e_cross_host_delivery.rs::send_bus_envelope`.
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

/// Poll (bounded) `famp inbox list --as alice` until a `class == "ack"`
/// line whose body carries the failed disposition appears, returning
/// text containing its `reason`. Mirrors
/// `gateway_harness::poll_inbox_contains`'s poll-with-deadline shape
/// (never a fixed `sleep()`-then-assert), with a different match
/// predicate — the failure ack mints a fresh id unrelated to the
/// original request's, so id/causality matching (`poll_inbox_contains`'s
/// own predicate) does not apply here.
///
/// The gateway's own backed stand-in connections are registered with
/// `Origin::Gateway` (`principal.rs`, Phase 14 D-01/D-17) — including
/// `bob`'s connection, the one this notification is sent on
/// (`egress.rs::notify_relay_failure`'s doc explains why it must be).
/// `famp inbox list` is one of Phase 14's five quarantine-tagging
/// rendering surfaces, so it renders THIS envelope's `body` as a
/// quarantine-wrapped STRING (`famp::cli::render::render_envelope_body`)
/// rather than a raw JSON object — `body.disposition`/`body.reason` are
/// therefore substrings of that wrapped text, not JSON fields, on this
/// surface. Still human-readable and still grep-able (the must-have this
/// plan exists to satisfy), just wrapped; this poller reads it back out
/// as wrapped text rather than asserting a JSON shape the CLI does not
/// actually produce for gateway-origin content.
fn poll_for_failure_ack(sock: &Path, home: &Path, as_name: &str, deadline: Duration) -> String {
    let start = Instant::now();
    loop {
        let out = famp_cmd(sock, home, &["inbox", "list", "--as", as_name]);
        if out.status.success() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            for line in stdout.lines() {
                let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                    continue;
                };
                // Phase 14 D-04/D-05: `famp inbox list` JSONL lines are
                // `{"origin": ..., "envelope": {...}}`.
                let env = v.get("envelope").unwrap_or(&v);
                if env.get("class").and_then(serde_json::Value::as_str) != Some("ack") {
                    continue;
                }
                let Some(body) = env.get("body") else {
                    continue;
                };
                // Non-quarantined shape (would apply if origin were ever
                // `Local`): a plain JSON object with `disposition`/`reason`.
                if let Some(reason) = body.get("reason").and_then(serde_json::Value::as_str) {
                    if body.get("disposition").and_then(serde_json::Value::as_str) == Some("failed")
                    {
                        return reason.to_string();
                    }
                }
                // Quarantined shape (the actual shape on this surface for
                // gateway-origin content): `body` is a STRING containing
                // the quarantine-wrapped JSON text.
                if let Some(wrapped) = body.as_str() {
                    if wrapped.contains("\"disposition\":\"failed\"")
                        && wrapped.contains(RELAY_FAILURE_REASON_PREFIX)
                    {
                        return wrapped.to_string();
                    }
                }
            }
            assert!(
                start.elapsed() <= deadline,
                "timed out after {deadline:?} waiting for a failed-disposition ack in \
                 {as_name}'s inbox on {}; last stdout: {stdout}",
                sock.display()
            );
        } else {
            assert!(
                start.elapsed() <= deadline,
                "timed out after {deadline:?} waiting for a failed-disposition ack in \
                 {as_name}'s inbox on {} (last `inbox list` call failed)",
                sock.display()
            );
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// This test never bootstraps mutual TOFU trust (no `peer export`/
/// `peer import`) — the gateway under test never needs to verify an
/// INBOUND envelope (nothing ever connects to it), only to attempt an
/// OUTBOUND relay that fails. But `main.rs` unconditionally loads
/// `$FAMP_HOME/gateway/peers.keyring` at startup regardless of whether
/// ingress is ever exercised, so an empty-but-present keyring file must
/// exist or the gateway subprocess exits immediately. Write one directly
/// via the same `famp_keyring::Keyring` API `famp peer import` itself
/// calls, rather than spinning up a peer just to produce this file as a
/// side effect.
fn init_empty_peers_keyring(home: &Path) {
    let dir = home.join("gateway");
    std::fs::create_dir_all(&dir).expect("create gateway dir");
    Keyring::new()
        .save_to_file(&dir.join("peers.keyring"))
        .expect("save empty peers keyring");
}

#[test]
fn reach05_failure_notification_surfaces_at_sender_when_peer_never_started() {
    ensure_famp_bin_built();

    let side = Side::new();
    init_empty_peers_keyring(side.home());

    // Every spawned subprocess is `ChildGuard`-wrapped (project convention,
    // 07-RESEARCH.md Pitfall 3) so a panicking test still reaps it.
    let _broker: ChildGuard = spawn_broker(&side);
    wait_for_broker_socket(&side.sock(), STARTUP_DEADLINE);

    let _alice_register: ChildGuard = spawn_register(&side, "alice");
    wait_until_live(&side.sock(), side.home(), "alice", STARTUP_DEADLINE);

    // Gateway backs `bob` locally and is told to relay to `hostb.test`
    // at a free loopback port nothing is bound to — bind-then-drop
    // (`pick_free_port`) guarantees connection-refused, a genuine
    // unreachable peer, never a simulated error. This gateway's own
    // listener/trust-cert values don't matter for this test (nothing
    // ever connects TO it); reused fixture stubs from
    // `e2e_cross_host_delivery.rs`'s topology.
    let listen_port = pick_free_port();
    let dead_peer_port = pick_free_port();
    // 17-03/D-30: own_domain is now REQUIRED — this gateway fronts REAL
    // alice's side (own_cert_stub "alice"), so its own-domain is
    // ALICE_DOMAIN, mirroring e2e_cross_host_delivery.rs's gateway A.
    let _gateway: ChildGuard = spawn_gateway(
        &side,
        "bob",
        listen_port,
        "alice",
        BOB_DOMAIN,
        dead_peer_port,
        "bob",
        ALICE_DOMAIN,
    );
    wait_until_live(&side.sock(), side.home(), "bob", STARTUP_DEADLINE);
    wait_for_https(
        SocketAddr::from(([127, 0, 0, 1], listen_port)),
        "alice",
        STARTUP_DEADLINE,
    );

    let alice: Principal = ALICE.parse().unwrap();
    let bob: Principal = BOB.parse().unwrap();

    // alice -> bob: a REQUEST that will drain into the gateway's `bob`
    // stand-in mailbox, be picked up by `run_egress`, and fail to relay
    // — the peer was never started.
    let request = build_request(MessageId::new_v7(), &alice, &bob);
    send_bus_envelope(&side.sock(), "alice", "bob", request);

    // The peer is NEVER started at any point in this test — the
    // assertion below is on alice's own mailbox, not a gateway log line.
    // `rendered` is the quarantine-wrapped text `poll_for_failure_ack`
    // returns (see its own doc comment) — the underlying `reason` field
    // is a substring of it, not the whole string, so the checks below
    // assert containment rather than a strict prefix match.
    let rendered = poll_for_failure_ack(&side.sock(), side.home(), "alice", POLL_DEADLINE);

    assert!(
        rendered.contains(RELAY_FAILURE_REASON_PREFIX),
        "failure-ack must carry the greppable prefix, got: {rendered}"
    );
    assert!(
        rendered.contains(&bob.to_string()),
        "failure-ack must name the intended (unreachable) recipient bob, got: {rendered}"
    );
}

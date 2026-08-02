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
//! `DeliverBody`/`AckBody`) directly via
//! `UnsignedEnvelope<B>`, sent unsigned onto the local bus (BUS-11) via
//! the sanctioned "sign-then-strip" pattern already used by
//! `famp-gateway`'s own `egress.rs` unit tests and
//! `famp/tests/common/cycle_driver.rs`. These are federation-crossable
//! (their bodies satisfy each class's strict `deny_unknown_fields` schema,
//! so `verify_inbound_any`'s typed decode at gateway ingress accepts
//! them). GW-03 now asserts that the inspector mirrors `famp-fsm::TaskFsm`
//! exactly: the terminal state comes from the deliver envelope's top-level
//! `terminal_status` header (completed/failed), not from control/cancel
//! envelopes.
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

use std::net::SocketAddr;
use std::path::Path;

use famp::{AuthorityScope, FampSigningKey, MessageId, Principal, TerminalStatus, Timestamp};
use famp_bus::{BusMessage, BusReply, Target};
use famp_envelope::body::{
    AckBody, AckDisposition, Bounds, Budget, CommitBody, DeliverBody, RequestBody,
};
use famp_envelope::{BodySchema, Causality, Relation, UnsignedEnvelope};

// 11-04 Task 2: the two-host subprocess harness (Side, famp_cmd,
// spawn_broker/register/gateway, wait_for_*/poll_* pollers, ALICE/BOB
// domain constants) was mechanically extracted into
// `common/gateway_harness.rs` so `e2e_shipping_surface.rs` can reuse it
// without copy-paste. No behavioral change — see gateway_harness.rs's
// module doc.
#[path = "common/gateway_harness.rs"]
mod gateway_harness;
use gateway_harness::{
    ensure_famp_bin_built, peer_export, peer_import, pick_free_port, poll_inbox_contains,
    poll_terminal_state, spawn_broker, spawn_gateway, spawn_register, wait_for_broker_socket,
    wait_for_tcp, wait_until_live, Side, ALICE, ALICE_DOMAIN, BOB, BOB_DOMAIN, POLL_DEADLINE,
    STARTUP_DEADLINE,
};

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

/// Live, not fixed: Phase 17's freshness gate (`ingress_guard::run_cheap_gates`,
/// INGR-01) rejects any envelope whose `ts` is more than `CLOCK_SKEW_WINDOW_SECS`
/// away from the real wall clock, and this file's envelopes flow through a real
/// `inbox_handler` on a real running gateway — a fixed literal here goes stale
/// the moment it ages past that window (it did: a hardcoded 2026-07-27 value
/// silently broke this test days later). Mirrors `famp_gateway::now_canonical_utc()`'s
/// exact canonical form via the same public re-export other tests already use
/// (see `revocation.rs::now_canonical_utc_shape`).
fn ts() -> Timestamp {
    Timestamp(famp_gateway::now_canonical_utc())
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
    wait_for_broker_socket(&side_a.sock(), STARTUP_DEADLINE);
    wait_for_broker_socket(&side_b.sock(), STARTUP_DEADLINE);

    // Real identities: alice on A, bob on B.
    let _alice_register = spawn_register(&side_a, "alice");
    let _bob_register = spawn_register(&side_b, "bob");
    wait_until_live(&side_a.sock(), side_a.home(), "alice", STARTUP_DEADLINE);
    wait_until_live(&side_b.sock(), side_b.home(), "bob", STARTUP_DEADLINE);

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

    // 17-03/D-30: own_domain is now REQUIRED on every gateway spawn —
    // gateway A (fronting REAL alice's side) gets ALICE_DOMAIN, gateway B
    // (fronting REAL bob's side) gets BOB_DOMAIN.
    let _gateway_a = spawn_gateway(
        &side_a,
        "bob",
        port_a,
        "alice",
        BOB_DOMAIN,
        port_b,
        "bob",
        ALICE_DOMAIN,
    );
    let _gateway_b = spawn_gateway(
        &side_b,
        "alice",
        port_b,
        "bob",
        ALICE_DOMAIN,
        port_a,
        "alice",
        BOB_DOMAIN,
    );

    wait_until_live(&side_a.sock(), side_a.home(), "bob", STARTUP_DEADLINE);
    wait_until_live(&side_b.sock(), side_b.home(), "alice", STARTUP_DEADLINE);
    wait_for_tcp(SocketAddr::from(([127, 0, 0, 1], port_a)), STARTUP_DEADLINE);
    wait_for_tcp(SocketAddr::from(([127, 0, 0, 1], port_b)), STARTUP_DEADLINE);

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

    // GW-03: a full cycle completed across both machines and BOTH sides
    // converge on the SAME terminal FSM state via `famp inspect tasks
    // --id <task_id> --json`. The terminal state comes from the deliver
    // envelope's top-level `terminal_status` header (set via
    // `with_terminal_status(TerminalStatus::Completed)` in build_deliver),
    // not from any appended control/cancel envelope — the inspector now
    // mirrors `famp-fsm::TaskFsm::step` exactly.
    let state_b = poll_terminal_state(&side_b.sock(), side_b.home(), &task_id_str, POLL_DEADLINE);
    let state_a = poll_terminal_state(&side_a.sock(), side_a.home(), &task_id_str, POLL_DEADLINE);
    assert_eq!(
        state_a, state_b,
        "both sides must converge on the same terminal FSM state"
    );
    assert_eq!(state_a, "COMPLETED");
}

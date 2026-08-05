//! Phase 17 Plan 05 Task 2 — the REACH-04 loopback proof: two
//! `famp-gateway` processes, neither one configured with the other's
//! address anywhere, exchange envelopes bidirectionally through a real
//! `famp-relay`, and both task FSMs reach a terminal state.
//!
//! ## Topology
//!
//! Side A runs broker A, the REAL `alice` identity, and gateway A, which
//! backs the bare name `bob` as a local stand-in for the remote `bob`.
//! Side B is symmetric (broker B, REAL `bob`, gateway B backing `alice`).
//! A third process — `famp-relay` — sits between them. Gateway A's
//! `--peer hostb.test=<relay-url>` and `--relay-fetch <relay-url>` both
//! point at the RELAY, never at gateway B's own `--listen` address; B is
//! symmetric. **The negative property this test exists to prove: neither
//! gateway's argument vector contains the other gateway's listen
//! address anywhere — the ONLY path between them is the relay.** A
//! passing test therefore demonstrates a working bidirectional path
//! under the Phase 13-decided reachability model, rather than merely
//! re-proving direct delivery (which `e2e_cross_host_delivery.rs` already
//! does and remains the regression control for the direct-peer path).
//!
//! Egress needs zero new code for this topology: `--peer <domain>=<url>`
//! already reaches the relay's enqueue route exactly like a direct peer
//! (both are mounted on `famp_transport_http::INBOX_ROUTE`). The inbound
//! half is what plan 05 built: `--relay-fetch <relay-url>` drives
//! `famp_gateway::relay_fetch::run_relay_fetch`, which polls the relay,
//! authorizes each drain with a signature over the gateway's own already
//! -loaded identity key (D-26), and hands every fetched envelope to the
//! SAME `ingress::ingest_inbound` core the direct HTTPS path uses.
//!
//! ## What this test does NOT prove
//!
//! Both processes run on loopback, on the SAME host. Genuine
//! cross-network NAT traversal remains UNPROVEN, and REACH-02 (real
//! symmetric-NAT validation, blocked on Ben's carrier hotspot) stays
//! explicitly OPEN regardless of this test's outcome. This is carried
//! forward as a clearly-marked pending item, the same pattern
//! `crates/famp-gateway/tests/gateway_usage_doc_accuracy.rs` and Phase
//! 10's DOC-04 precedent established — never silently implied closed.
//!
//! ## Harness patterns reused
//!
//! Built directly on `e2e_cross_host_delivery.rs`'s shape: two `Side`s
//! with their own isolated brokers, real `alice`/`bob` identities, mutual
//! TOFU via `peer_export`/`peer_import`, and typed envelopes constructed
//! directly (bypassing `famp send`, whose CLI always emits
//! `class: "audit_log"` — see that file's module doc for the full
//! rationale). `gateway_harness.rs::spawn_gateway` is NOT reused here:
//! its fixed shape always points `--peer` at the OTHER gateway's own
//! port, which is exactly the direct-address wiring this test must NOT
//! produce — this file builds its own gateway argument vector instead,
//! pointing every relay-shaped flag at the relay's URL.

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
use std::process::{Command, Stdio};

use assert_cmd::cargo::CommandCargoExt;
use famp::{AuthorityScope, FampSigningKey, MessageId, Principal, TerminalStatus, Timestamp};
use famp_bus::{BusMessage, BusReply, Target};
use famp_envelope::body::{
    AckBody, AckDisposition, Bounds, Budget, CommitBody, DeliverBody, RequestBody,
};
use famp_envelope::{BodySchema, Causality, Relation, UnsignedEnvelope};

#[path = "common/gateway_harness.rs"]
mod gateway_harness;
use gateway_harness::{
    cross_machine_fixture_dir, ensure_famp_bin_built, peer_export, peer_import, pick_free_port,
    poll_inbox_contains, poll_terminal_state, spawn_broker, spawn_register, wait_for_broker_socket,
    wait_for_https, wait_until_live, ChildGuard, Side, ALICE, ALICE_DOMAIN, BOB, BOB_DOMAIN,
    POLL_DEADLINE, STARTUP_DEADLINE,
};

/// `famp-relay`'s own `[[bin]]` is a SIBLING package to `famp-gateway`,
/// not a dependency of it — `cargo test -p famp-gateway` never builds it
/// as a side effect, unlike `famp-gateway`'s own bin. Explicit build
/// step, mirroring `gateway_harness::ensure_famp_bin_built`'s identical
/// rationale for the `famp` binary.
fn ensure_famp_relay_bin_built() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let status = Command::new(cargo)
        .args([
            "build",
            "--quiet",
            "-p",
            "famp-relay",
            "--bin",
            "famp-relay",
        ])
        .status()
        .expect("failed to invoke cargo to build the famp-relay binary");
    assert!(
        status.success(),
        "cargo build -p famp-relay --bin famp-relay failed"
    );
}

/// The second whitespace-separated field of a `famp peer export` line —
/// the base64url public key (`format_export_line`, `crates/famp/src/cli/
/// peer/export.rs`). Used here exactly as an operator would per D-27: the
/// peer runs `famp peer export`, and the resulting SECOND field is what
/// gets handed to the relay operator's `--domain <domain>=<pubkey>`.
fn extract_pubkey_field(export_line: &str) -> &str {
    export_line
        .split_whitespace()
        .nth(1)
        .expect("a famp peer export line must have a pubkey as its second field")
}

/// Build one gateway's full argument vector for the relay topology.
/// Every relay-shaped flag (`--peer`, `--relay-fetch`) points at
/// `relay_url` — NEVER at the other gateway's own `--listen` address,
/// which this vector never contains anywhere.
fn gateway_args_via_relay(
    side: &Side,
    backed_name: &str,
    listen_port: u16,
    own_cert_stub: &str,
    peer_domain: &str,
    relay_url: &str,
    trust_cert_stub: &str,
) -> Vec<String> {
    let fixtures = cross_machine_fixture_dir();
    vec![
        "--socket".to_owned(),
        side.sock().display().to_string(),
        "--listen".to_owned(),
        format!("127.0.0.1:{listen_port}"),
        "--tls-cert".to_owned(),
        fixtures
            .join(format!("{own_cert_stub}.crt"))
            .display()
            .to_string(),
        "--tls-key".to_owned(),
        fixtures
            .join(format!("{own_cert_stub}.key"))
            .display()
            .to_string(),
        "--backs".to_owned(),
        format!("agent:{peer_domain}/{backed_name}"),
        "--peer".to_owned(),
        format!("{peer_domain}={relay_url}"),
        "--relay-fetch".to_owned(),
        relay_url.to_owned(),
        "--trust-cert".to_owned(),
        fixtures
            .join(format!("{trust_cert_stub}.crt"))
            .display()
            .to_string(),
        backed_name.to_owned(),
    ]
}

/// Spawn `famp-gateway` with an already-built argument vector (see
/// [`gateway_args_via_relay`]) — `own_domain` is still mandatory and
/// unconditionally startup-fatal (17-03/D-30), set via `FAMP_OWN_DOMAIN`
/// exactly like `gateway_harness::spawn_gateway`.
fn spawn_gateway_with_args(side: &Side, own_domain: &str, args: &[String]) -> ChildGuard {
    ChildGuard::new(
        Command::cargo_bin("famp-gateway")
            .unwrap()
            .args(args)
            .env("FAMP_HOME", side.home())
            .env("FAMP_OWN_DOMAIN", own_domain)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    )
}

// ---------------------------------------------------------------------
// Envelope construction — identical shape/rationale to
// e2e_cross_host_delivery.rs (real typed classes, unsigned via the
// sanctioned sign-then-strip pattern, sent directly onto the local bus).
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

/// Live, not fixed — see `e2e_cross_host_delivery.rs::ts`'s identical
/// rationale: Phase 17's freshness gate rejects any envelope whose `ts`
/// drifts more than `CLOCK_SKEW_WINDOW_SECS` from the real wall clock,
/// and this file's envelopes flow through a real `ingest_inbound` on a
/// real running gateway (twice over, now — once at the relay-fetch
/// loop's own drain, once historically at the direct-POST path).
fn ts() -> Timestamp {
    Timestamp(famp_gateway::now_canonical_utc())
}

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
        scope: serde_json::json!({"task": "relay-bidirectional-e2e"}),
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
        scope: serde_json::json!({"task": "relay-bidirectional-e2e"}),
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
fn reach_04_bidirectional_delivery_through_relay_with_no_direct_peer_address() {
    ensure_famp_bin_built();
    ensure_famp_relay_bin_built();

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

    // Mutual TOFU trust bootstrap for the AGENT identities (alice/bob) --
    // unrelated to, and unaffected by, D-26's separate gateway-identity
    // signed-fetch mechanism below. Must happen before gateway startup:
    // the gateway loads its peers keyring once at startup, no hot-reload.
    let alice_blob = peer_export(side_a.home(), ALICE);
    peer_import(side_b.home(), &alice_blob);
    let bob_blob = peer_export(side_b.home(), BOB);
    peer_import(side_a.home(), &bob_blob);

    // D-26/D-27: each gateway's OWN identity public key, obtained via the
    // REAL operator workflow (`famp peer export`) rather than reaching
    // into the identity file -- this is the exact step D-27's follower
    // doc will instruct a relay operator to request from each peer.
    let gw_a_export = peer_export(side_a.home(), ALICE);
    let gw_a_pubkey = extract_pubkey_field(&gw_a_export).to_owned();
    let gw_b_export = peer_export(side_b.home(), BOB);
    let gw_b_pubkey = extract_pubkey_field(&gw_b_export).to_owned();

    // The relay: a third process, reusing the shared TLS fixture pair
    // ("bob.crt"/"bob.key") purely for channel encryption -- unrelated to
    // bob's own agent identity. D-27: queue ownership is explicit
    // operator config (`--domain <domain>=<pubkey>`), never TOFU.
    let relay_port = pick_free_port();
    let relay_url = format!("https://127.0.0.1:{relay_port}");
    let fixtures = cross_machine_fixture_dir();
    let _relay = ChildGuard::new(
        Command::cargo_bin("famp-relay")
            .unwrap()
            .arg("--listen")
            .arg(format!("127.0.0.1:{relay_port}"))
            .arg("--tls-cert")
            .arg(fixtures.join("bob.crt"))
            .arg("--tls-key")
            .arg(fixtures.join("bob.key"))
            .arg("--public-url")
            .arg(&relay_url)
            .arg("--domain")
            .arg(format!("{ALICE_DOMAIN}={gw_a_pubkey}"))
            .arg("--domain")
            .arg(format!("{BOB_DOMAIN}={gw_b_pubkey}"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    wait_for_https(
        SocketAddr::from(([127, 0, 0, 1], relay_port)),
        "bob",
        STARTUP_DEADLINE,
    );

    // Two gateways, each backing the OTHER side's principal as a local
    // stand-in (D-01) -- but, unlike e2e_cross_host_delivery.rs, every
    // relay-shaped flag below points at the RELAY, never at the sibling
    // gateway's own port.
    let port_a = pick_free_port();
    let port_b = pick_free_port();

    let args_a = gateway_args_via_relay(
        &side_a, "bob", port_a, "alice", BOB_DOMAIN, &relay_url, "bob",
    );
    let args_b = gateway_args_via_relay(
        &side_b,
        "alice",
        port_b,
        "bob",
        ALICE_DOMAIN,
        &relay_url,
        "bob",
    );

    // The negative property this test exists to prove: neither
    // gateway's argument vector contains the OTHER gateway's own listen
    // address anywhere -- a reviewer can read this off the assertion, not
    // only off a comment.
    let joined_a = args_a.join(" ");
    let joined_b = args_b.join(" ");
    assert!(
        joined_a.contains(relay_url.as_str()),
        "gateway A must be configured with the relay URL, got: {joined_a}"
    );
    assert!(
        !joined_a.contains(&format!(":{port_b}")),
        "gateway A's argument vector must NOT contain gateway B's own listen port, got: {joined_a}"
    );
    assert!(
        joined_b.contains(relay_url.as_str()),
        "gateway B must be configured with the relay URL, got: {joined_b}"
    );
    assert!(
        !joined_b.contains(&format!(":{port_a}")),
        "gateway B's argument vector must NOT contain gateway A's own listen port, got: {joined_b}"
    );

    // D-26: with signed-fetch, the relay half of each gateway's
    // configuration is one URL and nothing else -- assert no
    // credential-shaped value ever appears, so a future regression that
    // reintroduces a secret cannot pass silently.
    for joined in [&joined_a, &joined_b] {
        let lower = joined.to_lowercase();
        assert!(
            !lower.contains("token") && !lower.contains("secret"),
            "neither gateway's relay configuration may carry a credential-shaped value, got: \
             {joined}"
        );
    }

    let _gateway_a = spawn_gateway_with_args(&side_a, ALICE_DOMAIN, &args_a);
    let _gateway_b = spawn_gateway_with_args(&side_b, BOB_DOMAIN, &args_b);

    wait_until_live(&side_a.sock(), side_a.home(), "bob", STARTUP_DEADLINE);
    wait_until_live(&side_b.sock(), side_b.home(), "alice", STARTUP_DEADLINE);

    // --- Harness ready: two gateways, no address for each other, both
    // --- pointed only at the relay, both signed-fetch-authorized to
    // --- drain their own domain.

    let alice: Principal = ALICE.parse().unwrap();
    let bob: Principal = BOB.parse().unwrap();
    let task_id = MessageId::new_v7();
    let task_id_str = task_id.to_string();

    // 1. alice -> bob: REQUEST, relayed through the relay's enqueue
    //    route, drained by gateway B's relay-fetch loop.
    let request = build_request(task_id, &alice, &bob);
    send_bus_envelope(&side_a.sock(), "alice", "bob", request);

    poll_inbox_contains(
        &side_b.sock(),
        side_b.home(),
        "bob",
        &task_id_str,
        "request",
        POLL_DEADLINE,
    );

    // 2. bob -> alice: COMMIT -- the REVERSE direction, proving
    //    bidirectionality through the same relay.
    let commit = build_commit(task_id, &bob, &alice);
    send_bus_envelope(&side_b.sock(), "bob", "alice", commit);

    poll_inbox_contains(
        &side_a.sock(),
        side_a.home(),
        "alice",
        &task_id_str,
        "commit",
        POLL_DEADLINE,
    );

    // 3. bob -> alice: DELIVER (terminal_status=Completed; content-
    //    transparent relay through the relay path too).
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

    // 4. alice -> bob: ACK, closing the request/commit/deliver/ack cycle.
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

    // Both task FSMs reach a terminal state via `famp inspect tasks`, and
    // both sides converge on the SAME terminal state -- the REACH-04
    // must-have's proof point, now proven with no direct peer address
    // anywhere in either gateway's configuration.
    let state_b = poll_terminal_state(&side_b.sock(), side_b.home(), &task_id_str, POLL_DEADLINE);
    let state_a = poll_terminal_state(&side_a.sock(), side_a.home(), &task_id_str, POLL_DEADLINE);
    assert_eq!(
        state_a, state_b,
        "both sides must converge on the same terminal FSM state"
    );
    assert_eq!(state_a, "COMPLETED");
}

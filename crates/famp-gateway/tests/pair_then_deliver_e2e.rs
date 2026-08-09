//! G2 regression test: pair two agents and deliver a message that must
//! verify at ingress. Issue #43 shipped because `pair redeem` pinned
//! `agent:<domain>/gateway` while `send` signed from `agent:<domain>/<name>`,
//! so peers could never verify each other. Both halves were tested in
//! isolation (`pairing_e2e.rs` asserted the pins landed; `e2e_cross_host_delivery.rs`
//! delivered traffic). The seam between them — that the pinned identity
//! must match the sending identity — was untested until now.
//!
//! This test establishes trust through the REAL `famp pair` path and verifies
//! the DISCRIMINATING ASSERTION: the inviter's keyring must hold the redeemer
//! under the EXACT principal that egress will sign from. On revert of
//! `cli/pair/redeem.rs`'s `--as` fix, pairing pins `agent:<domain>/gateway`
//! while send builds `from = agent:<domain>/<redeemed-name>`, so this assertion
//! fails in ~2s (manifesting the P1 bug). The full delivery sequence follows,
//! proving that once pinning is correct, message delivery and ingress
//! verification work end-to-end.
//!
//! The test uses the DIRECT-PEER topology (one gateway's egress POSTs
//! directly to the other gateway's ingress), NOT the relay, because the
//! relay would require `peer_export`/`peer_import` for the gateway
//! identities — reintroducing the exact bootstrap this test is designed
//! to eliminate. The tradeoff is a one-directional delivery (redeemer→inviter)
//! instead of bidirectional, which is acceptable per G2's specification.
//!
//! ## Pairing sequence
//!
//! Side A (inviter) calls `pair invite --as agent:hosta.test/alice`.
//! Side B (redeemer) calls `pair redeem --as bob --from <A's invite URL>`.
//! Side A calls `pair status` to complete the two-phase pin (PAIR-07).
//! Both sides' keyrings now hold the other as `Active`.
//!
//! ## Delivery sequence
//!
//! Both sides start brokers, register alice/bob, start gateways.
//! Gateway A backs the remote `bob` proxy; gateway B backs the remote `alice` proxy.
//! Bob (side B) sends an envelope to alice; it egresses through gateway B,
//! travels via HTTPS to gateway A's ingress, ingress verifies the signature
//! against the pinned key, and delivers to alice's mailbox.

#![allow(unused_crate_dependencies)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::too_many_arguments
)]

use std::io::Cursor;
use std::net::SocketAddr;

use famp::cli::pair::{invite, redeem, status};
use famp::{AuthorityScope, FampSigningKey, MessageId, Principal, TerminalStatus, Timestamp};
use famp_bus::{BusMessage, BusReply, Target};
use famp_envelope::body::{Bounds, Budget, DeliverBody, RequestBody};
use famp_envelope::{BodySchema, Causality, Relation, UnsignedEnvelope};
use rand::{rngs::StdRng, SeedableRng};

#[path = "common/gateway_harness.rs"]
mod gateway_harness;
use gateway_harness::{
    ensure_famp_bin_built, peer_export, peer_import, pick_free_port, poll_inbox_contains,
    spawn_broker, spawn_gateway, spawn_register, wait_for_broker_socket, wait_for_https,
    wait_until_live, Side, ALICE, ALICE_DOMAIN, BOB, BOB_DOMAIN, POLL_DEADLINE, STARTUP_DEADLINE,
};

// -----
// Pairing helpers, modeled on pairing_e2e.rs
// -----

/// Set own-domain via file so parallel tests never race a process-global
/// env mutation.
fn set_own_domain(home: &std::path::Path, domain: &str) {
    std::fs::write(home.join("own-domain"), format!("{domain}\n")).unwrap();
}

/// Draw an invite code from side A's inviter, using the explicit `--as`
/// principal so we know which principal will be pinned.
fn draw_invite(home: &std::path::Path, as_principal: &str, now: &str, seed: u64) -> String {
    let mut artifact = Vec::new();
    let mut rng = StdRng::seed_from_u64(seed);
    invite::run_at(
        home,
        &invite::PairInviteArgs {
            as_principal: as_principal.to_string(),
            url: None,
            confirm_installed: true,
        },
        &mut artifact,
        now,
        &mut rng,
    )
    .expect("invite::run_at must succeed");
    let printed = String::from_utf8(artifact).unwrap();
    printed.trim_end().lines().last().unwrap().to_string()
}

/// Build the TLS-preconfigured `reqwest::Client` the SAME way
/// `cli::pair::redeem`'s own (private) `build_client` does.
fn stub_client() -> reqwest::Client {
    let tls = famp_transport_http::tls::build_client_config(None).unwrap();
    reqwest::Client::builder()
        .use_preconfigured_tls(tls)
        .build()
        .unwrap()
}

/// Spawn the pairing router on an ephemeral loopback TCP port.
/// Must be called from an async context (inside a tokio runtime).
async fn spawn_pairing_server_async(home: &std::path::Path, own_domain: &str) -> String {
    use famp::pairing::invite::pairing_store_path;
    use famp_gateway::pairing_ingress::PairingIngressState;

    let state = PairingIngressState::new(
        std::sync::Arc::new(pairing_store_path(home)),
        std::sync::Arc::new(famp::cli::peer::identity::gateway_identity_path(home)),
        std::sync::Arc::from(own_domain),
    );
    let router = famp_gateway::pairing_ingress::build_pairing_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral loopback port");
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{addr}")
}

// -----
// Envelope construction, modeled on e2e_cross_host_delivery.rs
// -----

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
        scope: serde_json::json!({"task": "pair-then-deliver-e2e"}),
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

fn send_bus_envelope(
    sock: &std::path::Path,
    bind_as: &str,
    to_name: &str,
    envelope: serde_json::Value,
) {
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

// -----
// Main test
// -----

#[test]
fn pair_then_deliver_signature_verification() {
    ensure_famp_bin_built();

    // Two sides: A (inviter), B (redeemer). The Same homes will be used for
    // both pairing AND gateways, so the signing key identity is shared.
    let side_a = Side::new();
    let side_b = Side::new();

    set_own_domain(side_a.home(), ALICE_DOMAIN);
    set_own_domain(side_b.home(), BOB_DOMAIN);

    // --- Phase 1: Pairing (in-process, before spawning brokers/gateways)
    //
    // The invite and redeem operations load/generate signing keys from
    // gateway_identity_path(home), which is also where the gateways will
    // load their keys later. So the key pair uses is the same key the
    // gateways will sign with.

    let invite_principal = ALICE; // agent:hosta.test/alice
    let code = draw_invite(side_a.home(), invite_principal, "2030-08-03T00:00:00Z", 1);

    // Create a tokio runtime for the entire pairing phase
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        // Spawn the pairing server
        let pairing_url = spawn_pairing_server_async(side_a.home(), ALICE_DOMAIN).await;

        // B redeems as "bob" (bare leaf name).
        let client = stub_client();
        let mut reader = Cursor::new(format!("{code}\n").into_bytes());
        redeem::run_at(
            side_b.home(),
            &redeem::PairRedeemArgs {
                confirm_key_change: false,
                from: pairing_url,
                as_identity: "bob".to_string(), // This is the KEY: bob, not gateway
                trust_cert: None,
            },
            &mut reader,
            &client,
            "2030-08-03T00:05:00Z",
        )
        .await
        .expect("redeem::run_at must succeed");
    });

    // A completes the pin via `famp pair status`.
    let mut status_out = Vec::new();
    status::run_at(
        side_a.home(),
        &mut status_out,
        "2030-08-03T00:10:00Z",
        false,
    )
    .expect("status::run_at must pin the Redeemed record");

    // Both sides' keyrings must hold the other's key with the CORRECT principal.
    // This is the SEAM PROOF for G2: pairing pins a principal, send builds the
    // `from` field with a principal, and they must match or ingress rejects as
    // `UnpinnedKey`. With the revert, inviter pins `agent:hostb.test/gateway`
    // instead of `agent:hostb.test/bob` (the actual send identity), so this
    // assertion fails in ~2s with a clear message — manifesting the bug in 0s
    // vs. the poll_inbox_contains timeout that would occur at delivery.
    let inviter_principal: Principal = invite_principal.parse().unwrap();
    let redeemer_principal: Principal = BOB.parse().unwrap();

    let redeemer_keyring = famp_keyring::Keyring::load_from_file(
        &famp::cli::peer::identity::gateway_peers_keyring_path(side_b.home()),
    )
    .unwrap();
    assert!(
        redeemer_keyring.get(&inviter_principal).is_some(),
        "redeemer's keyring must have inviter's key"
    );

    let inviter_keyring = famp_keyring::Keyring::load_from_file(
        &famp::cli::peer::identity::gateway_peers_keyring_path(side_a.home()),
    )
    .unwrap();
    assert!(
        inviter_keyring.get(&redeemer_principal).is_some(),
        "inviter's keyring must hold `agent:hostb.test/bob` — the EXACT principal \
         that egress will send as (see send/mod.rs:679). With the revert, redeem \
         pins `agent:hostb.test/gateway` instead, so paired peers never verify \
         each other (issue #43), and this assertion fails."
    );

    // --- Phase 2: Delivery (brokers + agents + gateways on the same homes)

    let _broker_a = spawn_broker(&side_a);
    let _broker_b = spawn_broker(&side_b);
    wait_for_broker_socket(&side_a.sock(), STARTUP_DEADLINE);
    wait_for_broker_socket(&side_b.sock(), STARTUP_DEADLINE);

    // Real identities.
    let _alice_register = spawn_register(&side_a, "alice");
    let _bob_register = spawn_register(&side_b, "bob");
    wait_until_live(&side_a.sock(), side_a.home(), "alice", STARTUP_DEADLINE);
    wait_until_live(&side_b.sock(), side_b.home(), "bob", STARTUP_DEADLINE);

    // Agent-level trust bootstrap: alice and bob export their keys and
    // import each other's. This is separate from the gateway pairing —
    // the gateways have ALREADY pinned each other's keys via the pair
    // protocol.
    let alice_blob = peer_export(side_a.home(), ALICE);
    peer_import(side_b.home(), &alice_blob);
    let bob_blob = peer_export(side_b.home(), BOB);
    peer_import(side_a.home(), &bob_blob);

    // Gateways: A backs the remote bob proxy, B backs the remote alice proxy.
    // Both gateways use the SAME home directories where pairing happened,
    // so they load the SAME signing keys that pairing established trust with.
    let port_a = pick_free_port();
    let port_b = pick_free_port();

    let _gateway_a = spawn_gateway(
        &side_a,
        "bob", // local proxy name for remote bob
        port_a,
        "alice",      // TLS cert stub (fixture)
        BOB_DOMAIN,   // remote domain (bob's domain)
        port_b,       // peer gateway port (where bob's gateway listens)
        "bob",        // trust cert stub
        ALICE_DOMAIN, // own domain (alice's domain)
    );

    let _gateway_b = spawn_gateway(
        &side_b,
        "alice", // local proxy name for remote alice
        port_b,
        "bob",        // TLS cert stub
        ALICE_DOMAIN, // remote domain (alice's domain)
        port_a,       // peer gateway port (where alice's gateway listens)
        "alice",      // trust cert stub
        BOB_DOMAIN,   // own domain (bob's domain)
    );

    wait_until_live(&side_a.sock(), side_a.home(), "bob", STARTUP_DEADLINE);
    wait_until_live(&side_b.sock(), side_b.home(), "alice", STARTUP_DEADLINE);
    wait_for_https(
        SocketAddr::from(([127, 0, 0, 1], port_a)),
        "alice",
        STARTUP_DEADLINE,
    );
    wait_for_https(
        SocketAddr::from(([127, 0, 0, 1], port_b)),
        "bob",
        STARTUP_DEADLINE,
    );

    // --- Delivery: bob sends a request to alice, it egresses and ingresses,
    // --- ingress verifies the signature against the PAIRED key,
    // --- and alice receives it.

    let alice: Principal = ALICE.parse().unwrap();
    let bob: Principal = BOB.parse().unwrap();
    let task_id = MessageId::new_v7();
    let task_id_str = task_id.to_string();

    // Bob sends a REQUEST to alice through gateway B.
    let request = build_request(task_id, &bob, &alice);
    send_bus_envelope(&side_b.sock(), "bob", "alice", request);

    // This is the discriminating assertion: alice must receive the request
    // in her mailbox after gateway B signs it and gateway A's ingress
    // verifies it against the paired key. If pairing pinned the wrong
    // principal, the signature verification fails and alice never sees it
    // (poll_inbox_contains times out).
    poll_inbox_contains(
        &side_a.sock(),
        side_a.home(),
        "alice",
        &task_id_str,
        "request",
        POLL_DEADLINE,
    );

    // Deliver a response to prove the reverse path works too.
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
}

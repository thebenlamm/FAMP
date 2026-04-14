//! FAMP v0.7 Personal Runtime — same-process happy-path example.
//!
//! Two agents (`agent:local/alice`, `agent:local/bob`) exchange a full
//! `request → commit → deliver → ack` cycle over an in-process
//! `MemoryTransport`, with every message signed and verified against a
//! pre-pinned `Keyring`. Exits 0 on success; prints an ordered typed
//! trace to stdout.
//!
//! Invoke: `cargo run --example personal_two_agents`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::similar_names
)] // example, not lib

// Silence workspace `unused_crate_dependencies` for deps pulled in via famp's
// Cargo.toml that the example does not reference directly.
use axum as _;
use base64 as _;
use clap as _;
use famp_inbox as _;
use famp_transport_http as _;
use rcgen as _;
use reqwest as _;
use serde as _;
use famp_taskdir as _;
use hex as _;
use rustls as _;
use sha2 as _;
use tempfile as _;
use uuid as _;
use thiserror as _;
use time as _;
use toml as _;
use tower as _;
use tower_http as _;
use url as _;

use famp::runtime::{adapter, process_one_message, RuntimeError};
use famp_core::{AuthorityScope, MessageId, Principal};
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_envelope::body::{
    AckBody, AckDisposition, Bounds, Budget, CommitBody, DeliverBody, RequestBody, TerminalStatus,
};
use famp_envelope::{AnySignedEnvelope, SignedEnvelope, Timestamp, UnsignedEnvelope};
use famp_fsm::TaskFsm;
use famp_keyring::Keyring;
use famp_transport::{MemoryTransport, Transport, TransportMessage};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

type Trace = Arc<Mutex<Vec<String>>>;

fn log_line(trace: &Trace, line: String) {
    println!("{line}");
    trace.lock().unwrap().push(line);
}

fn ts() -> Timestamp {
    Timestamp("2026-04-13T00:00:00Z".to_string())
}

fn two_key_bounds() -> Bounds {
    Bounds {
        deadline: Some("2026-05-01T00:00:00Z".to_string()),
        budget: Some(Budget {
            amount: "100".to_string(),
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

/// Build canonical wire bytes for a `SignedEnvelope<B>` — re-parse and
/// re-canonicalize so the runtime's canonical pre-check accepts them.
fn canonical_bytes<B: famp_envelope::BodySchema>(signed: &SignedEnvelope<B>) -> Vec<u8> {
    let encoded = signed.encode().expect("encode must succeed");
    let value: serde_json::Value =
        famp_canonical::from_slice_strict(&encoded).expect("strict parse must succeed");
    famp_canonical::canonicalize(&value).expect("canonicalize must succeed")
}

fn send_signed<B: famp_envelope::BodySchema>(
    transport: &MemoryTransport,
    sender: &Principal,
    recipient: &Principal,
    signed: &SignedEnvelope<B>,
) -> impl std::future::Future<Output = ()> + Send + 'static {
    let bytes = canonical_bytes(signed);
    let msg = TransportMessage {
        sender: sender.clone(),
        recipient: recipient.clone(),
        bytes,
    };
    let transport = transport.clone();
    async move {
        transport.send(msg).await.expect("send must succeed");
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let alice = Principal::from_str("agent:local/alice")?;
    let bob = Principal::from_str("agent:local/bob")?;

    // Generate two keypairs with OsRng (rand 0.8 -> rand_core 0.6, compatible
    // with ed25519-dalek 2.2 rand_core feature).
    let mut rng = rand::rngs::OsRng;
    let alice_dalek = ed25519_dalek::SigningKey::generate(&mut rng);
    let bob_dalek = ed25519_dalek::SigningKey::generate(&mut rng);

    let alice_sk = FampSigningKey::from_bytes(alice_dalek.to_bytes());
    let bob_sk = FampSigningKey::from_bytes(bob_dalek.to_bytes());
    let alice_vk: TrustedVerifyingKey = alice_sk.verifying_key();
    let bob_vk: TrustedVerifyingKey = bob_sk.verifying_key();

    // Pre-pinned keyrings (NOT TOFU — v0.7 Personal Profile).
    let alice_keyring = Keyring::new().with_peer(bob.clone(), bob_vk.clone())?;
    let bob_keyring = Keyring::new().with_peer(alice.clone(), alice_vk.clone())?;

    let transport = MemoryTransport::new();
    transport.register(alice.clone()).await;
    transport.register(bob.clone()).await;

    let trace: Trace = Arc::new(Mutex::new(Vec::new()));

    // --- Bob's responder task ---
    let bob_task = {
        let transport = transport.clone();
        let bob_keyring = bob_keyring.clone();
        let bob_p = bob.clone();
        let alice_p = alice.clone();
        let trace = trace.clone();
        let bob_sk = FampSigningKey::from_bytes(bob_dalek.to_bytes());
        tokio::spawn(async move {
            let mut fsm = TaskFsm::new();

            // 1. Receive request
            let req_msg = transport.recv(&bob_p).await.expect("recv request");
            let req_env =
                process_one_message(&req_msg, &bob_keyring, &mut fsm).expect("request must verify");
            assert!(matches!(req_env, AnySignedEnvelope::Request(_)));
            log_line(
                &trace,
                format!(
                    "[1] {} -> {}: Request",
                    adapter::envelope_sender(&req_env),
                    bob_p
                ),
            );

            // 2. Send commit
            let commit_body = CommitBody {
                scope: serde_json::json!({"task": "translate"}),
                scope_subset: None,
                bounds: two_key_bounds(),
                accepted_policies: vec!["policy://famp/v0.7/personal".to_string()],
                delegation_permissions: None,
                reporting_obligations: None,
                terminal_condition: serde_json::json!({"type": "final_delivery"}),
                conditions: None,
                natural_language_summary: None,
            };
            let commit = UnsignedEnvelope::<CommitBody>::new(
                MessageId::new_v7(),
                bob_p.clone(),
                alice_p.clone(),
                AuthorityScope::CommitLocal,
                ts(),
                commit_body,
            )
            .sign(&bob_sk)
            .expect("sign commit");
            send_signed(&transport, &bob_p, &alice_p, &commit).await;

            // 3. Send deliver (terminal=Completed)
            let deliver_body = DeliverBody {
                interim: false,
                artifacts: None,
                result: Some(serde_json::json!({"text": "Bonjour le monde."})),
                usage_metrics: None,
                error_detail: None,
                provenance: Some(serde_json::json!({"signer": "agent:local/bob"})),
                natural_language_summary: None,
            };
            let deliver = UnsignedEnvelope::<DeliverBody>::new(
                MessageId::new_v7(),
                bob_p.clone(),
                alice_p.clone(),
                AuthorityScope::Advisory,
                ts(),
                deliver_body,
            )
            .with_terminal_status(TerminalStatus::Completed)
            .sign(&bob_sk)
            .expect("sign deliver");
            send_signed(&transport, &bob_p, &alice_p, &deliver).await;

            // 4. Receive ack from alice
            let ack_msg = transport.recv(&bob_p).await.expect("recv ack");
            let ack_env =
                process_one_message(&ack_msg, &bob_keyring, &mut fsm).expect("ack must verify");
            assert!(matches!(ack_env, AnySignedEnvelope::Ack(_)));
            // Ack is wire-only; bob does not advance its FSM here.
        })
    };

    // --- Alice's driver task ---
    let alice_task = {
        let transport = transport.clone();
        let alice_keyring = alice_keyring.clone();
        let alice_p = alice.clone();
        let bob_p = bob.clone();
        let trace = trace.clone();
        let alice_sk = FampSigningKey::from_bytes(alice_dalek.to_bytes());
        tokio::spawn(async move {
            let mut fsm = TaskFsm::new();

            // 1. Send request to bob
            let req_body = RequestBody {
                scope: serde_json::json!({"task": "translate"}),
                bounds: two_key_bounds(),
                natural_language_summary: Some("translate to french".to_string()),
            };
            let req = UnsignedEnvelope::<RequestBody>::new(
                MessageId::new_v7(),
                alice_p.clone(),
                bob_p.clone(),
                AuthorityScope::Advisory,
                ts(),
                req_body,
            )
            .sign(&alice_sk)
            .expect("sign request");
            send_signed(&transport, &alice_p, &bob_p, &req).await;

            // 2. Receive commit
            let commit_msg = transport.recv(&alice_p).await.expect("recv commit");
            let commit_env = process_one_message(&commit_msg, &alice_keyring, &mut fsm)
                .expect("commit must verify");
            assert!(matches!(commit_env, AnySignedEnvelope::Commit(_)));
            log_line(
                &trace,
                format!(
                    "[2] {} -> {}: Commit",
                    adapter::envelope_sender(&commit_env),
                    alice_p
                ),
            );

            // 3. Receive deliver
            let deliver_msg = transport.recv(&alice_p).await.expect("recv deliver");
            let deliver_env = process_one_message(&deliver_msg, &alice_keyring, &mut fsm)
                .expect("deliver must verify");
            assert!(matches!(deliver_env, AnySignedEnvelope::Deliver(_)));
            log_line(
                &trace,
                format!(
                    "[3] {} -> {}: Deliver",
                    adapter::envelope_sender(&deliver_env),
                    alice_p
                ),
            );

            // 4. Send ack back to bob
            let ack_body = AckBody {
                disposition: AckDisposition::Completed,
                reason: None,
            };
            let ack = UnsignedEnvelope::<AckBody>::new(
                MessageId::new_v7(),
                alice_p.clone(),
                bob_p.clone(),
                AuthorityScope::Advisory,
                ts(),
                ack_body,
            )
            .sign(&alice_sk)
            .expect("sign ack");
            send_signed(&transport, &alice_p, &bob_p, &ack).await;
            log_line(&trace, format!("[4] {alice_p} -> {bob_p}: Ack"));
        })
    };

    alice_task.await?;
    bob_task.await?;

    // Self-verify trace ordering.
    let final_trace = trace.lock().unwrap();
    assert_eq!(
        final_trace.len(),
        4,
        "expected 4 trace lines, got {}",
        final_trace.len()
    );
    assert!(final_trace[0].contains("Request"));
    assert!(final_trace[1].contains("Commit"));
    assert!(final_trace[2].contains("Deliver"));
    assert!(final_trace[3].contains("Ack"));
    drop(final_trace);

    // Touch RuntimeError so the import is exercised even on the happy path.
    let _: fn() = || {
        let _ = std::mem::size_of::<RuntimeError>();
    };

    println!("OK: personal_two_agents complete");
    Ok(())
}

//! Shared happy-path cycle driver — extracted from
//! `crates/famp/examples/personal_two_agents.rs` in Phase 4 Plan 04-04 so both
//! the same-process HTTP safety-net test and the HTTP example can call one
//! canonical implementation of the alice/bob halves of the signed cycle.
//!
//! Generic over `T: Transport`, so the same driver runs on `MemoryTransport`
//! (Phase 3) or `HttpTransport` (Phase 4).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::too_many_arguments,
    dead_code
)]

use famp::runtime::{adapter, process_one_message, RuntimeError};
use famp_core::{AuthorityScope, MessageId, Principal};
use famp_crypto::FampSigningKey;
use famp_envelope::body::{
    AckBody, AckDisposition, Bounds, Budget, CommitBody, DeliverBody, RequestBody, TerminalStatus,
};
use famp_envelope::{AnySignedEnvelope, SignedEnvelope, Timestamp, UnsignedEnvelope};
use famp_fsm::TaskFsm;
use famp_keyring::Keyring;
use famp_transport::{Transport, TransportMessage};
use std::sync::{Arc, Mutex};

pub type Trace = Arc<Mutex<Vec<String>>>;

pub fn ts() -> Timestamp {
    Timestamp("2026-04-13T00:00:00Z".to_string())
}

pub fn two_key_bounds() -> Bounds {
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

pub fn canonical_bytes<B: famp_envelope::BodySchema>(signed: &SignedEnvelope<B>) -> Vec<u8> {
    let encoded = signed.encode().expect("encode must succeed");
    let value: serde_json::Value =
        famp_canonical::from_slice_strict(&encoded).expect("strict parse must succeed");
    famp_canonical::canonicalize(&value).expect("canonicalize must succeed")
}

pub fn log_line(trace: &Trace, line: String) {
    println!("{line}");
    trace.lock().unwrap().push(line);
}

async fn send_signed<T, B>(
    transport: &T,
    sender: &Principal,
    recipient: &Principal,
    signed: &SignedEnvelope<B>,
) where
    T: Transport,
    T::Error: std::fmt::Debug,
    B: famp_envelope::BodySchema,
{
    let bytes = canonical_bytes(signed);
    let msg = TransportMessage {
        sender: sender.clone(),
        recipient: recipient.clone(),
        bytes,
    };
    transport.send(msg).await.expect("transport send");
}

/// Drive bob's responder half of the signed cycle.
///
/// Generic over any `Transport` impl — used by both the same-process
/// MemoryTransport example (via the Phase 3 binary) and the Phase 4
/// HttpTransport tests/binary.
pub async fn drive_bob<T>(
    transport: &T,
    bob_keyring: &Keyring,
    bob: &Principal,
    alice: &Principal,
    bob_sk: &FampSigningKey,
    trace: &Trace,
) -> Result<(), RuntimeError>
where
    T: Transport,
    T::Error: std::fmt::Debug + Send + Sync + 'static,
{
    let mut fsm = TaskFsm::new();

    // 1. Receive request.
    let req_msg = transport
        .recv(bob)
        .await
        .unwrap_or_else(|e| panic!("recv request: {e:?}"));
    let req_env = process_one_message(&req_msg, bob_keyring, &mut fsm)?;
    assert!(matches!(req_env, AnySignedEnvelope::Request(_)));
    log_line(
        trace,
        format!(
            "[1] {} -> {}: Request",
            adapter::envelope_sender(&req_env),
            bob
        ),
    );

    // 2. Send commit.
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
        bob.clone(),
        alice.clone(),
        AuthorityScope::CommitLocal,
        ts(),
        commit_body,
    )
    .sign(bob_sk)
    .expect("sign commit");
    send_signed(transport, bob, alice, &commit).await;

    // 3. Send deliver (terminal=Completed).
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
        bob.clone(),
        alice.clone(),
        AuthorityScope::Advisory,
        ts(),
        deliver_body,
    )
    .with_terminal_status(TerminalStatus::Completed)
    .sign(bob_sk)
    .expect("sign deliver");
    send_signed(transport, bob, alice, &deliver).await;

    // 4. Receive ack from alice.
    let ack_msg = transport
        .recv(bob)
        .await
        .unwrap_or_else(|e| panic!("recv ack: {e:?}"));
    let ack_env = process_one_message(&ack_msg, bob_keyring, &mut fsm)?;
    assert!(matches!(ack_env, AnySignedEnvelope::Ack(_)));
    Ok(())
}

/// Drive alice's requester half of the signed cycle.
pub async fn drive_alice<T>(
    transport: &T,
    alice_keyring: &Keyring,
    alice: &Principal,
    bob: &Principal,
    alice_sk: &FampSigningKey,
    trace: &Trace,
) -> Result<(), RuntimeError>
where
    T: Transport,
    T::Error: std::fmt::Debug + Send + Sync + 'static,
{
    let mut fsm = TaskFsm::new();

    // 1. Send request.
    let req_body = RequestBody {
        scope: serde_json::json!({"task": "translate"}),
        bounds: two_key_bounds(),
        natural_language_summary: Some("translate to french".to_string()),
    };
    let req = UnsignedEnvelope::<RequestBody>::new(
        MessageId::new_v7(),
        alice.clone(),
        bob.clone(),
        AuthorityScope::Advisory,
        ts(),
        req_body,
    )
    .sign(alice_sk)
    .expect("sign request");
    send_signed(transport, alice, bob, &req).await;

    // 2. Receive commit.
    let commit_msg = transport
        .recv(alice)
        .await
        .unwrap_or_else(|e| panic!("recv commit: {e:?}"));
    let commit_env = process_one_message(&commit_msg, alice_keyring, &mut fsm)?;
    assert!(matches!(commit_env, AnySignedEnvelope::Commit(_)));
    log_line(
        trace,
        format!(
            "[2] {} -> {}: Commit",
            adapter::envelope_sender(&commit_env),
            alice
        ),
    );

    // 3. Receive deliver.
    let deliver_msg = transport
        .recv(alice)
        .await
        .unwrap_or_else(|e| panic!("recv deliver: {e:?}"));
    let deliver_env = process_one_message(&deliver_msg, alice_keyring, &mut fsm)?;
    assert!(matches!(deliver_env, AnySignedEnvelope::Deliver(_)));
    log_line(
        trace,
        format!(
            "[3] {} -> {}: Deliver",
            adapter::envelope_sender(&deliver_env),
            alice
        ),
    );

    // 4. Send ack.
    let ack_body = AckBody {
        disposition: AckDisposition::Completed,
        reason: None,
    };
    let ack = UnsignedEnvelope::<AckBody>::new(
        MessageId::new_v7(),
        alice.clone(),
        bob.clone(),
        AuthorityScope::Advisory,
        ts(),
        ack_body,
    )
    .sign(alice_sk)
    .expect("sign ack");
    send_signed(transport, alice, bob, &ack).await;
    log_line(trace, format!("[4] {alice} -> {bob}: Ack"));
    Ok(())
}

//! MemoryTransport adversarial rows — lifted from Phase 3 adversarial.rs.
//!
//! Uses `MemoryTransport::send_raw_for_test` (feature `test-util`, enabled via
//! famp's dev-dep on famp-transport).

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use super::fixtures::{alice, bob, case_bytes, ALICE_SECRET};
use super::harness::{assert_expected_error, Case};
use famp::runtime::process_one_message;
use famp_crypto::{FampSigningKey, TrustedVerifyingKey};
use famp_fsm::TaskFsm;
use famp_keyring::Keyring;
use famp_transport::{MemoryTransport, Transport, TransportMessage};

struct MemHarness {
    transport: MemoryTransport,
    bob_keyring: Keyring,
    fsm: TaskFsm,
}

async fn setup() -> MemHarness {
    let alice_sk = FampSigningKey::from_bytes(ALICE_SECRET);
    let alice_vk: TrustedVerifyingKey = alice_sk.verifying_key();
    let bob_keyring = Keyring::new().with_peer(alice(), alice_vk).unwrap();
    let transport = MemoryTransport::new();
    transport.register(alice()).await;
    transport.register(bob()).await;
    MemHarness {
        transport,
        bob_keyring,
        fsm: TaskFsm::new(),
    }
}

async fn run_case(case: Case) {
    let mut h = setup().await;
    let bytes = case_bytes(case);

    h.transport
        .send_raw_for_test(TransportMessage {
            sender: alice(),
            recipient: bob(),
            bytes,
        })
        .await
        .unwrap();

    let msg = h.transport.recv(&bob()).await.unwrap();
    let result = process_one_message(&msg, &h.bob_keyring, &mut h.fsm);
    match result {
        Err(e) => assert_expected_error(case, &e),
        Ok(env) => panic!("case {case:?} unexpectedly decoded: {env:?}"),
    }
}

#[tokio::test]
async fn memory_unsigned() {
    run_case(Case::Unsigned).await;
}

#[tokio::test]
async fn memory_wrong_key() {
    run_case(Case::WrongKey).await;
}

#[tokio::test]
async fn memory_canonical_divergence() {
    run_case(Case::CanonicalDivergence).await;
}

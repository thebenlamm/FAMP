#![allow(unused_crate_dependencies)]

// RED gate: this test FAILS to compile until Plan 01-02 wires the Broker actor.
// Compile-fail is the deliberate RED-first signal - see Plan 01-01 <objective>
// "RED-gate convention". DO NOT mark Plan 01-01 incomplete just because these
// scaffolds don't compile.

mod common;

use std::time::Instant;

use common::TestEnv;
use famp_bus::{Broker, BrokerInput, BusMessage, ClientId, Out};

#[test]
fn disconnect_clears_pending_await() {
    let env = TestEnv::new();
    let mut broker = Broker::new(env);
    let now = Instant::now();

    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1),
            msg: BusMessage::Await {
                timeout_ms: 30_000,
                task: None,
            },
        },
        now,
    );
    let out = broker.handle(BrokerInput::Disconnect(ClientId::from(1)), now);

    assert!(out.iter().any(|o| matches!(
        o,
        Out::UnparkAwait(ClientId(1)) | Out::ReleaseClient(ClientId(1))
    )));
}

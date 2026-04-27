#![allow(unused_crate_dependencies)]

// RED gate: this test FAILS to compile until Plan 01-02 wires the Broker actor.
// Compile-fail is the deliberate RED-first signal - see Plan 01-01 <objective>
// "RED-gate convention". DO NOT mark Plan 01-01 incomplete just because these
// scaffolds don't compile.

mod common;

use std::time::Instant;

use common::TestEnv;
use famp_bus::{Broker, BrokerInput, BusMessage, BusReply, ClientId, Out};

#[test]
fn pid_reuse_does_not_block_new_registration_after_disconnect() {
    let mut env = TestEnv::new();
    env.liveness_mut().mark_alive(1234);
    let mut broker = Broker::new(env);
    let now = Instant::now();

    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1),
            msg: BusMessage::Register {
                name: "alice".into(),
                pid: 1234,
            },
        },
        now,
    );
    let _ = broker.handle(BrokerInput::Disconnect(ClientId::from(1)), now);

    let out = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2),
            msg: BusMessage::Register {
                name: "alice".into(),
                pid: 1234,
            },
        },
        now,
    );

    assert!(out.iter().any(|o| matches!(
        o,
        Out::Reply(ClientId(2), BusReply::RegisterOk { active, .. }) if active == "alice"
    )));
}

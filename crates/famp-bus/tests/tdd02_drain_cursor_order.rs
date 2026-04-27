#![allow(unused_crate_dependencies)]

// RED gate: this test FAILS to compile until Plan 01-02 wires the Broker actor.
// Compile-fail is the deliberate RED-first signal - see Plan 01-01 <objective>
// "RED-gate convention". DO NOT mark Plan 01-01 incomplete just because these
// scaffolds don't compile.

mod common;

use std::time::Instant;

use common::TestEnv;
use famp_bus::{Broker, BrokerInput, BusMessage, BusReply, ClientId, MailboxName, Out};

#[test]
fn register_drain_replies_before_cursor_advance() {
    let env = TestEnv::new();
    env.mailbox().append(
        &MailboxName::Agent("alice".into()),
        br#"{"hello":"world"}"#.to_vec(),
    );
    let mut broker = Broker::new(env);

    let out = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1),
            msg: BusMessage::Register {
                name: "alice".into(),
                pid: 1234,
            },
        },
        Instant::now(),
    );

    assert!(matches!(
        out.as_slice(),
        [
            Out::Reply(
                ClientId(1),
                BusReply::RegisterOk { active, drained, .. }
            ),
            Out::AdvanceCursor {
                name: MailboxName::Agent(_),
                offset: 18,
            },
        ] if active == "alice" && drained.len() == 1
    ));
}

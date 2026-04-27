#![allow(clippy::unwrap_used, unused_crate_dependencies)]

mod common;

use std::time::Instant;

use common::TestEnv;
use famp_bus::{Broker, BrokerInput, BusMessage, BusReply, ClientId, MailboxName, Out, Target};
use serde_json::json;

fn hello(broker: &mut Broker<TestEnv>, client: u64, now: Instant) {
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(client),
            msg: BusMessage::Hello {
                bus_proto: 1,
                client: format!("client-{client}"),
            },
        },
        now,
    );
}

#[test]
fn register_drain_replies_before_cursor_advance() {
    let env = TestEnv::new();
    env.mailbox().append(
        &MailboxName::Agent("alice".into()),
        br#"{"hello":"world"}"#.to_vec(),
    );
    let mut broker = Broker::new(env);
    let now = Instant::now();
    hello(&mut broker, 1, now);

    let out = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1),
            msg: BusMessage::Register {
                name: "alice".into(),
                pid: 1234,
            },
        },
        now,
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

#[test]
fn send_emits_append_before_reply() {
    let env = TestEnv::new();
    let mut broker = Broker::new(env);
    let now = Instant::now();
    hello(&mut broker, 1, now);
    hello(&mut broker, 2, now);
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
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2),
            msg: BusMessage::Register {
                name: "bob".into(),
                pid: 5678,
            },
        },
        now,
    );

    let out = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1),
            msg: BusMessage::Send {
                to: Target::Agent { name: "bob".into() },
                envelope: json!({"body":"hello"}),
            },
        },
        now,
    );

    assert!(matches!(
        out.as_slice(),
        [
            Out::AppendMailbox {
                target: MailboxName::Agent(name),
                ..
            },
            Out::Reply(ClientId(1), BusReply::SendOk { .. }),
        ] if name == "bob"
    ));
}

#![allow(clippy::unwrap_used, unused_crate_dependencies)]

mod common;

use std::time::Instant;

use common::TestEnv;
use famp_bus::{Broker, BrokerInput, BusMessage, BusReply, ClientId, MailboxName, Out, Target};
use serde_json::json;

fn audit_log_envelope(seq: u64) -> serde_json::Value {
    json!({
        "famp": "0.5.2",
        "class": "audit_log",
        "scope": "standalone",
        "id": "01890000-0000-7000-8000-000000000001",
        "from": "agent:example.test/bob",
        "to": "agent:example.test/alice",
        "authority": "advisory",
        "ts": "2026-04-27T12:00:00Z",
        "body": {
            "event": "offline_message",
            "details": { "seq": seq }
        }
    })
}

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
    let line = famp_canonical::canonicalize(&audit_log_envelope(0)).unwrap();
    let expected_offset = u64::try_from(line.len() + 1).unwrap();
    env.mailbox()
        .append(&MailboxName::Agent("alice".into()), line);
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
                offset,
            },
        ] if active == "alice" && drained.len() == 1 && *offset == expected_offset
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
                envelope: audit_log_envelope(1),
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

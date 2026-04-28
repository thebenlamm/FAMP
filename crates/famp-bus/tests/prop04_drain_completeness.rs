#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    unused_crate_dependencies,
    clippy::match_same_arms
)]

mod common;

use common::TestEnv;
use famp_bus::*;
use proptest::prelude::*;
use serde_json::json;
use std::time::Instant;

fn audit_log_envelope(seq: usize) -> serde_json::Value {
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
            "details": { "offline_seq": seq }
        }
    })
}

fn hello_register(broker: &mut Broker<TestEnv>, client: u64, name: &str, now: Instant) {
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(client),
            msg: BusMessage::Hello {
                bus_proto: 1,
                client: name.into(),
                bind_as: None,
            },
        },
        now,
    );
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(client),
            msg: BusMessage::Register {
                name: name.into(),
                pid: 40_000 + u32::try_from(client).unwrap(),
            },
        },
        now,
    );
}

fn apply_mailbox(env: &TestEnv, out: &[Out]) {
    for item in out {
        if let Out::AppendMailbox { target, line } = item {
            env.mailbox().append(target, line.clone());
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn offline_then_online_register_drains_all_queued_envelopes(
        n_offline_sends in prop_oneof![1usize..=64],
        recipient_known_then_disconnected in prop_oneof![Just(true), Just(false)],
    ) {
        let env = TestEnv::new();
        let mut broker = Broker::new(env.clone());
        let now = Instant::now();
        hello_register(&mut broker, 1, "bob", now);

        if recipient_known_then_disconnected {
            hello_register(&mut broker, 2, "alice", now);
            let _ = broker.handle(BrokerInput::Disconnect(ClientId::from(2)), now);
        }

        for seq in 0..n_offline_sends {
            let out = broker.handle(
                BrokerInput::Wire {
                    client: ClientId::from(1),
                    msg: BusMessage::Send {
                        to: Target::Agent { name: "alice".into() },
                        envelope: audit_log_envelope(seq),
                    },
                },
                now,
            );
            apply_mailbox(&env, &out);
        }

        let client = if recipient_known_then_disconnected { 3 } else { 2 };
        let _ = broker.handle(
            BrokerInput::Wire {
                client: ClientId::from(client),
                msg: BusMessage::Hello {
                    bus_proto: 1,
                    client: "alice-reconnect".into(),
                    bind_as: None,
                },
            },
            now,
        );
        let out = broker.handle(
            BrokerInput::Wire {
                client: ClientId::from(client),
                msg: BusMessage::Register {
                    name: "alice".into(),
                    pid: 40_000 + u32::try_from(client).unwrap(),
                },
            },
            now,
        );

        let drained = match out.as_slice() {
            [Out::Reply(_, BusReply::RegisterOk { drained, .. }), Out::AdvanceCursor { .. }] => drained,
            other => panic!("unexpected register output: {other:?}"),
        };
        let observed: Vec<u64> = drained
            .iter()
            .map(|value| {
                let bytes = famp_canonical::canonicalize(value).unwrap();
                let typed = famp_envelope::AnyBusEnvelope::decode(&bytes).unwrap();
                assert!(matches!(typed, famp_envelope::AnyBusEnvelope::AuditLog(_)));
                value["body"]["details"]["offline_seq"].as_u64().unwrap()
            })
            .collect();
        prop_assert_eq!(observed, (0..n_offline_sends as u64).collect::<Vec<_>>());
    }
}

#[test]
fn malformed_drain_line_returns_error_and_does_not_advance_cursor() {
    let env = TestEnv::new();
    env.mailbox()
        .append(&MailboxName::Agent("alice".into()), b"{not json".to_vec());
    let mut broker = Broker::new(env);
    let now = Instant::now();
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1),
            msg: BusMessage::Hello {
                bus_proto: 1,
                client: "alice".into(),
                bind_as: None,
            },
        },
        now,
    );
    let out = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1),
            msg: BusMessage::Register {
                name: "alice".into(),
                pid: 40_001,
            },
        },
        now,
    );
    assert!(matches!(
        out.as_slice(),
        [Out::Reply(
            ClientId(1),
            BusReply::Err {
                kind: BusErrorKind::EnvelopeInvalid,
                ..
            }
        )]
    ));
    assert!(!out
        .iter()
        .any(|item| matches!(item, Out::AdvanceCursor { .. })));
}

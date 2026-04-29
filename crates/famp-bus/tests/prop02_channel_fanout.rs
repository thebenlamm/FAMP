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
use std::collections::BTreeSet;
use std::time::Instant;

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
                pid: 20_000 + u32::try_from(client).unwrap(),
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
    fn channel_fanout_records_each_sent_envelope_once(
        m_subscribers in prop_oneof![1usize..=8],
        n_messages in prop_oneof![1usize..=16],
        channel in prop_oneof![Just("#chan".to_string())],
    ) {
        let env = TestEnv::new();
        let mut broker = Broker::new(env.clone());
        let now = Instant::now();
        hello_register(&mut broker, 1, "sender", now);

        for subscriber in 0..m_subscribers {
            let client = 100 + subscriber as u64;
            let name = format!("sub-{subscriber}");
            hello_register(&mut broker, client, &name, now);
            let _ = broker.handle(
                BrokerInput::Wire {
                    client: ClientId::from(client),
                    msg: BusMessage::Join { channel: channel.clone() },
                },
                now,
            );
        }

        for seq in 0..n_messages {
            let out = broker.handle(
                BrokerInput::Wire {
                    client: ClientId::from(1),
                    msg: BusMessage::Send {
                        to: Target::Channel { name: channel.clone() },
                        envelope: json!({"channel_seq": seq}),
                    },
                },
                now,
            );
            apply_mailbox(&env, &out);
        }

        let drained = env
            .mailbox()
            .drain_from(&MailboxName::Channel(channel), 0)
            .unwrap();
        prop_assert_eq!(drained.lines.len(), n_messages);

        let observed: BTreeSet<u64> = drained
            .lines
            .iter()
            .map(|line| {
                let value: serde_json::Value = famp_canonical::from_slice_strict(line).unwrap();
                value["channel_seq"].as_u64().unwrap()
            })
            .collect();
        prop_assert_eq!(observed.len(), n_messages);
        prop_assert_eq!(observed, (0..n_messages as u64).collect());
    }
}

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
                pid: 10_000 + u32::try_from(client).unwrap(),
                cwd: None,
                listen: false,
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
    fn dm_fanin_preserves_per_sender_order(
        n_senders in prop_oneof![1usize..=8],
        msgs_per_sender in prop_oneof![1usize..=32],
    ) {
        let env = TestEnv::new();
        let mut broker = Broker::new(env.clone());
        let now = Instant::now();
        hello_register(&mut broker, 1, "alice", now);

        for sender_idx in 0..n_senders {
            let client = 100 + sender_idx as u64;
            let name = format!("sender-{sender_idx}");
            hello_register(&mut broker, client, &name, now);
            for seq in 0..msgs_per_sender {
                let out = broker.handle(
                    BrokerInput::Wire {
                        client: ClientId::from(client),
                        msg: BusMessage::Send {
                            to: Target::Agent { name: "alice".into() },
                            envelope: json!({"sender_idx": sender_idx, "seq": seq}),
                        },
                    },
                    now,
                );
                apply_mailbox(&env, &out);
            }
        }

        let drained = env
            .mailbox()
            .drain_from(&MailboxName::Agent("alice".into()), 0)
            .unwrap();
        let decoded: Vec<serde_json::Value> = drained
            .lines
            .iter()
            .map(|line| famp_canonical::from_slice_strict(line).unwrap())
            .collect();

        for sender_idx in 0..n_senders {
            let observed: Vec<usize> = decoded
                .iter()
                .filter(|value| value["sender_idx"].as_u64() == Some(sender_idx as u64))
                .map(|value| usize::try_from(value["seq"].as_u64().unwrap()).unwrap())
                .collect();
            prop_assert_eq!(observed, (0..msgs_per_sender).collect::<Vec<_>>());
        }
    }
}

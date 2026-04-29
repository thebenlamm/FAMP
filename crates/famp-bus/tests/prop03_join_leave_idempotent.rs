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
use std::time::Instant;

fn hello_register(broker: &mut Broker<TestEnv>, now: Instant) {
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
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1),
            msg: BusMessage::Register {
                name: "alice".into(),
                pid: 3001,
            },
        },
        now,
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn join_leave_idempotency_matches_last_operation(
        ops in proptest::collection::vec(prop_oneof![Just(true), Just(false)], 1..=64),
        channel in prop_oneof![Just("#chan".to_string())],
    ) {
        let env = TestEnv::new();
        let mut broker = Broker::new(env);
        let now = Instant::now();
        hello_register(&mut broker, now);
        let mut expected_joined = false;

        for op in ops {
            let msg = if op {
                expected_joined = true;
                BusMessage::Join { channel: channel.clone() }
            } else {
                expected_joined = false;
                BusMessage::Leave { channel: channel.clone() }
            };
            let _ = broker.handle(
                BrokerInput::Wire {
                    client: ClientId::from(1),
                    msg,
                },
                now,
            );
        }

        let out = broker.handle(
            BrokerInput::Wire {
                client: ClientId::from(1),
                msg: BusMessage::Whoami {},
            },
            now,
        );
        let joined = match out.as_slice() {
            [Out::Reply(ClientId(1), BusReply::WhoamiOk { joined, .. })] => {
                joined.contains(&channel)
            }
            other => panic!("unexpected whoami output: {other:?}"),
        };
        prop_assert_eq!(joined, expected_joined);
    }
}

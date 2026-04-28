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
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
enum Op {
    Register { slot: u8, name: u8, pid: u32 },
    Disconnect { slot: u8 },
    Tick,
}

fn arb_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0_u8..8, 0_u8..4, 1_u32..10_000)
            .prop_map(|(slot, name, pid)| { Op::Register { slot, name, pid } }),
        (0_u8..8).prop_map(|slot| Op::Disconnect { slot }),
        Just(Op::Tick),
    ]
}

fn hello(broker: &mut Broker<TestEnv>, client: ClientId, now: Instant) {
    let _ = broker.handle(
        BrokerInput::Wire {
            client,
            msg: BusMessage::Hello {
                bus_proto: 1,
                client: format!("client-{}", client.0),
                bind_as: None,
            },
        },
        now,
    );
}

fn assert_unique_sessions(out: &[Out]) {
    let rows = match out {
        [Out::Reply(_, BusReply::SessionsOk { rows })] => rows,
        other => panic!("unexpected sessions output: {other:?}"),
    };
    let mut counts = BTreeMap::<String, usize>::new();
    for row in rows {
        *counts.entry(row.name.clone()).or_default() += 1;
    }
    for (name, count) in counts {
        assert_eq!(
            count, 1,
            "connected name {name} is held by {count} sessions"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn pid_table_preserves_connected_name_uniqueness(
        ops in proptest::collection::vec(arb_op(), 1..=32),
    ) {
        let env = TestEnv::new();
        let mut broker = Broker::new(env);
        let start = Instant::now();
        hello(&mut broker, ClientId::from(999), start);

        for (idx, op) in ops.into_iter().enumerate() {
            let now = start + Duration::from_millis(idx as u64);
            match op {
                Op::Register { slot, name, pid } => {
                    let client = ClientId::from(u64::from(slot) + 1);
                    hello(&mut broker, client, now);
                    let _ = broker.handle(
                        BrokerInput::Wire {
                            client,
                            msg: BusMessage::Register {
                                name: format!("agent-{name}"),
                                pid,
                            },
                        },
                        now,
                    );
                }
                Op::Disconnect { slot } => {
                    let _ = broker.handle(
                        BrokerInput::Disconnect(ClientId::from(u64::from(slot) + 1)),
                        now,
                    );
                }
                Op::Tick => {
                    let _ = broker.handle(BrokerInput::Tick, now);
                }
            }

            let sessions = broker.handle(
                BrokerInput::Wire {
                    client: ClientId::from(999),
                    msg: BusMessage::Sessions {},
                },
                now,
            );
            assert_unique_sessions(&sessions);
        }
    }
}

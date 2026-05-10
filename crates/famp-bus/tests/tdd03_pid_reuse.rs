#![allow(clippy::unwrap_used, unused_crate_dependencies)]

mod common;

use std::time::Instant;

use common::TestEnv;
use famp_bus::{Broker, BrokerInput, BusMessage, BusReply, ClientId, Out};
use proptest::prelude::*;

fn hello(broker: &mut Broker<TestEnv>, client: u64, now: Instant) {
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(client),
            msg: BusMessage::Hello {
                bus_proto: 1,
                client: format!("client-{client}"),
                bind_as: None,
            },
        },
        now,
    );
}

#[test]
fn pid_reuse_does_not_block_new_registration_after_disconnect() {
    let mut env = TestEnv::new();
    env.liveness_mut().mark_alive(1234);
    let mut broker = Broker::new(env);
    let now = Instant::now();
    hello(&mut broker, 1, now);

    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1),
            msg: BusMessage::Register {
                name: "alice".into(),
                pid: 1234,
                cwd: None,
                listen: false,
            },
        },
        now,
    );
    let _ = broker.handle(BrokerInput::Disconnect(ClientId::from(1)), now);
    hello(&mut broker, 2, now);

    let out = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2),
            msg: BusMessage::Register {
                name: "alice".into(),
                pid: 1234,
                cwd: None,
                listen: false,
            },
        },
        now,
    );

    assert!(out.iter().any(|o| matches!(
        o,
        Out::Reply(ClientId(2), BusReply::RegisterOk { active, .. }) if active == "alice"
    )));
}

proptest! {
    #[test]
    fn disconnected_name_can_be_reused(pid in 1_u32..10_000, suffix in 0_u16..512) {
        let env = TestEnv::new();
        let mut broker = Broker::new(env);
        let now = Instant::now();
        let name = format!("agent-{suffix}");
        hello(&mut broker, 1, now);
        let _ = broker.handle(
            BrokerInput::Wire {
                client: ClientId::from(1),
                msg: BusMessage::Register { name: name.clone(), pid, cwd: None, listen: false },
            },
            now,
        );
        let _ = broker.handle(BrokerInput::Disconnect(ClientId::from(1)), now);
        hello(&mut broker, 2, now);

        let out = broker.handle(
            BrokerInput::Wire {
                client: ClientId::from(2),
                msg: BusMessage::Register { name: name.clone(), pid, cwd: None, listen: false },
            },
            now,
        );

        let registered = out.iter().any(|o| matches!(
            o,
            Out::Reply(ClientId(2), BusReply::RegisterOk { active, .. }) if active == &name
        ));
        prop_assert!(registered);
    }
}

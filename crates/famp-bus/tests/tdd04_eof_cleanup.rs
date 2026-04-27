#![allow(clippy::unwrap_used, unused_crate_dependencies)]

mod common;

use std::time::Instant;

use common::TestEnv;
use famp_bus::{Broker, BrokerInput, BusMessage, BusReply, ClientId, MailboxName, Out, Target};
use proptest::prelude::*;
use serde_json::json;

fn hello_register(broker: &mut Broker<TestEnv>, client: u64, name: &str, now: Instant) {
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
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(client),
            msg: BusMessage::Register {
                name: name.into(),
                pid: 1000 + u32::try_from(client).unwrap(),
            },
        },
        now,
    );
}

#[test]
fn disconnect_clears_pending_await() {
    let env = TestEnv::new();
    let mut broker = Broker::new(env);
    let now = Instant::now();
    hello_register(&mut broker, 1, "alice", now);

    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1),
            msg: BusMessage::Await {
                timeout_ms: 30_000,
                task: None,
            },
        },
        now,
    );
    let out = broker.handle(BrokerInput::Disconnect(ClientId::from(1)), now);

    assert!(out
        .iter()
        .any(|o| matches!(o, Out::ReleaseClient(ClientId(1)))));
}

#[test]
fn send_after_disconnect_routes_to_mailbox_not_dead_await() {
    let env = TestEnv::new();
    let mut broker = Broker::new(env);
    let now = Instant::now();
    hello_register(&mut broker, 1, "alice", now);
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1),
            msg: BusMessage::Await {
                timeout_ms: 30_000,
                task: None,
            },
        },
        now,
    );
    let _ = broker.handle(BrokerInput::Disconnect(ClientId::from(1)), now);
    hello_register(&mut broker, 2, "bob", now);

    let out = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2),
            msg: BusMessage::Send {
                to: Target::Agent {
                    name: "alice".into(),
                },
                envelope: json!({"body":"still queued"}),
            },
        },
        now,
    );

    assert!(out.iter().any(|o| matches!(
        o,
        Out::AppendMailbox {
            target: MailboxName::Agent(name),
            ..
        } if name == "alice"
    )));
    assert!(!out
        .iter()
        .any(|o| matches!(o, Out::Reply(ClientId(1), BusReply::AwaitOk { .. }))));
}

proptest! {
    #[test]
    fn disconnect_allows_future_sends_to_queue(body in "[a-z]{1,16}") {
        let env = TestEnv::new();
        let mut broker = Broker::new(env);
        let now = Instant::now();
        hello_register(&mut broker, 1, "alice", now);
        let _ = broker.handle(
            BrokerInput::Wire {
                client: ClientId::from(1),
                msg: BusMessage::Await { timeout_ms: 30_000, task: None },
            },
            now,
        );
        let _ = broker.handle(BrokerInput::Disconnect(ClientId::from(1)), now);
        hello_register(&mut broker, 2, "bob", now);

        let out = broker.handle(
            BrokerInput::Wire {
                client: ClientId::from(2),
                msg: BusMessage::Send {
                    to: Target::Agent { name: "alice".into() },
                    envelope: json!({"body": body}),
                },
            },
            now,
        );

        let queued = out.iter().any(|o| matches!(
            o,
            Out::AppendMailbox { target: MailboxName::Agent(name), .. } if name == "alice"
        ));
        let delivered_to_dead_client = out.iter().any(|o| matches!(
            o,
            Out::Reply(ClientId(1), BusReply::AwaitOk { .. })
        ));
        prop_assert!(queued);
        prop_assert!(!delivered_to_dead_client);
    }
}

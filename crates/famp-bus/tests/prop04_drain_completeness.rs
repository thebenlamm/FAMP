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
                bus_proto: BUS_PROTO_VERSION,
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
                cwd: None,
                listen: false,
                origin: Some(Origin::Local),
            },
        },
        now,
    );
}

fn apply_mailbox(env: &TestEnv, out: &[Out]) {
    for item in out {
        if let Out::AppendMailbox {
            target,
            line,
            origin,
        } = item
        {
            let stamped = stamp_line(line, *origin).unwrap();
            env.mailbox().append(target, stamped);
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
                    bus_proto: BUS_PROTO_VERSION,
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
                cwd: None,
                listen: false,
                origin: Some(Origin::Local),
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
            .map(|stamped| {
                let bytes = famp_canonical::canonicalize(&stamped.envelope).unwrap();
                let typed = famp_envelope::AnyBusEnvelope::decode(&bytes).unwrap();
                assert!(matches!(typed, famp_envelope::AnyBusEnvelope::AuditLog(_)));
                stamped.envelope["body"]["details"]["offline_seq"]
                    .as_u64()
                    .unwrap()
            })
            .collect();
        prop_assert_eq!(observed, (0..n_offline_sends as u64).collect::<Vec<_>>());
    }
}

/// Canonical wire bytes for one mailbox line (no trailing newline; the
/// in-memory mailbox stores one line per entry).
fn line(value: &serde_json::Value) -> Vec<u8> {
    famp_canonical::canonicalize(value).unwrap()
}

/// A Grok-style malformed envelope: valid JSON, but `causality.ref` is a
/// free-text narration where a UUID `MessageId` is required — exactly the
/// envelope that wedged scs-opus in the wild (fix 260611). `decode_line`
/// rejects it; the drain must SKIP it rather than abort.
fn malformed_envelope() -> serde_json::Value {
    json!({
        "famp": "0.5.2",
        "class": "audit_log",
        "scope": "standalone",
        "id": "01890000-0000-7000-8000-0000000000ff",
        "from": "agent:example.test/grok",
        "to": "agent:example.test/alice",
        "authority": "advisory",
        "ts": "2026-04-27T12:00:00Z",
        "causality": { "ref": "019eb something from opus refinement", "rel": "delivers" },
        "body": { "details": { "body": "Thanks for the refinements" } }
    })
}

/// Head-of-line resilience (register/inbox path): a single undecodable line
/// sandwiched between two good envelopes must be SKIPPED, both good
/// envelopes delivered, and the cursor advanced to EOF — NOT a hard error
/// that wedges the entire mailbox. (Pre-fix contract: this returned
/// `EnvelopeInvalid` and refused to advance the cursor. Inverted by fix
/// 260611.)
#[test]
fn malformed_drain_line_is_skipped_and_cursor_advances_on_register() {
    let env = TestEnv::new();
    let alice = MailboxName::Agent("alice".into());
    env.mailbox().append(&alice, line(&audit_log_envelope(0)));
    env.mailbox().append(&alice, line(&malformed_envelope()));
    env.mailbox().append(&alice, line(&audit_log_envelope(1)));
    let mut broker = Broker::new(env);
    let now = Instant::now();
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1),
            msg: BusMessage::Hello {
                bus_proto: BUS_PROTO_VERSION,
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
                cwd: None,
                listen: false,
                origin: None,
            },
        },
        now,
    );
    let drained = match out.as_slice() {
        [Out::Reply(ClientId(1), BusReply::RegisterOk { drained, .. }), Out::AdvanceCursor { .. }] => {
            drained
        }
        other => panic!("expected RegisterOk + AdvanceCursor (skip-and-advance), got {other:?}"),
    };
    // Both good envelopes delivered; the malformed one dropped from the batch.
    let seqs: Vec<u64> = drained
        .iter()
        .map(|v| {
            v.envelope["body"]["details"]["offline_seq"]
                .as_u64()
                .unwrap()
        })
        .collect();
    assert_eq!(
        seqs,
        vec![0, 1],
        "good envelopes survive, malformed skipped"
    );
}

/// Head-of-line resilience (await path — the live-wedged site). Phase 19 makes
/// every intentionally raw fixture fail closed to `Origin::Unknown`, so none
/// may satisfy `Await`; the raw poison and legacy-good records must still be
/// skipped with cursor progress so a later stamped Local record wakes the
/// parked client. The raw records remain byte-for-byte unchanged.
#[test]
fn malformed_drain_line_is_skipped_and_cursor_advances_on_await() {
    let env = TestEnv::new();
    let mailbox = env.mailbox().clone();
    let alice = MailboxName::Agent("alice".into());
    let mut broker = Broker::new(env);
    let now = Instant::now();
    hello_register(&mut broker, 1, "alice", now);
    mailbox.append(&alice, line(&audit_log_envelope(0)));
    mailbox.append(&alice, line(&malformed_envelope()));
    mailbox.append(&alice, line(&audit_log_envelope(1)));

    let park_out = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1),
            msg: BusMessage::Await {
                timeout_ms: 30_000,
                task: None,
            },
        },
        now,
    );
    assert!(
        park_out.iter().any(|out| matches!(
            out,
            Out::ParkAwait {
                client: ClientId(1),
                ..
            }
        )),
        "raw Unknown records must not satisfy Await: {park_out:?}"
    );

    hello_register(&mut broker, 2, "bob", now);
    let wake_out = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2),
            msg: BusMessage::Send {
                to: Target::Agent {
                    name: "alice".into(),
                },
                envelope: audit_log_envelope(2),
            },
        },
        now,
    );
    let Some(envelopes) = wake_out.iter().find_map(|out| match out {
        Out::Reply(ClientId(1), BusReply::AwaitOk { envelopes, .. }) => Some(envelopes),
        _ => None,
    }) else {
        panic!("later stamped Local record must wake past raw poison: {wake_out:?}")
    };
    let seqs: Vec<u64> = envelopes
        .iter()
        .map(|v| {
            v.envelope["body"]["details"]["offline_seq"]
                .as_u64()
                .unwrap()
        })
        .collect();
    assert_eq!(
        seqs,
        vec![2],
        "Await skips raw Unknown records and poison, delivering only later Local"
    );
    assert_eq!(envelopes[0].origin, Origin::Local);
}

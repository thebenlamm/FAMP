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
                cwd: None,
                listen: false,
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
                cwd: None,
                listen: false,
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
        .map(|v| v["body"]["details"]["offline_seq"].as_u64().unwrap())
        .collect();
    assert_eq!(
        seqs,
        vec![0, 1],
        "good envelopes survive, malformed skipped"
    );
}

/// Head-of-line resilience (await path — the live-wedged site). A listen-mode
/// agent draining its mailbox via `Await` must skip an undecodable line and
/// advance past it, delivering the good envelopes behind it. Pre-fix, the
/// `?` in `drain_await_batch` returned before advancing the offset, so the
/// cursor never moved past the bad line and the inbox stayed jammed forever
/// (this is exactly what happened to scs-opus). Seed AFTER register so the
/// await drains over the poison from offset 0.
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

    let out = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1),
            msg: BusMessage::Await {
                timeout_ms: 30_000,
                task: None,
            },
        },
        now,
    );
    let Some(envelopes) = out.iter().find_map(|o| match o {
        Out::Reply(ClientId(1), BusReply::AwaitOk { envelopes, .. }) => Some(envelopes),
        _ => None,
    }) else {
        panic!("expected AwaitOk (skip-and-advance over poison), got {out:?}")
    };
    let seqs: Vec<u64> = envelopes
        .iter()
        .map(|v| v["body"]["details"]["offline_seq"].as_u64().unwrap())
        .collect();
    assert_eq!(
        seqs,
        vec![0, 1],
        "await drain delivers good envelopes, skips the poison line"
    );
}

//! D-10 unit tests for `bind_as` proxy semantics.

use super::*;
use crate::{Broker, FakeLiveness, InMemoryMailbox, MailboxRead as _};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

#[derive(Debug, Default, Clone)]
struct TestEnv {
    mailbox: InMemoryMailbox,
    liveness: Rc<RefCell<FakeLiveness>>,
}

impl crate::MailboxRead for TestEnv {
    fn drain_from(
        &self,
        name: &crate::MailboxName,
        since_bytes: u64,
    ) -> Result<crate::DrainResult, crate::MailboxErr> {
        self.mailbox.drain_from(name, since_bytes)
    }
}

impl crate::LivenessProbe for TestEnv {
    fn is_alive(&self, pid: u32) -> bool {
        self.liveness.borrow().is_alive(pid)
    }
}

fn hello_canonical(broker: &mut Broker<TestEnv>, client: u64, name: &str, now: Instant) {
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
}

fn register(broker: &mut Broker<TestEnv>, client: u64, name: &str, pid: u32, now: Instant) {
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(client),
            msg: BusMessage::Register {
                name: name.into(),
                pid,
                cwd: None,
                listen: false,
            },
        },
        now,
    );
}

fn hello_proxy(broker: &mut Broker<TestEnv>, client: u64, bound: &str, now: Instant) -> Vec<Out> {
    broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(client),
            msg: BusMessage::Hello {
                bus_proto: 1,
                client: "proxy".into(),
                bind_as: Some(bound.into()),
            },
        },
        now,
    )
}

fn audit_log_envelope(seq: usize, sender: &str) -> serde_json::Value {
    serde_json::json!({
        "famp": "0.5.2",
        "class": "audit_log",
        "scope": "standalone",
        "id": format!("01890000-0000-7000-8000-{seq:012}"),
        "from": format!("agent:example.test/{sender}"),
        "to": "agent:example.test/dave",
        "authority": "advisory",
        "ts": "2026-05-15T12:00:00Z",
        "body": {
            "event": "famp.send.channel_post",
            "details": { "seq": seq }
        }
    })
}

/// Like `audit_log_envelope`, but shaped as a task reply: carries
/// `causality.ref` = `task` so `AwaitFilter::Task(task)` matches it (see
/// `filter_matches` in `awaiting.rs`).
fn audit_log_reply_envelope(
    seq: usize,
    sender: &str,
    recipient: &str,
    task: uuid::Uuid,
) -> serde_json::Value {
    serde_json::json!({
        "famp": "0.5.2",
        "class": "audit_log",
        "scope": "standalone",
        "id": format!("01890000-0000-7000-8000-{seq:012}"),
        "from": format!("agent:example.test/{sender}"),
        "to": format!("agent:example.test/{recipient}"),
        "authority": "advisory",
        "ts": "2026-05-15T12:00:00Z",
        "causality": { "rel": "delivers", "ref": task.to_string() },
        "body": {
            "event": "famp.send.deliver",
            "details": { "seq": seq }
        }
    })
}

fn apply_mailbox(env: &TestEnv, outs: &[Out]) {
    for out in outs {
        if let Out::AppendMailbox { target, line } = out {
            env.mailbox.append(target, line.clone());
        }
    }
}

#[test]
fn test_channel_burst_while_not_parked_batches_on_next_await() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env.clone());
    let now = Instant::now();

    for (client, name, pid) in [
        (1_u64, "alice", 100_u32),
        (2_u64, "bob", 200_u32),
        (3_u64, "carol", 300_u32),
        (4_u64, "dave", 400_u32),
    ] {
        hello_canonical(&mut broker, client, name, now);
        register(&mut broker, client, name, pid, now);
        let join_outs = broker.handle(
            BrokerInput::Wire {
                client: ClientId::from(client),
                msg: BusMessage::Join {
                    channel: "#burst".into(),
                    role: None,
                },
            },
            now,
        );
        apply_mailbox(&env, &join_outs);
    }

    for (client, sender, seq) in [
        (1_u64, "alice", 1_usize),
        (2_u64, "bob", 2_usize),
        (3_u64, "carol", 3_usize),
    ] {
        let outs = broker.handle(
            BrokerInput::Wire {
                client: ClientId::from(client),
                msg: BusMessage::Send {
                    to: Target::Channel {
                        name: "#burst".into(),
                    },
                    envelope: audit_log_envelope(seq, sender),
                },
            },
            now,
        );
        apply_mailbox(&env, &outs);
    }

    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(4_u64),
            msg: BusMessage::Await {
                timeout_ms: 10_000,
                task: None,
            },
        },
        now,
    );

    let (mailbox, envelopes, next_offset) = outs
        .into_iter()
        .find_map(|out| match out {
            Out::Reply(
                ClientId(4),
                BusReply::AwaitOk {
                    envelopes,
                    mailbox,
                    next_offset,
                },
            ) => Some((mailbox, envelopes, next_offset)),
            _ => None,
        })
        .expect("dave's Await should return the queued channel burst");

    assert_eq!(mailbox, MailboxName::Channel("#burst".into()));
    assert_eq!(envelopes.len(), 3, "dave must receive the whole burst");
    let observed: Vec<u64> = envelopes
        .iter()
        .map(|envelope| envelope["body"]["details"]["seq"].as_u64().unwrap())
        .collect();
    assert_eq!(observed, vec![1, 2, 3]);
    assert!(next_offset > 0, "AwaitOk should carry the resume offset");
}

#[test]
fn test_hello_bind_as_unregistered_returns_not_registered() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();
    // alice is not registered.
    let outs = hello_proxy(&mut broker, 1, "alice", now);
    assert_eq!(outs.len(), 1);
    match &outs[0] {
        Out::Reply(_, BusReply::HelloErr { kind, .. }) => {
            assert_eq!(*kind, BusErrorKind::NotRegistered);
        }
        other => panic!("expected HelloErr, got {other:?}"),
    }
}

#[test]
fn hello_rejects_unsupported_bus_proto() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();
    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Hello {
                bus_proto: BUS_PROTO_VERSION + 1,
                client: "newer-client".into(),
                bind_as: None,
            },
        },
        now,
    );
    match &outs[0] {
        Out::Reply(_, BusReply::HelloErr { kind, message }) => {
            assert_eq!(*kind, BusErrorKind::BrokerProtoMismatch);
            assert!(message.contains("expected bus_proto=1"));
        }
        other => panic!("expected HelloErr BrokerProtoMismatch, got {other:?}"),
    }
}

#[test]
fn test_hello_bind_as_dead_holder_returns_not_registered() {
    let env = TestEnv::default();
    env.liveness.borrow_mut().mark_dead(12345);
    let mut broker = Broker::new(env);
    let now = Instant::now();
    hello_canonical(&mut broker, 1, "alice", now);
    register(&mut broker, 1, "alice", 12345, now);
    let outs = hello_proxy(&mut broker, 2, "alice", now);
    match &outs[0] {
        Out::Reply(_, BusReply::HelloErr { kind, .. }) => {
            assert_eq!(*kind, BusErrorKind::NotRegistered);
        }
        other => panic!("expected HelloErr, got {other:?}"),
    }
}

#[test]
fn test_hello_bind_as_live_holder_succeeds() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();
    // Canonical holder for alice: client 1, pid 999 (alive by default).
    hello_canonical(&mut broker, 1, "alice-holder", now);
    register(&mut broker, 1, "alice", 999, now);
    // Proxy from client 2.
    let outs = hello_proxy(&mut broker, 2, "alice", now);
    match &outs[0] {
        Out::Reply(_, BusReply::HelloOk { .. }) => {}
        other => panic!("expected HelloOk, got {other:?}"),
    }
    // Proxy can Send under alice's identity.
    let send_outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::Send {
                to: Target::Agent { name: "bob".into() },
                envelope: serde_json::json!({"body": "hi"}),
            },
        },
        now,
    );
    let has_append = send_outs
        .iter()
        .any(|o| matches!(o, Out::AppendMailbox { .. }));
    assert!(has_append, "proxy Send must produce an AppendMailbox");
}

/// Fix B (260721): a session re-registering its OWN held name is
/// idempotent — it returns RegisterOk (refreshing the listen flag), not
/// -32101 NameTaken. A DIFFERENT client grabbing the held name still gets
/// NameTaken. This restores "just re-register" as a real recovery path
/// after a Claude Code /compact drops the register marker from the
/// listen-hook's transcript scan window.
#[test]
fn test_self_reregister_is_idempotent_but_others_are_rejected() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();

    // Client 1 registers "orchestrator" with listen OFF.
    hello_canonical(&mut broker, 1, "orchestrator", now);
    let reg = |broker: &mut Broker<TestEnv>, client: u64, listen: bool| -> Vec<Out> {
        broker.handle(
            BrokerInput::Wire {
                client: ClientId::from(client),
                msg: BusMessage::Register {
                    name: "orchestrator".into(),
                    pid: 999,
                    cwd: None,
                    listen,
                },
            },
            now,
        )
    };

    let first = reg(&mut broker, 1, false);
    assert!(
        first
            .iter()
            .any(|o| matches!(o, Out::Reply(_, BusReply::RegisterOk { .. }))),
        "first register should succeed: {first:?}"
    );
    assert!(
        !broker.state.clients[&ClientId::from(1_u64)].listen_mode,
        "listen should be off after first register"
    );

    // Same client re-registers with listen ON: idempotent success that
    // refreshes the listen flag — NOT NameTaken.
    let again = reg(&mut broker, 1, true);
    assert!(
        again
            .iter()
            .any(|o| matches!(o, Out::Reply(_, BusReply::RegisterOk { active, .. }) if active == "orchestrator")),
        "self re-register must return RegisterOk, got: {again:?}"
    );
    assert!(
        !again
            .iter()
            .any(|o| matches!(o, Out::Reply(_, BusReply::Err { .. }))),
        "self re-register must not error: {again:?}"
    );
    assert!(
        broker.state.clients[&ClientId::from(1_u64)].listen_mode,
        "listen flag should be refreshed to true by the idempotent re-register"
    );

    // A DIFFERENT live client trying to take the held name is still rejected.
    hello_canonical(&mut broker, 2, "orchestrator-intruder", now);
    let intruder = reg(&mut broker, 2, false);
    assert!(
        intruder.iter().any(|o| matches!(
            o,
            Out::Reply(_, BusReply::Err { kind, .. }) if *kind == BusErrorKind::NameTaken
        )),
        "a different client must still get NameTaken: {intruder:?}"
    );
}

#[test]
fn inbox_list_does_not_advance_broker_cursor() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();
    hello_canonical(&mut broker, 1, "alice-holder", now);
    register(&mut broker, 1, "alice", 999, now);

    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Inbox {
                since: Some(0),
                include_terminal: Some(true),
            },
        },
        now,
    );

    assert!(
        outs.iter()
            .any(|out| matches!(out, Out::Reply(_, BusReply::InboxOk { .. }))),
        "inbox should reply with InboxOk: {outs:?}"
    );
    assert!(
        !outs
            .iter()
            .any(|out| matches!(out, Out::AdvanceCursor { .. })),
        "inbox list must not advance the broker cursor: {outs:?}"
    );
}

#[test]
fn test_proxy_join_persists_after_disconnect() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();
    hello_canonical(&mut broker, 1, "alice-holder", now);
    register(&mut broker, 1, "alice", 999, now);
    // Proxy joins #x.
    let _ = hello_proxy(&mut broker, 2, "alice", now);
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::Join {
                channel: "#x".into(),
                role: None,
            },
        },
        now,
    );
    // Proxy disconnects.
    let _ = broker.handle(BrokerInput::Disconnect(ClientId::from(2_u64)), now);
    // Sessions from a fresh connection still shows alice in #x.
    hello_canonical(&mut broker, 3, "observer", now);
    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(3_u64),
            msg: BusMessage::Sessions {},
        },
        now,
    );
    let rows = outs
        .into_iter()
        .find_map(|o| match o {
            Out::Reply(_, BusReply::SessionsOk { rows }) => Some(rows),
            _ => None,
        })
        .expect("SessionsOk");
    let alice = rows
        .iter()
        .find(|r| r.name == "alice")
        .expect("alice should still appear in sessions");
    assert!(alice.joined.contains(&"#x".to_string()));
}

#[test]
fn test_proxy_op_after_holder_dies_returns_not_registered() {
    let env = TestEnv::default();
    let liveness_handle = Rc::clone(&env.liveness);
    let mut broker = Broker::new(env);
    let now = Instant::now();
    hello_canonical(&mut broker, 1, "alice-holder", now);
    register(&mut broker, 1, "alice", 999, now);
    let _ = hello_proxy(&mut broker, 2, "alice", now);
    // Mark holder dead via the shared liveness handle.
    liveness_handle.borrow_mut().mark_dead(999);
    // Proxy attempts a Send → should NotRegistered.
    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::Send {
                to: Target::Agent { name: "bob".into() },
                envelope: serde_json::json!({"body": "hi"}),
            },
        },
        now,
    );
    let kind = outs.iter().find_map(|o| match o {
        Out::Reply(_, BusReply::Err { kind, .. }) => Some(*kind),
        _ => None,
    });
    assert_eq!(kind, Some(BusErrorKind::NotRegistered));
}

#[test]
fn test_proxy_disconnect_does_not_remove_canonical_registration() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();
    hello_canonical(&mut broker, 1, "alice-holder", now);
    register(&mut broker, 1, "alice", 999, now);
    let _ = hello_proxy(&mut broker, 2, "alice", now);
    let _ = broker.handle(BrokerInput::Disconnect(ClientId::from(2_u64)), now);
    // Sessions from a fresh connection still shows alice.
    hello_canonical(&mut broker, 3, "observer", now);
    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(3_u64),
            msg: BusMessage::Sessions {},
        },
        now,
    );
    let rows = outs
        .into_iter()
        .find_map(|o| match o {
            Out::Reply(_, BusReply::SessionsOk { rows }) => Some(rows),
            _ => None,
        })
        .expect("SessionsOk");
    assert!(rows.iter().any(|r| r.name == "alice"));
}

// Helper: collect all ClientIds that received AwaitOk in a Vec<Out>.
fn count_await_oks(outs: &[Out]) -> Vec<ClientId> {
    outs.iter()
        .filter_map(|o| match o {
            Out::Reply(c, BusReply::AwaitOk { .. }) => Some(*c),
            _ => None,
        })
        .collect()
}

#[test]
fn test_send_agent_woken_true_when_waiter_parked() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();

    hello_canonical(&mut broker, 1, "alice", now);
    register(&mut broker, 1, "alice", 999, now);
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Await {
                timeout_ms: 10_000,
                task: None,
            },
        },
        now,
    );

    hello_canonical(&mut broker, 2, "bob", now);
    register(&mut broker, 2, "bob", 111, now);
    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::Send {
                to: Target::Agent {
                    name: "alice".into(),
                },
                envelope: serde_json::json!({"body": "hi"}),
            },
        },
        now,
    );

    let (reply_client, delivered) = outs
        .iter()
        .find_map(|o| match o {
            Out::Reply(client, BusReply::SendOk { delivered, .. }) => Some((*client, delivered)),
            _ => None,
        })
        .expect("SendOk must be present");
    assert_eq!(reply_client, ClientId::from(2_u64));
    assert_eq!(delivered.len(), 1);
    assert_eq!(
        delivered[0].to,
        Target::Agent {
            name: "alice".into()
        }
    );
    assert!(delivered[0].ok);
    assert!(delivered[0].woken);
}

#[test]
fn test_send_agent_woken_false_when_no_waiter() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();

    hello_canonical(&mut broker, 1, "alice", now);
    register(&mut broker, 1, "alice", 999, now);

    hello_canonical(&mut broker, 2, "bob", now);
    register(&mut broker, 2, "bob", 111, now);
    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::Send {
                to: Target::Agent {
                    name: "alice".into(),
                },
                envelope: serde_json::json!({"body": "hi"}),
            },
        },
        now,
    );

    let append_count = outs
        .iter()
        .filter(|o| {
            matches!(
                o,
                Out::AppendMailbox {
                    target: MailboxName::Agent(name),
                    ..
                } if name == "alice"
            )
        })
        .count();
    assert_eq!(append_count, 1);

    let delivered = outs
        .iter()
        .find_map(|o| match o {
            Out::Reply(_, BusReply::SendOk { delivered, .. }) => Some(delivered),
            _ => None,
        })
        .expect("SendOk must be present");
    assert_eq!(delivered.len(), 1);
    assert!(delivered[0].ok);
    assert!(!delivered[0].woken);
}

#[test]
fn test_send_agent_wakes_all_proxy_waiters() {
    // Two proxy waiters for alice; both must wake on a single DM.
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();

    // Canonical holder for alice: client 1, pid 999 (alive by default).
    hello_canonical(&mut broker, 1, "alice-holder", now);
    register(&mut broker, 1, "alice", 999, now);

    // Proxy 1 and proxy 2 both bind_as alice.
    let _ = hello_proxy(&mut broker, 2, "alice", now);
    let _ = hello_proxy(&mut broker, 3, "alice", now);

    // Park both proxies on Await.
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::Await {
                timeout_ms: 10_000,
                task: None,
            },
        },
        now,
    );
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(3_u64),
            msg: BusMessage::Await {
                timeout_ms: 10_000,
                task: None,
            },
        },
        now,
    );

    // Sender: canonical "bob" on client 4 sends DM to alice.
    hello_canonical(&mut broker, 4, "bob", now);
    register(&mut broker, 4, "bob", 111, now);
    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(4_u64),
            msg: BusMessage::Send {
                to: Target::Agent {
                    name: "alice".into(),
                },
                envelope: serde_json::json!({"body": "hi"}),
            },
        },
        now,
    );

    // AppendMailbox MUST be first.
    assert!(
        matches!(outs[0], Out::AppendMailbox { .. }),
        "AppendMailbox must precede any Reply; got {:?}",
        outs[0]
    );

    // Both proxies receive AwaitOk.
    let woken: std::collections::HashSet<ClientId> = count_await_oks(&outs).into_iter().collect();
    assert_eq!(
        woken,
        [ClientId::from(2_u64), ClientId::from(3_u64)]
            .into_iter()
            .collect(),
        "both proxy waiters must be woken"
    );

    // Exactly two UnparkAwait entries with the same ClientId set.
    let unparked: std::collections::HashSet<ClientId> = outs
        .iter()
        .filter_map(|o| match o {
            Out::UnparkAwait { client } => Some(*client),
            _ => None,
        })
        .collect();
    assert_eq!(
        woken, unparked,
        "UnparkAwait ClientId set must match AwaitOk set"
    );

    // Exactly one AppendMailbox for the agent mailbox.
    let mailbox_count = outs
        .iter()
        .filter(|o| {
            matches!(
                o,
                Out::AppendMailbox {
                    target: MailboxName::Agent(_),
                    ..
                }
            )
        })
        .count();
    assert_eq!(
        mailbox_count, 1,
        "exactly one AppendMailbox to agent mailbox"
    );
}

#[test]
fn test_canonical_plus_proxy_both_wake() {
    // Canonical alice (client 1) + one proxy (client 2); both parked on Await.
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();

    hello_canonical(&mut broker, 1, "alice-holder", now);
    register(&mut broker, 1, "alice", 999, now);
    let _ = hello_proxy(&mut broker, 2, "alice", now);

    // Park canonical alice on Await.
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Await {
                timeout_ms: 10_000,
                task: None,
            },
        },
        now,
    );
    // Park proxy on Await.
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::Await {
                timeout_ms: 10_000,
                task: None,
            },
        },
        now,
    );

    // Sender on client 3 sends DM to alice.
    hello_canonical(&mut broker, 3, "bob", now);
    register(&mut broker, 3, "bob", 222, now);
    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(3_u64),
            msg: BusMessage::Send {
                to: Target::Agent {
                    name: "alice".into(),
                },
                envelope: serde_json::json!({"body": "hi"}),
            },
        },
        now,
    );

    // AppendMailbox MUST be first.
    assert!(
        matches!(outs[0], Out::AppendMailbox { .. }),
        "AppendMailbox must precede any Reply"
    );

    let woken: std::collections::HashSet<ClientId> = count_await_oks(&outs).into_iter().collect();
    assert_eq!(
        woken,
        [ClientId::from(1_u64), ClientId::from(2_u64)]
            .into_iter()
            .collect(),
        "canonical holder and proxy must both be woken"
    );
}

#[test]
fn test_dead_proxy_does_not_wake() {
    // Two proxies for alice; canonical holder pid 999 is marked dead before send.
    // Neither proxy should receive AwaitOk; message still lands in mailbox.
    let env = TestEnv::default();
    let liveness_handle = Rc::clone(&env.liveness);
    let mut broker = Broker::new(env);
    let now = Instant::now();

    hello_canonical(&mut broker, 1, "alice-holder", now);
    register(&mut broker, 1, "alice", 999, now);

    let _ = hello_proxy(&mut broker, 2, "alice", now);
    let _ = hello_proxy(&mut broker, 4, "alice", now);

    // Park proxy 2 on Await.
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::Await {
                timeout_ms: 10_000,
                task: None,
            },
        },
        now,
    );
    // Park proxy 4 on Await.
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(4_u64),
            msg: BusMessage::Await {
                timeout_ms: 10_000,
                task: None,
            },
        },
        now,
    );

    // Kill the canonical holder.
    liveness_handle.borrow_mut().mark_dead(999);

    // Sender on client 3 sends DM to "alice".
    hello_canonical(&mut broker, 3, "bob", now);
    register(&mut broker, 3, "bob", 333, now);
    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(3_u64),
            msg: BusMessage::Send {
                to: Target::Agent {
                    name: "alice".into(),
                },
                envelope: serde_json::json!({"body": "hi"}),
            },
        },
        now,
    );

    // No AwaitOk — dead canonical holder gates all proxies out.
    let woken = count_await_oks(&outs);
    assert!(
        woken.is_empty(),
        "dead canonical holder must prevent proxy wake; woken: {woken:?}"
    );

    // AppendMailbox still happens (message lands in mailbox).
    let has_mailbox = outs.iter().any(|o| {
        matches!(
            o,
            Out::AppendMailbox {
                target: MailboxName::Agent(_),
                ..
            }
        )
    });
    assert!(
        has_mailbox,
        "AppendMailbox must still be emitted even when no waiter is woken"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn test_send_channel_wakes_all_member_waiters() {
    // alice (client 1) and bob (client 2) join #x; both parked on Await.
    // carol (client 3) sends to #x; both must wake, mailbox first.
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();

    // Register alice.
    hello_canonical(&mut broker, 1, "alice", now);
    register(&mut broker, 1, "alice", 100, now);

    // Register bob.
    hello_canonical(&mut broker, 2, "bob", now);
    register(&mut broker, 2, "bob", 200, now);

    // Both join #x.
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Join {
                channel: "#x".into(),
                role: None,
            },
        },
        now,
    );
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::Join {
                channel: "#x".into(),
                role: None,
            },
        },
        now,
    );

    // Park alice on Await.
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Await {
                timeout_ms: 10_000,
                task: None,
            },
        },
        now,
    );
    // Park bob on Await.
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::Await {
                timeout_ms: 10_000,
                task: None,
            },
        },
        now,
    );

    // carol (client 3) registers and sends to #x.
    hello_canonical(&mut broker, 3, "carol", now);
    register(&mut broker, 3, "carol", 300, now);
    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(3_u64),
            msg: BusMessage::Send {
                to: Target::Channel { name: "#x".into() },
                envelope: serde_json::json!({"body": "hello channel"}),
            },
        },
        now,
    );

    // AppendMailbox(Channel) must precede all AwaitOk replies.
    let first_awaitok_pos = outs
        .iter()
        .position(|o| matches!(o, Out::Reply(_, BusReply::AwaitOk { .. })));
    let channel_mailbox_pos = outs.iter().position(|o| {
        matches!(
            o,
            Out::AppendMailbox {
                target: MailboxName::Channel(_),
                ..
            }
        )
    });
    assert!(
        channel_mailbox_pos.is_some(),
        "channel AppendMailbox must be emitted"
    );
    if let Some(await_pos) = first_awaitok_pos {
        assert!(
            channel_mailbox_pos.unwrap() < await_pos,
            "channel AppendMailbox must precede first AwaitOk (D-04)"
        );
    }

    // Both alice and bob receive AwaitOk.
    let woken: std::collections::HashSet<ClientId> = count_await_oks(&outs).into_iter().collect();
    assert_eq!(
        woken,
        [ClientId::from(1_u64), ClientId::from(2_u64)]
            .into_iter()
            .collect(),
        "alice and bob must both be woken"
    );

    // SendOk with both alice and bob in delivered.
    let send_ok = outs.iter().find_map(|o| match o {
        Out::Reply(_, BusReply::SendOk { delivered, .. }) => Some(delivered),
        _ => None,
    });
    assert!(send_ok.is_some(), "SendOk must be present");
    let delivered_names: std::collections::HashSet<String> = send_ok
        .unwrap()
        .iter()
        .filter_map(|d| match &d.to {
            Target::Agent { name } => Some(name.clone()),
            Target::Channel { .. } => None,
        })
        .collect();
    assert!(
        delivered_names.contains("alice"),
        "alice must be in delivered"
    );
    assert!(delivered_names.contains("bob"), "bob must be in delivered");
}

// --- task_id_from regression tests ---

// --- set_listen tests ---------------------------------------------

fn find_set_listen_ok(outs: &[Out]) -> Option<bool> {
    outs.iter().find_map(|o| match o {
        Out::Reply(_, BusReply::SetListenOk { listen_mode }) => Some(*listen_mode),
        _ => None,
    })
}

fn find_err_kind(outs: &[Out]) -> Option<BusErrorKind> {
    outs.iter().find_map(|o| match o {
        Out::Reply(_, BusReply::Err { kind, .. }) => Some(*kind),
        _ => None,
    })
}

#[test]
fn set_listen_flips_canonical_holder_flag() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();
    hello_canonical(&mut broker, 1, "alice", now);
    register(&mut broker, 1, "alice", 999, now);
    // Default after register-with-listen=false is listen_mode=false.
    assert!(
        !broker
            .view()
            .clients
            .iter()
            .find(|c| c.name == "alice")
            .unwrap()
            .listen_mode
    );
    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::SetListen { listen: true },
        },
        now,
    );
    assert_eq!(find_set_listen_ok(&outs), Some(true));
    assert!(
        broker
            .view()
            .clients
            .iter()
            .find(|c| c.name == "alice")
            .unwrap()
            .listen_mode
    );
    // Flip back.
    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::SetListen { listen: false },
        },
        now,
    );
    assert_eq!(find_set_listen_ok(&outs), Some(false));
    assert!(
        !broker
            .view()
            .clients
            .iter()
            .find(|c| c.name == "alice")
            .unwrap()
            .listen_mode
    );
}

#[test]
fn set_listen_before_register_returns_not_registered() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();
    hello_canonical(&mut broker, 1, "alice", now);
    // No Register before SetListen.
    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::SetListen { listen: true },
        },
        now,
    );
    assert_eq!(find_err_kind(&outs), Some(BusErrorKind::NotRegistered));
    assert!(find_set_listen_ok(&outs).is_none());
}

#[test]
fn set_listen_from_proxy_returns_not_registered() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();
    hello_canonical(&mut broker, 1, "alice-holder", now);
    register(&mut broker, 1, "alice", 999, now);
    let _ = hello_proxy(&mut broker, 2, "alice", now);
    // Proxy (client 2) must not mutate canonical holder's flag.
    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::SetListen { listen: true },
        },
        now,
    );
    assert_eq!(find_err_kind(&outs), Some(BusErrorKind::NotRegistered));
    // Canonical holder's flag is unchanged (still default false).
    assert!(
        !broker
            .view()
            .clients
            .iter()
            .find(|c| c.name == "alice")
            .unwrap()
            .listen_mode
    );
}

#[test]
fn set_listen_from_proxy_does_not_touch_canonical_last_activity() {
    // Fix 5 regression: a rejected proxy SetListen must NOT advance
    // the canonical holder's `last_activity`. Before the fix, the
    // dispatch-loop `touch_activity` call mapped the proxy's
    // connection to the canonical holder before `set_listen` ran
    // its rejection, so a misbehaving proxy could make a holder
    // appear active when it wasn't.
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();
    hello_canonical(&mut broker, 1, "alice-holder", now);
    register(&mut broker, 1, "alice", 999, now);
    let _ = hello_proxy(&mut broker, 2, "alice", now);

    let baseline = broker
        .view()
        .clients
        .iter()
        .find(|c| c.name == "alice")
        .unwrap()
        .last_activity;

    // Sleep at least one millisecond so any unwanted timestamp
    // mutation would be observable (SystemTime::now resolution is
    // platform-dependent, but never finer than ms in practice).
    std::thread::sleep(std::time::Duration::from_millis(2));

    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::SetListen { listen: true },
        },
        now,
    );
    assert_eq!(find_err_kind(&outs), Some(BusErrorKind::NotRegistered));

    let after = broker
        .view()
        .clients
        .iter()
        .find(|c| c.name == "alice")
        .unwrap()
        .last_activity;
    assert_eq!(
        after, baseline,
        "rejected proxy SetListen must not advance canonical holder's last_activity"
    );
}

#[test]
fn set_listen_accepted_call_still_advances_last_activity() {
    // Regression guard for Fix 5: the SetListen exclusion in the
    // pre-dispatch `touch_activity` call must NOT prevent the
    // canonical holder's own accepted SetListen from updating
    // last_activity. The success path in `set_listen` explicitly
    // stamps `last_activity` (handle.rs:134 at edit time).
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();
    hello_canonical(&mut broker, 1, "alice", now);
    register(&mut broker, 1, "alice", 999, now);

    let baseline = broker
        .view()
        .clients
        .iter()
        .find(|c| c.name == "alice")
        .unwrap()
        .last_activity;
    std::thread::sleep(std::time::Duration::from_millis(2));

    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::SetListen { listen: true },
        },
        now,
    );
    assert_eq!(find_set_listen_ok(&outs), Some(true));

    let after = broker
        .view()
        .clients
        .iter()
        .find(|c| c.name == "alice")
        .unwrap()
        .last_activity;
    assert!(
        after > baseline,
        "accepted canonical SetListen must advance last_activity"
    );
}

#[test]
fn task_id_from_reads_envelope_id_field() {
    // Regression: previously read `task_id`, which was always absent,
    // so SendOk always returned Uuid::nil(). Field is named `id`.
    let envelope = serde_json::json!({
        "id": "0193abcd-ef01-7000-8000-000000000001",
        "from": "agent:local.bus/x",
        "to": "agent:local.bus/y",
    });
    let parsed = super::task_id_from(&envelope);
    assert_eq!(
        parsed,
        uuid::Uuid::parse_str("0193abcd-ef01-7000-8000-000000000001").unwrap(),
    );
    assert_ne!(parsed, uuid::Uuid::nil());
}

#[test]
fn task_id_from_returns_nil_when_id_absent() {
    let envelope = serde_json::json!({});
    assert_eq!(super::task_id_from(&envelope), uuid::Uuid::nil());
}

// ── Scope B (260619): inbox merges joined channels into the response ────────
//
// Pre-fix the broker's `BusMessage::Inbox` handler resolved only
// `MailboxName::Agent(name)` and silently dropped channel posts even
// when the canonical holder's `state.joined` set contained the channel.
// These tests pin the post-fix contract: (a) joined channels are merged
// into a single `InboxOk`, (b) per-channel cursors advance independently
// per holder, (c) `Leave` drops the channel cursor so a `Join` after
// leaving replays from a fresh end-offset rather than the stale one,
// and (d) the per-channel drain cap caps how many envelopes a single
// poll can return for a hot channel.

fn handshake_register_join(
    broker: &mut Broker<TestEnv>,
    env: &TestEnv,
    client: u64,
    name: &str,
    pid: u32,
    channel: &str,
    now: Instant,
) {
    hello_canonical(broker, client, name, now);
    register(broker, client, name, pid, now);
    let join_outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(client),
            msg: BusMessage::Join {
                channel: channel.into(),
                role: None,
            },
        },
        now,
    );
    apply_mailbox(env, &join_outs);
}

fn inbox_reply(outs: &[Out], client: u64) -> Option<(Vec<serde_json::Value>, u64)> {
    outs.iter().find_map(|out| match out {
        Out::Reply(
            ClientId(c),
            BusReply::InboxOk {
                envelopes,
                next_offset,
            },
        ) if *c == client => Some((envelopes.clone(), *next_offset)),
        _ => None,
    })
}

#[test]
fn inbox_merges_joined_channels_into_response() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env.clone());
    let now = Instant::now();

    // Two holders both join #planning.
    handshake_register_join(&mut broker, &env, 1, "alice", 100, "#planning", now);
    handshake_register_join(&mut broker, &env, 2, "bob", 200, "#planning", now);

    // bob posts a channel envelope. AppendMailbox is captured by the
    // test env in `apply_mailbox`, so the next drain_from sees it.
    let send_outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::Send {
                to: Target::Channel {
                    name: "#planning".into(),
                },
                envelope: audit_log_envelope(42, "bob"),
            },
        },
        now,
    );
    apply_mailbox(&env, &send_outs);

    // alice polls Inbox — should see bob's channel post.
    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Inbox {
                since: Some(0),
                include_terminal: None,
            },
        },
        now,
    );
    let (envelopes, _) = inbox_reply(&outs, 1).expect("alice should get InboxOk");
    assert_eq!(
        envelopes.len(),
        1,
        "alice's inbox should surface bob's #planning post; got {envelopes:?}"
    );
    assert_eq!(
        envelopes[0]["body"]["details"]["seq"].as_u64().unwrap(),
        42,
        "envelope payload must be bob's seq=42 post"
    );
}

#[test]
fn inbox_channel_cursor_advances_per_holder() {
    // After alice reads the channel, a second poll with no new posts
    // returns nothing — the per-channel cursor advanced server-side.
    let env = TestEnv::default();
    let mut broker = Broker::new(env.clone());
    let now = Instant::now();

    handshake_register_join(&mut broker, &env, 1, "alice", 100, "#planning", now);
    handshake_register_join(&mut broker, &env, 2, "bob", 200, "#planning", now);

    let send_outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::Send {
                to: Target::Channel {
                    name: "#planning".into(),
                },
                envelope: audit_log_envelope(1, "bob"),
            },
        },
        now,
    );
    apply_mailbox(&env, &send_outs);

    let first = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Inbox {
                since: Some(0),
                include_terminal: None,
            },
        },
        now,
    );
    let (envs1, _) = inbox_reply(&first, 1).unwrap();
    assert_eq!(envs1.len(), 1, "first poll surfaces the one post");

    let second = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Inbox {
                since: Some(0),
                include_terminal: None,
            },
        },
        now,
    );
    let (envs2, _) = inbox_reply(&second, 1).unwrap();
    assert!(
        envs2.is_empty(),
        "second poll must return no envelopes — per-channel cursor advanced; got {envs2:?}"
    );
}

/// Adversarial-review regression (2026-06-19, HIGH): a task-filtered
/// `Await` on a joined channel must NOT advance the inbox cursor past
/// envelopes whose task didn't match. Pre-fix, `Inbox` shared the same
/// `await_offsets[Channel(c)]` cursor that `await_envelope` writes
/// unconditionally (even when filter mismatch yields an empty batch),
/// so unrelated channel posts were silently dropped from later Inbox
/// polls.
#[test]
fn inbox_preserves_channel_envelopes_after_unmatched_task_filtered_await() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env.clone());
    let now = Instant::now();

    handshake_register_join(&mut broker, &env, 1, "alice", 100, "#planning", now);
    handshake_register_join(&mut broker, &env, 2, "bob", 200, "#planning", now);

    // Bob posts an envelope to the channel. The audit_log envelope has
    // no causality.ref, so an AwaitFilter::Task(_) will not match it.
    let send_outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::Send {
                to: Target::Channel {
                    name: "#planning".into(),
                },
                envelope: audit_log_envelope(9, "bob"),
            },
        },
        now,
    );
    apply_mailbox(&env, &send_outs);

    // Alice does a task-filtered Await for an unrelated task. The
    // channel batch will be empty (no match) but pre-fix the cursor
    // advanced anyway, robbing Inbox of bob's post.
    let other_task = uuid::Uuid::parse_str("01890000-0000-7000-8000-deadbeefcafe").unwrap();
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Await {
                timeout_ms: 0,
                task: Some(other_task),
            },
        },
        now,
    );

    // Alice's Inbox poll MUST still surface bob's post.
    let inbox_outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Inbox {
                since: Some(0),
                include_terminal: None,
            },
        },
        now,
    );
    let (envs, _) = inbox_reply(&inbox_outs, 1).expect("alice should get InboxOk");
    assert_eq!(
        envs.len(),
        1,
        "task-filtered Await must not eat unrelated channel envelopes from Inbox; got {envs:?}"
    );
    assert_eq!(
        envs[0]["body"]["details"]["seq"].as_u64().unwrap(),
        9,
        "envelope must be bob's seq=9 post"
    );
}

/// Debug 999.1 — pins the accepted boundary (human-verify checkpoint,
/// 2026-07-01): a strictly-filtered `Await` blocked behind an earlier,
/// real, filter-mismatched envelope must resolve as `AwaitTimeout` (never
/// a broker `Internal` error, never a spurious delivery of the mismatched
/// envelope). The blocking envelope must stay reachable on disk — not
/// silently discarded — and a later differently-filtered (here:
/// unfiltered) `Await` from the same client must successfully drain past
/// it, delivering BOTH the blocking envelope and the envelope that was
/// stuck behind it. This is the KNOWN, ACCEPTED, OUT-OF-SCOPE residual
/// (strict single-filter awaiter starvation); the complete fix is the
/// broker-owned-delivery-position redesign tracked at
/// `.planning/phases/999.11-broker-owned-delivery-position/SPEC.md`.
#[test]
fn strict_filtered_await_blocked_behind_mismatch_reports_timeout_then_unblocks_on_unfiltered_await()
{
    let env = TestEnv::default();
    let mut broker = Broker::new(env.clone());
    let now = Instant::now();

    hello_canonical(&mut broker, 1, "alice", now);
    register(&mut broker, 1, "alice", 100, now);
    hello_canonical(&mut broker, 2, "bob", now);
    register(&mut broker, 2, "bob", 200, now);

    let task_a = uuid::Uuid::parse_str("01890000-0000-7000-8000-00000000000a").unwrap();
    let task_b = uuid::Uuid::parse_str("01890000-0000-7000-8000-00000000000b").unwrap();

    // Bob's task-B reply lands first and sits un-drained: the earlier,
    // real, filter-mismatched envelope that will block alice's
    // task-A-filtered offset.
    let send_b = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::Send {
                to: Target::Agent {
                    name: "alice".into(),
                },
                envelope: audit_log_reply_envelope(1, "bob", "alice", task_b),
            },
        },
        now,
    );
    apply_mailbox(&env, &send_b);

    // Alice parks a strictly task-A-filtered Await. Nothing matches yet;
    // `drain_await_batch` walks into task B's real mismatch immediately
    // and stops (fully_drained=false, envelopes empty) instead of
    // skipping past it, so `await_envelope` parks rather than replying.
    let park_outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Await {
                timeout_ms: 10_000,
                task: Some(task_a),
            },
        },
        now,
    );
    assert!(
        park_outs.iter().any(
            |o| matches!(o, Out::ParkAwait { client, .. } if *client == ClientId::from(1_u64))
        ),
        "alice's task-A await should park, not reply immediately: {park_outs:?}"
    );

    // Bob's task-A reply arrives next. It matches alice's filter and
    // triggers the wake path (`waiting_clients_for_name` selects alice),
    // but task B's earlier reply is still sitting un-drained ahead of it
    // in the mailbox — `await_reply_for_mailbox`'s
    // `Ok(batch) if !batch.fully_drained` arm must fire (see the
    // operator-visible `tracing::info!` added alongside it).
    let send_a = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::Send {
                to: Target::Agent {
                    name: "alice".into(),
                },
                envelope: audit_log_reply_envelope(2, "bob", "alice", task_a),
            },
        },
        now,
    );

    let alice_wake_reply = send_a
        .iter()
        .find_map(|o| match o {
            Out::Reply(client, reply) if *client == ClientId::from(1_u64) => Some(reply),
            _ => None,
        })
        .expect("alice must get a reply on wake");
    assert!(
        matches!(alice_wake_reply, BusReply::AwaitTimeout {}),
        "blocked filtered await must resolve as AwaitTimeout, not an Internal error or a \
         mismatched delivery: {alice_wake_reply:?}"
    );

    apply_mailbox(&env, &send_a);

    // The blocking envelope (task B's reply) was never discarded: a
    // later, differently-filtered (here: unfiltered) Await from alice
    // must drain past it and deliver BOTH envelopes — proving nothing
    // was permanently lost.
    let drain_outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Await {
                timeout_ms: 0,
                task: None,
            },
        },
        now,
    );
    let envelopes = drain_outs
        .iter()
        .find_map(|o| match o {
            Out::Reply(client, BusReply::AwaitOk { envelopes, .. })
                if *client == ClientId::from(1_u64) =>
            {
                Some(envelopes.clone())
            }
            _ => None,
        })
        .expect("unfiltered await must deliver both stranded envelopes");
    assert_eq!(
        envelopes.len(),
        2,
        "unfiltered await must unblock and deliver BOTH the blocking task-B envelope and \
         the previously-stuck task-A envelope: {envelopes:?}"
    );
    assert_eq!(envelopes[0]["body"]["details"]["seq"].as_u64().unwrap(), 1);
    assert_eq!(envelopes[1]["body"]["details"]["seq"].as_u64().unwrap(), 2);
}

fn park_unfiltered_await(broker: &mut Broker<TestEnv>, client: u64, now: Instant) {
    let park_outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(client),
            msg: BusMessage::Await {
                timeout_ms: 10_000,
                task: None,
            },
        },
        now,
    );
    assert!(
        park_outs
            .iter()
            .any(|o| matches!(o, Out::ParkAwait { client: c, .. } if *c == ClientId::from(client))),
        "client {client} should park: {park_outs:?}"
    );
}

/// Assert the author was not unparked / await-woken by their own channel post.
fn assert_author_await_survived_own_channel_post(
    broker: &Broker<TestEnv>,
    send_outs: &[Out],
    author: u64,
) {
    let author_id = ClientId::from(author);
    assert!(
        !send_outs
            .iter()
            .any(|o| matches!(o, Out::UnparkAwait { client } if *client == author_id)),
        "author must not be unparked by own channel post: {send_outs:?}"
    );
    assert!(
        !send_outs.iter().any(|o| matches!(
            o,
            Out::Reply(
                client,
                BusReply::AwaitOk { .. }
                    | BusReply::AwaitTimeout {}
                    | BusReply::Err {
                        kind: BusErrorKind::Internal,
                        ..
                    }
            ) if *client == author_id
        )),
        "author must not get await wake/timeout/Internal for own post: {send_outs:?}"
    );
    assert!(
        broker.state.pending_awaits.contains_key(&author_id),
        "author's parked await must survive own channel post"
    );
}

fn assert_member_woke_with_authors_post(broker: &Broker<TestEnv>, send_outs: &[Out], member: u64) {
    let member_id = ClientId::from(member);
    let reply = send_outs.iter().find_map(|o| match o {
        Out::Reply(client, reply) if *client == member_id => Some(reply),
        _ => None,
    });
    match reply {
        Some(BusReply::AwaitOk { envelopes, .. }) => {
            assert_eq!(envelopes.len(), 1, "member should receive author's post");
            assert_eq!(
                envelopes[0]["from"].as_str().unwrap(),
                "agent:example.test/alice"
            );
        }
        other => panic!("member must get AwaitOk with author's post, got {other:?}"),
    }
    assert!(
        send_outs
            .iter()
            .any(|o| matches!(o, Out::UnparkAwait { client } if *client == member_id)),
        "member must be unparked: {send_outs:?}"
    );
    assert!(
        !broker.state.pending_awaits.contains_key(&member_id),
        "member should no longer be parked after delivery"
    );
}

/// Issue #15: a listen-mode agent parked on `Await` that posts to a
/// channel it has joined must stay parked. Before the fix, `send_channel`
/// woke the author, the self-authored drain produced an empty fully-
/// drained batch, and `await_reply_for_mailbox` returned `Internal` —
/// killing the Stop-hook 23h listener.
#[test]
fn self_authored_channel_post_does_not_wake_or_kill_authors_parked_await() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env.clone());
    let now = Instant::now();

    handshake_register_join(&mut broker, &env, 1, "alice", 100, "#planning", now);
    handshake_register_join(&mut broker, &env, 2, "bob", 200, "#planning", now);
    park_unfiltered_await(&mut broker, 1, now);
    park_unfiltered_await(&mut broker, 2, now);
    assert!(
        broker
            .state
            .pending_awaits
            .contains_key(&ClientId::from(1_u64))
            && broker
                .state
                .pending_awaits
                .contains_key(&ClientId::from(2_u64)),
        "both alice and bob must be parked before the post"
    );

    let send_outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Send {
                to: Target::Channel {
                    name: "#planning".into(),
                },
                envelope: audit_log_envelope(7, "alice"),
            },
        },
        now,
    );

    assert_author_await_survived_own_channel_post(&broker, &send_outs, 1);
    assert_member_woke_with_authors_post(&broker, &send_outs, 2);
}

/// Adversarial-review regression (2026-06-19, MEDIUM): channel Inbox
/// must NOT deliver the holder's OWN channel posts. The Await path
/// already applies `is_self_authored` filtering (pub/sub: a publisher
/// never receives its own posts); Inbox was inconsistent and would
/// hand the sender their own envelope back.
#[test]
fn inbox_does_not_deliver_self_authored_channel_posts() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env.clone());
    let now = Instant::now();

    handshake_register_join(&mut broker, &env, 1, "alice", 100, "#planning", now);

    // Alice posts to the channel.
    let send_outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Send {
                to: Target::Channel {
                    name: "#planning".into(),
                },
                envelope: audit_log_envelope(7, "alice"),
            },
        },
        now,
    );
    apply_mailbox(&env, &send_outs);

    // Alice polls Inbox — should NOT see her own envelope.
    let inbox_outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Inbox {
                since: Some(0),
                include_terminal: None,
            },
        },
        now,
    );
    let (envs, _) = inbox_reply(&inbox_outs, 1).expect("alice should get InboxOk");
    assert!(
        envs.is_empty(),
        "alice's Inbox must not deliver her own channel posts; got {envs:?}"
    );
}

#[test]
fn leave_drops_channel_cursor() {
    // After alice leaves the channel, the channel cursor in
    // `await_offsets` is dropped. A rejoin re-initializes the cursor
    // to the channel's join-time end-offset (the value `Join` sets),
    // so posts after the rejoin are seen, and pre-leave posts the
    // holder has already read are not double-delivered.
    let env = TestEnv::default();
    let mut broker = Broker::new(env.clone());
    let now = Instant::now();

    handshake_register_join(&mut broker, &env, 1, "alice", 100, "#planning", now);
    handshake_register_join(&mut broker, &env, 2, "bob", 200, "#planning", now);

    // Bob posts; alice drains via Inbox (advancing the cursor).
    let send1 = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::Send {
                to: Target::Channel {
                    name: "#planning".into(),
                },
                envelope: audit_log_envelope(1, "bob"),
            },
        },
        now,
    );
    apply_mailbox(&env, &send1);
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Inbox {
                since: Some(0),
                include_terminal: None,
            },
        },
        now,
    );

    // Confirm alice's per-channel Inbox cursor is set post-drain.
    // (HIGH-fix 2026-06-19: Inbox now uses inbox_offsets, not
    // await_offsets — the two were split so a task-filtered Await
    // cannot eat envelopes out of Inbox's view.)
    let mailbox = MailboxName::Channel("#planning".into());
    let before_leave = broker
        .state
        .clients
        .get(&ClientId::from(1_u64))
        .and_then(|state| state.inbox_offsets.get(&mailbox).copied());
    assert!(
        before_leave.is_some_and(|cursor| cursor > 0),
        "alice should have a per-channel Inbox cursor after drain; got {before_leave:?}"
    );

    // Alice leaves.
    let _ = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Leave {
                channel: "#planning".into(),
            },
        },
        now,
    );

    // Both cursors must drop on leave (await_offsets and inbox_offsets).
    let after_leave_inbox = broker
        .state
        .clients
        .get(&ClientId::from(1_u64))
        .and_then(|state| state.inbox_offsets.get(&mailbox).copied());
    assert!(
        after_leave_inbox.is_none(),
        "leave() must drop the inbox cursor; got {after_leave_inbox:?}"
    );
    let after_leave = broker
        .state
        .clients
        .get(&ClientId::from(1_u64))
        .and_then(|state| state.await_offsets.get(&mailbox).copied());
    assert!(
        after_leave.is_none(),
        "leave() must drop the await cursor; got {after_leave:?}"
    );
}

// ── 260708-g01: register/join drain size warning ─────────────────────────────
//
// `drained_span` is the pure kernel of the oversized-drain WARN: it decides,
// from `(start_offset, records)` alone, whether `warn_if_drain_oversized`
// fires. Tested directly so the threshold logic is covered without pulling a
// tracing-capture dependency into famp-bus (BUS-01: the crate stays tokio-free
// and dep-free; `just check-no-tokio-in-bus` gates it).
//
// NOTE: the WARN is a WARN, not a cap. `decode_lines` keeps `cap: None` and
// still delivers every record. These tests must not be read as truncation.

fn rec(start: u64, end: u64) -> DrainedRecord {
    DrainedRecord {
        bytes: Vec::new(),
        start,
        end,
    }
}

#[test]
fn drained_span_of_empty_records_is_zero() {
    assert_eq!(drained_span(0, &[]), 0);
    assert_eq!(drained_span(9_999, &[]), 0);
}

#[test]
fn drained_span_measures_start_offset_to_last_record_end() {
    let records = [rec(100, 150), rec(150, 275)];
    assert_eq!(drained_span(100, &records), 175);
    // A drain that began before the first record still spans to the last end.
    assert_eq!(drained_span(0, &records), 275);
}

#[test]
fn drained_span_saturates_when_start_offset_is_past_the_last_end() {
    // Never underflows; an inverted range simply reports a zero-byte span,
    // so the WARN cannot fire spuriously on a u64 wrap.
    assert_eq!(drained_span(500, &[rec(100, 150)]), 0);
}

#[test]
fn drain_warn_threshold_is_half_the_reply_frame_limit() {
    assert_eq!(
        DRAIN_WARN_BYTES * 2,
        MAX_FRAME_BYTES as u64,
        "DRAIN_WARN_BYTES must stay pinned to half of MAX_FRAME_BYTES; if the \
         frame limit moves, this warning's headroom moves with it"
    );
}

#[test]
fn drain_warn_fires_only_strictly_above_the_threshold() {
    // Exactly at the threshold: not oversized. One byte past: oversized.
    let at = [rec(0, DRAIN_WARN_BYTES)];
    let past = [rec(0, DRAIN_WARN_BYTES + 1)];
    assert!(drained_span(0, &at) <= DRAIN_WARN_BYTES);
    assert!(drained_span(0, &past) > DRAIN_WARN_BYTES);
}

#[test]
fn oversized_drain_is_warned_but_never_truncated() {
    // The contract §3.1 pins: decode_lines delivers EVERY decodable record
    // regardless of span. Two oversized records in, two envelopes out.
    let mailbox = crate::MailboxName::Agent("alice".to_string());
    let big = 5 * 1024 * 1024;
    let line = |seq: u64| {
        famp_canonical::canonicalize(&serde_json::json!({
            "famp": "0.5.2",
            "class": "audit_log",
            "scope": "standalone",
            "id": "01890000-0000-7000-8000-000000000001",
            "from": "agent:example.test/bob",
            "to": "agent:example.test/alice",
            "authority": "advisory",
            "ts": "2026-04-27T12:00:00Z",
            "body": { "event": "offline_message", "details": { "seq": seq } }
        }))
        .expect("fixture envelope must canonicalize")
    };
    // Byte spans are synthetic: `drained_span` reads the carried offsets, not
    // `bytes.len()`, so a small payload can stand in for an oversized record.
    let records = vec![
        DrainedRecord {
            bytes: line(0),
            start: 0,
            end: big,
        },
        DrainedRecord {
            bytes: line(1),
            start: big,
            end: big * 2,
        },
    ];
    assert!(
        drained_span(0, &records) > DRAIN_WARN_BYTES,
        "fixture must be oversized for this test to mean anything"
    );
    let drained = DrainResult {
        next_offset: big * 2,
        records,
    };
    let decoded = decode_lines(&mailbox, 0, &drained);
    assert_eq!(
        decoded.len(),
        2,
        "cap: None — an oversized drain must WARN, not truncate"
    );
}

// ── 260708-l1x (#11): a mailbox that shrinks beneath a holder's cursor ───────
//
// `/famp-clear` truncates `~/.famp/mailboxes/*.jsonl` while the broker is
// running and still holding in-memory cursors into them. Pre-fix the holder's
// cursor never came back down, so the holder silently stopped receiving — and
// in a debug build `drain_await_batch`'s `debug_assert_eq!(next_offset,
// drained.next_offset)` panicked the whole broker actor task instead.

fn alice_mailbox() -> MailboxName {
    MailboxName::Agent("alice".into())
}

/// The exact bytes the broker would append for `envelope` (canonical JSON,
/// no trailing `\n` — `InMemoryMailbox` supplies the framing).
fn mailbox_line(envelope: &serde_json::Value) -> Vec<u8> {
    famp_canonical::canonicalize(envelope).expect("fixture envelope must canonicalize")
}

fn await_now(broker: &mut Broker<TestEnv>, client: u64, now: Instant) -> Vec<Out> {
    broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(client),
            msg: BusMessage::Await {
                timeout_ms: 10_000,
                task: None,
            },
        },
        now,
    )
}

fn await_ok(outs: &[Out], client: u64) -> Option<(Vec<serde_json::Value>, u64)> {
    outs.iter().find_map(|out| match out {
        Out::Reply(
            ClientId(c),
            BusReply::AwaitOk {
                envelopes,
                next_offset,
                ..
            },
        ) if *c == client => Some((envelopes.clone(), *next_offset)),
        _ => None,
    })
}

fn parked(outs: &[Out]) -> bool {
    outs.iter().any(|out| matches!(out, Out::ParkAwait { .. }))
}

fn await_cursor(broker: &Broker<TestEnv>, client: u64, mailbox: &MailboxName) -> Option<u64> {
    broker
        .state
        .clients
        .get(&ClientId::from(client))
        .and_then(|state| state.await_offsets.get(mailbox).copied())
}

fn inbox_cursor(broker: &Broker<TestEnv>, client: u64, mailbox: &MailboxName) -> Option<u64> {
    broker
        .state
        .clients
        .get(&ClientId::from(client))
        .and_then(|state| state.inbox_offsets.get(mailbox).copied())
}

fn seqs(envelopes: &[serde_json::Value]) -> Vec<u64> {
    envelopes
        .iter()
        .map(|envelope| envelope["body"]["details"]["seq"].as_u64().unwrap())
        .collect()
}

/// Register alice (client 1) and bob (client 2) as canonical holders.
fn register_alice_and_bob(broker: &mut Broker<TestEnv>, now: Instant) {
    hello_canonical(broker, 1, "alice", now);
    register(broker, 1, "alice", 100, now);
    hello_canonical(broker, 2, "bob", now);
    register(broker, 2, "bob", 200, now);
}

/// bob DMs alice through the broker, landing the append in the test mailbox.
fn bob_dms_alice(
    broker: &mut Broker<TestEnv>,
    env: &TestEnv,
    seq: usize,
    now: Instant,
) -> Vec<Out> {
    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(2_u64),
            msg: BusMessage::Send {
                to: Target::Agent {
                    name: "alice".into(),
                },
                envelope: audit_log_envelope(seq, "bob"),
            },
        },
        now,
    );
    apply_mailbox(env, &outs);
    outs
}

/// #11, agent mailbox, plain (unparked) await path.
///
/// Pre-fix: `walk` seeds `next_offset = since` and returns it untouched for a
/// zero-record drain, so `await_envelope`'s `if batch.next_offset != since`
/// never fires and the cursor stays at the pre-truncation offset forever. The
/// holder never sees another message. (In this debug-assertions build it
/// panics in `drain_await_batch` before it even gets that far.)
#[test]
fn truncated_agent_mailbox_heals_cursor_and_delivers_after_regrowth() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env.clone());
    let now = Instant::now();
    register_alice_and_bob(&mut broker, now);

    for seq in [1, 2] {
        bob_dms_alice(&mut broker, &env, seq, now);
    }

    let outs = await_now(&mut broker, 1, now);
    let (envelopes, stale_cursor) = await_ok(&outs, 1).expect("alice drains both DMs");
    assert_eq!(seqs(&envelopes), vec![1, 2]);
    assert_eq!(
        await_cursor(&broker, 1, &alice_mailbox()),
        Some(stale_cursor)
    );

    // External truncation, mid-flight: `/famp-clear` empties the file.
    env.mailbox.truncate(&alice_mailbox());

    // The healing drain. Nothing to deliver, so alice parks — but her cursor
    // MUST come down to the mailbox's new end before she does.
    let outs = await_now(&mut broker, 1, now);
    assert!(await_ok(&outs, 1).is_none(), "nothing to deliver yet");
    assert!(parked(&outs), "alice parks: {outs:?}");
    assert_eq!(
        await_cursor(&broker, 1, &alice_mailbox()),
        Some(0),
        "cursor must clamp to the truncated mailbox's end (was {stale_cursor})"
    );

    // The mailbox regrows. Append directly so alice's parked await is not
    // woken — this pins the plain drain path, not the wake path.
    env.mailbox.append(
        &alice_mailbox(),
        mailbox_line(&audit_log_envelope(3, "bob")),
    );

    let outs = await_now(&mut broker, 1, now);
    let (envelopes, next_offset) =
        await_ok(&outs, 1).expect("a healed cursor must deliver the regrown record");
    assert_eq!(seqs(&envelopes), vec![3]);
    let eof = env
        .mailbox
        .drain_from(&alice_mailbox(), 0)
        .unwrap()
        .next_offset;
    assert_eq!(next_offset, eof);
}

/// #11, channel mailbox, `Inbox` path.
///
/// Pre-fix `inbox`'s channel loop hits `if drained.records.is_empty() {
/// continue; }` and skips the `inbox_offsets` write-back entirely, so a
/// truncated channel mailbox strands the cursor exactly the same way. No
/// `debug_assert` guards this path, so pre-fix it fails silently rather than
/// panicking.
#[test]
fn truncated_channel_mailbox_heals_inbox_cursor_and_delivers_after_regrowth() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env.clone());
    let now = Instant::now();
    let channel = MailboxName::Channel("#planning".into());

    handshake_register_join(&mut broker, &env, 1, "alice", 100, "#planning", now);
    handshake_register_join(&mut broker, &env, 2, "bob", 200, "#planning", now);

    let post = |broker: &mut Broker<TestEnv>, env: &TestEnv, seq: usize| {
        let outs = broker.handle(
            BrokerInput::Wire {
                client: ClientId::from(2_u64),
                msg: BusMessage::Send {
                    to: Target::Channel {
                        name: "#planning".into(),
                    },
                    envelope: audit_log_envelope(seq, "bob"),
                },
            },
            now,
        );
        apply_mailbox(env, &outs);
    };
    let poll_inbox = |broker: &mut Broker<TestEnv>| {
        broker.handle(
            BrokerInput::Wire {
                client: ClientId::from(1_u64),
                msg: BusMessage::Inbox {
                    since: Some(0),
                    include_terminal: None,
                },
            },
            now,
        )
    };

    post(&mut broker, &env, 1);
    post(&mut broker, &env, 2);

    let outs = poll_inbox(&mut broker);
    let (envelopes, _) = inbox_reply(&outs, 1).expect("alice gets InboxOk");
    assert_eq!(seqs(&envelopes), vec![1, 2]);
    let stale_cursor = inbox_cursor(&broker, 1, &channel).expect("channel cursor set");
    assert!(stale_cursor > 0);

    env.mailbox.truncate(&channel);

    let outs = poll_inbox(&mut broker);
    let (envelopes, _) = inbox_reply(&outs, 1).expect("alice gets InboxOk");
    assert!(envelopes.is_empty(), "nothing to deliver yet");
    assert_eq!(
        inbox_cursor(&broker, 1, &channel),
        Some(0),
        "the empty-records `continue` must not skip the clamp (was {stale_cursor})"
    );

    post(&mut broker, &env, 3);
    let outs = poll_inbox(&mut broker);
    let (envelopes, _) = inbox_reply(&outs, 1).expect("alice gets InboxOk");
    assert_eq!(
        seqs(&envelopes),
        vec![3],
        "a healed inbox cursor must deliver the regrown channel post"
    );
}

/// #11, debug-build broker panic, wake path.
///
/// alice parks a clean await; the mailbox is then truncated beneath her; bob's
/// send wakes her. `await_reply_for_mailbox` → `drain_await_batch` reaches the
/// `fully_drained` arm with `next_offset = stale_cursor` and
/// `drained.next_offset = 0`, and `debug_assert_eq!` panics the broker actor
/// task — taking down every connected client. `cargo test` builds with debug
/// assertions on, so this test IS the reproduction.
#[test]
fn truncated_mailbox_wake_path_does_not_panic_and_delivers_the_trigger() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env.clone());
    let now = Instant::now();
    register_alice_and_bob(&mut broker, now);

    bob_dms_alice(&mut broker, &env, 1, now);
    let outs = await_now(&mut broker, 1, now);
    let (envelopes, stale_cursor) = await_ok(&outs, 1).expect("alice drains the first DM");
    assert_eq!(seqs(&envelopes), vec![1]);
    assert!(stale_cursor > 0);

    // A clean park: mailbox is caught up, nothing truncated yet.
    let outs = await_now(&mut broker, 1, now);
    assert!(parked(&outs), "alice parks with nothing new: {outs:?}");

    // Now the mailbox shrinks beneath the parked holder.
    env.mailbox.truncate(&alice_mailbox());

    // bob's send takes the wake path, folding the trigger envelope into the
    // reply without re-draining it.
    let outs = bob_dms_alice(&mut broker, &env, 2, now);
    let (envelopes, next_offset) =
        await_ok(&outs, 1).expect("the wake must deliver through a truncated mailbox");
    assert_eq!(seqs(&envelopes), vec![2]);

    // The folded trigger offset is framed from `next_offset`, so a stale
    // `next_offset` would also hand the client a cursor past the real EOF.
    let eof = env
        .mailbox
        .drain_from(&alice_mailbox(), 0)
        .unwrap()
        .next_offset;
    assert_eq!(
        next_offset, eof,
        "the reply's resume offset must match where the record actually landed"
    );
    assert_eq!(await_cursor(&broker, 1, &alice_mailbox()), Some(eof));
}

/// #11, KNOWN BOUNDARY — not a bug being fixed, a limit being pinned.
///
/// The clamp lands the cursor on the drain's `next_offset`, i.e. the mailbox's
/// CURRENT end. A record appended between the truncation and the first drain
/// that observes it therefore sits BELOW the clamped cursor and is skipped:
/// clamping cannot recover it, because the past-EOF drain that detects the
/// truncation returns zero records by construction. The alternative — clamping
/// to 0 and replaying whatever the file now holds — trades this bounded loss
/// for duplicate delivery, and is not what #11 specifies.
///
/// What the clamp does guarantee: the holder is no longer stalled forever.
#[test]
fn record_appended_before_the_healing_drain_is_skipped_but_does_not_stall() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env.clone());
    let now = Instant::now();
    register_alice_and_bob(&mut broker, now);

    for seq in [1, 2] {
        bob_dms_alice(&mut broker, &env, seq, now);
    }
    let outs = await_now(&mut broker, 1, now);
    let (_, stale_cursor) = await_ok(&outs, 1).expect("alice drains both DMs");

    env.mailbox.truncate(&alice_mailbox());
    // Appended BEFORE any drain observes the truncation, and below the stale
    // cursor. Direct append so alice's (unparked) state is untouched.
    env.mailbox.append(
        &alice_mailbox(),
        mailbox_line(&audit_log_envelope(3, "bob")),
    );
    let eof = env
        .mailbox
        .drain_from(&alice_mailbox(), 0)
        .unwrap()
        .next_offset;
    assert!(eof < stale_cursor, "fixture: the mailbox really did shrink");

    let outs = await_now(&mut broker, 1, now);
    assert!(
        await_ok(&outs, 1).is_none(),
        "seq 3 is below the clamped cursor: documented loss, not delivery"
    );
    assert!(parked(&outs));
    assert_eq!(await_cursor(&broker, 1, &alice_mailbox()), Some(eof));

    // ...but the holder is unstalled: the next record arrives.
    env.mailbox.append(
        &alice_mailbox(),
        mailbox_line(&audit_log_envelope(4, "bob")),
    );
    let outs = await_now(&mut broker, 1, now);
    let (envelopes, _) = await_ok(&outs, 1).expect("the holder must not be stalled");
    assert_eq!(seqs(&envelopes), vec![4]);
}

/// quick-260709-9zu: proves an `Await` requesting more than 1h is no longer
/// clamped to the old `MAX_AWAIT_MS` 1h ceiling. The FAMP Stop hook parks
/// with `--timeout 23h`; before this fix `awaiting.rs:70` silently clamped
/// every await to 3_600_000 ms, so a listen window went deaf ~1h after its
/// last turn. This test fails if `MAX_AWAIT_MS` is reverted to 1h: the
/// 3700s Tick (past the old 1h/3600s ceiling) would then expire the await.
#[test]
fn await_over_one_hour_survives_past_old_1h_ceiling() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();

    hello_canonical(&mut broker, 1, "alice", now);
    register(&mut broker, 1, "alice", 100, now);

    let park_outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Await {
                timeout_ms: 23 * 60 * 60 * 1000,
                task: None,
            },
        },
        now,
    );
    assert!(
        park_outs.iter().any(
            |o| matches!(o, Out::ParkAwait { client, .. } if *client == ClientId::from(1_u64))
        ),
        "a 23h await should park, not reply immediately: {park_outs:?}"
    );

    // Advance past the OLD 1h ceiling (3700s > 3600s).
    let tick_outs = broker.handle(BrokerInput::Tick, now + Duration::from_secs(3700));
    assert!(
        !tick_outs.iter().any(|o| matches!(
            o,
            Out::Reply(client, BusReply::AwaitTimeout {}) if *client == ClientId::from(1_u64)
        )),
        "a 23h await must NOT expire at the old 1h ceiling: {tick_outs:?}"
    );
    assert!(
        broker
            .state
            .pending_awaits
            .contains_key(&ClientId::from(1_u64)),
        "alice's await must still be parked past the old 1h ceiling"
    );
}

/// WR-05 guard regression: a hostile `u64::MAX` timeout must still be
/// clamped by `.min(MAX_AWAIT_MS)` rather than overflowing `Instant + Duration`
/// (which panics and would take down the whole broker actor task).
#[test]
fn await_u64_max_timeout_parks_without_overflow_panic() {
    let env = TestEnv::default();
    let mut broker = Broker::new(env);
    let now = Instant::now();

    hello_canonical(&mut broker, 1, "alice", now);
    register(&mut broker, 1, "alice", 100, now);

    let outs = broker.handle(
        BrokerInput::Wire {
            client: ClientId::from(1_u64),
            msg: BusMessage::Await {
                timeout_ms: u64::MAX,
                task: None,
            },
        },
        now,
    );
    assert!(
        outs.iter().any(
            |o| matches!(o, Out::ParkAwait { client, .. } if *client == ClientId::from(1_u64))
        ),
        "a u64::MAX timeout must still park (clamped by WR-05), not panic: {outs:?}"
    );
}

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use crate::broker::state::{ClientState, ParkedAwait};
use crate::{
    AwaitFilter, Broker, BrokerEnv, BrokerInput, BusErrorKind, BusMessage, BusReply, ClientId,
    Delivered, MailboxName, Out, SessionRow, Target, MAX_FRAME_BYTES,
};

pub(crate) fn handle<E: BrokerEnv>(
    broker: &mut Broker<E>,
    input: BrokerInput,
    now: Instant,
) -> Vec<Out> {
    match input {
        BrokerInput::Wire { client, msg } => handle_wire(broker, client, msg, now),
        BrokerInput::Disconnect(client) => disconnect(broker, client),
        BrokerInput::Tick => tick(broker, now),
    }
}

fn handle_wire<E: BrokerEnv>(
    broker: &mut Broker<E>,
    client: ClientId,
    msg: BusMessage,
    now: Instant,
) -> Vec<Out> {
    if !matches!(msg, BusMessage::Hello { .. })
        && broker.state.clients.get(&client).map(|c| c.handshaked) != Some(true)
    {
        return vec![err(
            client,
            BusErrorKind::BrokerProtoMismatch,
            "Hello required as first frame",
        )];
    }

    match msg {
        BusMessage::Hello {
            bus_proto,
            client: _,
            bind_as,
        } => hello(broker, client, bus_proto, bind_as),
        BusMessage::Register { name, pid } => register(broker, client, name, pid),
        BusMessage::Send { to, envelope } => send(broker, client, to, envelope),
        BusMessage::Inbox {
            since,
            include_terminal: _,
        } => inbox(broker, client, since),
        BusMessage::Await { timeout_ms, task } => {
            await_envelope(broker, client, timeout_ms, task, now)
        }
        BusMessage::Join { channel } => join(broker, client, channel),
        BusMessage::Leave { channel } => leave(broker, client, channel),
        BusMessage::Sessions {} => sessions(broker, client),
        BusMessage::Whoami {} => whoami(broker, client),
    }
}

/// D-10 Hello handler. `bind_as = None` is the existing canonical-holder
/// path. `bind_as = Some(name)` is the proxy path: the broker validates
/// `name` maps to a live registered holder, and rejects with
/// `HelloErr { NotRegistered }` if not.
fn hello<E: BrokerEnv>(
    broker: &mut Broker<E>,
    client: ClientId,
    bus_proto: u32,
    bind_as: Option<String>,
) -> Vec<Out> {
    if let Some(name) = bind_as {
        // D-10: locate the canonical live registered holder for `name`.
        // Cache the holder PID for the per-op liveness re-check; if the
        // holder process has died between Register and our Hello, treat
        // the bind_as as unregistered.
        let holder_pid = broker.state.clients.values().find_map(|state| {
            if state.connected && state.name.as_deref() == Some(name.as_str()) {
                state.pid
            } else {
                None
            }
        });
        let alive = holder_pid.is_some_and(|pid| broker.env.is_alive(pid));
        if !alive {
            return vec![Out::Reply(
                client,
                BusReply::HelloErr {
                    kind: BusErrorKind::NotRegistered,
                    message: format!("bind_as identity '{name}' is not registered"),
                },
            )];
        }
        broker.state.clients.insert(
            client,
            ClientState {
                handshaked: true,
                bus_proto,
                name: None,
                pid: None,
                joined: BTreeSet::new(),
                connected: true,
                bind_as: Some(name),
            },
        );
        return vec![Out::Reply(client, BusReply::HelloOk { bus_proto: 1 })];
    }
    broker.state.clients.insert(
        client,
        ClientState {
            handshaked: true,
            bus_proto,
            name: None,
            pid: None,
            joined: BTreeSet::new(),
            connected: true,
            bind_as: None,
        },
    );
    vec![Out::Reply(client, BusReply::HelloOk { bus_proto: 1 })]
}

fn register<E: BrokerEnv>(
    broker: &mut Broker<E>,
    client: ClientId,
    name: String,
    pid: u32,
) -> Vec<Out> {
    // BL-05: PID 0 has POSIX-special semantics for `kill(2)` (targets
    // the calling pgrp). A client claiming PID 0 would always pass
    // `is_alive`, defeating the D-10 per-op liveness gate. Reject the
    // Register frame outright so the name is never bound to PID 0.
    if pid == 0 {
        return vec![err(
            client,
            BusErrorKind::EnvelopeInvalid,
            "pid 0 is not a valid process identifier",
        )];
    }
    // D-10: a proxy (`bind_as = Some`) connection MUST NOT register;
    // it is read/write-through to its bound canonical holder. Reject
    // with NotRegistered (the proxy can disconnect and reconnect with
    // `bind_as = None` to register cleanly).
    if let Some(state) = broker.state.clients.get(&client) {
        if state.bind_as.is_some() {
            return vec![err(
                client,
                BusErrorKind::NotRegistered,
                "proxy (bind_as) connection cannot register",
            )];
        }
    }

    let name_taken = broker
        .state
        .clients
        .values()
        .any(|c| c.connected && c.name.as_deref() == Some(name.as_str()));
    if name_taken {
        return vec![err(
            client,
            BusErrorKind::NameTaken,
            "name already registered",
        )];
    }

    let mailbox = MailboxName::Agent(name.clone());
    let since = broker.state.cursors.get(&mailbox).copied().unwrap_or(0);
    let drained = match broker.env.drain_from(&mailbox, since) {
        Ok(drained) => drained,
        Err(error) => return vec![err(client, BusErrorKind::Internal, error.to_string())],
    };
    let decoded = match decode_lines(drained.lines) {
        Ok(values) => values,
        Err(message) => return vec![err(client, BusErrorKind::EnvelopeInvalid, message)],
    };

    let peers = connected_names(&broker.state.clients);
    let Some(state) = broker.state.clients.get_mut(&client) else {
        return vec![err(
            client,
            BusErrorKind::BrokerProtoMismatch,
            "Hello required as first frame",
        )];
    };
    state.name = Some(name.clone());
    state.pid = Some(pid);
    state.connected = true;

    vec![
        Out::Reply(
            client,
            BusReply::RegisterOk {
                active: name,
                drained: decoded,
                peers,
            },
        ),
        Out::AdvanceCursor {
            name: mailbox,
            offset: drained.next_offset,
        },
    ]
}

fn send<E: BrokerEnv>(
    broker: &mut Broker<E>,
    client: ClientId,
    to: Target,
    envelope: serde_json::Value,
) -> Vec<Out> {
    // D-10: resolve via effective_identity so a proxy connection can
    // send under the bound canonical holder's name. The from-stamp on
    // the encoded envelope MUST be the resolved identity (NOT the
    // proxy's own None-name). `encode_envelope` operates on the JSON
    // value as-is; identity is implicit in the broker's state and is
    // not currently stamped onto the envelope here — that responsibility
    // is left to the CLI/MCP caller for v0.9 (the envelope already
    // carries `from` from the higher layer). We still gate the op on
    // a resolvable + live identity.
    if resolve_op_identity(broker, client).is_err() {
        return vec![err(
            client,
            BusErrorKind::NotRegistered,
            "client is not registered",
        )];
    }
    let line = match encode_envelope(&envelope, client) {
        Ok(line) => line,
        Err(reply) => return vec![reply],
    };

    match to {
        Target::Agent { name } => send_agent(broker, client, name, envelope, line),
        Target::Channel { name } => send_channel(broker, client, name, &envelope, line),
    }
}

fn send_agent<E: BrokerEnv>(
    broker: &mut Broker<E>,
    sender: ClientId,
    name: String,
    envelope: serde_json::Value,
    line: Vec<u8>,
) -> Vec<Out> {
    if let Some(waiting) = waiting_client_for_name(broker, &name, &envelope) {
        broker.state.pending_awaits.remove(&waiting);
        return vec![
            Out::Reply(waiting, BusReply::AwaitOk { envelope }),
            Out::UnparkAwait { client: waiting },
            send_ok(sender, Target::Agent { name }, true),
        ];
    }

    vec![
        Out::AppendMailbox {
            target: MailboxName::Agent(name.clone()),
            line,
        },
        send_ok(sender, Target::Agent { name }, true),
    ]
}

fn send_channel<E: BrokerEnv>(
    broker: &mut Broker<E>,
    sender: ClientId,
    name: String,
    envelope: &serde_json::Value,
    line: Vec<u8>,
) -> Vec<Out> {
    let members = broker
        .state
        .channels
        .get(&name)
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for member in &members {
        if let Some(waiting) = waiting_client_for_name(broker, member, envelope) {
            broker.state.pending_awaits.remove(&waiting);
            out.push(Out::Reply(
                waiting,
                BusReply::AwaitOk {
                    envelope: envelope.clone(),
                },
            ));
            out.push(Out::UnparkAwait { client: waiting });
        }
    }
    let task_id = task_id_from(envelope);
    out.push(Out::AppendMailbox {
        target: MailboxName::Channel(name),
        line,
    });
    out.push(Out::Reply(
        sender,
        BusReply::SendOk {
            task_id,
            delivered: members
                .into_iter()
                .map(|member| Delivered {
                    to: Target::Agent { name: member },
                    ok: true,
                })
                .collect(),
        },
    ));
    out
}

fn inbox<E: BrokerEnv>(broker: &Broker<E>, client: ClientId, since: Option<u64>) -> Vec<Out> {
    // D-10: a proxy connection's `Inbox` reads the canonical holder's
    // mailbox via effective_identity.
    let Ok(name) = resolve_op_identity(broker, client) else {
        return vec![err(
            client,
            BusErrorKind::NotRegistered,
            "client is not registered",
        )];
    };
    let mailbox = MailboxName::Agent(name);
    let drained = match broker.env.drain_from(&mailbox, since.unwrap_or(0)) {
        Ok(drained) => drained,
        Err(error) => return vec![err(client, BusErrorKind::Internal, error.to_string())],
    };
    let envelopes = match decode_lines(drained.lines) {
        Ok(values) => values,
        Err(message) => return vec![err(client, BusErrorKind::EnvelopeInvalid, message)],
    };

    vec![
        Out::Reply(
            client,
            BusReply::InboxOk {
                envelopes,
                next_offset: drained.next_offset,
            },
        ),
        Out::AdvanceCursor {
            name: mailbox,
            offset: drained.next_offset,
        },
    ]
}

fn await_envelope<E: BrokerEnv>(
    broker: &mut Broker<E>,
    client: ClientId,
    timeout_ms: u64,
    task: Option<uuid::Uuid>,
    now: Instant,
) -> Vec<Out> {
    // D-10: proxy connections can `Await` on the canonical holder's
    // mailbox; reject if neither a registered holder nor a live proxy
    // binding is present.
    if resolve_op_identity(broker, client).is_err() {
        return vec![err(
            client,
            BusErrorKind::NotRegistered,
            "client is not registered",
        )];
    }
    let filter = task.map_or(AwaitFilter::Any, AwaitFilter::Task);
    let deadline = now + Duration::from_millis(timeout_ms);
    broker.state.pending_awaits.insert(
        client,
        ParkedAwait {
            client,
            filter: filter.clone(),
            deadline,
        },
    );
    vec![Out::ParkAwait {
        client,
        filter,
        deadline,
    }]
}

fn join<E: BrokerEnv>(broker: &mut Broker<E>, client: ClientId, channel: String) -> Vec<Out> {
    // D-10: resolve effective identity; for proxies, the holder ID is
    // the canonical registered slot, NOT the proxy connection.
    let Ok(name) = resolve_op_identity(broker, client) else {
        return vec![err(
            client,
            BusErrorKind::NotRegistered,
            "client is not registered",
        )];
    };
    broker
        .state
        .channels
        .entry(channel.clone())
        .or_default()
        .insert(name.clone());
    // D-10: mutate the canonical holder's `joined` set, not the proxy's.
    // For canonical holders this resolves to `client` itself; for
    // proxies it resolves to the live registered holder of `name`.
    let target_client = canonical_holder_id(broker, &name).unwrap_or(client);
    if let Some(state) = broker.state.clients.get_mut(&target_client) {
        state.joined.insert(channel.clone());
    }

    let mailbox = MailboxName::Channel(channel.clone());
    let since = broker.state.cursors.get(&mailbox).copied().unwrap_or(0);
    let drained = match broker.env.drain_from(&mailbox, since) {
        Ok(drained) => drained,
        Err(error) => return vec![err(client, BusErrorKind::Internal, error.to_string())],
    };
    let decoded = match decode_lines(drained.lines) {
        Ok(values) => values,
        Err(message) => return vec![err(client, BusErrorKind::EnvelopeInvalid, message)],
    };
    let members = broker
        .state
        .channels
        .get(&channel)
        .map(|members| members.iter().cloned().collect())
        .unwrap_or_default();

    vec![
        Out::Reply(
            client,
            BusReply::JoinOk {
                channel,
                members,
                drained: decoded,
            },
        ),
        Out::AdvanceCursor {
            name: mailbox,
            offset: drained.next_offset,
        },
    ]
}

fn leave<E: BrokerEnv>(broker: &mut Broker<E>, client: ClientId, channel: String) -> Vec<Out> {
    // D-10: resolve effective identity; for proxies, mutate the
    // canonical holder's `joined` set rather than the proxy's.
    let Ok(name) = resolve_op_identity(broker, client) else {
        return vec![err(
            client,
            BusErrorKind::NotRegistered,
            "client is not registered",
        )];
    };
    if let Some(members) = broker.state.channels.get_mut(&channel) {
        members.remove(&name);
    }
    let target_client = canonical_holder_id(broker, &name).unwrap_or(client);
    if let Some(state) = broker.state.clients.get_mut(&target_client) {
        state.joined.remove(&channel);
    }
    vec![Out::Reply(client, BusReply::LeaveOk { channel })]
}

fn sessions<E: BrokerEnv>(broker: &Broker<E>, client: ClientId) -> Vec<Out> {
    let rows = broker
        .state
        .clients
        .values()
        .filter(|state| state.connected)
        .filter_map(|state| {
            Some(SessionRow {
                name: state.name.clone()?,
                pid: state.pid?,
                joined: state.joined.iter().cloned().collect(),
            })
        })
        .collect();
    vec![Out::Reply(client, BusReply::SessionsOk { rows })]
}

fn whoami<E: BrokerEnv>(broker: &Broker<E>, client: ClientId) -> Vec<Out> {
    // D-10: a proxy connection's `whoami` returns the bound canonical
    // identity (and that holder's joined set) — not the proxy's own
    // empty state. Liveness re-check: if the proxy's holder has died,
    // surface `active = None` (consistent with NotRegistered semantics).
    let (active, joined) = broker.state.clients.get(&client).map_or_else(
        || (None, Vec::new()),
        |state| {
            if state.name.is_some() {
                // Canonical holder.
                (state.name.clone(), state.joined.iter().cloned().collect())
            } else if let Some(ref bound) = state.bind_as {
                // Proxy: surface the canonical holder's identity + joined.
                if proxy_holder_alive(broker, bound) {
                    let holder_joined = canonical_holder_id(broker, bound)
                        .and_then(|id| broker.state.clients.get(&id))
                        .map_or_else(Vec::new, |h| h.joined.iter().cloned().collect());
                    (Some(bound.clone()), holder_joined)
                } else {
                    (None, Vec::new())
                }
            } else {
                (None, Vec::new())
            }
        },
    );
    vec![Out::Reply(client, BusReply::WhoamiOk { active, joined })]
}

fn disconnect<E: BrokerEnv>(broker: &mut Broker<E>, client: ClientId) -> Vec<Out> {
    // D-10: branch on canonical-holder vs. proxy. A proxy disconnect
    // is a no-op for the canonical name — it does NOT clear the
    // canonical holder's `joined` set, does NOT remove the canonical
    // name from any channel member set, and does NOT touch
    // `sessions.jsonl` (the proxy never appended a row).
    let (canonical_name, is_proxy) = broker.state.clients.get(&client).map_or_else(
        || (None, false),
        |state| {
            (
                state.name.clone(),
                state.bind_as.is_some() && state.name.is_none(),
            )
        },
    );

    if is_proxy {
        // BL-03: drop the dead entry from the map so per-tick iteration
        // (`canonical_holder_id`, `proxy_holder_alive`, `connected_names`,
        // tick's liveness sweep) does not grow O(N) with dead proxies.
        broker.state.clients.remove(&client);
        broker.state.pending_awaits.remove(&client);
        return vec![Out::ReleaseClient(client)];
    }

    // Canonical holder (or unbound, never-registered) cleanup path:
    if let Some(name) = canonical_name {
        for members in broker.state.channels.values_mut() {
            members.remove(&name);
        }
    }
    // BL-03: drop the dead entry from the map (see proxy branch above).
    broker.state.clients.remove(&client);
    broker.state.pending_awaits.remove(&client);
    vec![Out::ReleaseClient(client)]
}

fn tick<E: BrokerEnv>(broker: &mut Broker<E>, now: Instant) -> Vec<Out> {
    let dead_clients: Vec<ClientId> = broker
        .state
        .clients
        .iter()
        .filter_map(|(client, state)| {
            let pid = state.pid?;
            (!broker.env.is_alive(pid)).then_some(*client)
        })
        .collect();
    for client in dead_clients {
        let _ = disconnect(broker, client);
    }

    let expired: Vec<ClientId> = broker
        .state
        .pending_awaits
        .iter()
        .filter_map(|(client, parked)| (now >= parked.deadline).then_some(*client))
        .collect();
    let mut out = Vec::with_capacity(expired.len() * 2);
    for client in expired {
        broker.state.pending_awaits.remove(&client);
        out.push(Out::Reply(client, BusReply::AwaitTimeout {}));
        out.push(Out::UnparkAwait { client });
    }
    out
}

/// Pre-D-10 helper kept for callers that need the *registered* name
/// (the canonical-holder slot) explicitly, regardless of any proxy
/// binding. Most ops should use [`resolve_op_identity`] instead.
#[allow(dead_code)]
fn registered_name<E: BrokerEnv>(broker: &Broker<E>, client: ClientId) -> Option<String> {
    broker
        .state
        .clients
        .get(&client)
        .filter(|state| state.connected)
        .and_then(|state| state.name.clone())
}

/// D-10: resolve the effective identity for `client`. Returns the
/// registered holder's name (`state.name`) for canonical connections,
/// the bound holder's name (`state.bind_as`) for proxy connections,
/// or `Err(NotRegistered)` if neither is set.
///
/// This is the central identity-resolution entry point — every
/// identity-required op (`Send`, `Inbox`, `Await`, `Join`, `Leave`,
/// `Whoami`) calls into it instead of `state.name` directly.
fn effective_identity(state: &ClientState) -> Result<String, BusErrorKind> {
    if let Some(ref name) = state.name {
        return Ok(name.clone());
    }
    if let Some(ref bound) = state.bind_as {
        return Ok(bound.clone());
    }
    Err(BusErrorKind::NotRegistered)
}

/// D-10: per-op liveness re-check for proxy connections. Returns true
/// iff the canonical holder of `bound` is still connected AND its PID
/// answers `is_alive`. Called by every identity-required op when the
/// caller is a proxy (`state.bind_as = Some(_)`).
fn proxy_holder_alive<E: BrokerEnv>(broker: &Broker<E>, bound: &str) -> bool {
    broker.state.clients.values().any(|h| {
        h.connected
            && h.name.as_deref() == Some(bound)
            && h.pid.is_some_and(|pid| broker.env.is_alive(pid))
    })
}

/// D-10: `ClientId` of the canonical live holder for `bound`, or
/// `None` if no holder is currently registered. Used by Join/Leave to
/// mutate the canonical holder's `joined` set instead of the proxy's.
fn canonical_holder_id<E: BrokerEnv>(broker: &Broker<E>, bound: &str) -> Option<ClientId> {
    broker.state.clients.iter().find_map(|(id, state)| {
        if state.connected && state.name.as_deref() == Some(bound) {
            Some(*id)
        } else {
            None
        }
    })
}

/// D-10: resolve effective identity AND verify proxy liveness in one
/// step. Returns `Err(NotRegistered)` if the connection has no
/// resolvable identity OR if it is a proxy whose holder has died.
fn resolve_op_identity<E: BrokerEnv>(
    broker: &Broker<E>,
    client: ClientId,
) -> Result<String, BusErrorKind> {
    let state = broker
        .state
        .clients
        .get(&client)
        .ok_or(BusErrorKind::NotRegistered)?;
    if !state.connected {
        return Err(BusErrorKind::NotRegistered);
    }
    let identity = effective_identity(state)?;
    // Canonical holder owns the slot; no liveness re-check needed.
    if state.name.is_some() {
        return Ok(identity);
    }
    // Proxy: re-verify the canonical holder is still alive.
    if proxy_holder_alive(broker, &identity) {
        Ok(identity)
    } else {
        Err(BusErrorKind::NotRegistered)
    }
}

fn connected_names(clients: &std::collections::BTreeMap<ClientId, ClientState>) -> Vec<String> {
    clients
        .values()
        .filter(|state| state.connected)
        .filter_map(|state| state.name.clone())
        .collect()
}

fn waiting_client_for_name<E: BrokerEnv>(
    broker: &Broker<E>,
    name: &str,
    envelope: &serde_json::Value,
) -> Option<ClientId> {
    broker.state.pending_awaits.values().find_map(|parked| {
        let state = broker.state.clients.get(&parked.client)?;
        // D-10: a proxy connection's effective identity matches via
        // `state.bind_as` when its `state.name` is None.
        let waiting_name = state.name.as_deref().or(state.bind_as.as_deref())?;
        (state.connected && waiting_name == name && filter_matches(&parked.filter, envelope))
            .then_some(parked.client)
    })
}

fn filter_matches(filter: &AwaitFilter, envelope: &serde_json::Value) -> bool {
    match filter {
        AwaitFilter::Any => true,
        AwaitFilter::Task(task_id) => envelope
            .get("task_id")
            .and_then(serde_json::Value::as_str)
            .and_then(|raw| uuid::Uuid::parse_str(raw).ok())
            .is_some_and(|candidate| &candidate == task_id),
    }
}

fn encode_envelope(envelope: &serde_json::Value, client: ClientId) -> Result<Vec<u8>, Out> {
    let line = match famp_canonical::canonicalize(envelope) {
        Ok(line) => line,
        Err(error) => {
            return Err(err(
                client,
                BusErrorKind::EnvelopeInvalid,
                error.to_string(),
            ));
        }
    };
    if line.len() > MAX_FRAME_BYTES {
        return Err(err(
            client,
            BusErrorKind::EnvelopeTooLarge,
            "envelope too large",
        ));
    }
    Ok(line)
}

fn decode_lines(lines: Vec<Vec<u8>>) -> Result<Vec<serde_json::Value>, String> {
    lines
        .into_iter()
        .map(|line| {
            famp_envelope::AnyBusEnvelope::decode(&line).map_err(|error| {
                format!("drain line rejected by AnyBusEnvelope::decode: {error}")
            })?;
            famp_canonical::from_slice_strict::<serde_json::Value>(&line)
                .map_err(|error| error.to_string())
        })
        .collect()
}

fn send_ok(client: ClientId, to: Target, ok: bool) -> Out {
    Out::Reply(
        client,
        BusReply::SendOk {
            task_id: uuid::Uuid::nil(),
            delivered: vec![Delivered { to, ok }],
        },
    )
}

fn task_id_from(envelope: &serde_json::Value) -> uuid::Uuid {
    envelope
        .get("task_id")
        .and_then(serde_json::Value::as_str)
        .and_then(|raw| uuid::Uuid::parse_str(raw).ok())
        .unwrap_or_else(uuid::Uuid::nil)
}

fn err(client: ClientId, kind: BusErrorKind, message: impl Into<String>) -> Out {
    Out::Reply(
        client,
        BusReply::Err {
            kind,
            message: message.into(),
        },
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod d10_tests {
    //! D-10 unit tests for `bind_as` proxy semantics.

    use super::*;
    use crate::{Broker, FakeLiveness, InMemoryMailbox};
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Instant;

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
                },
            },
            now,
        );
    }

    fn hello_proxy(
        broker: &mut Broker<TestEnv>,
        client: u64,
        bound: &str,
        now: Instant,
    ) -> Vec<Out> {
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
}

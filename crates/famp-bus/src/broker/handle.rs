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
        } => {
            broker.state.clients.insert(
                client,
                ClientState {
                    handshaked: true,
                    bus_proto,
                    name: None,
                    pid: None,
                    joined: BTreeSet::new(),
                    connected: true,
                },
            );
            vec![Out::Reply(client, BusReply::HelloOk { bus_proto: 1 })]
        }
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

fn register<E: BrokerEnv>(
    broker: &mut Broker<E>,
    client: ClientId,
    name: String,
    pid: u32,
) -> Vec<Out> {
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
    if registered_name(broker, client).is_none() {
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
        Target::Channel { name } => send_channel(broker, client, name, envelope, line),
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
    envelope: serde_json::Value,
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
        if let Some(waiting) = waiting_client_for_name(broker, member, &envelope) {
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
    out.push(Out::AppendMailbox {
        target: MailboxName::Channel(name.clone()),
        line,
    });
    out.push(Out::Reply(
        sender,
        BusReply::SendOk {
            task_id: task_id_from(&envelope),
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

fn inbox<E: BrokerEnv>(broker: &mut Broker<E>, client: ClientId, since: Option<u64>) -> Vec<Out> {
    let Some(name) = registered_name(broker, client) else {
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
    let Some(name) = registered_name(broker, client) else {
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
    if let Some(state) = broker.state.clients.get_mut(&client) {
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
    let Some(name) = registered_name(broker, client) else {
        return vec![err(
            client,
            BusErrorKind::NotRegistered,
            "client is not registered",
        )];
    };
    if let Some(members) = broker.state.channels.get_mut(&channel) {
        members.remove(&name);
    }
    if let Some(state) = broker.state.clients.get_mut(&client) {
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
    let (active, joined) = broker.state.clients.get(&client).map_or_else(
        || (None, Vec::new()),
        |state| (state.name.clone(), state.joined.iter().cloned().collect()),
    );
    vec![Out::Reply(client, BusReply::WhoamiOk { active, joined })]
}

fn disconnect<E: BrokerEnv>(broker: &mut Broker<E>, client: ClientId) -> Vec<Out> {
    let name = broker
        .state
        .clients
        .get(&client)
        .and_then(|state| state.name.clone());
    if let Some(state) = broker.state.clients.get_mut(&client) {
        state.connected = false;
        state.joined.clear();
    }
    if let Some(name) = name {
        for members in broker.state.channels.values_mut() {
            members.remove(&name);
        }
    }
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

fn registered_name<E: BrokerEnv>(broker: &Broker<E>, client: ClientId) -> Option<String> {
    broker
        .state
        .clients
        .get(&client)
        .filter(|state| state.connected)
        .and_then(|state| state.name.clone())
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
        let waiting_name = state.name.as_deref()?;
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

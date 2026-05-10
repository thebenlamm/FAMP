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
    let already_handshaked = broker.state.clients.get(&client).map(|c| c.handshaked) == Some(true);
    if !matches!(msg, BusMessage::Hello { .. }) && !already_handshaked {
        return vec![err(
            client,
            BusErrorKind::BrokerProtoMismatch,
            "Hello required as first frame",
        )];
    }
    // WR-10 / WR-11: a second Hello on a handshaked connection would
    // overwrite the existing ClientState (wiping `name`, `pid`, and
    // `joined`, AND silently rotating `bind_as`). That released the
    // canonical-holder slot and let a misbehaving / malicious proxy
    // un-register the canonical holder or rotate identities mid-
    // connection. Reject the second Hello.
    if matches!(msg, BusMessage::Hello { .. }) && already_handshaked {
        return vec![err(
            client,
            BusErrorKind::BrokerProtoMismatch,
            "Hello already received on this connection",
        )];
    }

    match msg {
        BusMessage::Hello {
            bus_proto,
            client: _,
            bind_as,
        } => hello(broker, client, bus_proto, bind_as),
        BusMessage::Register {
            name,
            pid,
            cwd,
            listen,
        } => register(broker, client, name, pid, cwd, listen),
        BusMessage::Send { to, envelope } => send(broker, client, to, &envelope),
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
        BusMessage::Inspect { kind } => {
            // INSP-RPC-02: read-only. The actor does NOT call
            // famp_inspect_server::dispatch here because the
            // Identities handler needs mailbox metadata that lives
            // on disk. Sentinel the request out; the executor builds
            // BrokerCtx and dispatches.
            vec![Out::InspectRequest { client, kind }]
        }
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
                cwd: None,
                listen_mode: false,
                registered_at: std::time::SystemTime::now(),
                last_activity: std::time::SystemTime::now(),
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
            cwd: None,
            listen_mode: false,
            registered_at: std::time::SystemTime::now(),
            last_activity: std::time::SystemTime::now(),
        },
    );
    vec![Out::Reply(client, BusReply::HelloOk { bus_proto: 1 })]
}

fn register<E: BrokerEnv>(
    broker: &mut Broker<E>,
    client: ClientId,
    name: String,
    pid: u32,
    cwd: Option<String>,
    listen: bool,
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
    state.cwd = cwd;
    state.listen_mode = listen;
    let now_wall = std::time::SystemTime::now();
    state.registered_at = now_wall;
    state.last_activity = now_wall;

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
    envelope: &serde_json::Value,
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
    let line = match encode_envelope(envelope, client) {
        Ok(line) => line,
        Err(reply) => return vec![reply],
    };

    match to {
        Target::Agent { name } => send_agent(broker, client, name, envelope, line),
        Target::Channel { name } => send_channel(broker, client, &name, envelope, line),
    }
}

fn send_agent<E: BrokerEnv>(
    broker: &mut Broker<E>,
    sender: ClientId,
    name: String,
    envelope: &serde_json::Value,
    line: Vec<u8>,
) -> Vec<Out> {
    // WR-09: extract task_id from the envelope so the SendOk reply
    // carries the real task identity (matches send_channel). The
    // pre-fix path always returned Uuid::nil() for agent DMs, leaving
    // `famp send` and the `famp_send` MCP tool unable to surface the
    // task id to downstream callers.
    let task_id = task_id_from(envelope);
    let waiters = waiting_clients_for_name(broker, &name, envelope);
    let woken = !waiters.is_empty();

    // D-04: AppendMailbox FIRST, before any AwaitOk reply.
    let mut out = Vec::with_capacity(2 + 2 * waiters.len());
    out.push(Out::AppendMailbox {
        target: MailboxName::Agent(name.clone()),
        line,
    });

    if !waiters.is_empty() {
        tracing::debug!(waiters = waiters.len(), name = %name, "wake_broadcast");
        for waiting in &waiters {
            broker.state.pending_awaits.remove(waiting);
            out.push(Out::Reply(
                *waiting,
                BusReply::AwaitOk {
                    envelope: envelope.clone(),
                },
            ));
            out.push(Out::UnparkAwait { client: *waiting });
        }
    }

    out.push(send_ok(
        sender,
        task_id,
        Target::Agent { name },
        true,
        woken,
    ));
    out
}

fn send_channel<E: BrokerEnv>(
    broker: &mut Broker<E>,
    sender: ClientId,
    name: &str,
    envelope: &serde_json::Value,
    line: Vec<u8>,
) -> Vec<Out> {
    let members = broker.state.channels.get(name).cloned().unwrap_or_default();
    let task_id = task_id_from(envelope);
    let mut out = Vec::new();

    // D-04: AppendMailbox FIRST, before any AwaitOk reply. Previously
    // this lived AFTER the waiter loop, opening a race window where
    // a woken awaiter could read SendOk before the message was on disk.
    out.push(Out::AppendMailbox {
        target: MailboxName::Channel(name.to_owned()),
        line,
    });

    for member in &members {
        let waiters = waiting_clients_for_name(broker, member, envelope);
        if waiters.is_empty() {
            continue;
        }
        tracing::debug!(waiters = waiters.len(), name = %member, "wake_broadcast");
        for waiting in &waiters {
            broker.state.pending_awaits.remove(waiting);
            out.push(Out::Reply(
                *waiting,
                BusReply::AwaitOk {
                    envelope: envelope.clone(),
                },
            ));
            out.push(Out::UnparkAwait { client: *waiting });
        }
    }

    out.push(Out::Reply(
        sender,
        BusReply::SendOk {
            task_id,
            delivered: members
                .into_iter()
                // 260508-ib4: channel-aware woken is out of scope for this
                // plan; per-member woken in fan-out is deferred. SendOk
                // reports woken=false for channel rows even when a member
                // was parked on Await and got woken via the wake loop above.
                .map(|member| Delivered {
                    // woken is intentionally false for channel rows.
                    to: Target::Agent { name: member },
                    ok: true,
                    woken: false,
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
    const MAX_AWAIT_MS: u64 = 60 * 60 * 1000; // 1 hour

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
    // WR-05: cap timeout_ms before adding to `now`. `Instant + Duration`
    // panics on overflow; `Duration::from_millis(u64::MAX)` is ~584M
    // years and a malicious or buggy client sending the max would crash
    // the broker actor task (taking down every connected client).
    let timeout_ms = timeout_ms.min(MAX_AWAIT_MS);
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
    //
    // WR-07: snapshot (name, pid, joined) for the canonical-holder
    // branch BEFORE clearing state, so the executor can write a
    // SessionRow with the correct joined set.
    let (canonical_snapshot, is_proxy) = broker.state.clients.get(&client).map_or_else(
        || (None, false),
        |state| {
            let is_proxy = state.bind_as.is_some() && state.name.is_none();
            let snapshot = if is_proxy {
                None
            } else {
                state.name.clone().and_then(|name| {
                    state
                        .pid
                        .map(|pid| (name, pid, state.joined.iter().cloned().collect::<Vec<_>>()))
                })
            };
            (snapshot, is_proxy)
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
    if let Some((ref name, _, _)) = canonical_snapshot {
        for members in broker.state.channels.values_mut() {
            members.remove(name);
        }
    }
    // BL-03: drop the dead entry from the map (see proxy branch above).
    broker.state.clients.remove(&client);
    broker.state.pending_awaits.remove(&client);
    let mut outs = Vec::with_capacity(2);
    if let Some((name, pid, joined)) = canonical_snapshot {
        outs.push(Out::SessionEnded { name, pid, joined });
    }
    outs.push(Out::ReleaseClient(client));
    outs
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
    let mut out = Vec::new();
    for client in dead_clients {
        // WR-08: thread the disconnect Out vec through tick's return
        // instead of discarding it. Without this, Out::ReleaseClient
        // and Out::SessionEnded for liveness-discovered dead clients
        // never reach the executor — leaking the per-client reply
        // sender and skipping the SessionRow write.
        out.extend(disconnect(broker, client));
    }

    let expired: Vec<ClientId> = broker
        .state
        .pending_awaits
        .iter()
        .filter_map(|(client, parked)| (now >= parked.deadline).then_some(*client))
        .collect();
    out.reserve(expired.len() * 2);
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

fn waiting_clients_for_name<E: BrokerEnv>(
    broker: &Broker<E>,
    name: &str,
    envelope: &serde_json::Value,
) -> Vec<ClientId> {
    broker
        .state
        .pending_awaits
        .values()
        .filter_map(|parked| {
            let state = broker.state.clients.get(&parked.client)?;
            if !state.connected {
                return None;
            }
            // Canonical holder: state.name == Some(name).
            // Proxy: state.name is None AND state.bind_as == Some(name)
            //        AND canonical holder for `name` is still alive.
            let matches_name = match (&state.name, &state.bind_as) {
                (Some(n), _) => n == name,
                (None, Some(b)) => b == name && proxy_holder_alive(broker, name),
                _ => false,
            };
            if matches_name && filter_matches(&parked.filter, envelope) {
                Some(parked.client)
            } else {
                None
            }
        })
        .collect()
}

fn filter_matches(filter: &AwaitFilter, envelope: &serde_json::Value) -> bool {
    match filter {
        AwaitFilter::Any => true,
        AwaitFilter::Task(task_id) => {
            // Extract the task-scoped UUID the same way poll.rs does:
            //   class == "request" → the envelope id IS the task id.
            //   all other classes  → causality["ref"] links back to the
            //                        originating request id (the task id).
            // There is no top-level `task_id` field in FAMP envelopes.
            let raw_id = match envelope.get("class").and_then(serde_json::Value::as_str) {
                Some("request") => envelope.get("id").and_then(serde_json::Value::as_str),
                _ => envelope
                    .get("causality")
                    .and_then(|c| c.get("ref"))
                    .and_then(serde_json::Value::as_str),
            };
            raw_id
                .and_then(|raw| uuid::Uuid::parse_str(raw).ok())
                .is_some_and(|candidate| &candidate == task_id)
        }
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

fn send_ok(client: ClientId, task_id: uuid::Uuid, to: Target, ok: bool, woken: bool) -> Out {
    Out::Reply(
        client,
        BusReply::SendOk {
            task_id,
            delivered: vec![Delivered { to, ok, woken }],
        },
    )
}

fn task_id_from(envelope: &serde_json::Value) -> uuid::Uuid {
    envelope
        .get("id")
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
                    cwd: None,
                    listen: false,
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
                Out::Reply(client, BusReply::SendOk { delivered, .. }) => {
                    Some((*client, delivered))
                }
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
        let woken: std::collections::HashSet<ClientId> =
            count_await_oks(&outs).into_iter().collect();
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

        let woken: std::collections::HashSet<ClientId> =
            count_await_oks(&outs).into_iter().collect();
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
                },
            },
            now,
        );
        let _ = broker.handle(
            BrokerInput::Wire {
                client: ClientId::from(2_u64),
                msg: BusMessage::Join {
                    channel: "#x".into(),
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
        let woken: std::collections::HashSet<ClientId> =
            count_await_oks(&outs).into_iter().collect();
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
}

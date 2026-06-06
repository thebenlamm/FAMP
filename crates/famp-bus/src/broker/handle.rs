use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use crate::broker::identity::{canonical_holder_id, proxy_holder_alive, resolve_op_identity};
use crate::broker::state::{ClientState, ParkedAwait};
use crate::{
    AwaitFilter, Broker, BrokerEnv, BrokerInput, BusErrorKind, BusMessage, BusReply, ClientId,
    Delivered, MailboxName, MemberInfo, Out, SessionRow, Target, BUS_PROTO_VERSION,
    MAX_FRAME_BYTES,
};

const AWAIT_BATCH_CAP: usize = 50;

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

    // Fix 5 (2026-05-12): exclude `SetListen` from the pre-dispatch
    // `touch_activity` call. `set_listen` rejects proxy callers with
    // NotRegistered, but `touch_activity` already mapped the proxy
    // connection's activity onto the canonical holder via
    // `canonical_holder_id`, making a holder appear active when no
    // legitimate op actually happened. The success path inside
    // `set_listen` still stamps `state.last_activity` explicitly
    // (handle.rs `set_listen` body), so canonical-holder activity
    // tracking is preserved for accepted calls only.
    if !matches!(
        msg,
        BusMessage::Hello { .. } | BusMessage::Register { .. } | BusMessage::SetListen { .. }
    ) {
        touch_activity(broker, client);
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
        BusMessage::Join { channel, role } => join(broker, client, channel, role),
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
        BusMessage::SetListen { listen } => set_listen(broker, client, listen),
    }
}

/// Fix 1 (2026-05-12): flip the canonical holder's `listen_mode` flag in
/// place. Used by the `famp_set_listen` MCP tool so an agent can opt
/// into/out of Stop-hook auto-wake without re-registering (which would
/// re-drain the mailbox from offset 0).
///
/// Proxy (`bind_as`) connections are rejected with `NotRegistered`,
/// mirroring the Register rejection at the top of `register` above.
/// Slot ownership is canonical-holder-only; a proxy must reconnect
/// without `bind_as` and `Register` itself before issuing `SetListen`.
fn set_listen<E: BrokerEnv>(broker: &mut Broker<E>, client: ClientId, listen: bool) -> Vec<Out> {
    let Some(state) = broker.state.clients.get_mut(&client) else {
        return vec![err(
            client,
            BusErrorKind::NotRegistered,
            "client is not registered",
        )];
    };
    if !state.connected {
        return vec![err(
            client,
            BusErrorKind::NotRegistered,
            "client is not registered",
        )];
    }
    if state.bind_as.is_some() && state.name.is_none() {
        // Proxy connection: refuse to mutate the canonical holder's slot.
        return vec![err(
            client,
            BusErrorKind::NotRegistered,
            "proxy (bind_as) connection cannot set_listen",
        )];
    }
    if state.name.is_none() {
        return vec![err(
            client,
            BusErrorKind::NotRegistered,
            "client is not registered",
        )];
    }
    state.listen_mode = listen;
    state.last_activity = std::time::SystemTime::now();
    vec![Out::Reply(
        client,
        BusReply::SetListenOk {
            listen_mode: listen,
        },
    )]
}

fn touch_activity<E: BrokerEnv>(broker: &mut Broker<E>, client: ClientId) {
    let target = broker.state.clients.get(&client).and_then(|state| {
        if !state.connected {
            None
        } else if state.name.is_some() {
            Some(client)
        } else {
            state
                .bind_as
                .as_deref()
                .and_then(|bound| canonical_holder_id(broker, bound))
        }
    });

    if let Some(target) = target {
        if let Some(state) = broker.state.clients.get_mut(&target) {
            state.last_activity = std::time::SystemTime::now();
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
    if bus_proto != BUS_PROTO_VERSION {
        return vec![Out::Reply(
            client,
            BusReply::HelloErr {
                kind: BusErrorKind::BrokerProtoMismatch,
                message: format!(
                    "client bus_proto={bus_proto} is not supported by this broker; expected bus_proto={BUS_PROTO_VERSION}"
                ),
            },
        )];
    }

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
                await_offsets: BTreeMap::default(),
            },
        );
        return vec![Out::Reply(
            client,
            BusReply::HelloOk {
                bus_proto: BUS_PROTO_VERSION,
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
            bind_as: None,
            cwd: None,
            listen_mode: false,
            registered_at: std::time::SystemTime::now(),
            last_activity: std::time::SystemTime::now(),
            await_offsets: BTreeMap::default(),
        },
    );
    vec![Out::Reply(
        client,
        BusReply::HelloOk {
            bus_proto: BUS_PROTO_VERSION,
        },
    )]
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
    // Register drain-from-start: the in-memory `cursors` map was never
    // populated (deleted in fix 260512-jdv); preserving the historical
    // since=0 behavior. Replay-on-restart is tracked separately.
    let since: u64 = 0;
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
    state
        .await_offsets
        .insert(mailbox.clone(), drained.next_offset);

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
    let line_len = line.len();

    // D-04: AppendMailbox FIRST, before any AwaitOk reply.
    let mut out = Vec::with_capacity(2 + 2 * waiters.len());
    out.push(Out::AppendMailbox {
        target: MailboxName::Agent(name.clone()),
        line,
    });

    if !waiters.is_empty() {
        tracing::debug!(waiters = waiters.len(), name = %name, "wake_broadcast");
        for waiting in &waiters {
            let Some(parked) = broker.state.pending_awaits.remove(waiting) else {
                continue;
            };
            let mailbox = MailboxName::Agent(name.clone());
            let reply = await_reply_for_mailbox(
                broker,
                *waiting,
                &mailbox,
                &parked.filter,
                Some((envelope, line_len)),
            );
            out.push(Out::Reply(*waiting, reply));
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
    let line_len = line.len();
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
            let Some(parked) = broker.state.pending_awaits.remove(waiting) else {
                continue;
            };
            let mailbox = MailboxName::Channel(name.to_owned());
            let reply = await_reply_for_mailbox(
                broker,
                *waiting,
                &mailbox,
                &parked.filter,
                Some((envelope, line_len)),
            );
            out.push(Out::Reply(*waiting, reply));
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

    vec![Out::Reply(
        client,
        BusReply::InboxOk {
            envelopes,
            next_offset: drained.next_offset,
        },
    )]
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
    // binding is present. Delivery offsets are stored on the canonical
    // holder so one-shot proxy awaits do not replay from zero every call.
    let Ok((identity, owner)) = resolve_await_owner(broker, client) else {
        return vec![err(
            client,
            BusErrorKind::NotRegistered,
            "client is not registered",
        )];
    };
    let filter = task.map_or(AwaitFilter::Any, AwaitFilter::Task);

    for mailbox in await_mailboxes(broker, owner, &identity) {
        let since = await_offset(broker, owner, &mailbox);
        let batch = match drain_await_batch(broker, owner, &mailbox, &filter, None) {
            Ok(batch) => batch,
            Err((kind, message)) => return vec![err(client, kind, message)],
        };
        if batch.next_offset != since {
            set_await_offset(broker, owner, &mailbox, batch.next_offset);
        }
        if !batch.envelopes.is_empty() {
            return vec![Out::Reply(
                client,
                BusReply::AwaitOk {
                    envelopes: batch.envelopes,
                    mailbox: batch.mailbox,
                    next_offset: batch.next_offset,
                },
            )];
        }
    }

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

#[allow(clippy::needless_pass_by_value)]
fn join<E: BrokerEnv>(
    broker: &mut Broker<E>,
    client: ClientId,
    channel: String,
    role: Option<String>,
) -> Vec<Out> {
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

    // Store the declared role in `channel_roles` if provided.
    if let Some(ref r) = role {
        broker
            .state
            .channel_roles
            .insert((channel.clone(), name), r.clone());
    }

    let mailbox = MailboxName::Channel(channel.clone());
    // Join drain-from-start: the in-memory `cursors` map was never
    // populated (deleted in fix 260512-jdv); preserving the historical
    // since=0 behavior.
    let since: u64 = 0;
    let drained = match broker.env.drain_from(&mailbox, since) {
        Ok(drained) => drained,
        Err(error) => return vec![err(client, BusErrorKind::Internal, error.to_string())],
    };
    let decoded = match decode_lines(drained.lines) {
        Ok(values) => values,
        Err(message) => return vec![err(client, BusErrorKind::EnvelopeInvalid, message)],
    };
    if let Some(state) = broker.state.clients.get_mut(&target_client) {
        state
            .await_offsets
            .insert(mailbox.clone(), drained.next_offset);
    }
    // Build MemberInfo list: look up each member's role from channel_roles.
    let members: Vec<MemberInfo> = broker
        .state
        .channels
        .get(&channel)
        .map(|member_names| {
            member_names
                .iter()
                .map(|member_name| MemberInfo {
                    name: member_name.clone(),
                    role: broker
                        .state
                        .channel_roles
                        .get(&(channel.clone(), member_name.clone()))
                        .cloned(),
                })
                .collect()
        })
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
    // Clean up role entry to avoid leaking stale roles.
    broker.state.channel_roles.remove(&(channel.clone(), name));
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

fn connected_names(clients: &std::collections::BTreeMap<ClientId, ClientState>) -> Vec<String> {
    clients
        .values()
        .filter(|state| state.connected)
        .filter_map(|state| state.name.clone())
        .collect()
}

#[derive(Debug)]
struct AwaitBatch {
    mailbox: MailboxName,
    envelopes: Vec<serde_json::Value>,
    next_offset: u64,
}

fn resolve_await_owner<E: BrokerEnv>(
    broker: &Broker<E>,
    client: ClientId,
) -> Result<(String, ClientId), BusErrorKind> {
    let identity = resolve_op_identity(broker, client)?;
    let owner = canonical_holder_id(broker, &identity).unwrap_or(client);
    Ok((identity, owner))
}

fn await_mailboxes<E: BrokerEnv>(
    broker: &Broker<E>,
    owner: ClientId,
    identity: &str,
) -> Vec<MailboxName> {
    let mut mailboxes = vec![MailboxName::Agent(identity.to_owned())];
    if let Some(state) = broker.state.clients.get(&owner) {
        mailboxes.extend(state.joined.iter().cloned().map(MailboxName::Channel));
    }
    mailboxes
}

fn await_offset<E: BrokerEnv>(broker: &Broker<E>, owner: ClientId, mailbox: &MailboxName) -> u64 {
    broker
        .state
        .clients
        .get(&owner)
        .and_then(|state| state.await_offsets.get(mailbox).copied())
        .unwrap_or(0)
}

fn set_await_offset<E: BrokerEnv>(
    broker: &mut Broker<E>,
    owner: ClientId,
    mailbox: &MailboxName,
    offset: u64,
) {
    if let Some(state) = broker.state.clients.get_mut(&owner) {
        state.await_offsets.insert(mailbox.clone(), offset);
    }
}

fn await_reply_for_mailbox<E: BrokerEnv>(
    broker: &mut Broker<E>,
    client: ClientId,
    mailbox: &MailboxName,
    filter: &AwaitFilter,
    trigger: Option<(&serde_json::Value, usize)>,
) -> BusReply {
    let Ok((_, owner)) = resolve_await_owner(broker, client) else {
        return BusReply::Err {
            kind: BusErrorKind::NotRegistered,
            message: "client is not registered".into(),
        };
    };
    match drain_await_batch(broker, owner, mailbox, filter, trigger) {
        Ok(batch) if !batch.envelopes.is_empty() => {
            set_await_offset(broker, owner, mailbox, batch.next_offset);
            BusReply::AwaitOk {
                envelopes: batch.envelopes,
                mailbox: batch.mailbox,
                next_offset: batch.next_offset,
            }
        }
        Ok(batch) => {
            set_await_offset(broker, owner, mailbox, batch.next_offset);
            BusReply::Err {
                kind: BusErrorKind::Internal,
                message: "await wake produced no matching envelopes".into(),
            }
        }
        Err((kind, message)) => BusReply::Err { kind, message },
    }
}

fn drain_await_batch<E: BrokerEnv>(
    broker: &Broker<E>,
    owner: ClientId,
    mailbox: &MailboxName,
    filter: &AwaitFilter,
    trigger: Option<(&serde_json::Value, usize)>,
) -> Result<AwaitBatch, (BusErrorKind, String)> {
    let since = await_offset(broker, owner, mailbox);
    let drained = broker
        .env
        .drain_from(mailbox, since)
        .map_err(|error| (BusErrorKind::Internal, error.to_string()))?;

    let mut next_offset = since;
    let mut envelopes = Vec::new();
    for line in drained.lines {
        let line_next_offset = next_offset + (line.len() + 1) as u64;
        let value =
            decode_line(&line).map_err(|message| (BusErrorKind::EnvelopeInvalid, message))?;
        if filter_matches(filter, &value) {
            envelopes.push(value);
            if envelopes.len() == AWAIT_BATCH_CAP {
                return Ok(AwaitBatch {
                    mailbox: mailbox.clone(),
                    envelopes,
                    next_offset: line_next_offset,
                });
            }
        }
        next_offset = line_next_offset;
    }
    debug_assert_eq!(next_offset, drained.next_offset);

    if let Some((trigger_envelope, trigger_line_len)) = trigger {
        let trigger_next_offset = next_offset + (trigger_line_len + 1) as u64;
        if filter_matches(filter, trigger_envelope) {
            envelopes.push(trigger_envelope.clone());
        }
        next_offset = trigger_next_offset;
    }

    Ok(AwaitBatch {
        mailbox: mailbox.clone(),
        envelopes,
        next_offset,
    })
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
    lines.into_iter().map(|line| decode_line(&line)).collect()
}

fn decode_line(line: &[u8]) -> Result<serde_json::Value, String> {
    famp_envelope::AnyBusEnvelope::decode(line)
        .map_err(|error| format!("drain line rejected by AnyBusEnvelope::decode: {error}"))?;
    famp_canonical::from_slice_strict::<serde_json::Value>(line).map_err(|error| error.to_string())
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
#[path = "handle/tests.rs"]
mod d10_tests;

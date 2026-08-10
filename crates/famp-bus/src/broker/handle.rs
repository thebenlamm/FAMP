use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use crate::broker::awaiting::{await_envelope, await_reply_for_mailbox, waiting_clients_for_name};
use crate::broker::drain_walk::{is_self_authored, walk, DrainCap, DrainPolicy};
use crate::broker::identity::{canonical_holder_id, proxy_holder_alive, resolve_op_identity};
use crate::broker::state::ClientState;
use crate::{
    encode_frame, AwaitFilter, Broker, BrokerEnv, BrokerInput, BusErrorKind, BusMessage, BusReply,
    ClientId, Delivered, DrainResult, DrainedRecord, MailboxName, MemberInfo, Origin, Out,
    SessionRow, StampedEnvelope, Target, BUS_PROTO_VERSION, MAX_FRAME_BYTES,
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

    // Fix 5 (2026-05-12): exclude `SetListen` from the pre-dispatch
    // `touch_activity` call. `set_listen` rejects proxy callers with
    // NotRegistered, but `touch_activity` already mapped the proxy
    // connection's activity onto the canonical holder via
    // `canonical_holder_id`, making a holder appear active when no
    // legitimate op actually happened. The success path inside
    // `set_listen` still stamps `state.last_activity` explicitly
    // (handle.rs `set_listen` body), so canonical-holder activity
    // tracking is preserved for accepted calls only.
    //
    // 260810-hac: `SetWakeAddr` joins the exclusion for the identical
    // reason — `set_wake_addr` also rejects proxy callers, so letting the
    // pre-dispatch `touch_activity` run first would let a rejected proxy
    // frame refresh the canonical holder's `last_activity`. Its success
    // path stamps `last_activity` explicitly, same as `set_listen`.
    if !matches!(
        msg,
        BusMessage::Hello { .. }
            | BusMessage::Register { .. }
            | BusMessage::SetListen { .. }
            | BusMessage::SetWakeAddr { .. }
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
            origin,
        } => register(broker, client, name, pid, cwd, listen, origin),
        BusMessage::Send { to, envelope } => send(broker, client, to, &envelope),
        BusMessage::Inbox {
            since,
            include_terminal,
        } => inbox(broker, client, since, include_terminal),
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
        BusMessage::SetWakeAddr { wake_addr } => set_wake_addr(broker, client, wake_addr),
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

/// D1 (260810-hac, spec `2026-08-10-native-wake-ping-design.md`): record
/// the canonical holder's Claude Code host `SendMessage` address so a
/// later DM to this holder can hand the SENDING model a content-free wake
/// ping. Issued by the `famp_register` MCP tool right after `RegisterOk`.
///
/// Proxy (`bind_as`) connections are rejected with `NotRegistered`,
/// reusing `set_listen`'s canonical-holder guard verbatim — slot ownership
/// is canonical-holder-only (T-hac-02).
///
/// `wake_addr` is peer-controlled: ANY bus client can send this frame, and
/// only the broker sees them all, so the shape check lives HERE rather
/// than in the MCP tool that normally produces the value (T-hac-03). A
/// value that fails [`famp_bus::wake_addr_valid`] stores `None` and the
/// reply echoes `None` — fail-open to no-ping, NEVER an `Err` (an error
/// here would surface as a registration failure for a purely optional
/// latency optimization).
///
/// Deliberately independent of `listen_mode`: storage is unconditional,
/// and the listen flag is consulted at DELIVERY time instead, so a holder
/// that later calls `famp_set_listen(true)` becomes ping-eligible without
/// re-registering.
fn set_wake_addr<E: BrokerEnv>(
    broker: &mut Broker<E>,
    client: ClientId,
    wake_addr: Option<String>,
) -> Vec<Out> {
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
            "proxy (bind_as) connection cannot set_wake_addr",
        )];
    }
    if state.name.is_none() {
        return vec![err(
            client,
            BusErrorKind::NotRegistered,
            "client is not registered",
        )];
    }
    let stored = wake_addr.filter(|candidate| crate::wake_addr_valid(candidate));
    state.wake_addr.clone_from(&stored);
    state.last_activity = std::time::SystemTime::now();
    vec![Out::Reply(
        client,
        BusReply::SetWakeAddrOk { wake_addr: stored },
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
                .and_then(|bound| canonical_holder_id(&broker.state, bound))
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
        // D-10: a proxy bind_as is valid only if the named canonical
        // holder is currently registered AND its process is still live.
        // If the holder died between its Register and our Hello, treat
        // the bind_as as unregistered. This is the Hello-time gate; the
        // same check re-runs per-op via `identity::proxy_holder_alive`.
        if !proxy_holder_alive(broker, &name) {
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
                name: None,
                pid: None,
                joined: BTreeSet::new(),
                connected: true,
                bind_as: Some(name),
                cwd: None,
                listen_mode: false,
                wake_addr: None,
                // D-01/D-02: Hello never carries an origin; only Register
                // declares one. `Origin::Unknown` here is the fail-closed
                // default, not a special case for the proxy path.
                origin: Origin::Unknown,
                registered_at: std::time::SystemTime::now(),
                last_activity: std::time::SystemTime::now(),
                await_offsets: BTreeMap::default(),
                inbox_offsets: BTreeMap::default(),
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
            name: None,
            pid: None,
            joined: BTreeSet::new(),
            connected: true,
            bind_as: None,
            cwd: None,
            listen_mode: false,
            wake_addr: None,
            // D-01/D-02: Hello never carries an origin; only Register
            // declares one.
            origin: Origin::Unknown,
            registered_at: std::time::SystemTime::now(),
            last_activity: std::time::SystemTime::now(),
            await_offsets: BTreeMap::default(),
            inbox_offsets: BTreeMap::default(),
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
    origin: Option<Origin>,
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

    // Idempotent self-re-register (260721): a name is "taken" only when a
    // *different* live client holds it. Excluding the calling client lets a
    // session re-register its own held name and fall through to the normal
    // path below (which refreshes listen_mode and returns RegisterOk),
    // rather than getting -32101. This is what makes "just re-register" a
    // real recovery path after a Claude Code /compact drops the register
    // marker out of the listen-hook's transcript scan window — the fresh
    // RegisterOk re-lands a successful marker the hook can find again.
    // NameTaken stays reserved for a genuinely different session grabbing a
    // held name.
    let name_taken =
        broker.state.clients.iter().any(|(id, c)| {
            *id != client && c.connected && c.name.as_deref() == Some(name.as_str())
        });
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
    // Phase 14 plan 14-02: `RegisterOk.drained` carries `StampedEnvelope`
    // elements — build them from decode_lines' per-record origin (the
    // same pattern `inbox()` uses for `InboxOk.envelopes`).
    let decoded: Vec<StampedEnvelope> = decode_lines(&mailbox, since, &drained)
        .into_iter()
        .map(|(origin, envelope)| StampedEnvelope { origin, envelope })
        .collect();

    // Peers snapshot is taken BEFORE binding so a first-time register does
    // not list itself (matches pre-#14 behaviour). A self re-register still
    // appears because its prior bind is already on the map.
    let peers = connected_names(&broker.state.clients);
    let reply = BusReply::RegisterOk {
        active: name.clone(),
        drained: decoded,
        peers,
    };
    // #14: encode-before-commit. `Out::Reply` is only written after this
    // handler returns; if we bound the name first and the write loop then
    // hit `encode_frame` FrameTooLarge, the client never saw RegisterOk but
    // the broker still held the name → retry got NameTaken until reaping.
    // Refuse without mutating when the reply itself cannot be framed.
    if let Err(error) = encode_frame(&reply) {
        return vec![err(
            client,
            BusErrorKind::EnvelopeTooLarge,
            format!("RegisterOk exceeds 16 MiB reply-frame limit: {error}"),
        )];
    }

    let Some(state) = broker.state.clients.get_mut(&client) else {
        return vec![err(
            client,
            BusErrorKind::BrokerProtoMismatch,
            "Hello required as first frame",
        )];
    };
    state.name = Some(name);
    state.pid = Some(pid);
    state.connected = true;
    state.cwd = cwd;
    state.listen_mode = listen;
    // D-01: `unwrap_or_default()` resolves an absent `origin` field to
    // `Origin::Unknown` (the enum's `Default`), NEVER `Origin::Local`. A
    // Register frame that omits `origin` can never produce a trusted
    // stamp.
    state.origin = origin.unwrap_or_default();
    let now_wall = std::time::SystemTime::now();
    state.registered_at = now_wall;
    state.last_activity = now_wall;
    state
        .await_offsets
        .insert(mailbox.clone(), drained.next_offset);

    vec![
        Out::Reply(client, reply),
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
    // value as-is; identity is implicit in the broker's state. As of
    // T-11-18 the `from` the CLI/MCP caller stamped onto the envelope
    // is no longer trusted verbatim — it is checked against the
    // resolved identity immediately below, before any mailbox write.
    let Ok(effective_identity) = resolve_op_identity(broker, client) else {
        return vec![err(
            client,
            BusErrorKind::NotRegistered,
            "client is not registered",
        )];
    };

    // T-11-18: bind the envelope `from` to the authenticated connection's
    // effective identity. `resolve_op_identity` proves the connection is
    // live and registered (canonical or a live-proxy `bind_as`), but does
    // NOT constrain what `from` string the caller wrote into the envelope
    // JSON — until now that was left to the CLI/MCP caller (see the
    // module doc above). A registered `alice` connection could carry
    // `from = .../mallory` and the bus would happily stamp + relay it,
    // so a locally-registered agent could forge another agent's `from`
    // (and, once past the bus, the gateway would sign it as that other
    // agent's domain — T-11-19). Reuse `is_self_authored`'s leaf-split
    // convention (`from.rsplit('/').next()`) rather than hand-rolling a
    // second Principal-leaf parse: `from` is `agent:<domain>/<name>`,
    // and only the trailing `/<name>` segment is compared.
    //
    // This is safe for the gateway relay path: `ingress.rs` inserts each
    // remote sender's envelope through THAT sender's own backing
    // connection (`guard.get_mut(sender.name())`), so
    // `effective_identity == from`'s leaf holds for relayed envelopes
    // too — `e2e_cross_host_delivery.rs` is the regression control.
    if !is_self_authored(envelope, Some(&effective_identity)) {
        return vec![err(
            client,
            BusErrorKind::EnvelopeInvalid,
            "envelope 'from' does not match the authenticated identity",
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
    // D-02: resolve the SENDER's declared origin before mutating any
    // state below (the borrow is immutable and short-lived).
    let origin = client_origin(broker, sender);
    let waiters = waiting_clients_for_name(broker, &name, envelope, origin);
    let woken = !waiters.is_empty();
    // D2 (260810-hac): resolve the recipient's wake address for the
    // SENDER's reply. Gated on BOTH conditions, not either:
    //   - the recipient's listen flag, because a window that opted out of
    //     auto-wake must not be pinged; and
    //   - the SENDING client's declared origin being Local (T-hac-04),
    //     because a gateway-relayed remote sender cannot call
    //     `SendMessage` anyway and must never learn a local socket path.
    // Resolved BEFORE the mutations below, alongside `origin`, so the
    // borrow is immutable and short-lived.
    let wake_addr = recipient_wake_addr(broker, &name, origin);
    // The executor persists the provenance wrapper, not the inner canonical
    // envelope. Await's folded trigger offset must therefore use the exact
    // stamped record length or its cursor lands before the real JSONL EOF.
    let line_len = match crate::stamp_line(&line, origin) {
        Ok(stamped) => stamped.len(),
        Err(_) => {
            return vec![err(
                sender,
                BusErrorKind::Internal,
                "failed to stamp mailbox record",
            )];
        }
    };

    // D-04: AppendMailbox FIRST, before any AwaitOk reply.
    let mut out = Vec::with_capacity(2 + 2 * waiters.len());
    out.push(Out::AppendMailbox {
        target: MailboxName::Agent(name.clone()),
        line,
        origin,
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
                Some((origin, envelope, line_len)),
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
        wake_addr,
    ));
    out
}

/// D2 (260810-hac): the wake address to hand back to a DM's SENDER, or
/// `None` when no ping is warranted.
///
/// Returns `None` unless all three hold: the sender's declared origin is
/// `Local` (T-hac-04 — a remote principal proxied by the gateway must
/// never learn a local socket path); a canonical holder for `name` exists;
/// and that holder has listen mode ON with a validated address stored.
fn recipient_wake_addr<E: BrokerEnv>(
    broker: &Broker<E>,
    name: &str,
    sender_origin: Origin,
) -> Option<String> {
    if sender_origin != Origin::Local {
        return None;
    }
    let holder = canonical_holder_id(&broker.state, name)?;
    let state = broker.state.clients.get(&holder)?;
    if !state.listen_mode {
        return None;
    }
    state.wake_addr.clone()
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
    // D-02: resolve the SENDER's declared origin before mutating any
    // state below.
    let origin = client_origin(broker, sender);
    let line_len = match crate::stamp_line(&line, origin) {
        Ok(stamped) => stamped.len(),
        Err(_) => {
            return vec![err(
                sender,
                BusErrorKind::Internal,
                "failed to stamp mailbox record",
            )];
        }
    };
    let mut out = Vec::new();

    // D-04: AppendMailbox FIRST, before any AwaitOk reply. Previously
    // this lived AFTER the waiter loop, opening a race window where
    // a woken awaiter could read SendOk before the message was on disk.
    out.push(Out::AppendMailbox {
        target: MailboxName::Channel(name.to_owned()),
        line,
        origin,
    });

    for member in &members {
        // Issue #15: standard pub/sub — a publisher does not receive their
        // own channel posts. `drain_await_batch` already skips self-authored
        // envelopes, but if we still *wake* the author, the empty fully-
        // drained batch used to return `BusReply::Err{Internal}` and kill
        // the parked `famp_await` (disarming listen-mode Stop hooks). Skip
        // the author at selection so they stay parked.
        if is_self_authored(envelope, Some(member)) {
            continue;
        }
        let waiters = waiting_clients_for_name(broker, member, envelope, origin);
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
                Some((origin, envelope, line_len)),
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
                    // D2 (260810-hac): wake_addr is intentionally absent
                    // on channel rows, for the same reason `woken` is
                    // false above — per-member state is not resolved on
                    // the fan-out path. Leaving it absent is also the
                    // conservative choice: a channel post would otherwise
                    // hand the sender every listening member's socket
                    // path in one reply.
                    wake_addr: None,
                })
                .collect(),
        },
    ));
    out
}

/// Scope B (260619): per-channel drain cap. A hot channel with thousands
/// of envelopes must not bloat a single `Inbox` response from a slow
/// reader. The cap is per channel per poll — across N joined channels
/// the worst-case response is N * CHANNEL_DRAIN_CAP envelopes. Picked
/// to match Await's batching posture (see `awaiting::drain_await_batch`).
const CHANNEL_DRAIN_CAP: usize = 256;

// Phase 14 D-17 pushed this over the 100-line pedantic threshold (the
// agent-mailbox and per-channel drain loops now build `StampedEnvelope`
// elements instead of bare `Value`s). Matches the existing precedent for
// this lint in the workspace (`famp-inspect-server::tasks`,
// `famp::cli::daemon::status`, `famp::cli::mcp::server`,
// `famp::cli::send`) — splitting the per-channel drain loop into its own
// function here would separate the cursor-advance bookkeeping from the
// `envelopes` accumulation it is tightly coupled to, which is a real risk
// in security-critical code, not a stylistic win.
#[allow(clippy::too_many_lines)]
fn inbox<E: BrokerEnv>(
    broker: &mut Broker<E>,
    client: ClientId,
    since: Option<u64>,
    // Scope B (260619): the flag is propagated end-to-end through the
    // handler signature so the destructure no longer drops it. Broker-
    // side terminal filtering against the task FSM is v1 scope — it
    // requires the bus actor (a pure transport crate) to read
    // `famp-taskdir` for per-task FSM state, which crosses the
    // famp-bus / famp-cli architecture boundary. The wire shape is
    // already correct, so the v1 filter slot bolts in without changing
    // `BusMessage::Inbox`.
    _include_terminal: Option<bool>,
) -> Vec<Out> {
    // D-10: a proxy connection's `Inbox` reads the canonical holder's
    // mailbox via effective_identity.
    let Ok(name) = resolve_op_identity(broker, client) else {
        return vec![err(
            client,
            BusErrorKind::NotRegistered,
            "client is not registered",
        )];
    };

    // Read the agent mailbox using the client-supplied cursor. This
    // preserves the pre-Scope-B `next_offset` contract — clients and
    // `famp inbox ack` still drive the agent-mailbox cursor.
    let agent_mailbox = MailboxName::Agent(name.clone());
    let agent_since = since.unwrap_or(0);
    let agent_drained = match broker.env.drain_from(&agent_mailbox, agent_since) {
        Ok(drained) => drained,
        Err(error) => return vec![err(client, BusErrorKind::Internal, error.to_string())],
    };
    // D-17: InboxOk carries `StampedEnvelope` elements — build them from
    // decode_lines' per-record origin.
    let mut envelopes: Vec<StampedEnvelope> =
        decode_lines(&agent_mailbox, agent_since, &agent_drained)
            .into_iter()
            .map(|(origin, envelope)| StampedEnvelope { origin, envelope })
            .collect();
    let agent_next_offset = agent_drained.next_offset;

    // Scope B (260619): merge each joined channel's new envelopes into
    // the response. Cursors are per-canonical-holder-per-channel and
    // live in `await_offsets[MailboxName::Channel(c)]`. Initialized to
    // the channel's join-time end-offset by `join()`, so first-poll
    // semantics are "everything posted AFTER I joined". Per-channel
    // drain is capped at `CHANNEL_DRAIN_CAP` SCANNED records per poll
    // (not delivered envelopes — self-authored and undecodable records
    // consume budget too) so a hot channel cannot block other members
    // or bloat one response; the leftover lines are picked up by the
    // next poll.
    // `resolve_op_identity` (line 526) succeeded for this `name`, which
    // means either (a) `client` is itself the canonical holder of `name`,
    // or (b) `client` is a proxy whose canonical holder passed
    // `proxy_holder_alive`. Both paths guarantee a canonical holder for
    // `name` exists in `broker.state.clients`. A silent `unwrap_or(client)`
    // fallback would route per-channel cursor writes to the proxy's slot
    // on a broken invariant — a wrong-slot write, not a crash. Panic
    // instead so any future refactor of `resolve_op_identity` that
    // weakens this guarantee fails loud.
    #[allow(clippy::expect_used)]
    let canonical = canonical_holder_id(&broker.state, &name)
        .expect("resolve_op_identity succeeded above; canonical holder must exist for `name`");
    let joined_channels: Vec<String> = broker
        .state
        .clients
        .get(&canonical)
        .map(|state| state.joined.iter().cloned().collect())
        .unwrap_or_default();

    let mut cursor_advances: Vec<(MailboxName, u64)> = Vec::new();
    for channel in &joined_channels {
        let mailbox = MailboxName::Channel(channel.clone());
        // Scope B HIGH-fix (260619): read from `inbox_offsets`, NOT
        // `await_offsets`. The two cursors are intentionally
        // independent — a task-filtered `Await` that scans past
        // unrelated channel posts must not eat Inbox's view of those
        // same posts. Initialized at `Join` time alongside
        // `await_offsets` (see `join()` above).
        let cursor = broker
            .state
            .clients
            .get(&canonical)
            .and_then(|state| state.inbox_offsets.get(&mailbox).copied())
            .unwrap_or(0);
        let drained = match broker.env.drain_from(&mailbox, cursor) {
            Ok(drained) => drained,
            // A channel with no on-disk mailbox yet (no sends since
            // broker boot) is not an error — the drain returns empty,
            // not NotFound. Other errors (CorruptLine etc.) abort the
            // poll so the operator sees the breakage.
            Err(error) => return vec![err(client, BusErrorKind::Internal, error.to_string())],
        };
        if drained.records.is_empty() {
            // Fix 260708-l1x (#11): an empty drain still carries news when the
            // channel mailbox has shrunk beneath this holder's cursor —
            // `drained.next_offset` is the file's new end. Skipping the
            // write-back here (as this `continue` used to do unconditionally)
            // stranded the cursor above EOF forever, and the holder silently
            // stopped seeing the channel. `walk` clamps the Await path; this
            // loop never reaches `walk` on an empty drain, so it clamps here.
            if drained.next_offset < cursor {
                tracing::warn!(
                    channel = %channel,
                    stale_cursor = cursor,
                    clamped_to = drained.next_offset,
                    "channel mailbox shrank beneath the holder's Inbox cursor; clamping (external truncation, e.g. /famp-clear)"
                );
                cursor_advances.push((mailbox, drained.next_offset));
            }
            continue;
        }

        let truncated = drained.records.len() > CHANNEL_DRAIN_CAP;
        if truncated {
            tracing::debug!(
                channel = %channel,
                cap = CHANNEL_DRAIN_CAP,
                total = drained.records.len(),
                "inbox_channel_drain_capped"
            );
        }

        // Scope B MEDIUM-fix (260619): pub/sub default — a publisher does
        // not receive its own channel posts. The cursor advances past both
        // delivered envelopes AND skipped (self-authored / undecodable)
        // records so they never replay on the next poll.
        //
        // `Scanned(CHANNEL_DRAIN_CAP)`, NOT `Delivered` — the cap bounds
        // the WORK done per poll for hot-channel backpressure, so skipped
        // records consume budget too. Records past the cap stay on disk
        // and surface on the next poll.
        //
        // `AwaitFilter::Any` makes `walk`'s filter-mismatch stop branch
        // unreachable here, so the walk never halts mid-batch.
        let outcome = walk(
            &mailbox,
            cursor,
            &drained,
            &DrainPolicy {
                filter: &AwaitFilter::Any,
                skip_self_authored: Some(&name),
                require_local_origin: false,
                cap: Some(DrainCap::Scanned(CHANNEL_DRAIN_CAP)),
            },
        );
        envelopes.extend(
            outcome
                .delivered
                .into_iter()
                .map(|(origin, envelope)| StampedEnvelope { origin, envelope }),
        );
        // When un-truncated, outcome.next_offset equals drained.next_offset
        // by construction (we walked every record); the explicit branch
        // keeps intent local to the cap path.
        let effective_next_offset = if truncated {
            outcome.next_offset
        } else {
            drained.next_offset
        };
        cursor_advances.push((mailbox, effective_next_offset));
    }

    let reply = BusReply::InboxOk {
        envelopes,
        next_offset: agent_next_offset,
    };
    // #14: encode-before-commit for channel `inbox_offsets`. Pre-fix the
    // cursors advanced even when the write loop later failed FrameTooLarge on
    // the combined InboxOk — the client never saw the envelopes but a retry
    // started past them (skipped channel posts). Agent-mailbox position is
    // still client-tracked via `since` (no broker commit here).
    if let Err(error) = encode_frame(&reply) {
        return vec![err(
            client,
            BusErrorKind::EnvelopeTooLarge,
            format!("InboxOk exceeds 16 MiB reply-frame limit: {error}"),
        )];
    }

    // Stage all per-channel cursor advances only after the reply is known to
    // encode. Drain loop above only borrowed broker immutably.
    for (mailbox, offset) in cursor_advances {
        if let Some(state) = broker.state.clients.get_mut(&canonical) {
            state.inbox_offsets.insert(mailbox, offset);
        }
    }

    vec![Out::Reply(client, reply)]
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
    // D-10: mutate the canonical holder's `joined` set, not the proxy's.
    // For canonical holders this resolves to `client` itself; for
    // proxies it resolves to the live registered holder of `name`.
    let target_client = canonical_holder_id(&broker.state, &name).unwrap_or(client);

    let mailbox = MailboxName::Channel(channel.clone());
    // Join drain-from-start: the in-memory `cursors` map was never
    // populated (deleted in fix 260512-jdv); preserving the historical
    // since=0 behavior. Drain BEFORE committing membership so a drain
    // or encode failure cannot leave the holder half-joined (#14).
    let since: u64 = 0;
    let drained = match broker.env.drain_from(&mailbox, since) {
        Ok(drained) => drained,
        Err(error) => return vec![err(client, BusErrorKind::Internal, error.to_string())],
    };
    // Phase 14 plan 14-02: `JoinOk.drained` carries `StampedEnvelope`
    // elements — build them from decode_lines' per-record origin.
    let decoded: Vec<StampedEnvelope> = decode_lines(&mailbox, since, &drained)
        .into_iter()
        .map(|(origin, envelope)| StampedEnvelope { origin, envelope })
        .collect();

    // Prospective members list as it will look after this join commits
    // (existing members + self, with role applied only when provided).
    let mut member_names: BTreeSet<String> = broker
        .state
        .channels
        .get(&channel)
        .cloned()
        .unwrap_or_default();
    member_names.insert(name.clone());
    let members: Vec<MemberInfo> = member_names
        .iter()
        .map(|member_name| {
            let member_role = if member_name == &name {
                role.clone().or_else(|| {
                    broker
                        .state
                        .channel_roles
                        .get(&(channel.clone(), member_name.clone()))
                        .cloned()
                })
            } else {
                broker
                    .state
                    .channel_roles
                    .get(&(channel.clone(), member_name.clone()))
                    .cloned()
            };
            MemberInfo {
                name: member_name.clone(),
                role: member_role,
            }
        })
        .collect();

    let reply = BusReply::JoinOk {
        channel: channel.clone(),
        members,
        drained: decoded,
    };
    // #14: encode-before-commit (same half-success class as register).
    if let Err(error) = encode_frame(&reply) {
        return vec![err(
            client,
            BusErrorKind::EnvelopeTooLarge,
            format!("JoinOk exceeds 16 MiB reply-frame limit: {error}"),
        )];
    }

    broker
        .state
        .channels
        .entry(channel.clone())
        .or_default()
        .insert(name.clone());
    if let Some(state) = broker.state.clients.get_mut(&target_client) {
        state.joined.insert(channel.clone());
        state
            .await_offsets
            .insert(mailbox.clone(), drained.next_offset);
        // Scope B HIGH-fix (260619): seed the per-holder Inbox cursor
        // to the same join-time end-offset, decoupled from await_offsets
        // so a task-filtered Await on this channel cannot eat envelopes
        // out of Inbox's view.
        state
            .inbox_offsets
            .insert(mailbox.clone(), drained.next_offset);
    }
    // Store the declared role in `channel_roles` if provided.
    if let Some(ref r) = role {
        broker
            .state
            .channel_roles
            .insert((channel, name), r.clone());
    }

    vec![
        Out::Reply(client, reply),
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
    let target_client = canonical_holder_id(&broker.state, &name).unwrap_or(client);
    if let Some(state) = broker.state.clients.get_mut(&target_client) {
        state.joined.remove(&channel);
        // Scope B (260619): drop the per-channel cursors so a subsequent
        // Join replays from the channel's join-time end-offset (set
        // inside `join()`). Without this, a leave → rejoin would carry
        // a stale post-leave cursor, silently skipping envelopes
        // posted while the holder was a member. Both `await_offsets`
        // (used by `await_envelope`) and `inbox_offsets` (HIGH-fix,
        // used by `fn inbox`'s channel branch) are dropped.
        let channel_mailbox = MailboxName::Channel(channel.clone());
        state.await_offsets.remove(&channel_mailbox);
        state.inbox_offsets.remove(&channel_mailbox);
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
                    let holder_joined = canonical_holder_id(&broker.state, bound)
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

/// D-02/T-14-06 (Phase 14): resolve `client`'s declared [`Origin`] at
/// `Out::AppendMailbox` time.
///
/// Fail-closed: a client absent from `broker.state.clients` resolves to
/// `Origin::Unknown`, matching every other absence path in this module.
///
/// A canonical holder's own `ClientState.origin` (set by `register()`)
/// is authoritative. A proxy (`bind_as`) connection resolves through the
/// canonical holder it is bound to — the SAME lookup convention
/// `resolve_op_identity` uses — so a `famp send` issued over a proxy
/// connection on behalf of a gateway-registered holder carries that
/// holder's `Gateway` origin forward, rather than laundering it down to
/// `Unknown` (T-14-06: a proxy connection must not silently downgrade
/// provenance).
fn client_origin<E: BrokerEnv>(broker: &Broker<E>, client: ClientId) -> Origin {
    let Some(state) = broker.state.clients.get(&client) else {
        return Origin::Unknown;
    };
    if state.name.is_some() {
        return state.origin;
    }
    let Some(bound) = state.bind_as.as_deref() else {
        return Origin::Unknown;
    };
    canonical_holder_id(&broker.state, bound)
        .and_then(|holder_id| broker.state.clients.get(&holder_id))
        .map_or(Origin::Unknown, |holder| holder.origin)
}

fn connected_names(clients: &std::collections::BTreeMap<ClientId, ClientState>) -> Vec<String> {
    clients
        .values()
        .filter(|state| state.connected)
        .filter_map(|state| state.name.clone())
        .collect()
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

/// Decode every drained line into a typed envelope `Value`, SKIPPING any
/// line that fails decode rather than aborting the whole batch.
///
/// Head-of-line resilience (fix 260611): a single malformed/non-conformant
/// envelope (e.g. a foreign implementation that wrote a bad `causality.ref`
/// or omitted a required field) must NOT wedge a receiver's entire mailbox.
/// Each undecodable line is dropped from the delivered batch and logged
/// LOUDLY (`WARN` with mailbox + byte offset + decode error) so a
/// misbehaving peer stays visible — silent skipping would hide
/// cross-implementation data loss, the worst interop failure mode. The
/// raw line is retained in the append-only mailbox file, which is itself
/// the recovery store (no quarantine sidecar needed).
///
/// `start_offset` is the byte offset the drain began at; offsets reported
/// in the warning are absolute file offsets so the bad line can be located.
///
/// Two `DrainPolicy` values below look like oversights and are NOT. Do not
/// "fix" them:
///
/// - `skip_self_authored: None` — this walk serves the DM / `Register` /
///   `Join` / agent-mailbox `Inbox` paths. A message a client addressed to
///   ITSELF must be delivered. Only channel pub/sub suppresses self-authored
///   records.
/// - `cap: None` — the drain is deliberately unbounded here on all three
///   call sites (`register`, `join`, and agent-mailbox `inbox`). Bounding
///   it would TRUNCATE the drain, silently changing Register/Join/Inbox
///   semantics (a client would come up — or poll — having never seen part
///   of its own mailbox). That is a real design change and belongs with the
///   retention work in §3.1 of the 2026-07-08 refactoring review (backlog
///   999.11), not here. The interim guard is [`DRAIN_WARN_BYTES`] below: an
///   oversized drain gets an operator-visible WARN, and still delivers every
///   record. Do not "fix" the cliff with a silent `.take(N)`.
fn decode_lines(
    mailbox: &MailboxName,
    start_offset: u64,
    drained: &DrainResult,
) -> Vec<(crate::Origin, serde_json::Value)> {
    warn_if_drain_oversized(mailbox, start_offset, &drained.records);
    walk(
        mailbox,
        start_offset,
        drained,
        &DrainPolicy {
            filter: &AwaitFilter::Any,
            skip_self_authored: None,
            require_local_origin: false,
            cap: None,
        },
    )
    .delivered
}

/// Half of [`MAX_FRAME_BYTES`] (16 MiB). A drain whose byte span crosses
/// this is one doubling away from the reply-frame ceiling.
///
/// `decode_lines` is shared by **three** reply paths (issue #14), not just
/// register/join:
///
/// - `RegisterOk.drained` — full agent-mailbox drain at register
/// - `JoinOk.drained` — full channel-mailbox drain at join
/// - `InboxOk.envelopes` — agent-mailbox branch of `Inbox` (channel branch
///   uses `walk` directly with a scanned-record cap, not this helper)
///
/// Each of those is encoded into a SINGLE reply frame that `codec` rejects
/// above `MAX_FRAME_BYTES`. So a mailbox past 16 MiB makes **register, join,
/// and `famp_inbox` (since=0 / first-poll)** fail — not only registration.
/// MCP `famp_inbox` defaults `since` from the session cursor (#13) but the
/// first call of a session (and any explicit `since: 0`) still drains from
/// the head. No retention or compaction exists yet; this threshold buys an
/// operator one halving of headroom. Register, join, **and** inbox encode
/// the reply *before* committing state (#14) so a frame failure cannot leave
/// a half-applied bind, half-join, or advanced channel `inbox_offsets`.
const DRAIN_WARN_BYTES: u64 = 8 * 1024 * 1024;

/// Byte span the drain covers: `start_offset` to the last record's `end`.
///
/// Sourced entirely from the offsets `DrainedRecord` already carries — no
/// framing arithmetic is re-derived here (§3.2 removed exactly that). An
/// empty `records` slice spans zero bytes. `saturating_sub` keeps the result
/// sane if a caller ever passes a `start_offset` past the last record's end;
/// the drain simply is not reported as oversized.
fn drained_span(start_offset: u64, records: &[DrainedRecord]) -> u64 {
    records
        .last()
        .map_or(0, |last| last.end.saturating_sub(start_offset))
}

/// Emit exactly one WARN when a `decode_lines` drain (register / join /
/// agent-mailbox inbox) approaches the reply-frame limit. Does NOT truncate —
/// see the `cap: None` note on [`decode_lines`].
fn warn_if_drain_oversized(mailbox: &MailboxName, start_offset: u64, records: &[DrainedRecord]) {
    let drained_bytes = drained_span(start_offset, records);
    if drained_bytes > DRAIN_WARN_BYTES {
        tracing::warn!(
            mailbox = %mailbox,
            drained_bytes,
            records = records.len(),
            limit = MAX_FRAME_BYTES,
            "mailbox drain approaching the 16 MiB reply-frame limit; RegisterOk / \
             JoinOk / InboxOk will fail to encode once it is exceeded (no \
             retention/compaction exists yet — see backlog 999.11)"
        );
    }
}

/// D-17 (Phase 14): the single drain-decode site. Strict-parses `line`,
/// splits off any [`Origin`] stamp via [`crate::split_stamped`], and
/// validates the INNER envelope value against
/// `famp_envelope::AnyBusEnvelope::decode` — never `famp-envelope` itself
/// (frozen, D-16). A legacy pre-Phase-14 line (no stamp) and a
/// stamp-shaped line both flow through the same path; `split_stamped`'s
/// own fail-closed polarity (D-02) is what decides `Origin::Unknown` vs
/// an explicit stamp — this function does not re-decide it.
pub(super) fn decode_line(line: &[u8]) -> Result<(crate::Origin, serde_json::Value), String> {
    let raw: serde_json::Value =
        famp_canonical::from_slice_strict(line).map_err(|error| error.to_string())?;
    let (origin, inner) = crate::split_stamped(&raw);
    let inner_bytes = famp_canonical::canonicalize(inner)
        .map_err(|error| format!("re-canonicalizing inner envelope failed: {error}"))?;
    famp_envelope::AnyBusEnvelope::decode(&inner_bytes)
        .map_err(|error| format!("drain line rejected by AnyBusEnvelope::decode: {error}"))?;
    Ok((origin, inner.clone()))
}

fn send_ok(
    client: ClientId,
    task_id: uuid::Uuid,
    to: Target,
    ok: bool,
    woken: bool,
    wake_addr: Option<String>,
) -> Out {
    Out::Reply(
        client,
        BusReply::SendOk {
            task_id,
            delivered: vec![Delivered {
                to,
                ok,
                woken,
                wake_addr,
            }],
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

pub(super) fn err(client: ClientId, kind: BusErrorKind, message: impl Into<String>) -> Out {
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

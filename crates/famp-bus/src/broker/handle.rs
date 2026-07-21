use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use crate::broker::awaiting::{await_envelope, await_reply_for_mailbox, waiting_clients_for_name};
use crate::broker::drain_walk::{walk, DrainCap, DrainPolicy};
use crate::broker::identity::{canonical_holder_id, proxy_holder_alive, resolve_op_identity};
use crate::broker::state::ClientState;
use crate::{
    AwaitFilter, Broker, BrokerEnv, BrokerInput, BusErrorKind, BusMessage, BusReply, ClientId,
    Delivered, DrainResult, DrainedRecord, MailboxName, MemberInfo, Out, SessionRow, Target,
    BUS_PROTO_VERSION, MAX_FRAME_BYTES,
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
    let decoded = decode_lines(&mailbox, since, &drained);

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

/// Scope B (260619): per-channel drain cap. A hot channel with thousands
/// of envelopes must not bloat a single `Inbox` response from a slow
/// reader. The cap is per channel per poll — across N joined channels
/// the worst-case response is N * CHANNEL_DRAIN_CAP envelopes. Picked
/// to match Await's batching posture (see `awaiting::drain_await_batch`).
const CHANNEL_DRAIN_CAP: usize = 256;

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
    let mut envelopes = decode_lines(&agent_mailbox, agent_since, &agent_drained);
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
                cap: Some(DrainCap::Scanned(CHANNEL_DRAIN_CAP)),
            },
        );
        envelopes.extend(outcome.delivered);
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

    // Stage all per-channel cursor advances after the read loop so the
    // borrow checker stays happy (drain_from borrows broker immutably).
    for (mailbox, offset) in cursor_advances {
        if let Some(state) = broker.state.clients.get_mut(&canonical) {
            state.inbox_offsets.insert(mailbox, offset);
        }
    }

    vec![Out::Reply(
        client,
        BusReply::InboxOk {
            envelopes,
            next_offset: agent_next_offset,
        },
    )]
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
    let target_client = canonical_holder_id(&broker.state, &name).unwrap_or(client);
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
    let decoded = decode_lines(&mailbox, since, &drained);
    if let Some(state) = broker.state.clients.get_mut(&target_client) {
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
///   `Join` paths. A message a client addressed to ITSELF must be delivered.
///   Only channel pub/sub suppresses self-authored records.
/// - `cap: None` — the register-path drain is deliberately unbounded here.
///   Bounding it would TRUNCATE the drain, silently changing Register/Join
///   semantics (a client would come up having never seen part of its own
///   mailbox). That is a real design change and belongs with the retention
///   work in §3.1 of the 2026-07-08 refactoring review (backlog 999.11), not
///   here. The interim guard is [`DRAIN_WARN_BYTES`] below: an oversized
///   drain gets an operator-visible WARN, and still delivers every record.
fn decode_lines(
    mailbox: &MailboxName,
    start_offset: u64,
    drained: &DrainResult,
) -> Vec<serde_json::Value> {
    warn_if_drain_oversized(mailbox, start_offset, &drained.records);
    walk(
        mailbox,
        start_offset,
        drained,
        &DrainPolicy {
            filter: &AwaitFilter::Any,
            skip_self_authored: None,
            cap: None,
        },
    )
    .delivered
}

/// Half of [`MAX_FRAME_BYTES`] (16 MiB). A register/join drain whose byte
/// span crosses this is one doubling away from the reply-frame ceiling.
///
/// `decode_lines` output becomes `RegisterOk.drained` / `JoinOk.drained`,
/// encoded into a SINGLE reply frame that `codec` rejects above
/// `MAX_FRAME_BYTES`. So a mailbox that grows past 16 MiB makes registration
/// itself fail — with no prior signal, because no retention or compaction
/// exists yet. This threshold buys an operator one halving of headroom.
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

/// Emit exactly one WARN when a register/join drain approaches the reply-frame
/// limit. Does NOT truncate — see the `cap: None` note on [`decode_lines`].
fn warn_if_drain_oversized(mailbox: &MailboxName, start_offset: u64, records: &[DrainedRecord]) {
    let drained_bytes = drained_span(start_offset, records);
    if drained_bytes > DRAIN_WARN_BYTES {
        tracing::warn!(
            mailbox = %mailbox,
            drained_bytes,
            records = records.len(),
            limit = MAX_FRAME_BYTES,
            "mailbox drain approaching the 16 MiB reply-frame limit; registration \
             will fail once it is exceeded (no retention/compaction exists yet — \
             see backlog 999.11)"
        );
    }
}

pub(super) fn decode_line(line: &[u8]) -> Result<serde_json::Value, String> {
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

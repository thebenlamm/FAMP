//! Await subsystem for the broker actor.
//!
//! Extracted from `handle.rs` (§1). Owns the 9 interdependent await
//! functions: parking/draining client `Await` requests, per-mailbox
//! offset accounting, and waiter wake matching. Dispatch (`handle`)
//! calls `await_envelope`, `await_reply_for_mailbox`, and
//! `waiting_clients_for_name`; everything else here is module-private.
//! Pure mechanical move from handle.rs; zero behavior change.

use std::time::{Duration, Instant};

use crate::broker::handle::{decode_line, err};
use crate::broker::identity::{
    canonical_holder_id, effective_identity, proxy_holder_alive, resolve_op_identity,
};
use crate::broker::state::ParkedAwait;
use crate::mailbox::JSONL_RECORD_TERMINATOR_LEN;
use crate::{AwaitFilter, Broker, BrokerEnv, BusErrorKind, BusReply, ClientId, MailboxName, Out};

const AWAIT_BATCH_CAP: usize = 50;

pub(super) fn await_envelope<E: BrokerEnv>(
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

#[derive(Debug)]
struct AwaitBatch {
    mailbox: MailboxName,
    envelopes: Vec<serde_json::Value>,
    next_offset: u64,
    /// `false` when `drain_await_batch` stopped early because it walked
    /// into a real, filter-mismatched envelope (Debug 999.1) rather than
    /// exhausting everything currently drained. `await_reply_for_mailbox`
    /// uses this to distinguish "genuinely nothing new for this filter"
    /// (safe to report as a timeout) from other empty-batch causes.
    fully_drained: bool,
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

pub(super) fn await_reply_for_mailbox<E: BrokerEnv>(
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
        Ok(batch) if !batch.fully_drained => {
            // Debug 999.1: this wake was selected because the newly
            // arrived envelope matched `filter` (see
            // `waiting_clients_for_name`), but an EARLIER, already-on-disk
            // envelope for a different task sat between this client's
            // offset and the new arrival and correctly blocked the drain
            // (see `drain_await_batch`) rather than being silently
            // skipped. Nothing new was actually deliverable to this
            // filter yet. Report it the same way a normal expiry would —
            // NOT as an Internal error — so the client simply retries;
            // the blocking entry is drained once some call (this
            // client's own differently-filtered await, or another
            // consumer) walks past it.
            //
            // Debug 999.1 operator-visible signal: this is the live
            // replacement for the SPEC's original (now-dead-code) M3
            // eprintln target. Without this, a filtered await blocked
            // behind an earlier unmatched real envelope is a silent
            // `AwaitTimeout` indistinguishable from "nothing arrived" —
            // undiagnosable from the operator side. Log the mailbox,
            // filter, and the byte offset the drain stalled at so
            // `famp inspect broker`-style debugging can see it.
            tracing::info!(
                mailbox = %mailbox,
                filter = ?filter,
                blocked_at_offset = batch.next_offset,
                "filtered await blocked behind an earlier unmatched envelope; reporting AwaitTimeout (999.1 known boundary, see backlog 999.11)"
            );
            set_await_offset(broker, owner, mailbox, batch.next_offset);
            BusReply::AwaitTimeout {}
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

    // Self-filter: standard pub/sub semantics — a publisher does not receive
    // its own posts. When draining a channel mailbox, resolve the awaiter's
    // identity once here and skip any envelope whose `from` matches. The
    // offset still advances past skipped lines so the cursor never re-sees
    // them (same invariant as the head-of-line skip below). `None` on
    // non-channel mailboxes so the check is a no-op on the agent-inbox path.
    let awaiter_identity: Option<String> = if matches!(mailbox, MailboxName::Channel(_)) {
        broker
            .state
            .clients
            .get(&owner)
            .and_then(|s| effective_identity(s).ok())
    } else {
        None
    };

    let mut next_offset = since;
    let mut envelopes = Vec::new();
    let mut fully_drained = true;
    for record in drained.records {
        let line = &record.bytes;
        // Framing math lives in the `MailboxRead` impl, not here: `record.end`
        // IS the cursor value for "consumed exactly this record".
        let line_next_offset = record.end;
        // Head-of-line resilience (fix 260611): a single undecodable line
        // must NOT wedge the await drain. The pre-fix `?` returned BEFORE
        // `next_offset` advanced, so the cursor never moved past a bad line
        // and a listen-mode agent's inbox stayed jammed forever. Skip the
        // line, advance past it, and log LOUDLY so the misbehaving peer
        // stays visible. (Mirrors `decode_lines` on the inbox/register path.)
        match decode_line(line) {
            Ok(value) => {
                if is_self_authored(&value, awaiter_identity.as_deref()) {
                    // Permanently unmatchable under ANY filter (an awaiter
                    // never receives its own posts) — safe to advance past
                    // unconditionally, same invariant as the undecodable
                    // case below.
                    next_offset = line_next_offset;
                    continue;
                }
                if filter_matches(filter, &value) {
                    envelopes.push(value);
                    next_offset = line_next_offset;
                    if envelopes.len() == AWAIT_BATCH_CAP {
                        return Ok(AwaitBatch {
                            mailbox: mailbox.clone(),
                            envelopes,
                            next_offset,
                            // Cap reached, not a filter-mismatch stop —
                            // irrelevant here since the caller's
                            // non-empty-envelopes arm matches first, but
                            // kept accurate: there may be more undrained
                            // data beyond this cap.
                            fully_drained: false,
                        });
                    }
                    continue;
                }
                // Debug 999.1 (broker await_offset skip): a task-filter
                // mismatch is NOT permanently unmatchable the way a
                // self-authored or undecodable line is — a future
                // differently-filtered (or unfiltered) `Await` call from
                // this same owner may still want this envelope. The
                // persisted `next_offset` is a single linear cursor shared
                // across every future call regardless of filter, so it
                // cannot represent "skip this one for THIS filter, but
                // keep it reachable for others" without also risking
                // re-delivering entries we already returned earlier in
                // this same batch. Stop draining right here: `next_offset`
                // never advances past an envelope this call didn't
                // actually hand back to the caller. Remaining lines in
                // this batch (even ones that would have matched) are
                // picked up on a later call, once nothing upstream of
                // them is still blocked.
                fully_drained = false;
                break;
            }
            Err(error) => {
                tracing::warn!(
                    mailbox = %mailbox,
                    byte_offset = next_offset,
                    error = %error,
                    "skipping undecodable mailbox line (head-of-line resilience)"
                );
                // An undecodable line can never match any filter either —
                // permanently unmatchable, same as self-authored above.
                next_offset = line_next_offset;
            }
        }
    }

    // The wake-trigger envelope is only safe to fold in when this call
    // scanned the whole currently-drained batch cleanly (no earlier
    // filter-mismatch left pending). Applying it after an early stop would
    // advance `next_offset` past the trigger while an earlier mismatch is
    // still un-drained, reproducing the exact same bug for the wake path.
    if fully_drained {
        debug_assert_eq!(next_offset, drained.next_offset);
        if let Some((trigger_envelope, trigger_line_len)) = trigger {
            // The wake-trigger envelope was never drained, so there is no
            // `DrainedRecord` to source an offset from — frame it by hand,
            // but from the shared terminator constant, not a magic `1`.
            let trigger_next_offset =
                next_offset + trigger_line_len as u64 + JSONL_RECORD_TERMINATOR_LEN;
            let trigger_self_authored =
                is_self_authored(trigger_envelope, awaiter_identity.as_deref());
            if trigger_self_authored {
                // Permanently unmatchable — same as the main-loop case.
                next_offset = trigger_next_offset;
            } else if filter_matches(filter, trigger_envelope) {
                envelopes.push(trigger_envelope.clone());
                next_offset = trigger_next_offset;
            }
            // else: a real, filter-mismatched trigger envelope. Same
            // Debug-999.1 reasoning as the main loop — do not advance
            // `next_offset` past it; it stays reachable for a future,
            // differently-filtered call.
        }
    }

    Ok(AwaitBatch {
        mailbox: mailbox.clone(),
        envelopes,
        next_offset,
        fully_drained,
    })
}

/// Returns `true` when the envelope's `from` field ends in `/<awaiter>`,
/// indicating the awaiter authored the message.
///
/// Envelope `from` format: `agent:<host>/<name>`. Splitting on `/` and
/// comparing the last segment is host-agnostic and avoids URI parsing.
/// Returns `false` when `awaiter_identity` is `None` (non-channel path),
/// when `from` is absent/malformed, or when the names do not match.
pub(super) fn is_self_authored(
    envelope: &serde_json::Value,
    awaiter_identity: Option<&str>,
) -> bool {
    let Some(awaiter) = awaiter_identity else {
        return false;
    };
    let Some(from) = envelope.get("from").and_then(|v| v.as_str()) else {
        return false;
    };
    from.rsplit('/')
        .next()
        .is_some_and(|sender| sender == awaiter)
}

pub(super) fn waiting_clients_for_name<E: BrokerEnv>(
    broker: &Broker<E>,
    name: &str,
    envelope: &serde_json::Value,
) -> Vec<ClientId> {
    broker
        .state
        .pending_awaits
        .iter()
        .filter_map(|(client, parked)| {
            let state = broker.state.clients.get(client)?;
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
                Some(*client)
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

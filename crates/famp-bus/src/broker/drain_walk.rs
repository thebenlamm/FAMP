//! The single JSONL drain walk: decode → decide → advance.
//!
//! Before this module existed the same loop lived in four places
//! (`drain_await_batch`, `inbox`'s channel loop, `decode_lines`, and the
//! wake-trigger fold). Debug 999.1 and Scope B were each a fix applied to
//! exactly one copy. Everything that decides deliver-vs-skip-vs-stop and
//! advances a mailbox cursor now goes through [`walk`]; the three call sites
//! differ only by a [`DrainPolicy`] literal.
//!
//! Synchronous, allocation-light, tokio-free (BUS-01).

use crate::broker::handle::decode_line;
use crate::{AwaitFilter, DrainResult, MailboxName};

/// How a walk is allowed to stop early. The two variants are NOT
/// interchangeable and collapsing them into one `usize` silently changes
/// behavior on both paths — this is the single easiest thing to get wrong
/// in this module.
///
/// - [`DrainCap::Delivered`] stops after the n-th DELIVERED envelope.
///   Skipped records (self-authored, undecodable) consume no budget, so a
///   mailbox full of skips still walks to the end. This is `Await`'s
///   `AWAIT_BATCH_CAP`: it bounds the size of the reply.
/// - [`DrainCap::Scanned`] walks at most the first n RECORDS regardless of
///   how many were delivered. Skipped records DO consume budget. This is
///   `Inbox`'s `CHANNEL_DRAIN_CAP`: it bounds the work done per poll on a
///   hot channel, and the leftover records surface on the next poll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DrainCap {
    Delivered(usize),
    Scanned(usize),
}

/// The three axes on which the drain consumers legitimately differ.
pub(super) struct DrainPolicy<'a> {
    /// Which envelopes this caller wants. `AwaitFilter::Any` makes the
    /// filter-mismatch stop branch unreachable, which is why the `Inbox`
    /// and `Register`/`Join` paths can share this walk with `Await`.
    pub filter: &'a AwaitFilter,
    /// `Some(identity)` applies channel pub/sub semantics: a publisher does
    /// not receive its own posts. `None` DELIVERS self-authored records —
    /// correct on the DM / register / join paths, where a message a client
    /// addressed to itself must still arrive.
    pub skip_self_authored: Option<&'a str>,
    /// `None` walks every record handed in.
    pub cap: Option<DrainCap>,
}

pub(super) struct WalkOutcome {
    pub delivered: Vec<serde_json::Value>,
    /// The cursor value the caller should persist. Never advances past a
    /// record this walk did not either deliver or prove permanently
    /// unmatchable (Debug 999.1 invariant).
    pub next_offset: u64,
    /// `false` when the walk stopped early — either on a real,
    /// filter-mismatched envelope (Debug 999.1) or because a cap was hit.
    /// `true` means every currently-drained record was accounted for.
    pub fully_drained: bool,
}

/// Walk `drained`'s records, deciding deliver / skip / stop per record and
/// advancing the cursor exactly once per accounted-for record.
///
/// `since` seeds `next_offset`, so a zero-record walk returns the caller's
/// own cursor untouched. Every subsequent advance is sourced from
/// `DrainedRecord::end` — no framing arithmetic happens here.
///
/// Takes the whole [`DrainResult`], not a bare `&[DrainedRecord]`, because the
/// seed depends on `drained.next_offset` as well as `since` (see the clamp
/// below). Passing the two separately would let a call site hand in a
/// `next_offset` that does not belong to the records it also handed in — the
/// exact class of mistake this module exists to make impossible.
pub(super) fn walk(
    mailbox: &MailboxName,
    since: u64,
    drained: &DrainResult,
    policy: &DrainPolicy<'_>,
) -> WalkOutcome {
    let records = &drained.records;
    // `Scanned(n)` truncates the walk to the first n records and, if that
    // truncated anything, the drain is by definition not fully consumed.
    // Computed before the loop so the cap kind never leaks into the body.
    let (scan, mut fully_drained) = match policy.cap {
        Some(DrainCap::Scanned(n)) => (records.get(..n).unwrap_or(records), records.len() <= n),
        _ => (records.as_slice(), true),
    };
    let delivered_cap = match policy.cap {
        Some(DrainCap::Delivered(n)) => Some(n),
        _ => None,
    };

    let mut delivered: Vec<serde_json::Value> = Vec::new();
    // Fix 260708-l1x (#11): the mailbox can shrink beneath our cursor —
    // `/famp-clear` truncates mailbox files while the broker holds in-memory
    // offsets into them. The drain is authoritative about where the file now
    // ends; a cursor past that point is stale, not an invariant violation.
    // Clamp DOWN to it, and only to it, so forward progress is preserved on
    // every non-truncation path:
    //
    //   records present     → next_offset > since  → min == since  (no-op)
    //   empty, EOF == cursor→ next_offset == since → min == since  (no-op)
    //   mid-line `since`    → production snaps forward, so
    //                         next_offset == file_len >= since
    //                                              → min == since  (no-op;
    //                         the loop below then overwrites from record.end)
    //   TRUNCATED           → next_offset < since  → min == next_offset (heal)
    //
    // Silent healing is how this stayed invisible for so long: warn when it
    // actually fires.
    if drained.next_offset < since {
        tracing::warn!(
            mailbox = %mailbox,
            stale_cursor = since,
            clamped_to = drained.next_offset,
            "mailbox shrank beneath the holder's cursor; clamping (external truncation, e.g. /famp-clear)"
        );
    }
    let mut next_offset = since.min(drained.next_offset);

    for record in scan {
        match decode_line(&record.bytes) {
            Err(error) => {
                // Head-of-line resilience (fix 260611): a single undecodable
                // record must NOT wedge the drain. The pre-fix `?` returned
                // BEFORE the cursor advanced, so a listen-mode agent's inbox
                // stayed jammed forever behind one malformed line from a
                // foreign implementation. Skip it, advance past it, and log
                // LOUDLY so the misbehaving peer stays visible. An
                // undecodable record can never match any filter, so
                // advancing past it is unconditionally safe. The raw line
                // stays in the append-only mailbox file, which is itself the
                // recovery store.
                tracing::warn!(
                    mailbox = %mailbox,
                    byte_offset = record.start,
                    error = %error,
                    "skipping undecodable mailbox line (head-of-line resilience)"
                );
                next_offset = record.end;
            }
            Ok(value) => {
                if is_self_authored(&value, policy.skip_self_authored) {
                    // Permanently unmatchable under ANY filter (a subscriber
                    // never receives its own posts) — safe to advance past
                    // unconditionally, same invariant as the undecodable case.
                    next_offset = record.end;
                    continue;
                }
                if filter_matches(policy.filter, &value) {
                    delivered.push(value);
                    next_offset = record.end;
                    if delivered_cap == Some(delivered.len()) {
                        // Cap reached, not a filter-mismatch stop: there may
                        // be more undrained data beyond this point.
                        return WalkOutcome {
                            delivered,
                            next_offset,
                            fully_drained: false,
                        };
                    }
                    continue;
                }
                // Debug 999.1 (broker await_offset skip): a task-filter
                // mismatch is NOT permanently unmatchable the way a
                // self-authored or undecodable record is — a future
                // differently-filtered (or unfiltered) call from this same
                // owner may still want this envelope. The persisted
                // `next_offset` is a single linear cursor shared across every
                // future call regardless of filter, so it cannot represent
                // "skip this one for THIS filter, but keep it reachable for
                // others" without also risking re-delivering entries already
                // returned earlier in this same batch. Stop right here:
                // `next_offset` never advances past an envelope this call
                // didn't actually hand back. Remaining records (even ones
                // that would have matched) are picked up on a later call,
                // once nothing upstream of them is still blocked.
                fully_drained = false;
                break;
            }
        }
    }

    WalkOutcome {
        delivered,
        next_offset,
        fully_drained,
    }
}

/// Returns `true` when the envelope's `from` field ends in `/<identity>`,
/// indicating the reader authored the message.
///
/// Envelope `from` format: `agent:<host>/<name>`. Splitting on `/` and
/// comparing the last segment is host-agnostic and avoids URI parsing.
/// Returns `false` when `identity` is `None` (non-channel path), when
/// `from` is absent/malformed, or when the names do not match.
pub(super) fn is_self_authored(envelope: &serde_json::Value, identity: Option<&str>) -> bool {
    let Some(reader) = identity else {
        return false;
    };
    let Some(from) = envelope.get("from").and_then(|v| v.as_str()) else {
        return false;
    };
    from.rsplit('/')
        .next()
        .is_some_and(|sender| sender == reader)
}

pub(super) fn filter_matches(filter: &AwaitFilter, envelope: &serde_json::Value) -> bool {
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

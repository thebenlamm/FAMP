//! Bounded, in-memory, opaque-bytes store-and-forward queue (Task 1,
//! REACH-04).
//!
//! `RelayQueues` never parses a queued body. It stores exactly the bytes
//! a POST presented and returns exactly those bytes, unchanged, on a
//! later authorized drain — including a body that is not valid UTF-8.
//! Depth and age are both bounded so a temporarily-offline peer's
//! backlog cannot grow the relay's memory without limit.

use std::collections::{HashMap, VecDeque};

use time::OffsetDateTime;

/// Maximum number of queued entries held per destination domain at once.
///
/// A store-and-forward relay is a buffer for a peer that is temporarily
/// offline, not unbounded storage — see the drop-oldest rationale on
/// [`RelayQueues::enqueue`] for why the bound is enforced by eviction
/// rather than by rejecting new writes outright.
pub const RELAY_QUEUE_MAX_PER_DOMAIN: usize = 1024;

/// How long a queued entry survives before [`RelayQueues::sweep`] (or a
/// [`RelayQueues::drain`] on the same domain) reclaims it, in seconds.
pub const RELAY_ENTRY_TTL_SECS: i64 = 900;

/// Maximum request body size accepted by either relay route.
///
/// Applied via the same streaming
/// `tower_http::limit::RequestBodyLimitLayer` the gateway uses — a
/// request above this cap is rejected 413 without being fully buffered
/// into memory.
pub const RELAY_MAX_BODY_BYTES: usize = 1_048_576;

/// Maximum number of entries a single authorized fetch drains at once. A
/// domain with more queued entries than this keeps the remainder queued
/// for the next fetch.
pub const RELAY_FETCH_MAX_BATCH: usize = 64;

/// One queued, opaque envelope.
#[derive(Debug, Clone)]
pub struct QueuedEnvelope {
    /// The recipient principal string, taken VERBATIM from the enqueuing
    /// POST's URL path — never re-derived from the envelope body.
    ///
    /// This is a specific security property, not an implementation
    /// shortcut: the receiving gateway's `MisaddressedRecipient` check
    /// compares the request-supplied recipient against the SIGNED `to`
    /// inside the envelope. If the receiving gateway instead re-read the
    /// recipient out of the envelope itself, that check would become
    /// tautological on the relay path and would silently stop doing
    /// work. Carrying the path value verbatim through the relay keeps
    /// the relay path's checks identical to the direct-POST path's.
    pub recipient: String,
    /// The opaque envelope bytes, stored and returned byte-identical.
    /// Never parsed, never inspected, never logged.
    pub bytes: Vec<u8>,
    pub queued_at: OffsetDateTime,
}

/// Distinguishes a clean enqueue from one that had to evict the oldest
/// entry to make room under [`RELAY_QUEUE_MAX_PER_DOMAIN`].
///
/// The caller (the enqueue route handler) logs the drop when it happens
/// — see [`RelayQueues::enqueue`]'s doc comment for why the drop is
/// logged and never silent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnqueueOutcome {
    Queued,
    QueuedAfterDroppingOldest,
}

/// Bounded, TTL-swept, in-memory per-domain queue of opaque envelope
/// bytes.
///
/// Keyed by destination DOMAIN (not by recipient) — one relay serves
/// every recipient at a domain through the same queue, and the
/// recipient distinction lives on [`QueuedEnvelope::recipient`].
#[derive(Default)]
pub struct RelayQueues {
    by_domain: HashMap<String, VecDeque<QueuedEnvelope>>,
}

impl RelayQueues {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Store `bytes` verbatim for `domain`, tagged with `recipient` and
    /// `now`.
    ///
    /// Drop-oldest under sustained overflow, not reject-newest: a relay
    /// is a store-and-forward buffer for a peer that is temporarily
    /// offline, so under sustained overflow the newest messages are the
    /// ones most likely to still matter, and dropping the oldest keeps
    /// the queue useful rather than wedging it. The drop is LOGGED by
    /// the caller (never silent) — a silently dropped envelope is
    /// exactly the fire-and-forget failure REACH-05 exists to remove.
    ///
    /// **Known, accepted residual (T-17-37):** this route is
    /// unauthenticated by design (see `http`'s module doc), so anyone
    /// who knows this relay's URL and a served domain can flood a queue
    /// and evict legitimate entries — and the evicted sender already
    /// received a 202. The honest fixes (enqueue-side authorization, or
    /// Phase 16 making sender identity known to the relay) are both out
    /// of scope here; this is named, not papered over.
    pub fn enqueue(
        &mut self,
        domain: &str,
        recipient: String,
        bytes: Vec<u8>,
        now: OffsetDateTime,
    ) -> EnqueueOutcome {
        let queue = self.by_domain.entry(domain.to_owned()).or_default();
        let outcome = if queue.len() >= RELAY_QUEUE_MAX_PER_DOMAIN {
            queue.pop_front();
            EnqueueOutcome::QueuedAfterDroppingOldest
        } else {
            EnqueueOutcome::Queued
        };
        queue.push_back(QueuedEnvelope {
            recipient,
            bytes,
            queued_at: now,
        });
        outcome
    }

    /// Sweep expired entries for `domain`, then pop up to `max` entries
    /// from the front (FIFO) and return them. An unknown domain returns
    /// an empty list, never an error.
    pub fn drain(&mut self, domain: &str, now: OffsetDateTime, max: usize) -> Vec<QueuedEnvelope> {
        let Some(queue) = self.by_domain.get_mut(domain) else {
            return Vec::new();
        };
        retain_unexpired(queue, now);
        let take = max.min(queue.len());
        queue.drain(..take).collect()
    }

    /// Drop expired entries across every domain and remove any queue
    /// that is left empty, so the outer map cannot grow without bound
    /// from domains nobody ever fetches.
    pub fn sweep(&mut self, now: OffsetDateTime) {
        for queue in self.by_domain.values_mut() {
            retain_unexpired(queue, now);
        }
        self.by_domain.retain(|_, queue| !queue.is_empty());
    }

    #[must_use]
    pub fn depth(&self, domain: &str) -> usize {
        self.by_domain.get(domain).map_or(0, VecDeque::len)
    }
}

/// Drop entries older than [`RELAY_ENTRY_TTL_SECS`] (inclusive boundary:
/// an entry exactly `RELAY_ENTRY_TTL_SECS` old is still kept).
fn retain_unexpired(queue: &mut VecDeque<QueuedEnvelope>, now: OffsetDateTime) {
    queue.retain(|entry| (now - entry.queued_at).whole_seconds() <= RELAY_ENTRY_TTL_SECS);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn now() -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }

    #[test]
    fn enqueue_on_empty_queue_stores_bytes_and_depth_is_one() {
        let mut q = RelayQueues::new();
        let outcome = q.enqueue(
            "hosta.test",
            "agent:hosta.test/alice".to_owned(),
            vec![1, 2, 3],
            now(),
        );
        assert_eq!(outcome, EnqueueOutcome::Queued);
        assert_eq!(q.depth("hosta.test"), 1);
    }

    #[test]
    fn drain_returns_fifo_order_with_recipient_and_empties_queue() {
        let mut q = RelayQueues::new();
        let t = now();
        q.enqueue("hosta.test", "r1".to_owned(), vec![1], t);
        q.enqueue("hosta.test", "r2".to_owned(), vec![2], t);
        q.enqueue("hosta.test", "r3".to_owned(), vec![3], t);

        let drained = q.drain("hosta.test", t, RELAY_FETCH_MAX_BATCH);
        assert_eq!(drained.len(), 3);
        assert_eq!(drained[0].recipient, "r1");
        assert_eq!(drained[0].bytes, vec![1]);
        assert_eq!(drained[1].recipient, "r2");
        assert_eq!(drained[2].recipient, "r3");
        assert_eq!(q.depth("hosta.test"), 0);
    }

    #[test]
    fn drain_returns_at_most_max_batch_and_leaves_remainder_queued() {
        let mut q = RelayQueues::new();
        let t = now();
        for i in 0..(RELAY_FETCH_MAX_BATCH + 5) {
            q.enqueue("hosta.test", i.to_string(), vec![], t);
        }
        let drained = q.drain("hosta.test", t, RELAY_FETCH_MAX_BATCH);
        assert_eq!(drained.len(), RELAY_FETCH_MAX_BATCH);
        assert_eq!(q.depth("hosta.test"), 5);
    }

    #[test]
    fn drain_on_unknown_domain_returns_empty_not_error() {
        let mut q = RelayQueues::new();
        let drained = q.drain("nobody.test", now(), RELAY_FETCH_MAX_BATCH);
        assert!(drained.is_empty());
    }

    #[test]
    fn entries_older_than_ttl_are_dropped_by_sweep_and_never_returned_by_drain() {
        let mut q = RelayQueues::new();
        let t0 = now();
        q.enqueue("hosta.test", "r1".to_owned(), vec![9], t0);

        let past_ttl = t0 + time::Duration::seconds(RELAY_ENTRY_TTL_SECS + 1);
        q.sweep(past_ttl);
        assert_eq!(q.depth("hosta.test"), 0);

        // Re-seed and prove `drain` itself sweeps first, independent of
        // an explicit `sweep` call.
        q.enqueue("hosta.test", "r2".to_owned(), vec![9], t0);
        let drained = q.drain("hosta.test", past_ttl, RELAY_FETCH_MAX_BATCH);
        assert!(drained.is_empty());
    }

    #[test]
    fn entry_exactly_at_ttl_boundary_is_kept() {
        let mut q = RelayQueues::new();
        let t0 = now();
        q.enqueue("hosta.test", "r1".to_owned(), vec![9], t0);
        let at_boundary = t0 + time::Duration::seconds(RELAY_ENTRY_TTL_SECS);
        let drained = q.drain("hosta.test", at_boundary, RELAY_FETCH_MAX_BATCH);
        assert_eq!(
            drained.len(),
            1,
            "exactly RELAY_ENTRY_TTL_SECS old must still be returned"
        );
    }

    #[test]
    fn enqueue_at_cap_drops_oldest_and_depth_stays_at_cap() {
        let mut q = RelayQueues::new();
        let t = now();
        for i in 0..RELAY_QUEUE_MAX_PER_DOMAIN {
            let outcome = q.enqueue("hosta.test", i.to_string(), vec![], t);
            assert_eq!(outcome, EnqueueOutcome::Queued);
        }
        assert_eq!(q.depth("hosta.test"), RELAY_QUEUE_MAX_PER_DOMAIN);

        let outcome = q.enqueue("hosta.test", "overflow".to_owned(), vec![], t);
        assert_eq!(outcome, EnqueueOutcome::QueuedAfterDroppingOldest);
        assert_eq!(q.depth("hosta.test"), RELAY_QUEUE_MAX_PER_DOMAIN);

        // The oldest ("0") must be gone; the newest ("overflow") present.
        let drained = q.drain("hosta.test", t, RELAY_QUEUE_MAX_PER_DOMAIN);
        assert_eq!(
            drained[0].recipient, "1",
            "entry '0' must have been evicted as the oldest"
        );
        assert_eq!(drained.last().unwrap().recipient, "overflow");
    }

    #[test]
    fn bytes_round_trip_byte_identical_including_invalid_utf8() {
        let mut q = RelayQueues::new();
        let invalid_utf8 = vec![0xFF, 0xFE, 0x00, 0x80, 0x01];
        assert!(
            String::from_utf8(invalid_utf8.clone()).is_err(),
            "fixture must actually be invalid UTF-8"
        );
        q.enqueue("hosta.test", "r1".to_owned(), invalid_utf8.clone(), now());
        let drained = q.drain("hosta.test", now(), RELAY_FETCH_MAX_BATCH);
        assert_eq!(drained[0].bytes, invalid_utf8);
    }

    #[test]
    fn sweep_removes_emptied_queues_from_the_outer_map() {
        let mut q = RelayQueues::new();
        let t0 = now();
        q.enqueue("hosta.test", "r1".to_owned(), vec![], t0);
        let past_ttl = t0 + time::Duration::seconds(RELAY_ENTRY_TTL_SECS + 1);
        q.sweep(past_ttl);
        assert_eq!(q.depth("hosta.test"), 0);
        assert!(
            !q.by_domain.contains_key("hosta.test"),
            "an emptied queue's map entry must be removed, not left as an empty VecDeque"
        );
    }

    #[test]
    fn sweep_does_not_touch_a_different_domains_entries() {
        let mut q = RelayQueues::new();
        let t0 = now();
        q.enqueue("hosta.test", "old".to_owned(), vec![], t0);
        let past_ttl = t0 + time::Duration::seconds(RELAY_ENTRY_TTL_SECS + 1);
        q.enqueue("hostb.test", "fresh".to_owned(), vec![], past_ttl);
        q.sweep(past_ttl);
        assert_eq!(q.depth("hosta.test"), 0);
        assert_eq!(q.depth("hostb.test"), 1);
    }
}

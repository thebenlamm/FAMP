# FAMP 999.11 — Broker-Owned Delivery Position

- **Date:** 2026-07-08
- **Status:** Design (awaiting approval — not yet sent to `/consult-architect`)
- **Author:** Ben Lamm + Claude
- **Supersedes:** the "recommended fix shape (not yet designed in detail)" section of `.planning/phases/999.11-broker-owned-delivery-position/SPEC.md`
- **Co-designs:** `.planning/ROADMAP.md` Phase 999.2 (concurrent-consumer inbox semantics), folded into this doc rather than planned separately (999.2's stub had zero real content — see Corrections below)
- **Acceptance test (already RED on HEAD, uncommitted):** `crates/famp/tests/inbox_unread_matches_delivered.rs`

## TL;DR

Every mailbox today exposes **two** in-memory delivery cursors (`ClientState.await_offsets`, `ClientState.inbox_offsets` in `crates/famp-bus/src/broker/state.rs`) plus one bounded on-disk metadata bug (`.{name}.cursor`, advanced only by `register`/`join`/CLI `inbox ack`). A single `u64` per `(owner, mailbox)` cannot represent "fully consumed up to X under filter A" and "only consumed up to Y<X under filter B" when the two filters' matches interleave on disk. That's the accepted residual left behind by 999.1 (`2253dcc`): a client that only ever polls `await --task A` can have A's envelope permanently stuck behind an earlier, un-drained, unmatched task-B envelope.

This doc replaces both in-memory maps with **one `ConsumerPosition` per `(owner, mailbox, consumer_key)`** — a monotonic `scanned_offset` plus a bounded hole-set of byte ranges scanned-but-not-yet-delivered to that consumer. It **fixes** (not just diagnoses) the 999.1 residual, eliminates the filter-lattice overlap hazard (`AwaitFilter::Any ⊇ Task(uuid)`), and resolves 999.2's concurrent-consumer race in the same model via an opt-in `consumer_key`.

## Corrections inherited from prior review (do not relitigate)

Per `REFACTORING-REVIEW-2026-07-08.md` §0 (adversarially reviewed, corrections withdrawn a prior Critical finding): this is **not** a retention/compaction problem. Measured 2026-07-08: largest mailbox 138 KB, ~2.8 MB total across ~180 mailboxes — no retention pressure exists. Retention is explicitly out of scope; `min(all consumer scanned_offsets)` falls out of this design as a byproduct that would *later* enable a retention decision, and that is the only relationship between the two.

999.2's ROADMAP stub (filed 2026-04-24, never planned) asked "what happens when two processes both call `famp await` against the same `FAMP_HOME`?" and expected "serialize cleanly... exactly one consumer gets each entry." That is competing-consumer (exactly-once) framing. **This design deliberately does not build that as the only mode** — see Decision 2 below.

## Current state (for reference)

- `crates/famp-bus/src/broker/state.rs:51,63` — `await_offsets` / `inbox_offsets`, both `BTreeMap<MailboxName, u64>`, living on the **canonical holder's** `ClientState` (not the connection's), because one-shot CLI/MCP `Await`/`Inbox` calls reconnect with a fresh `ClientId` every invocation (`crates/famp-bus/src/proto.rs:28`) — position has to live on the registered *name* or repeated calls would replay from scratch.
- `crates/famp-bus/src/broker/identity.rs::canonical_holder_id` collapses every proxy connection (`bind_as: Some(name)`) onto that same canonical `ClientState`. This is the literal mechanism of 999.2's race: two different proxy connections (or canonical + proxy) share one offset per mailbox today, full stop.
- `crates/famp-bus/src/broker/awaiting.rs::drain_await_batch` (999.1-fixed): stops advancing the instant it hits a real, filter-mismatched envelope; does not lose data, but also does not resolve it — the mismatch just sits there forever if the consumer never asks with a broader filter.
- `crates/famp-bus/src/broker/drain_walk.rs::walk` — the one drain mechanism (`WalkOutcome { delivered, next_offset, fully_drained }`), shipped PR #8. Its current invariant: `next_offset` never advances past a record that was neither delivered nor proven unmatchable. This design generalizes that invariant (see Decision 4).

## Decision 1 — unify the cursor shape

Replace `await_offsets` and `inbox_offsets` with one structure per `(owner, mailbox, consumer_key)`:

```rust
struct ConsumerPosition {
    scanned_offset: u64,
    /// Byte ranges [start, end) scanned but not yet delivered to THIS
    /// consumer_key. Re-read from the mailbox file on match — never a
    /// copy of envelope content, so the file stays the single source
    /// of truth (per SPEC.md's constraint).
    holes: BTreeMap<u64, u64>,
}
```

Every poll (`Await` or `Inbox`) drains the mailbox to current EOF *for this consumer*. `scanned_offset` always advances fully now — safe, because a real envelope that doesn't match this call's filter is recorded as a hole instead of silently discarded. The call's filter is applied against `holes ∪ newly-scanned`; matches are delivered **and removed** from `holes`; non-matches remain as holes for a future call (any filter, this same `consumer_key`) to potentially match.

Self-authored posts and undecodable lines never enter `holes` (unchanged from the 999.1 resolution) — they are permanently unmatchable under any filter, so `scanned_offset` passes over them with no trace, exactly as today.

**Why this fixes the 999.1 residual, not just diagnoses it:** a client polling only `--task A` still fully drains every call, but the unmatched `B` in between survives as a hole rather than sitting past an unreachable single cursor. It is delivered the moment any future call from this `consumer_key` matches it (a broader `Any` poll, or `--task B`) — genuinely reachable, not "diagnosable-but-stuck."

**Why this kills the filter-lattice overlap hazard:** `AwaitFilter::Any` matches (and structurally is a superset of) every `Task(uuid)`. Under a naive per-filter cursor map this creates duplicate-delivery risk (an `Any` call and a later `Task(A)` call both claiming to own progress over the same bytes). Under the hole-set model there is exactly one `holes` map per `consumer_key` — delivery removes the entry, so nothing is left to double-deliver against.

**Truncation clamp**: `holes` inherits the same shrink-clamp obligation as `scanned_offset` (issue #11/#16) — any hole `[start, end)` with `end` beyond a shrunk mailbox's new length is dropped (with the existing `tracing::warn`), per-hole, not just on the single scalar cursor.

## Decision 2 — `consumer_key` resolves 999.2 in the same model

**Default (zero wire/config change):** each identity gets two implicit keys, `<name>/await` and `<name>/inbox`. This exactly mirrors today's shared-position behavior — which is *also* what ROADMAP 999.2's original stub asked for as its "expected" behavior ("exactly one consumer gets each entry"). Two anonymous processes racing `famp await --as alice` with no override still safely share one `ConsumerPosition`; `famp-bus` is a single-threaded pure actor (no tokio, one message at a time — BUS-01), so this requires no lock, only the existing serialized dispatch.

**Broadcast is opt-in:** a second concurrent reader of the same identity supplies an explicit `consumer_key` (new optional field on `BusMessage::Register`/Hello, e.g. `--consumer proxy-2` CLI flag / `consumer_key` MCP register arg) to get its own independent `ConsumerPosition` — it now sees every envelope independently rather than competing for entries with the default reader.

**New consumer_key bootstrap:** `scanned_offset` initializes to **EOF-at-first-poll**, not 0 — mirroring today's register-drains-to-EOF behavior, so a newly-declared reader doesn't replay the whole mailbox on first contact.

**GC:** the default (`<name>/await`, `<name>/inbox`) keys live exactly as long as the canonical registration does (unchanged from today — no new lifecycle). Explicit non-default keys have no natural owning connection, so they need an idle-eviction TTL (placeholder proposal: mirror the existing await-deadline config shape, `tracing::warn` on evict). This is deliberately a simple placeholder, not a retention system.

Why this is deferred to opt-in rather than the only mode: it is fully backward compatible (zero behavior change for every existing single-reader caller — the overwhelming common case today), and it doesn't force new session-identity plumbing onto callers who don't need concurrent independent visibility. Nothing about this default blocks turning it on broadly later if a real multi-reader use case appears (e.g. the v1.0 federation gateway).

## Decision 3 — Await and Inbox stay behaviorally independent

`<name>/await` and `<name>/inbox` are **separate** `consumer_key`s, not one shared position. An envelope delivered via `Await` is *not* thereby consumed for a later `Inbox` call, and vice versa. This preserves the exact behavior Scope-B (2026-06-19, commit `ad77c56`) deliberately introduced — splitting the cursors specifically because a task-filtered `Await` was silently robbing `Inbox` of channel posts — and the listen-mode wake→`famp_inbox` flow that depends on `Inbox` still being able to show what `Await` already surfaced. What *does* unify across them: one `ConsumerPosition` struct, one drain/hole-set mechanism, one set of invariants — replacing two divergent ad-hoc maps with one consistent one, which is the literal "subsumes both `await_offsets` and `inbox_offsets`" requirement, just not via a single shared position.

## Decision 4 — GC, ack, and the `since` field

- **Real `ack`:** HANDOFF §4 flagged the MCP `famp_inbox` schema's dead `ack`/`offset` fields (deleted rather than wired, specifically to avoid writing to the doomed old cursor). Against the unified model, `ack(consumer_key, offset)` is simply: remove the hole at `offset` (or all holes `< offset` under an explicit "I've handled everything up to here" semantics — TBD at implementation time, not a design-blocking choice).
- **`min(all consumer scanned_offsets)` per mailbox** falls out for free as the byproduct SPEC.md says should unlock (never implement) a later retention decision.
- **`since` (client-supplied)** stays an explicit escape hatch only (`since: 0` full-replay, already true for MCP per issue #13's fix) — never promoted to authoritative. The broker's per-consumer position is the only source of truth for "what has this consumer seen."
- **`include_terminal`/`TaskStateRead`:** explicitly OUT of 999.11's build scope (per START-PROMPT). Noting only, for a future item: once a `TaskStateRead` supertrait exists, holes belonging to envelopes for tasks in an absorbing terminal state become a natural (not urgent) eviction trigger.

## Testing

- **Acceptance test (already written, RED on HEAD, intentionally left uncommitted per this session's scope):** `crates/famp/tests/inbox_unread_matches_delivered.rs` — pins HANDOFF §6's exact criterion (`inspect identities`'s `unread` must equal what a subsequent `famp_inbox` call actually returns). Confirms the on-disk `.cursor`-vs-MCP-session-cursor divergence live: `unread=2`, `delivered=1`.
- **`prop04_drain_completeness.rs` / `tdd02_drain_cursor_order.rs` are NOT edited.** Their current invariant ("delivered ∪ unmatchable = scanned") is generalized, not violated, by this design: `delivered ∪ hole ∪ unmatchable = scanned`. Per the START-PROMPT's explicit instruction, this generalization is *surfaced here* rather than silently implemented — implementation should add a **new** property test asserting the generalized invariant, extending coverage without touching the pinned files.
- Extend to interleave `Inbox` and `Await` on a shared channel in the new property test, per the 2026-06-19 lesson (per-handler tests didn't catch the Scope-B cursor-share bug; only cross-handler interleaving did).
- New test needed at implementation time: two `consumer_key`s on one identity, broadcast-verified (both see the same envelope independently) — the direct regression test for 999.2's original filed scenario.

## Non-goals (explicitly out of scope for 999.11)

- Mailbox retention, rotation, or compaction (separate, later decision — SPEC.md is explicit and this doc does not relitigate it).
- Crash-safety / fsync ordering.
- `include_terminal` broker-side filtering (tracked separately; noted as future GC synergy only).
- A general-purpose competing-consumer (exactly-once, claim/lock) primitive — Decision 2 gives broadcast, opt-in; a real work-queue semantic is not requested anywhere in the project's history and is not built here.

## Open items before implementation

1. `ack`'s exact semantics (single-offset vs range) — small, non-blocking, decide during planning.
2. The idle-eviction TTL default for non-default `consumer_key`s — needs a number; propose during planning, not a design blocker.
3. Wire/schema changes: new optional `consumer_key` field on Register/Hello and the MCP register tool; new `famp_inbox`/`famp_await` `ack` arguments. Standard additive, backward-compatible change per this repo's established pattern (mirrors `include_terminal`'s wire-accepted-but-optional precedent).

## Next step

Per the START-PROMPT: send this design to `/consult-architect` before any implementation planning. This decision constrains the v1.0 federation cursor model (SPEC's forcing function) and touches the `famp-bus` no-tokio pure-actor boundary (BUS-01, CI-gated) — judgment weight warrants outside review before commitment.

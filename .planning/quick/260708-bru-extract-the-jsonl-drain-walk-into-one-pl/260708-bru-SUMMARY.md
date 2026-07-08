---
phase: quick-260708-bru
plan: 01
subsystem: famp-bus / broker
status: complete
tags: [refactor, drain-walk, cursor, jsonl, BUS-01, 999.11]
requires:
  - famp-bus::MailboxRead
  - famp-bus::broker::handle::decode_line
provides:
  - famp-bus::DrainedRecord
  - famp-bus::JSONL_RECORD_TERMINATOR_LEN (pub)
  - famp-bus::broker::drain_walk::{walk, DrainPolicy, DrainCap, WalkOutcome}
affects:
  - crates/famp-bus/src/mailbox.rs
  - crates/famp-bus/src/lib.rs
  - crates/famp-bus/src/broker/mod.rs
  - crates/famp-bus/src/broker/awaiting.rs
  - crates/famp-bus/src/broker/handle.rs
  - crates/famp-bus/src/broker/drain_walk.rs
  - crates/famp-bus/tests/prop01_dm_fanin_order.rs
  - crates/famp-bus/tests/prop02_channel_fanout.rs
  - crates/famp/src/cli/broker/mailbox_env.rs
tech-stack:
  added: []
  patterns:
    - "Producer-side framing: the MailboxRead impl (the only code that knows the on-disk layout) emits per-record byte ranges; consumers never re-derive them."
    - "Policy literals over policy flags: divergent behaviors survive as explicit struct fields at each call site rather than being normalized away."
key-files:
  created:
    - crates/famp-bus/src/broker/drain_walk.rs
  modified:
    - crates/famp-bus/src/mailbox.rs
    - crates/famp-bus/src/lib.rs
    - crates/famp-bus/src/broker/mod.rs
    - crates/famp-bus/src/broker/awaiting.rs
    - crates/famp-bus/src/broker/handle.rs
    - crates/famp/src/cli/broker/mailbox_env.rs
    - crates/famp-bus/tests/prop01_dm_fanin_order.rs
    - crates/famp-bus/tests/prop02_channel_fanout.rs
decisions:
  - "JSONL_RECORD_TERMINATOR_LEN stays in famp-bus (promoted pub(crate) -> pub); it cannot move to famp-inbox without violating BUS-01."
  - "DrainCap is an enum with two semantics (Delivered / Scanned), not a single usize."
  - "decode_lines keeps cap: None and skip_self_authored: None; both are behavior-preserving and now documented as intentional."
metrics:
  duration: ~75 min
  completed: 2026-07-08
  commits: 3
  tasks: 3
---

# Quick Task 260708-bru: Extract the JSONL Drain Walk Summary

Collapsed the four-times-duplicated JSONL drain walk (decode → decide → advance)
into one `drain_walk::walk(records, policy) -> WalkOutcome`, after widening
`DrainResult` so every drained record carries its own absolute `start`/`end`
byte range and no consumer re-derives framing math.

## Commits

| # | Hash | Task |
|---|------|------|
| 1 | `c1cb418` | `refactor(260708-bru): widen DrainResult with per-record byte offsets` |
| 2 | `046308e` | `refactor(260708-bru): consume carried offsets, promote terminator const to pub` |
| 3 | `949d5a4` | `refactor(260708-bru): extract drain_walk::walk; collapse three drain loops` |

Each commit is independently green: `cargo build --workspace --all-targets`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p famp-bus`,
`cargo test -p famp --lib`, `just check-no-tokio-in-bus`, `cargo fmt --all -- --check`.
Commit 3 additionally passed `cargo test --workspace --doc`.

`prop04_drain_completeness.rs` and `tdd02_drain_cursor_order.rs` stayed green with
a **zero-byte diff** after each commit (the `git diff --name-only` emptiness gate
was run before every commit and returned empty every time).

## What Shipped

**`DrainedRecord { bytes, start, end }`** (`mailbox.rs`, re-exported from
`famp_bus`). `DrainResult.lines: Vec<Vec<u8>>` became
`DrainResult.records: Vec<DrainedRecord>`. Both `MailboxRead` producers
(`InMemoryMailbox::drain_from`, `read_raw_from`) populate all three fields.
`record.end` is the single producer of the next cursor value.

**`famp_bus::JSONL_RECORD_TERMINATOR_LEN`** promoted `pub(crate)` → `pub` and
re-exported, with a doc comment naming the `famp-inbox/src/read.rs:175` mirror it
is coupled to.

**`crates/famp-bus/src/broker/drain_walk.rs`** (new) exporting
`walk`, `DrainPolicy`, `DrainCap`, `WalkOutcome`. `is_self_authored` and
`filter_matches` moved here from `awaiting.rs`. Three call sites, three policy
literals, zero re-derived loops:

| Call site | `filter` | `skip_self_authored` | `cap` |
|---|---|---|---|
| `awaiting::drain_await_batch` | task filter | awaiter identity (channels only) | `Delivered(50)` |
| `handle::inbox` channel loop | `Any` | `Some(name)` | `Scanned(256)` |
| `handle::decode_lines` | `Any` | `None` | `None` |

`famp-bus` gained zero dependencies and remains tokio-free (BUS-01 gate run
before every commit).

## Planner Findings, Confirmed

### F-1 — Terminator constant stays in `famp-bus` (do not re-attempt the move)

The original review suggested moving `JSONL_RECORD_TERMINATOR_LEN` to
`famp-inbox`. **This is impossible.** `famp-inbox` depends on `tokio`
(`fs`, `sync`, `io-util`, `rt`); `famp-bus` must stay tokio-free (BUS-01,
enforced by `just check-no-tokio-in-bus`). Adding `famp-inbox` to `famp-bus`
fails the CI gate; importing `famp-bus` from `famp-inbox` inverts the layering
(durable storage sits *below* the pure actor).

Resolution: the constant stays in `famp-bus/src/mailbox.rs`, promoted to `pub`
and re-exported. The `famp` crate already depends on `famp-bus`, so
`read_raw_from` imports it. `famp-inbox`'s own `+ 1` at `read.rs:175` is
untouched and cross-referenced from the constant's doc comment. That file has
a zero-byte diff.

### F-2 — `read_raw_from` must stay hand-rolled

Confirmed by inspection: `famp_inbox::read::read_from` returns
`Vec<(serde_json::Value, u64)>` — parsed JSON. The broker re-decodes each line
via `AnyBusEnvelope::decode` and requires the on-disk bytes verbatim
(canonical-JSON byte-exactness). `read_raw_from` keeps returning raw bytes and
keeps its snap-forward.

### F-3 — `DrainCap` has two semantics, not one

Collapsing the two caps into a single `cap: Option<usize>` would have silently
changed behavior on both paths:

- **`DrainCap::Delivered(n)`** — stops after the n-th *delivered* envelope.
  Skipped records (self-authored, undecodable) consume no budget. This is
  `Await`'s `AWAIT_BATCH_CAP = 50`: it bounds the *size of the reply*.
- **`DrainCap::Scanned(n)`** — walks at most the first n *records* regardless of
  how many were delivered. Skipped records DO consume budget. This is `Inbox`'s
  `CHANNEL_DRAIN_CAP = 256`: it bounds the *work done per poll* on a hot channel.

Both semantics are documented on the enum, which is the single easiest thing to
get wrong in this module.

### F-4 — Mid-line cursor: a correctness improvement, not a cosmetic no-op

Carried offsets change the emitted `next_offset` **only when the incoming cursor
lands mid-line**. For every line-aligned cursor the emitted offsets are
byte-identical to the old accumulator arithmetic.

Previously, `read_raw_from` snapped a mid-line `since_bytes` forward to the next
record boundary, but the consumers' accumulators seeded from the *unsnapped*
`since` and added `len + 1` per line — so the emitted `next_offset` started
*behind* the first real record and stayed behind by the snap distance.

Where this matters: `decode_lines`' `start_offset` traces back to
`BusMessage::Inbox { since }` (`proto.rs:153-158` → `handle.rs:542`,
`since.unwrap_or(0)`) — a **wire-deserialized, client-supplied** field, not a
server-derived cursor. A client may pass a mid-line `since`. For the
agent-mailbox `Inbox` path this is therefore a **correctness improvement on
untrusted input**: the client now receives a snapped-forward, self-consistent
`next_offset` instead of one that lags into the middle of a record. The old
wrong value was not preserved.

(The `debug_assert_eq!(next_offset, drained.next_offset)` at `awaiting.rs`
fires only inside `drain_await_batch`'s `fully_drained` branch and says nothing
about `decode_lines`. It was **not** used as justification here.)

## Hardcoded `+ 1` Sites

`grep -rn 'len() + 1' crates/famp-bus/src crates/famp/src/cli/broker` now returns
nothing.

**Eliminated (5 sites):**

| File | Old | New |
|---|---|---|
| `awaiting.rs` (main drain loop) | `next_offset + (line.len() + 1) as u64` | `record.end` |
| `awaiting.rs` (wake-trigger fold) | `next_offset + (trigger_line_len + 1) as u64` | `next_offset + trigger_line_len as u64 + JSONL_RECORD_TERMINATOR_LEN` |
| `handle.rs` (inbox channel loop) | `line_offset + line.len() as u64 + JSONL_RECORD_TERMINATOR_LEN` | `record.end` (whole loop replaced by `walk`) |
| `handle.rs::decode_lines` | `offset + (line.len() + 1) as u64` | `record.end`, then the whole body replaced by `walk` |
| `mailbox_env.rs::read_raw_from` | `running += (rel as u64) + 1;` | `running += (rel as u64) + JSONL_RECORD_TERMINATOR_LEN;` |

**Deliberately left (3 sites), with reasons:**

1. `famp-inbox/src/read.rs:175` — the `+ 1` mirror. Cannot import the constant
   (F-1). Cross-referenced from the constant's doc comment; file untouched.
2. `mailbox_env.rs::read_raw_from`, `cursor = line_end + 1;` — this `1` is a
   **byte-index step past the `\n` we just found**, not a framing width. Using
   the `u64` constant here would force a cast. Left as a literal with an
   explanatory comment.
3. `mailbox_env.rs::read_raw_from`, snap-forward `start + off + 1` — same
   reasoning: byte-index arithmetic on a `usize`, not framing width.

Also unchanged: `mailbox.rs`'s `InMemoryMailbox::drain_from` already used the
constant in both places.

## Doc Comment Correction

`handle.rs:555` previously read *"Per-channel drain is capped at
`CHANNEL_DRAIN_CAP` envelopes per poll"*. The code caps **scanned lines**, not
delivered envelopes — self-authored and undecodable records consume cap budget.
**Corrected** in commit 2 to say `CHANNEL_DRAIN_CAP` SCANNED records, with the
reason inline. This is exactly the F-3 confusion the `DrainCap` enum now makes
unrepresentable.

## Downstream Unblocked

`fully_drained` — until now a bespoke field on `AwaitBatch` computed only inside
`drain_await_batch` — is now a field of `WalkOutcome` and is therefore available
on the **Inbox path for free**. That is the signal backlog **999.11
(broker-owned cursor)** needs: the broker can now tell, on any drain path,
whether it consumed everything currently on disk or stopped early. §3.1's 16 MiB
register cliff also becomes a one-line change (`cap: None` →
`Some(DrainCap::Scanned(n))` in `decode_lines`), though that remains
deliberately out of scope here.

## Deviations from Plan

### 1. [Rule 3 – blocking] `decode_lines`' WARN `byte_offset` swap deferred from Task 2 to Task 3

**Found during:** Task 2.
**Issue:** Task 2 as written asked for `decode_lines`' WARN to source
`byte_offset = record.start` while also keeping the `offset` accumulator and the
`start_offset` parameter. With `record.start` supplying the WARN and `record.end`
supplying the advance, nothing reads `offset` — rustc's `unused_assignments`
fires, and with `-D warnings` (workspace lints) the build fails. `start_offset`
would also become an unused parameter.
**Fix:** In Task 2, `decode_lines` advances from `record.end` (killing the `+ 1`,
satisfying Task 2's grep gate) while the WARN keeps reading the `offset`
accumulator, which stays alive and seeded from `start_offset`. Task 3 then
replaces the whole body with `walk`, whose WARN uses `record.start`. **End state
is byte-identical to the plan's**; only the intermediate commit differs.
**Files:** `crates/famp-bus/src/broker/handle.rs`. **Commits:** `046308e`, `949d5a4`.

### 2. [Mechanical] Two non-pinned property tests updated

`prop01_dm_fanin_order.rs` and `prop02_channel_fanout.rs` read `drained.lines`
directly. The plan named only `tests/common/mod.rs` (correctly, as a pass-through
that needed no change) and did not mention these two. They were updated
mechanically to `drained.records` / `record.bytes` — assertions unchanged.
Neither is a pinned file. **Commit:** `c1cb418`.

### 3. [Mechanical] Two additional `decode_lines` call sites

The plan named `handle.rs:547` (the agent-mailbox Inbox path). Two more exist:
`handle.rs:317` (`register`) and `handle.rs:713` (`join`). Both updated the same
way. `decode_lines` also took `&[DrainedRecord]` rather than `Vec<DrainedRecord>`
to avoid `clippy::needless_pass_by_value` once its body became a `walk` call.

### 4. [Additive] Two new unit tests pinning `start`/`end`

Per Task 1's `<action>`, `start`/`end` are pinned by tests:
`mailbox.rs::in_memory_mailbox_records_carry_absolute_offsets` and
`mailbox_env.rs::drain_from_midline_offset_snaps_forward_with_absolute_offsets`
(the latter pins the F-4 mid-line snap-forward behavior directly).

## Threat Model Outcomes

| Threat ID | Disposition | Outcome |
|---|---|---|
| T-bru-01 (head-of-line skip DoS) | mitigate | Ported verbatim into `walk`'s `Err` arm; `next_offset = record.end`. `prop04_drain_completeness.rs` green, zero-byte diff. |
| T-bru-02 (self-filter info disclosure) | mitigate | `skip_self_authored` is an explicit per-call-site policy field: `Some(identity)` on both channel paths, `None` only on the DM/register/join path where self-delivery is correct. |
| T-bru-03 (cursor advance past undelivered record) | mitigate | `walk`'s filter-mismatch branch sets `fully_drained = false` and `break`s WITHOUT advancing `next_offset`. `tdd02_drain_cursor_order.rs` green, zero-byte diff. |
| T-bru-04 (uncapped `decode_lines`, 16 MiB register cliff) | accept | Out of scope. `cap: None` is the behavior-preserving value and is now documented as intentional, so a future reader will not "fix" it accidentally. |
| T-bru-SC (dependency installs) | mitigate | Zero new crates. `just check-no-tokio-in-bus` passed before every commit. |

## Threat Flags

None. This is a behavior-preserving extraction; no new network endpoints, auth
paths, file access patterns, or schema changes at trust boundaries were
introduced. The one behavioral change (F-4) *narrows* attack surface: a
client-supplied mid-line `since` on the `Inbox` wire path now yields a
self-consistent snapped-forward cursor instead of one lagging into a record.

## Known Stubs

None.

## Self-Check: PASSED

- `crates/famp-bus/src/broker/drain_walk.rs` — FOUND
- Commit `c1cb418` — FOUND
- Commit `046308e` — FOUND
- Commit `949d5a4` — FOUND
- `git diff --name-only HEAD -- prop04_drain_completeness.rs tdd02_drain_cursor_order.rs crates/famp-inbox/src/read.rs` — EMPTY
- `grep -rn 'allow(' crates/famp-bus/src/broker/drain_walk.rs` — EMPTY (no lint suppression added)
- `grep -rn 'len() + 1' crates/famp-bus/src crates/famp/src/cli/broker` — EMPTY
- `just check-no-tokio-in-bus` — PASSED

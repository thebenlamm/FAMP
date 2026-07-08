---
phase: quick-260708-bru
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/famp-bus/src/mailbox.rs
  - crates/famp-bus/src/lib.rs
  - crates/famp-bus/src/broker/awaiting.rs
  - crates/famp-bus/src/broker/handle.rs
  - crates/famp-bus/src/broker/drain_walk.rs
  - crates/famp-bus/src/broker/mod.rs
  - crates/famp/src/cli/broker/mailbox_env.rs
autonomous: true
requirements:
  - REFACTOR-3.2
must_haves:
  truths:
    - "`DrainResult` carries a per-record byte range for every drained JSONL line; no consumer re-derives framing math."
    - "Exactly one function decides deliver-vs-skip-vs-stop and advances the cursor over drained records."
    - "The three divergent policies (task filter, self-authored skip, cap kind) survive as distinct policy literals at their call sites."
    - "`decode_lines` still has no self-authored skip and no cap; its WARN log still reports an absolute `byte_offset`."
    - "`read_raw_from` still returns on-disk bytes verbatim and still snaps forward past a mid-line `since_bytes`."
    - "`prop04_drain_completeness.rs` and `tdd02_drain_cursor_order.rs` pass with a zero-byte diff."
    - "`famp-bus` still has no tokio in its dependency tree (BUS-01)."
  artifacts:
    - crates/famp-bus/src/broker/drain_walk.rs
    - crates/famp-bus/src/mailbox.rs
  key_links:
    - "`DrainedRecord.end` is the ONLY producer of the next cursor value; `MailboxRead` impls (InMemoryMailbox, DiskMailboxEnv) are the only producers of `DrainedRecord`."
    - "`famp_bus::JSONL_RECORD_TERMINATOR_LEN` is the single terminator-width constant, consumed by both `famp-bus` and the `famp` crate's `read_raw_from`."
    - "`drain_walk::walk` is called by `drain_await_batch`, `inbox`'s channel loop, and `decode_lines` — three policy literals, one loop."
---

<objective>
Extract the four-times-duplicated JSONL drain-walk (decode → decide → advance)
into one `walk(records, policy) -> WalkOutcome`, by first making `DrainResult`
carry per-record byte offsets so no consumer re-derives framing math.

Purpose: This loop is the most correctness-critical code in FAMP. 999.1 and
Scope B were each fixes applied to exactly one copy. A new consumer physically
cannot forget to advance past a skipped line once there is one walk. This is
also the structural prerequisite for backlog 999.11 (broker-owned cursor) and
for §3.1's 16 MiB register cliff — `fully_drained`, today a bespoke field on
`AwaitBatch`, becomes available on the Inbox path for free.

Output: `famp-bus/src/broker/drain_walk.rs` (new), a widened `DrainResult`,
a `pub` terminator constant, and three call sites collapsed to policy literals.
</objective>

<execution_context>
@/Users/benlamm/Workspace/FAMP/.claude/gsd-core/workflows/execute-plan.md
@/Users/benlamm/Workspace/FAMP/.claude/gsd-core/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@CLAUDE.md
@REFACTORING-REVIEW-2026-07-08.md

@crates/famp-bus/src/mailbox.rs
@crates/famp-bus/src/broker/awaiting.rs
@crates/famp-bus/src/broker/handle.rs
@crates/famp/src/cli/broker/mailbox_env.rs
@crates/famp-inbox/src/read.rs
</context>

<planner_findings>

Read this before Task 1. Three of the task spec's suggestions are wrong against
the actual code. Verified, not assumed.

**F-1 — The terminator constant CANNOT move to `famp-inbox`.**
`crates/famp-inbox/Cargo.toml` depends on `tokio` (`fs`, `sync`, `io-util`,
`rt`). `famp-bus` does not depend on `famp-inbox` today, and `just
check-no-tokio-in-bus` is a CI gate that greps `cargo tree -p famp-bus` for
tokio. Adding `famp-inbox` to `famp-bus` would fail that gate and violate the
hard constraint "famp-bus must gain no new dependencies and stay tokio-free
(BUS-01)."

Resolution, which unblocks the same structural problem the review named: keep
the constant in `famp-bus/src/mailbox.rs`, change `pub(crate)` → `pub`, and
re-export it from `famp_bus`'s root. The `famp` crate already depends on
`famp-bus` (`crates/famp/Cargo.toml:64`), so `read_raw_from` can import it. That
is precisely the blocker the prior review's quick-win #5 hit. `famp-inbox`'s own
`+1` at `read.rs:175` stays untouched (surgical changes only; it is not one of
the four sites, and importing `famp-bus` from `famp-inbox` would invert the
layering).

**F-2 — `MailboxRead` impls CANNOT source offsets from `famp_inbox::read::read_from`.**
`read_from` returns `Vec<(serde_json::Value, u64)>` — parsed JSON. The broker
re-decodes each line via `AnyBusEnvelope::decode` and requires the on-disk bytes
verbatim (canonical-JSON byte-exactness). `read_raw_from` must stay hand-rolled.
The task spec asked us to "check whether"; the answer is no.

**F-3 — The `cap` axis has TWO semantics, not one.**
- `drain_await_batch` caps on **delivered envelopes** (`envelopes.len() == AWAIT_BATCH_CAP` after a push).
- `inbox`'s channel loop caps on **scanned records** (`.take(CHANNEL_DRAIN_CAP)` over `drained.lines`).

Collapsing these into one `cap: Option<usize>` silently changes behavior on both
paths. `DrainCap` must be an enum: `Delivered(usize)` | `Scanned(usize)`.

**F-4 — Known, intentional divergence introduced by carried offsets (mid-line cursor only).**
Today `drain_await_batch` and `decode_lines` seed their accumulator with
`since` / `start_offset` and add `len + 1` per line. `read_raw_from` snaps a
mid-line `since_bytes` forward to `snapped`, so with a mid-line cursor the
accumulator starts *behind* the first real record. Carried offsets make the
mid-line case correct instead of wrong. For every line-aligned cursor the
emitted offsets are byte-identical. Do not "preserve" the old wrong value.

**Correction (plan-checker, 2026-07-08).** The original wording cited
`debug_assert_eq!(next_offset, drained.next_offset)` at `awaiting.rs:309` as
proof that "the codebase already assumes `since == snapped`." That assert fires
only inside `drain_await_batch`'s `fully_drained` branch. It says nothing about
`decode_lines`, whose `start_offset` traces to `BusMessage::Inbox { since }`
(`proto.rs:153-158` → `handle.rs:542`, `since.unwrap_or(0)`) — a
**wire-deserialized, client-supplied** field, not a server-derived cursor. A
client may pass a mid-line `since`. So for the agent-mailbox `Inbox` path this
is not a no-op: carried offsets are a **correctness improvement on untrusted
input** (the client gets a snapped-forward, self-consistent `next_offset`
instead of one that lags into the middle of a record). Still safe, still the
right move — but state it this way in the SUMMARY, and do not repeat the
overstated `debug_assert` justification.

</planner_findings>

<tasks>

<task type="auto">
  <name>Task 1: Widen DrainResult to carry per-record byte offsets</name>
  <files>crates/famp-bus/src/mailbox.rs, crates/famp-bus/src/lib.rs, crates/famp-bus/src/broker/awaiting.rs, crates/famp-bus/src/broker/handle.rs, crates/famp/src/cli/broker/mailbox_env.rs</files>
  <action>
Introduce `DrainedRecord` in `crates/famp-bus/src/mailbox.rs`:

`pub struct DrainedRecord` with three public fields: `bytes: Vec<u8>` (the line
WITHOUT its trailing newline, exactly as `lines` held it today), `start: u64`
(absolute byte offset of the record's first byte), `end: u64` (absolute byte
offset one past the record's terminating `\n`; i.e. the cursor value a consumer
advances to after consuming exactly this record). Derive `Debug, Clone,
PartialEq, Eq`.

Change `DrainResult.lines: Vec<Vec<u8>>` to `records: Vec<DrainedRecord>`. Keep
`next_offset: u64` exactly as-is. Re-export `DrainedRecord` alongside
`DrainResult` from `crates/famp-bus/src/lib.rs:62`.

Update the two `MailboxRead` producers to emit `start`/`end`:
- `InMemoryMailbox::drain_from` (mailbox.rs:163) — its existing `cursor` walk already
  computes both bounds; push `DrainedRecord { bytes: line, start: cursor, end: next }`
  when `cursor >= since_bytes`. Keep the `CursorOutOfRange` check and the
  `next_offset = cursor` (total) semantics unchanged.
- `read_raw_from` (mailbox_env.rs:163) — inside the `while cursor < total` loop,
  `start = cursor as u64`, `end = running + rel + 1` (equivalently: the value
  `running` takes after the existing `running += (rel as u64) + 1`). Leave the
  hand-rolled `+ 1`s in place for this task; Task 2 replaces them. Preserve the
  snap-forward, the tail-tolerance `break`, and the three early-return
  `Vec::new()` arms verbatim — only the element type changes.

`crates/famp-bus/tests/common/mod.rs` is a pass-through delegate and needs no
change; do not touch it.

Update the THREE consumers mechanically so they still compute their own offsets
exactly as today — this task changes zero policy:
- `awaiting.rs:234` — `for line in drained.lines` becomes `for record in drained.records`,
  then `let line = &record.bytes;`. Leave `let line_next_offset = next_offset + (line.len() + 1) as u64;` alone.
- `handle.rs:600,604,610,624` — `drained.lines` becomes `drained.records`;
  `.is_empty()` / `.len()` / `.iter().take(..)` all still apply. Inside the loop,
  bind `let line = &record.bytes;`. Leave the `JSONL_RECORD_TERMINATOR_LEN` math alone.
- `handle.rs:975` `decode_lines` — change the parameter to `records: Vec<DrainedRecord>`,
  bind `let line = &record.bytes;`. Leave the `(line.len() + 1)` accumulator alone
  (it feeds the WARN `byte_offset`; it is NOT dead).
- `handle.rs:547` — the `decode_lines(&agent_mailbox, agent_since, agent_drained.lines)`
  call site passes `agent_drained.records`.

Also update the two unit tests in `mailbox.rs` (`in_memory_mailbox_accounts_for_newline_offsets`)
and `mailbox_env.rs` (`append_then_drain_round_trips`, `drain_from_nonzero_offset_skips_consumed_lines`,
`drain_from_missing_file_returns_empty`) to assert against `records` — assert
`bytes`, and add `start`/`end` assertions so the new field is pinned by a test.

Do NOT touch `crates/famp-bus/tests/prop04_drain_completeness.rs` or
`crates/famp-bus/tests/tdd02_drain_cursor_order.rs`. If either needs editing,
this task is wrong — stop and report.

Watch `clippy::pedantic`: `DrainedRecord` needs `#[must_use]` on any constructor
you add (prefer struct-literal construction, no constructor). `unwrap_used` /
`expect_used` are `deny`.

Then: `cargo fmt --all`, and commit as
`refactor(260708-bru): widen DrainResult with per-record byte offsets`.
  </action>
  <verify>
    <automated>cargo build --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p famp-bus && cargo test -p famp --lib && just check-no-tokio-in-bus && cargo fmt --all -- --check && test -z "$(git diff --name-only HEAD -- crates/famp-bus/tests/prop04_drain_completeness.rs crates/famp-bus/tests/tdd02_drain_cursor_order.rs)"</automated>
  </verify>
  <done>`DrainResult.records: Vec<DrainedRecord>` exists with `bytes`/`start`/`end`; both `MailboxRead` impls populate all three; all three consumers still compute their own offsets with unchanged math; the two pinned property tests pass with a zero-byte diff; workspace builds clippy-clean; `famp-bus` still tokio-free.</done>
</task>

<task type="auto">
  <name>Task 2: Switch consumers to carried offsets; promote JSONL_RECORD_TERMINATOR_LEN to pub</name>
  <files>crates/famp-bus/src/mailbox.rs, crates/famp-bus/src/lib.rs, crates/famp-bus/src/broker/awaiting.rs, crates/famp-bus/src/broker/handle.rs, crates/famp/src/cli/broker/mailbox_env.rs</files>
  <action>
Per finding F-1: do NOT move the constant to `famp-inbox` (tokio; BUS-01;
`just check-no-tokio-in-bus`). Instead, in `crates/famp-bus/src/mailbox.rs:25`
change `pub(crate) const JSONL_RECORD_TERMINATOR_LEN: u64 = 1;` to `pub const`,
re-export it from `crates/famp-bus/src/lib.rs:62`, and update its doc comment to
say it is the workspace-wide JSONL framing width — noting that `famp-inbox`
cannot import it (layering: `famp-inbox` is tokio-backed durable storage below
the pure-actor bus) and that `famp_inbox::read::read_from`'s `+ 1` at
`read.rs:175` is the mirror it is coupled to. Do not edit `famp-inbox`.

Delete each hardcoded terminator arithmetic, one consumer at a time, sourcing
the value from `DrainedRecord` where a record exists:

1. `awaiting.rs:235` — replace `let line_next_offset = next_offset + (line.len() + 1) as u64;`
   with `let line_next_offset = record.end;`. Every existing `next_offset = line_next_offset`
   assignment stays where it is. Keep `debug_assert_eq!(next_offset, drained.next_offset)`
   at line 309.

2. `awaiting.rs:311` (the trigger fold) — the wake-trigger envelope is NOT a
   drained record, so there is no carried offset for it. Replace
   `next_offset + (trigger_line_len + 1) as u64` with
   `next_offset + trigger_line_len as u64 + JSONL_RECORD_TERMINATOR_LEN`.
   Behavior is bit-identical; the magic `1` is gone.

3. `handle.rs:625` (inbox channel loop) — replace
   `line_offset + line.len() as u64 + JSONL_RECORD_TERMINATOR_LEN` with
   `record.end`, and replace the WARN's `byte_offset = line_offset` with
   `byte_offset = record.start`. `line_offset` remains as the running cursor
   fed to `effective_next_offset`.

4. `handle.rs:983` (`decode_lines`) — replace `let line_next_offset = offset + (line.len() + 1) as u64;`
   with `let line_next_offset = record.end;` and the WARN's `byte_offset = offset`
   with `byte_offset = record.start`. Keep the accumulator and the
   `start_offset` parameter: the accumulator seeds `offset` for the zero-record
   case and documents the walk; F-4 explains why the emitted value can now
   differ (strictly for the better) on a mid-line cursor.

5. `mailbox_env.rs:227` — replace `running += (rel as u64) + 1;` with
   `running += (rel as u64) + JSONL_RECORD_TERMINATOR_LEN;` (import from
   `famp_bus`). Also swap `cursor = line_end + 1;` to use the constant only if
   it typechecks cleanly as a `usize` — if it forces a cast, leave `+ 1` there;
   that `1` is a byte-index step, not framing width. `read_raw_from` keeps
   returning verbatim on-disk bytes and keeps its snap-forward.

6. `mailbox.rs:180,194` (`InMemoryMailbox`) already uses the constant. No change.

Then: `cargo fmt --all`, and commit as
`refactor(260708-bru): consume carried offsets, promote terminator const to pub`.

Traps (from `<planner_findings>` and the review): do not add a self-authored
skip to `decode_lines`; do not add a cap to `decode_lines`; do not delete
`decode_lines`' offset accumulator. Do not touch the two pinned property tests.
  </action>
  <verify>
    <automated>cargo build --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p famp-bus && cargo test -p famp --lib && just check-no-tokio-in-bus && cargo fmt --all -- --check && test -z "$(git diff --name-only HEAD -- crates/famp-bus/tests/prop04_drain_completeness.rs crates/famp-bus/tests/tdd02_drain_cursor_order.rs crates/famp-inbox/src/read.rs)"</automated>
  </verify>
  <done>`famp_bus::JSONL_RECORD_TERMINATOR_LEN` is `pub` and re-exported; zero hardcoded framing `+ 1`s remain in `awaiting.rs`, `handle.rs`, or `mailbox_env.rs` (`grep -n 'len() + 1' crates/famp-bus/src crates/famp/src/cli/broker` returns nothing); `read_raw_from` still returns raw bytes with snap-forward intact; `famp-inbox` untouched; both pinned property tests green with a zero-byte diff.</done>
</task>

<task type="auto">
  <name>Task 3: Extract drain_walk::walk and collapse the three walk sites to policy literals</name>
  <files>crates/famp-bus/src/broker/drain_walk.rs, crates/famp-bus/src/broker/mod.rs, crates/famp-bus/src/broker/awaiting.rs, crates/famp-bus/src/broker/handle.rs</files>
  <action>
Create `crates/famp-bus/src/broker/drain_walk.rs` and declare it in
`crates/famp-bus/src/broker/mod.rs`. Synchronous, allocation-light, no new
crate dependencies (BUS-01).

Types:
- `pub(super) enum DrainCap { Delivered(usize), Scanned(usize) }` — per finding
  F-3 these are NOT interchangeable. `Delivered(n)` stops after the n-th
  DELIVERED envelope (await's `AWAIT_BATCH_CAP`). `Scanned(n)` walks at most the
  first n RECORDS regardless of how many were delivered (inbox's
  `CHANNEL_DRAIN_CAP` `.take()`). Document that distinction in the enum's doc
  comment — it is the single easiest thing to get wrong here.
- `pub(super) struct DrainPolicy<'a> { filter: &'a AwaitFilter, skip_self_authored: Option<&'a str>, cap: Option<DrainCap> }`.
  `skip_self_authored: Some(identity)` is channel-only pub/sub semantics; `None`
  means deliver self-authored records (correct for DMs — see the doc comment at
  `awaiting.rs:220`).
- `pub(super) struct WalkOutcome { delivered: Vec<serde_json::Value>, next_offset: u64, fully_drained: bool }`.

`pub(super) fn walk(mailbox: &MailboxName, since: u64, records: &[DrainedRecord], policy: &DrainPolicy<'_>) -> WalkOutcome`

Exact semantics — this is a MOVE of existing code, not a redesign. Port the
`drain_await_batch` doc comments (999.1 filter-mismatch rationale, head-of-line
resilience, self-authored advance invariant) onto the corresponding branches:

- `next_offset` starts at `since`; `fully_drained` starts `true`.
- If `cap == Some(Scanned(n))`: iterate only `records.iter().take(n)`, and set
  `fully_drained = false` iff `records.len() > n`. Compute that BEFORE the loop.
- For each record: `decode_line(&record.bytes)`.
  - `Err(error)` → `tracing::warn!(mailbox = %mailbox, byte_offset = record.start, error = %error, "skipping undecodable mailbox line (head-of-line resilience)")`, then `next_offset = record.end`. (Permanently unmatchable under any filter.)
  - `Ok(value)` and `is_self_authored(&value, policy.skip_self_authored)` → `next_offset = record.end; continue;` (no deliver).
  - `Ok(value)` and `filter_matches(policy.filter, &value)` → push to `delivered`, `next_offset = record.end`. If `cap == Some(Delivered(n))` and `delivered.len() == n`, return immediately with `fully_drained: false`.
  - `Ok(value)`, filter mismatch → `fully_drained = false; break;` WITHOUT advancing `next_offset` (the 999.1 invariant: the cursor never passes an envelope this call did not hand back). With `AwaitFilter::Any` this branch is unreachable, which is why the inbox and register paths can share the walk.

Move `is_self_authored` and `filter_matches` from `awaiting.rs` into
`drain_walk.rs` and re-export / import as needed — `waiting_clients_for_name`
still calls `filter_matches`. Keep `is_self_authored`'s existing signature
(`Option<&str>` → `false` when `None`).

Collapse the three call sites:

1. `awaiting.rs::drain_await_batch` — build
   `DrainPolicy { filter, skip_self_authored: awaiter_identity.as_deref(), cap: Some(DrainCap::Delivered(AWAIT_BATCH_CAP)) }`,
   call `walk(mailbox, since, &drained.records, &policy)`, and map
   `WalkOutcome` → `AwaitBatch { mailbox: mailbox.clone(), envelopes: outcome.delivered, next_offset: outcome.next_offset, fully_drained: outcome.fully_drained }`.
   Keep the trigger-fold block (`if fully_drained { ... }`) verbatim, including
   the `debug_assert_eq!` and the `JSONL_RECORD_TERMINATOR_LEN` math from Task 2.
   Note the cap arm in the old code returned early with `fully_drained: false`;
   `DrainCap::Delivered` reproduces that, so the trigger fold is correctly
   skipped on a capped batch — same as today.

2. `handle.rs::inbox` channel loop — compute
   `let truncated = drained.records.len() > CHANNEL_DRAIN_CAP;` and keep the
   existing `tracing::debug!(channel, cap, total = drained.records.len(), "inbox_channel_drain_capped")`
   under `if truncated`. Keep the `if drained.records.is_empty() { continue; }` guard.
   Then `walk(&mailbox, cursor, &drained.records, &DrainPolicy { filter: &AwaitFilter::Any, skip_self_authored: Some(&name), cap: Some(DrainCap::Scanned(CHANNEL_DRAIN_CAP)) })`.
   `envelopes.extend(outcome.delivered)`. Keep the explicit branch:
   `let effective_next_offset = if truncated { outcome.next_offset } else { drained.next_offset };`
   and the deferred `cursor_advances` write-back loop.

3. `handle.rs::decode_lines` — becomes a thin wrapper returning
   `walk(mailbox, start_offset, &records, &DrainPolicy { filter: &AwaitFilter::Any, skip_self_authored: None, cap: None }).delivered`.
   `skip_self_authored: None` and `cap: None` are the behavior-preserving values
   and are correct, not oversights: a DM addressed to yourself must deliver, and
   adding the register-path cap is a separate change (the 16 MiB cliff, §3.1).
   State both facts in `decode_lines`' doc comment so the next reader does not
   "fix" them. Keep its existing doc comment about head-of-line resilience.
   `start_offset` stays a parameter (it is `walk`'s `since`).

`clippy::too_many_lines` has bitten this repo (see commit 53ebade). `walk` with
the ported rationale comments will be near the 100-line limit — clippy excludes
comment and blank lines from the count, but if it still fires, extract the
per-record decision into a private `fn decide(...) -> Step` rather than
suppressing the lint. Never add `#[allow]` to silence it. `unwrap_used` /
`expect_used` are `deny`.

Then: `cargo fmt --all`, and commit as
`refactor(260708-bru): extract drain_walk::walk; collapse three drain loops`.
  </action>
  <verify>
    <automated>cargo build --workspace --all-targets && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p famp-bus && cargo test -p famp --lib && cargo test --workspace --doc && just check-no-tokio-in-bus && cargo fmt --all -- --check && test -z "$(git diff --name-only HEAD -- crates/famp-bus/tests/prop04_drain_completeness.rs crates/famp-bus/tests/tdd02_drain_cursor_order.rs)"</automated>
  </verify>
  <done>`crates/famp-bus/src/broker/drain_walk.rs` exists and exports `walk`, `DrainPolicy`, `DrainCap`, `WalkOutcome`; `drain_await_batch`, `inbox`'s channel loop, and `decode_lines` each contain exactly one `walk(...)` call and no per-record decode/skip/advance loop of their own; `DrainCap::Delivered(50)` / `DrainCap::Scanned(256)` / `cap: None` are the three literals; `decode_lines` still passes `skip_self_authored: None` and `cap: None`; both pinned property tests green with a zero-byte diff; clippy pedantic clean with no new `#[allow]`.</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| on-disk mailbox JSONL → broker | Foreign/malformed envelope bytes written by any local agent (incl. non-FAMP implementations) cross into the broker's decode path. |
| drained record → delivered envelope | The walk decides what a client receives; a cursor bug is a delivery bug. |

## STRIDE Threat Register

| Threat ID | Category | Component | Severity | Disposition | Mitigation Plan |
|-----------|----------|-----------|----------|-------------|-----------------|
| T-bru-01 | Denial of Service | `drain_walk::walk` head-of-line skip | high | mitigate | Task 3 ports the 260611 head-of-line resilience verbatim: an undecodable record WARNs and advances `next_offset = record.end`. Regression-pinned by `prop04_drain_completeness.rs`. |
| T-bru-02 | Information Disclosure | `DrainPolicy.skip_self_authored` | medium | mitigate | Channel self-filter is preserved as an explicit policy field per call site; `Some(identity)` on both channel paths, `None` only on the DM/register path where self-delivery is correct. Pinned by the `done` criteria of Task 3. |
| T-bru-03 | Tampering | cursor advance past an undelivered record | high | mitigate | The 999.1 filter-mismatch branch must `break` WITHOUT advancing `next_offset`. Pinned by `tdd02_drain_cursor_order.rs`, which must stay green untouched after every task. |
| T-bru-04 | Denial of Service | `decode_lines` uncapped drain (16 MiB register cliff) | medium | accept | Explicitly out of scope for this extraction task (§3.1 / separate quick win). `cap: None` is the behavior-preserving value; adding a cap here would be a silent policy change. |
| T-bru-SC | Tampering | dependency installs | high | mitigate | No new crates are added in any task. `just check-no-tokio-in-bus` runs in every task's verify gate. |
</threat_model>

<verification>
Run after every task, before its commit (`cargo nextest` is banned here — it
hangs in the test-binary `--list` phase on this repo; plain `cargo test`):

1. `cargo build --workspace --all-targets`
2. `cargo clippy --workspace --all-targets -- -D warnings` (== `just lint`)
3. `cargo test -p famp-bus` — includes `prop04_drain_completeness` and `tdd02_drain_cursor_order`
4. `cargo test -p famp --lib` — `mailbox_env.rs` unit tests
5. `just check-no-tokio-in-bus` — BUS-01 gate
6. `cargo fmt --all && cargo fmt --all -- --check` — pre-commit hook is fmt-check only; never `--no-verify`
7. `git diff --name-only HEAD -- crates/famp-bus/tests/prop04_drain_completeness.rs crates/famp-bus/tests/tdd02_drain_cursor_order.rs` must be EMPTY. If a task required editing either file, that task's design is wrong — stop and report rather than editing the test.

Final task additionally runs `cargo test --workspace --doc` and, before wrap,
`just ci` for full CI parity.
</verification>

<success_criteria>
- Framing math (`+ 1`) exists in exactly two places in the workspace:
  `famp-bus`'s `JSONL_RECORD_TERMINATOR_LEN` definition and
  `famp-inbox/src/read.rs:175` (untouched, cross-referenced by doc comment).
- One `walk` function performs decode → decide → advance; three call sites, three
  policy literals, zero re-derived loops.
- `DrainCap::Delivered(50)`, `DrainCap::Scanned(256)`, and `cap: None` preserve
  the three divergent caps exactly. No cap added to `decode_lines`.
- No self-authored skip added to `decode_lines`; its offset accumulator survives
  and feeds the WARN `byte_offset`.
- `read_raw_from` returns on-disk bytes verbatim; snap-forward intact.
- `famp-bus` gained zero dependencies and remains tokio-free.
- `prop04_drain_completeness.rs` and `tdd02_drain_cursor_order.rs` are green with
  a zero-byte diff after each of the three commits.
- Three commits, each independently green.
</success_criteria>

<output>
Create `.planning/quick/260708-bru-extract-the-jsonl-drain-walk-into-one-pl/260708-bru-SUMMARY.md` when done.

The SUMMARY MUST record:
- The F-1 deviation (constant stayed in `famp-bus`, promoted to `pub`, because
  `famp-inbox` is tokio-backed and `famp-bus` is BUS-01 tokio-free) so the next
  reader does not re-attempt the move.
- The F-3 `DrainCap` two-semantics split.
- The F-4 mid-line-cursor divergence (now correct where it was previously wrong;
  unobservable for line-aligned cursors).
- That `fully_drained` is now available on the Inbox path, which is the signal
  backlog 999.11 (broker-owned cursor) needs.
</output>

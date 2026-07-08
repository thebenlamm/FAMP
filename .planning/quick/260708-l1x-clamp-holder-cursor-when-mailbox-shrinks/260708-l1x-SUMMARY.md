---
quick_id: 260708-l1x
description: Clamp holder cursor when the mailbox shrinks; align the MailboxRead test double (issues #11, #12)
date: 2026-07-08
status: complete
branch: fix/cursor-clamp-on-truncation
commits:
  - 659170c fix(260708-l1x): align InMemoryMailbox with production on a past-EOF cursor (#12)
  - 37d1e94 test(260708-l1x): failing tests for cursor stranded by mailbox truncation (#11)
  - 856472d fix(260708-l1x): clamp holder cursor when the mailbox shrinks beneath it (#11)
---

# 260708-l1x — Clamp holder cursor when the mailbox shrinks

Three tasks, executed in the mandated order. Strict TDD: Task 2's four tests
were committed failing (37d1e94) and go green in Task 3 (856472d), with no
`#[ignore]`, no skip, and no post-hoc edit to make them pass.

**Files changed:** `crates/famp-bus/src/mailbox.rs`,
`crates/famp-bus/src/broker/drain_walk.rs`,
`crates/famp-bus/src/broker/awaiting.rs`,
`crates/famp-bus/src/broker/handle.rs`,
`crates/famp-bus/src/broker/handle/tests.rs`.

`crates/famp-bus/tests/prop04_drain_completeness.rs` and
`tdd02_drain_cursor_order.rs` are **byte-identical** — `git diff --name-only`
against both is empty at HEAD. The clamp is the no-op the plan claimed on every
path those two exercise.

`famp-bus` gained no dependencies; `just check-no-tokio-in-bus` passes.
Full gate green at each of the three commits (`cargo fmt --all`,
`cargo build --workspace --all-targets`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test -p famp-bus`,
`cargo test -p famp --lib`, `cargo test --workspace`). No `cargo nextest`, no
`--no-verify`. The known `http_happy_path` `ReqwestFailed(TimedOut)` artifact
did not appear; `cargo test --workspace` was clean on the first run.

---

## 1. Was the `>=` vs `>` change in Task 1 a true no-op at `since == total`?

**Yes.** Evidence, from the code rather than from assumption.

The pre-change double errored on `since_bytes > total` and otherwise fell
through to the walk loop. At `since_bytes == total`:

- **Records:** the loop pushes a record only when `cursor >= since_bytes`.
  `cursor` takes the values `0, e₀, e₀+e₁, …` and its final value *after* the
  last iteration is `total`. Inside the last iteration `cursor` is still
  `total - eₙ₋₁ < total == since_bytes`, so no record is ever pushed.
  Result: `records == []`.
- **`next_offset`:** the loop returns `next_offset: cursor`, and `cursor` ends
  at exactly `total`.

So the fall-through produced `(records: [], next_offset: total)` — the identical
value the new `since_bytes >= total` short-circuit returns. The `>=` branch is a
pure short-circuit at the boundary and only changes behavior in the genuinely
past-EOF region (`> total`), which is the region production already clamped and
the double used to error on.

Pinned by two boundary tests added in the same commit, both of which would fail
if the short-circuit ever stopped being a short-circuit:
`cursor_exactly_at_eof_is_an_empty_drain_at_eof` and
`empty_mailbox_at_zero_cursor_clamps_to_zero`.

The second one covers a boundary the plan did not name: `total == 0`,
`since == 0`. Here `0 >= 0` fires where the loop used to fall through, and it
must **not** be confused with the *absent*-mailbox branch a few lines above,
which echoes `since_bytes` back instead of returning `0`. Same answer at
`since == 0`; different answers for any other cursor. The distinction is now
tested rather than incidental.

`grep -rn 'CursorOutOfRange' crates/` returns exactly one hit: a doc comment in
`mailbox.rs` that names the deleted variant historically. Zero code references.
The compiler agreed — no call site needed touching, as the plan predicted.

---

## 2. Verbatim RED output from Task 2 (pre-fix, at commit 37d1e94)

`cargo test -p famp-bus --lib`:

```
failures:

---- broker::handle::d10_tests::record_appended_before_the_healing_drain_is_skipped_but_does_not_stall stdout ----

thread 'broker::handle::d10_tests::record_appended_before_the_healing_drain_is_skipped_but_does_not_stall' panicked at crates/famp-bus/src/broker/awaiting.rs:256:9:
assertion `left == right` failed
  left: 556
 right: 278
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- broker::handle::d10_tests::truncated_agent_mailbox_heals_cursor_and_delivers_after_regrowth stdout ----

thread 'broker::handle::d10_tests::truncated_agent_mailbox_heals_cursor_and_delivers_after_regrowth' panicked at crates/famp-bus/src/broker/awaiting.rs:256:9:
assertion `left == right` failed
  left: 556
 right: 0

---- broker::handle::d10_tests::truncated_mailbox_wake_path_does_not_panic_and_delivers_the_trigger stdout ----

thread 'broker::handle::d10_tests::truncated_mailbox_wake_path_does_not_panic_and_delivers_the_trigger' panicked at crates/famp-bus/src/broker/awaiting.rs:256:9:
assertion `left == right` failed
  left: 278
 right: 0

---- broker::handle::d10_tests::truncated_channel_mailbox_heals_inbox_cursor_and_delivers_after_regrowth stdout ----

thread 'broker::handle::d10_tests::truncated_channel_mailbox_heals_inbox_cursor_and_delivers_after_regrowth' panicked at crates/famp-bus/src/broker/handle/tests.rs:1975:5:
assertion `left == right` failed: the empty-records `continue` must not skip the clamp (was 556)
  left: Some(556)
 right: Some(0)


failures:
    broker::handle::d10_tests::record_appended_before_the_healing_drain_is_skipped_but_does_not_stall
    broker::handle::d10_tests::truncated_agent_mailbox_heals_cursor_and_delivers_after_regrowth
    broker::handle::d10_tests::truncated_channel_mailbox_heals_inbox_cursor_and_delivers_after_regrowth
    broker::handle::d10_tests::truncated_mailbox_wake_path_does_not_panic_and_delivers_the_trigger

test result: FAILED. 65 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Each failure is the mechanism the issue describes, and the two distinct failure
shapes are themselves the finding:

- **Three panics at `awaiting.rs:256`** — the debug-build broker-actor panic.
  `left` is the stale cursor (`556` = two 277-byte envelopes + framing; `278` =
  one), `right` is the drain's real `next_offset` after truncation. This is #11's
  "Additional: debug-build broker panic" reproduced exactly. In a release build
  these three would instead have hung silently on the stranded cursor.
- **One assertion failure at `tests.rs:1975`** — the channel/`Inbox` path has no
  `debug_assert`, so it fails on the stranded cursor directly: `Some(556)`
  where the healed value is `Some(0)`. This is `handle.rs:601`'s
  `if drained.records.is_empty() { continue; }` skipping the `inbox_offsets`
  write-back.

All four are green at 856472d. None of them passed against pre-fix code, so none
needed rewriting.

**Test 4 is an addition to the plan's three** (deviation, disclosed):
`record_appended_before_the_healing_drain_is_skipped_but_does_not_stall` pins a
known boundary rather than fixing a bug. Rationale in §5.

---

## 3. `debug_assert_eq!` (`awaiting.rs:256`) — **replaced** with `tracing::warn!`

Replaced. Two independent reasons, the second of which I did not expect to find.

**Reason 1 (the plan's, and #11's): it is not an invariant.** A mailbox that
shrinks beneath a cursor is an expected external event — `/famp-clear` truncates
`~/.famp/mailboxes/*.jsonl` while the broker runs. Asserting on it converts a
routine operator action into a panic of the broker *actor task*, taking down
every connected client. The blast radius of the instrument exceeds the blast
radius of the condition it detects.

**Reason 2 (found while verifying the plan's no-op claim): the assert was
already reachable with no truncation involved, so the clamp alone would not
have made it sound.** `DiskMailboxEnv::read_raw_from` has three early returns
that hand back `records: []` together with `next_offset = file_len` while
`since < file_len`:

- `mailbox_env.rs:201-206` — `since` lands mid-line inside a **partial trailing
  line** (no `\n` after it), so snap-forward finds no boundary.
- `mailbox_env.rs:209-214` — snap-forward lands exactly on `bytes.len()`.

In both, `walk` correctly declines to advance past a record it never saw, so it
returns `next_offset == since`, while `drained.next_offset == file_len > since`.
`fully_drained` is `true` (zero records, `Delivered` cap never hit). Pre-fix
*and* post-fix, `debug_assert_eq!(since, file_len)` fires. The clamp cannot
rescue this: `since.min(file_len) == since` when `since < file_len`, which is
the correct value — the walk is right and the assert is wrong.

Reaching it requires a mid-line cursor, which requires a prior truncate-then-
regrow that shifted record boundaries, plus a partially-written trailing line.
Rare, but reachable, and precisely the neighbourhood this bug lives in. Had I
kept the assert on the strength of "the clamp satisfies it in the truncation
case," I would have shipped a broker that still panics on the adjacent case.

The replacement warns with `mailbox`, `walk_next_offset`, and
`drain_next_offset` rather than asserting. Note this warn is *silent on the
truncation path* — after the clamp, `next_offset == drained.next_offset == 0` —
so it fires only on the genuinely surprising disagreement, which is what makes
it worth logging.

Separately, `walk` emits its own `tracing::warn!` whenever a clamp actually
fires (`drained.next_offset < since`), naming mailbox, stale cursor and new
cursor; the channel `Inbox` loop emits the equivalent warn on its own clamp,
because it returns before reaching `walk` on an empty drain. Silent healing is
how this stayed invisible.

---

## 4. `walk` takes `&DrainResult`, not a bare `u64`

`&DrainResult`. The seed now depends on two values (`since` and the drain's end
offset) that must describe the *same* drain. A `drain_next_offset: u64`
parameter lets a call site pass a `next_offset` belonging to records it did not
also pass — which is the exact class of mistake `drain_walk` was extracted to
make impossible (its module doc: four copies of the loop, each fixed once).
Making the type carry the pairing means the compiler enforces it.

Concretely: `walk(mailbox, since, &drained, policy)` at all three call sites
(`awaiting.rs` `drain_await_batch`, `handle.rs` channel `Inbox` loop,
`handle.rs` `decode_lines`). `decode_lines` was threaded through to match even
though it discards `next_offset` — it still needs it for the seed. Its three
callers (`register`, `inbox`'s agent-mailbox read, `join`) now pass `&drained`.
Cost: one line in `handle/tests.rs`'s `oversized_drain_is_warned_but_never_truncated`,
which built a bare `Vec<DrainedRecord>` and now wraps it in a `DrainResult`.

`decode_lines`'s clamp is inert on the `register`/`join` paths (`since = 0`, and
nothing is `< 0`), and live on `inbox`'s agent-mailbox read, where `since` is
**client-supplied** and can legitimately point past EOF. That path already
returned `agent_drained.next_offset` to the client, so behavior is unchanged;
what's new is that a client handing in a past-EOF cursor now produces an
operator-visible warn instead of nothing.

---

## 5. What made me doubt the clamp — one real finding

The three non-truncation paths are no-ops exactly as the plan claims, and the
untouched `prop04`/`tdd02` plus a green `cargo test --workspace` are the
evidence. **But the plan's Task 2 test 1, written literally, cannot pass.**

The plan says: *"shrink the `InMemoryMailbox` … Append one new record. Assert a
subsequent `await_envelope` **delivers it**."* It does not, and cannot, under
`since.min(drain_next_offset)`. The arithmetic:

1. `await_offsets[Agent("alice")] = 556` (two envelopes).
2. Truncate. Append one 277-byte envelope. The mailbox now ends at `278`.
3. `drain_from(mailbox, 556)`: `556 >= 278`, so — by the past-EOF contract Task 1
   just pinned — it returns `records: []`, `next_offset: 278`. **The new record
   is not in the drain, by construction.**
4. Clamp: `556.min(278) == 278`. The cursor heals to `278` — which is *past* the
   record sitting at `[0, 278)`. It is skipped, permanently.

The clamp always lands the cursor on the mailbox's **current end**, so anything
present at clamp time is below it. The drain that *detects* truncation returns
zero records; there is nothing to deliver from it. Re-draining from the clamped
offset returns empty too (the clamped offset *is* EOF). The only way to deliver
that record is to clamp to `0` and replay whatever the file now holds — trading
this bounded loss for duplicate delivery, a semantics change (at-most-once →
at-least-once on the truncation path) that neither the plan nor #11 authorises.
I did not make that call unilaterally; see the flag below.

So I wrote test 1 in the only orderings that are actually satisfiable, and added
a fourth test to pin the gap rather than let it go unrecorded:

- `truncated_agent_mailbox_heals_cursor_and_delivers_after_regrowth` — truncate,
  heal (asserting the cursor comes down to `0`), *then* append, then deliver.
- `record_appended_before_the_healing_drain_is_skipped_but_does_not_stall` —
  truncate, append, heal. Asserts the record **is skipped** (documented loss) and
  that the holder is nonetheless **unstalled**: the next record arrives. This
  test is the honest statement of what the clamp buys.

The two channel and wake-path tests deliver cleanly, because in both the append
happens after the drain that observes the truncation.

### Flag for the issue author (not a blocker; nothing was changed on this basis)

**`min`-clamping does not fix #11's own reproduction as written.** #11's repro is:
truncate → *then* send a new message → then await. Under `min`, that message
lands below the clamped cursor and is lost; the await times out once, and only
the *next* message arrives. #11's step 5 ("Restart the broker → messages appear")
describes cursor-reset-to-0 semantics, which `min` does not provide.

`min` converts **unbounded, permanent** message loss into **bounded** loss —
exactly the records written between the truncation and the first drain that
observes it — and eliminates the debug-build broker panic. That is a large,
correct improvement and it is what the plan specifies, so it is what shipped.
Whether the residual bounded loss is acceptable, or whether truncation should
instead reset the cursor to `0` and replay (accepting duplicates under a partial
truncation), is a genuine at-most-once / at-least-once decision that belongs to
whoever owns #11. `record_appended_before_the_healing_drain_is_skipped_but_does_not_stall`
is the executable statement of the current choice; flipping the decision means
flipping that test, which is the right place for the argument to happen.

### Two smaller things worth recording

- **`handle.rs:641`'s `effective_next_offset` needed no change.** It reads
  `drained.next_offset` on the un-truncated branch and `outcome.next_offset`
  on the capped branch. Truncation only ever yields an empty drain, which
  returns at the `continue` above, so neither branch is reachable with a stale
  cursor. Confirmed by inspection rather than assumed.
- **`awaiting.rs:51`'s `if batch.next_offset != since` now fires** (`278 != 556`)
  and writes the clamped cursor. No second guard was added, per the plan.
</content>

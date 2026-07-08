# 999.11 — Handoff Brief

**Written:** 2026-07-08 · **For:** whoever picks up 999.11 in a fresh window
**Read `SPEC.md` first.** This supplements it; it does not replace it.

The SPEC was filed 2026-07-01 from inside the 999.1 debug session. Since then a
full-repo refactoring review (`REFACTORING-REVIEW-2026-07-08.md`) reached the same
problem from the opposite direction and found it to be **larger than the SPEC
scopes it**. This brief is the evidence inventory the SPEC lacks, plus one scope
conflict that must be resolved before design starts.

---

## 1. What changed since the SPEC was filed

**§3.2 shipped (PR #8, merged).** The decode-decide-advance loop over drained
mailbox records — previously written four times with divergent invariants — is now
one function: `crates/famp-bus/src/broker/drain_walk.rs::walk(records, &DrainPolicy)
-> WalkOutcome { delivered, next_offset, fully_drained }`.

Why this matters to you specifically:

- **Offsets advance in exactly one place.** The SPEC's "recommended fix shape"
  (swap one `u64` per `(owner, mailbox)` for a keyed map of cursors) was a six-site
  change when the SPEC was written. It is now a one-function change.
- **`fully_drained` is available on the Inbox path for free.** It was bespoke to
  `AwaitBatch`, invented for 999.1 to distinguish "nothing new for this filter"
  from "blocked behind an unmatched envelope." It is now a `WalkOutcome` field.
  That is precisely the signal a per-filter cursor needs.
- **`DrainCap::{Delivered(n), Scanned(n)}`.** The cap axis turned out to be two
  semantics. Don't collapse them.
- Five hardcoded `+ 1` framing sites are gone; `JSONL_RECORD_TERMINATOR_LEN` is
  `pub` in `famp-bus`. (It cannot move to `famp-inbox` — that crate depends on
  tokio and `just check-no-tokio-in-bus` is a CI gate.)

**Also shipped (quick wins):** `DRAIN_WARN_BYTES = 8 MiB` warn on the register/join
drain — an interim signal before the cliff in §3 below. The MCP `famp_inbox` schema
no longer advertises an `ack` it never implemented (see §4).

---

## 2. The SPEC undercounts the authorities. There are five, not one.

The SPEC says: *"Every mailbox exposes exactly one linear delivery position per
(owner, mailbox): `await_offsets`."* That is true **of the await path only**. As of
Scope B (2026-06-19) and earlier, the system has five independent answers to "where
has this reader got to?":

| # | Authority | Location | Advanced by | Read by |
|---|---|---|---|---|
| 1 | `ClientState.await_offsets` | `famp-bus/src/broker/state.rs:45-52` | `Await` only | `Await`, waiter view |
| 2 | `ClientState.inbox_offsets` | `state.rs:53-63` (channels only) | `Inbox` only | `Inbox` |
| 3 | `since: Option<u64>` wire param | `handle.rs` `inbox` (agent mailbox) | the **client** | `Inbox` |
| 4 | `.<name>.cursor` on disk | `cli/broker/cursor_exec.rs` | register, join, `famp inbox ack` | `inspect identities` **unread** |
| 5 | "read the whole file" | `mcp/tools/verify.rs` (`read_all`, no cursor) | n/a | `famp_verify` |

(`famp_channel_log` is a sixth surface: caller-supplied `since`, returns a
`next_offset` nobody persists.)

**Authority 2 exists because of authority 1.** Scope B's HIGH-fix added
`inbox_offsets` precisely because a task-filtered `Await` was eating channel posts
`Inbox` should have surfaced. The fix was to *add an authority*, not to give
position an owner. That is the shape of this problem: **each incident adds a
cursor.** 999.11 is the decision to stop.

**Consequence you can observe right now:** `famp inspect identities`' `unread`
column reads authority 4 — a file that **no read path advances** (only register,
join, and the CLI-only `famp inbox ack`). A listen-mode agent consuming everything
via `famp_await` shows monotonically growing unread forever. The project's own
debugging memo ("run `famp inspect identities` first; delivery usually works,
cursor-behind is the failure") is describing a structural property, not a bug.

---

## 3. Scope conflict — resolve this before designing

**SPEC "Out of scope":** *"Any change to on-disk mailbox format/rotation."*
**SPEC "Recommended fix shape":** *"the underlying mailbox file itself remaining
the single source of truth (never rewritten/compacted out from under a slower
cursor)."*

**The review disagrees on the first and agrees with the second.** Its finding:

> The mailbox is an append-only JSONL log that nothing ever truncates, rotates,
> or compacts. Because history is permanent, "where has this reader got to?"
> becomes load-bearing — and five authorities appear. **The cursors are coping
> mechanisms for never deleting.**

Grep for `rotate|truncate|compact` across `famp-inbox` and `cli/broker`: only test
names and one doc comment. Evidence that permanence is already costing:

- **A 16 MiB registration cliff.** `handle.rs` hardcodes `let since: u64 = 0;` on
  register with the comment *"preserving the historical since=0 behavior."*
  `decode_lines` on that path has **no cap**, and the resulting `RegisterOk.drained`
  is encoded into one reply frame checked against `MAX_FRAME_BYTES = 16 MiB`
  (`famp-bus/src/codec.rs:6,26`). A mailbox crossing 16 MiB makes **registration
  itself fail.** The 8 MiB WARN shipped this week is a smoke alarm, not a fix.
- **`/famp-clear`** — a hand-maintained skill whose only purpose is deleting
  mailboxes. A workaround skill for a missing retention policy.
- **Inspect degrades to `budget_exceeded`.** `read_message_snapshot`
  (`cli/broker/mod.rs`) `read_all`s every mailbox on every `Tasks`/`Messages`
  inspect, under a 500 ms budget with one permit. Cost grows monotonically with
  deployment lifetime — the observability tool dies exactly when you need it.

**These two positions are reconcilable, and that is the design.** The SPEC forbids
compacting *out from under a slower cursor*. Compaction below
`min(all known cursors for that mailbox)` never does. **Retention is not a separate
concern from position — it is the reason position is hard.** Once the broker owns
every cursor, it knows the min, and compaction becomes safe and trivial. Own
position *in order to* enable retention.

Recommendation: **amend the SPEC's out-of-scope list** rather than working around
it. If the reviewer who filed it disagrees, that disagreement is the first thing
to settle — it changes the whole shape.

---

## 4. Decisions already made that constrain you

- **MCP `famp_inbox` has no `ack`.** Its schema advertised `action: "ack"` and
  `offset` as *required*, and the tool read neither — `action: "ack"` silently
  performed a `list`. On 2026-07-08 the fields were **deleted from the schema
  rather than wired**, on the explicit reasoning that wiring them would make the
  primary agent surface a writer of authority 4 — the exact cursor 999.11 plans to
  delete. **A real `ack` is yours to design, against the unified cursor.** Don't
  re-add it to the old one.
- **`include_terminal` is parked on a false blocker.** The wire flag exists and is
  a no-op; five doc sites say this is because *"famp-bus would need to depend on
  famp-taskdir, crossing the boundary."* That is wrong. `famp-taskdir` is sync and
  tokio-free, and `BrokerEnv = MailboxRead + LivenessProbe` (`famp-bus/src/env.rs:6`)
  **already performs synchronous disk reads inside the actor loop**. Add a
  `TaskStateRead` supertrait; famp-bus gains zero dependencies. Safe because
  terminal FSM states are **absorbing**, so skip-and-advance past a terminal
  envelope is permanently valid — it does *not* reproduce 999.1, whose whole
  problem is that a task filter is *impermanent*. This is a natural `DrainPolicy`
  axis. Consider folding it in.
- **999.2 must be co-designed.** The SPEC is emphatic and correct: concurrent
  canonical + proxy consumers share the same offset. Fixing 999.11 alone risks
  re-opening it.

---

## 5. The trap

Byte offsets are the wire contract on `BusReply::InboxOk { next_offset }`. If you
compact, a live client holding a pre-compaction offset must not silently read the
wrong record.

**`famp-inbox/src/read.rs:97` snaps a mid-line cursor forward to a line boundary.**
That will **mask** a stale post-compaction offset rather than reject it. A client
resuming from a stale offset gets *plausible, wrong* data — no error, no warning.

Decide explicitly: version the mailbox file, or refuse to compact below any live
client's held offset. Do not leave this to the snap-forward.

Related, already true: `decode_lines`' `start_offset` traces to `BusMessage::Inbox
{ since }` — a **client-supplied, wire-deserialized** field, not a server-derived
cursor. Untrusted input already reaches the framing math. (§3.2's carried offsets
made this *more* correct, not less — the client now gets a snapped-forward,
self-consistent offset. But note the input is untrusted.)

---

## 6. First move — write the failing test

Before any design, add a test asserting:

> `famp inspect identities`' `unread` for an identity equals the number of
> envelopes a subsequent `famp_inbox` actually returns for it.

**It will fail today.** That failing test is the specification for 999.11 and the
acceptance criterion for "position has an owner." It converts a five-authority
architecture argument into one red bar.

Then extend `crates/famp-bus/tests/prop04_drain_completeness.rs` to interleave
`Inbox` and `Await` on a shared channel. Per the 2026-06-19 lesson: per-handler
tests did not catch the cursor-share bug. Only cross-handler interleaving did.

---

## 7. Do not redo

- **999.1's fix.** Shipped, human-verified (`2253dcc`). This item documents the
  *residual*, not a regression.
- **The drain-walk extraction.** Done (PR #8). Build on `DrainPolicy`/`WalkOutcome`.
- **`prop04_drain_completeness.rs` / `tdd02_drain_cursor_order.rs`.** These pin the
  invariants. If your change requires editing them, the change is wrong — or you
  have found a genuine semantics decision that deserves its own discussion.
- **The v1.0 federation crates.** Parked behind the spike-first decision (2026-06-08).

## 8. Reading order

1. `SPEC.md` (this directory) — the original filing, the 999.2 coupling, the
   forcing function (v1.0 gateway must not inherit this).
2. `REFACTORING-REVIEW-2026-07-08.md` §3.1 (retention + ownership, the fused root
   cause) and §3.2 (what shipped, and why it was the keystone).
3. `.planning/debug/resolved/999-1-await-fsm-ordering.md` — full investigation
   history and the 999.1 fix rationale.
4. `crates/famp-bus/src/broker/drain_walk.rs` — the one function you will change.
5. `crates/famp-bus/src/broker/state.rs:45-63` — the two in-memory cursor maps and
   their (individually correct, mutually uncoordinated) decision records.

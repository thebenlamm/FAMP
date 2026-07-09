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

- **Offsets advance in exactly one place *inside the bus actor*.** (`cli/broker/mailbox_env.rs::read_raw_from` and `cursor_exec` remain separate.) The SPEC's "recommended fix shape"
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

## 2. One correction to the SPEC

The SPEC says: *"Every mailbox exposes exactly one linear delivery position per
(owner, mailbox): `await_offsets`."* True of the **await path only**. Scope B
(2026-06-19) added a **second** in-memory cursor, `ClientState.inbox_offsets`
(`famp-bus/src/broker/state.rs:53-63`), because a task-filtered `Await` was eating
channel posts `Inbox` should have surfaced. The fix added an authority rather than
giving position an owner. **That is the pattern 999.11 exists to stop.**

So: **two** in-memory cursors, not one. Plus a separate metadata bug — the on-disk
`.<name>.cursor` (`cli/broker/cursor_exec.rs`), which only `register`, `join`, and
the CLI-only `famp inbox ack` advance, is what `famp inspect identities` reports as
`unread`. Since no *read* path advances it, `unread` counts envelopes `famp_await`
already consumed since the last register. Bounded by session length (register
re-advances it) — not unbounded. Fix with, or after, 999.11.

**Retention/compaction is NOT part of this, and an earlier draft of this brief was
wrong to say otherwise.** That draft argued rotation was upstream of cursor ownership
and told you to amend the SPEC's out-of-scope list. **Withdrawn.** Retention cannot
fix 999.1: the blocking envelope sits *at* the starved filter's cursor, which is the
`min`, so compaction below `min(all cursors)` can never remove it. The Scope-B cursor
share reproduces on a 3-line mailbox. Keep rotation out of scope, exactly as the SPEC
says. Owning position yields `min(cursors)` as a byproduct, which enables a *later*,
independent retention decision. That is the only relationship.

Calibration: largest mailbox 138 KB; all mailboxes 2.6 MB. There is no retention
problem yet. The `/famp-clear` skill cited as evidence for one turned out to be a
dead command pointing at a v0.8 script.

## 3. 999.2 is the real coupling — engage it

The SPEC is emphatic and correct: *"design and resolve 999.11 and 999.2 together —
do not patch one without the other."* Concurrent canonical + proxy consumers share
the same offset, so one consumer's drain robs another's. Neither the refactoring
review nor the first draft of this brief engaged 999.2 at all. **That, not retention,
is the blocker.** Start there.

Also now settled and in your way if you don't know it: mailbox files can shrink
beneath a cursor. `walk()` clamps (`since.min(drained.next_offset)`) and warns; the
old `debug_assert_eq!` that panicked the broker actor is gone (#11, #16). Any
per-consumer cursor map you build inherits that clamp obligation **per cursor**.

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

---

## Addendum — 2026-07-08: unread divergence fixed narrowly; redesign re-parked

**(a) The §2/§6 divergence is fixed narrowly.** The `unread`-vs-delivered
divergence this HANDOFF diagnosed in §2 ("no *read* path advances [the on-disk
cursor]") and pinned as the acceptance test in §6 is now fixed — narrowly, not
via the broader redesign. The MCP `famp_inbox` read path (`cli/mcp/tools/inbox.rs`)
now write-throughs its returned `next_offset` to the on-disk `.{name}.cursor` via
`cursor_exec::execute_advance_cursor` — the same atomic writer the CLI `famp
inbox ack` path already used — taking `max(current_disk_cursor, next_offset)` so
a manual `since: 0` full-replay never rewinds the disk cursor. `famp inspect
identities`' `unread` no longer lags the MCP session cursor. The §6 acceptance
test (`crates/famp/tests/inbox_unread_matches_delivered.rs`) flipped RED to GREEN
in commit `fda9de9`.

**(b) The broader redesign is RE-PARKED, not abandoned.** Owning delivery
position at the broker (one authority instead of five: the two in-memory
maps in §2, the on-disk cursor just patched above, plus 999.2's shared-offset
coupling) remains parked behind the federation spike per the 2026-07-01
v0.12-reliability-bucket decision (Matt+Magnus: local-case-black-hole).
This narrow fix removes one symptom (the inspector-vs-delivered drift) without
touching the underlying five-authority architecture §2 diagnoses, and does
**not** engage 999.2 (§3) at all. The full design doc survives at
`docs/superpowers/specs/2026-07-08-999-11-broker-owned-delivery-position-design.md`
for whenever the spike fires and this is picked back up.

**(c) Must-fix findings before implementation resumes.** Three independent
reviews of the design doc found it is not yet ready to implement as written.
Most notably:
- The design doc as written does **not** repoint the two cursor authorities
  the way this narrow fix just did for the disk cursor — it needs to account
  for the write-through pattern now shipped here, or supersede it cleanly with
  the unified cursor, not layer on top of it.
- The doc's **"bounded hole-set" claim was found to be UNBOUNDED** in the exact
  starvation scenario it targets (the 999.1/999.2 concurrent-consumer coupling
  §3 describes). Any resumed design work must close this gap before it is
  treated as ready to build.

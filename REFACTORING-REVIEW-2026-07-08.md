# Repo Refactoring Review — FAMP

_Generated 2026-07-08 · scope: full Rust workspace (15 crates, ~50k LOC incl. tests) · supersedes [REFACTORING-REVIEW-2026-06-05.md](REFACTORING-REVIEW-2026-06-05.md)_

**Status of the prior review:** findings §1 (split `handle.rs`), §2 (typed
`EnvelopeView`), and §4 (centralize the MCP `CliError → ToolError` map) shipped.
§3 (decompose `inspect-server`) is ~2/3 done (1130 → 720 LOC; `tasks.rs`,
`messages.rs`, `parse.rs` extracted). Quick win #5 (shared framing const) was
applied at 2 of ~7 sites. **None of those four findings recur below.** The
modularity debt the last review named is largely paid. What remains is a
*data-model* problem it did not see.

---

## 1. Executive Summary

The codebase is disciplined, densely commented with decision rationale, and
well-tested. Layering (Layer 0 primitives → Layer 1 bus → CLI/MCP surface) is
real, not aspirational. The remaining debt is **not modularity** — it is a
single unowned data model, and everything expensive traces back to it.

**The mailbox is an append-only JSONL log that nothing ever truncates,
rotates, or compacts.** Because history is permanent, "where has this reader
got to?" becomes the load-bearing question — and *five* independent answers now
exist, each locally correct, each introduced weeks apart, none composing:
in-memory `await_offsets`, in-memory `inbox_offsets`, the client-supplied
`since` parameter, the on-disk `.<name>.cursor` file, and "read the whole file"
(`famp_verify`). The 999.1 filtered-await starvation bug, the 2026-06-19
Scope-B cursor-share bug, the permanently-wrong `unread` count in `famp inspect
identities`, the `/famp-clear` skill that exists to manually delete mailboxes,
and the "double-print" context-cost pattern documented in
`docs/CLAUDE-CODE-CONTEXT-GUIDE.md` are all the same finding wearing five hats.

Highest-leverage opportunities, in order:

1. **Retention + delivery-position ownership** (Critical) — the fused root cause.
2. **The JSONL drain-walk, written four times with divergent invariants** (High)
   — the mechanism through which #1 produces bugs. Cheap, mechanical, and it is
   the prerequisite for #1 being tractable.
3. **`include_terminal` is parked on a factually incorrect blocker** (Medium) —
   `BrokerEnv` is the extension point; nobody used it.
4. **Inspect full-scans every mailbox per call, under a 500 ms budget** (Medium)
   — the observability tool degrades exactly when you need it.
5. **MCP tool schemas and tool implementations have no enforced
   correspondence** (Medium) — `famp_inbox` advertises `action:"ack"` and reads
   neither `action` nor `offset`.

Problem class: **data model + retention**, not architecture and not modularity.
Nothing here needs a rewrite. #2, #3, #5 and all five quick wins are
independently shippable today; #1 is the design work already filed as backlog
999.11, and this review is the evidence inventory that item lacks.

---

## 2. Mental Model of the Codebase

**Layers** (as documented, and accurate):

- **Layer 0 — protocol primitives**, transport-neutral: `famp-canonical`
  (RFC 8785 JCS), `famp-crypto` (Ed25519 + `FAMP-sig-v1\0` domain prefix),
  `famp-core`, `famp-fsm` (5-state task FSM, terminals absorbing),
  `famp-envelope`, `famp-inbox` (durable JSONL), `famp-taskdir` (per-task TOML).
- **Layer 1 — local bus**: `famp-bus` is a **pure actor** —
  `handle(BrokerInput) -> Vec<Out>`, no tokio, no I/O, no signatures on the
  local path (BUS-01/BUS-11). All effects are `Out` sentinels the executor
  performs. Inspect (`-proto`/`-server`/`-client`) rides as opaque JSON over the
  same socket (`BusReply::InspectOk { payload: Value }`).
- **Surface** — `famp` binary: clap CLI + MCP stdio server. MCP tools delegate to
  CLI `run_at_structured` entry points. This is good and worth defending: there
  is almost no business logic duplicated between the two surfaces.
- **Layer 2 — deferred federation**: `famp-transport`, `famp-transport-http`,
  `famp-keyring`. Zero live callers, deliberately parked behind the v1.0 gate.

**The impure half.** Because `famp-bus` performs no I/O, its executor —
`crates/famp/src/cli/broker/mod.rs` (869 LOC, 2nd-highest churn) — absorbed
everything the actor cannot do: the UDS accept loop, `Out` execution, taskdir
walking, mailbox-metadata reads, and `BrokerCtx` construction for the inspect
server. This is deliberate dependency inversion (INSP-RPC-02 keeps the inspect
crate tokio-free) and it is unit-tested in place. It is not, by itself, a
finding — but it is where two of the findings below surface.

**Historical pivots visible in the code.** v0.8 was HTTPS federation with
per-identity `FAMP_HOME`, TOFU pinning, and Ed25519 on every hop. v0.9 collapsed
same-host agents onto the UDS bus and dropped crypto locally. The fossils are
legible: `decode_line` calls the *one-arg* `AnyBusEnvelope::decode` (structural,
no verifier); eight `CliError` variants describe a federation CLI that commit
`1935bef` deleted. The parked v1.0 crates are the intentional residue; the dead
`CliError` variants are the unintentional kind.

**Where the oldest and newest code meet.** `broker/handle.rs` is still the #1
churn file (24 of the last 250 commits) *after* the prior review's split. That
is not evidence the split cut the wrong seam — dispatch surfaces attract commits
because every feature has a handler. But one genuine seam remains: **cursor
lifecycle is smeared across four handlers** (`register` seeds, `join` seeds,
`leave`/`disconnect` drop, `inbox`/`await` advance). See §3.1.

---

## 3. Highest-Impact Refactoring Opportunities

### 1. The mailbox log has no retention story, and delivery position has no owner

**Severity:** Critical
**Type:** Data model / Architecture

**Why it matters.** No code path truncates, rotates, or compacts
`mailboxes/*.jsonl` — grep for `rotate|truncate|compact` across `famp-inbox` and
`cli/broker` returns only test names and one doc comment. History is therefore
permanent, which turns "where has this reader got to?" into the system's central
question. Five subsystems answer it differently. The consequences are already
paid, repeatedly:

- **999.1** (filtered-await starvation) — a filtered `Await` blocks behind an
  earlier unmatched envelope. Fixed minimally; the residual starvation was
  accepted and refiled as **999.11, "broker-owned delivery position redesign."**
- **Scope B, 2026-06-19** — `Inbox` and `Await` shared one cursor, so a
  task-filtered await silently ate channel posts `Inbox` should have surfaced.
  The fix was to *add a second cursor map* (`inbox_offsets`), not to give
  position an owner. This is the shape of the problem: each incident adds an
  authority.
- **`famp inspect identities` reports a permanently-wrong `unread` count.**
  `read_mailbox_meta_for` (`crates/famp/src/cli/broker/mod.rs:616-633`) computes
  `unread` by diffing the mailbox against the **on-disk** `.cursor` file. That
  file is written only by `Out::AdvanceCursor` — emitted at
  `famp-bus/src/broker/handle.rs:349` (register) and `:758` (join) — and by
  `famp inbox ack` (CLI-only, no broker round-trip). **No read path advances
  it.** A listen-mode agent that consumes everything via `famp_await` shows
  monotonically growing unread forever. The project's own debugging memo
  ("run `famp inspect identities` first; delivery usually works, cursor-behind
  is the failure") is describing a structural property, not a bug.
- **Register replays the entire file.** `handle.rs:310-313` hardcodes
  `let since: u64 = 0;` with the comment *"preserving the historical since=0
  behavior. Replay-on-restart is tracked separately."* `decode_lines` on that
  path has **no cap**, and the resulting `RegisterOk.drained` is encoded into one
  reply frame against `MAX_FRAME_BYTES = 16 MiB` (`famp-bus/src/codec.rs:6,26`).
  A mailbox that crosses 16 MiB makes **registration itself fail**. That is a
  cliff, not a slowdown. (Mitigating: `await_offsets` is snapped to
  `drained.next_offset` immediately afterward at `handle.rs:336-338`, and the
  MCP `famp_register` tool discards `drained` and surfaces only the count — so
  the symptom is a full-history wire transfer, not message re-delivery.)
- **The user maintains a manual `/famp-clear` skill** to truncate mailboxes.
  A workaround skill for a missing retention policy is the clearest possible
  signal.

**Evidence.** The five authorities:

| # | Authority | Location | Advanced by | Read by |
|---|---|---|---|---|
| 1 | `ClientState.await_offsets` | `famp-bus/src/broker/state.rs:45-52` | `Await` only | `Await`, waiter view |
| 2 | `ClientState.inbox_offsets` | `state.rs:53-63` (channels only) | `Inbox` only | `Inbox` |
| 3 | `since: Option<u64>` param | `handle.rs:514` (agent mailbox) | the *client* | `Inbox` |
| 4 | `.<name>.cursor` on disk | `cli/broker/cursor_exec.rs` | register, join, `famp inbox ack` | `inspect identities` unread |
| 5 | "read the whole file" | `mcp/tools/verify.rs` (`read_all`, no cursor) | n/a | `famp_verify` |

(`famp_channel_log` is a sixth surface: caller-supplied `since`, returns a
`next_offset` nobody persists.)

**Root problem.** Three locally-correct ownership decisions, each with a
decision record in the code (`state.rs:45-63`; `cli/inbox/mod.rs:5` —
*"Cursor management remains client-side (RESEARCH §6)"*; `state.rs:77-81` —
*"Disk cursor truth for inspector unread counts lives at…"*), made ~6 weeks
apart, that do not compose. Upstream of all three: **nothing is ever deleted**,
so a cursor is the only thing standing between a reader and the beginning of
time. Retention and delivery-position are one design, and it has never been
designed once.

**Recommended refactor direction.** Two moves, in this order:

1. **Give retention an owner first — it is the smaller change and it shrinks
   the second.** At broker boot (and on a `tick` interval), compact each mailbox
   below `min(all known cursors for that mailbox)`. Rewrite offsets as
   `absolute_base + relative`, or simpler: keep absolute byte offsets and
   compact by *rewriting the file with a recorded base offset* in a sidecar, so
   existing `u64` offsets stay valid. Even a crude `--max-mailbox-bytes` guard
   that logs loudly at 8 MiB buys you out of the registration cliff today.
2. **Then collapse the position authorities.** The target shape is one
   broker-owned `BTreeMap<(Holder, MailboxName, Surface), u64>`, where `Surface ∈
   {Inbox, Await}` — this *keeps* the Scope-B independence that was correct while
   putting both maps under one type with one advance function. Persist it beside
   the mailboxes so it survives restart, and derive the inspector's `unread`
   from **that** rather than from a fourth file. The client-supplied `since`
   becomes an *override* for replay/debugging, not the default authority.

**Expected payoff.** One advance function instead of four inline
`.insert(mailbox, offset)` sites; `unread` becomes true; register stops
transferring history; 999.11 becomes a scoped change rather than a redesign;
`/famp-clear` can be deleted. Bug surface shrinks by construction — the class of
"surface A ate surface B's envelopes" becomes unrepresentable.

**Migration strategy.** Land §3.2 (the drain extraction) first — it gives you
one place where offsets advance, which is what makes this refactor a change to
*one function* instead of six. Then: (a) ship the compaction guard + loud log,
zero semantic change; (b) introduce the unified cursor map alongside the existing
two, writing to both and asserting equality in tests for one release; (c) flip
readers over; (d) delete `await_offsets`/`inbox_offsets` and the disk `.cursor`
writer. The property tests in `famp-bus/tests/prop04_drain_completeness.rs` and
`tdd02_drain_cursor_order.rs` already pin the invariants you must not break.

**Risks / cautions.** Compaction mutates the file `famp-inbox`'s "bytes-signed =
bytes-stored" invariant depends on — compact by whole records only, never
re-encode a line. Byte offsets are the wire contract on `BusReply::InboxOk
{ next_offset }`; a live client holding a pre-compaction offset must not silently
read the wrong record. `famp-inbox/src/read.rs:97` already snaps a mid-line
cursor to a line boundary, which will *mask* a stale post-compaction offset
rather than reject it. Decide explicitly: version the mailbox file, or refuse to
compact below any live client's held offset. **This is the one part of this
review that deserves a design doc before code**, and 999.11 is where it belongs.

---

### 2. The JSONL drain-walk is implemented four times, with divergent invariants

**Severity:** High
**Type:** Data model / Duplicated decision-making

**Why it matters.** Walking a drained batch — decode each line, decide whether
to deliver it, advance the byte offset past it — is the single most
correctness-critical loop in the system, and it is written four times. The
divergences are not cosmetic; they are *where the bugs live*. 999.1 and Scope B
were both fixes applied to one copy of this loop.

**Evidence.**

| Site | Filter | Self-authored skip | Head-of-line skip | Cap | Framing math |
|---|---|---|---|---|---|
| `famp-bus/src/broker/awaiting.rs:235` (`drain_await_batch`) | task filter | yes | yes | `AWAIT_BATCH_CAP = 50` | `(line.len() + 1)` |
| `famp-bus/src/broker/handle.rs:625` (`inbox`, channel loop) | — | yes | yes | `CHANNEL_DRAIN_CAP = 256` | `JSONL_RECORD_TERMINATOR_LEN` |
| `famp-bus/src/broker/handle.rs:983` (`decode_lines`) | — | **no** | yes | **none** | `(line.len() + 1)` |
| `famp/src/cli/broker/mailbox_env.rs:163-235` (`read_raw_from`) | — | — | tail-tolerant snap-forward | — | `+1`, by hand |

Plus `famp-bus/src/mailbox.rs:180,194` (`InMemoryMailbox`), whose framing mirrors
`famp-inbox/src/read.rs` — coupled by a comment (`mailbox.rs:20-24`), not a type.
`read_raw_from`'s doc comment (`mailbox_env.rs:159`) likewise pins itself to
`famp_inbox::read::read_from` in prose.

Two clarifications the evidence forced, against my first reading:

- **`decode_lines`' missing self-authored skip is correct, not a latent bug.**
  `awaiting.rs:220` documents self-filtering as channel-only pub/sub semantics;
  a DM addressed to yourself *should* deliver. Good.
- **`decode_lines`' offset accumulator is not dead** — it feeds `byte_offset` in
  the WARN log at `handle.rs:986-989`. Also fine.

The real divergence is the **missing cap** on `decode_lines` (the register/join
path), which is the 16 MiB registration cliff from §3.1, and the fact that
`JSONL_RECORD_TERMINATOR_LEN` is `pub(crate)` in `famp-bus`
(`mailbox.rs:25`) — it *cannot* be shared with `famp-inbox` or the `famp` crate
even in principle. The prior review's quick win #5 was structurally incapable of
finishing.

**Root problem.** `DrainResult { lines: Vec<Vec<u8>>, next_offset: u64 }`
throws away per-line offsets. Every consumer must therefore re-derive the
framing math to know where each record ended — and each one re-derives the
*policy* (skip, cap, advance) alongside it, because there is no type that
carries "a record, its offset, and the decision to skip it."

**Recommended refactor direction.** Make the type carry what the consumers keep
re-deriving:

- Have `MailboxRead::drain_from` return `Vec<DrainedRecord { bytes, start, end }>`
  (`famp_inbox::read::read_from` already computes `(value, end_offset)` pairs
  internally — it is throwing the offset away too).
- Extract one `fn walk<'a>(records, policy: DrainPolicy) -> WalkOutcome`, where
  `DrainPolicy` names the four axes above (`filter`, `skip_self_authored`,
  `cap`) and `WalkOutcome` carries `{ delivered, next_offset, fully_drained }`.
  All four call sites become a policy literal.
- Promote the terminator const into `famp-inbox` as `pub`, and delete the three
  hardcoded `+ 1`s.

**Expected payoff.** Framing math exists once. The `fully_drained` flag —
currently a bespoke field on `AwaitBatch` invented for 999.1 — becomes available
to the `Inbox` path for free, which is exactly the signal §3.1 needs. Adding a
cap to the register drain becomes a one-word change. A new consumer physically
cannot forget to advance past a skipped line, which is the bug class that wedged
scs-opus's mailbox on 2026-06-11.

**Migration strategy.** Purely mechanical and behavior-preserving if done in
this order: (1) widen `DrainResult` to carry per-record offsets, leaving all
consumers computing their own (green); (2) switch consumers to read the carried
offsets one at a time, deleting each `+1` (green after each); (3) extract `walk`
and collapse the four loops. `prop04_drain_completeness.rs` and
`tdd02_drain_cursor_order.rs` cover this ground; do not touch policy while
moving code.

**Risks / cautions.** `read_raw_from` must keep handing back **on-disk bytes
verbatim** (the broker re-decodes via `AnyBusEnvelope::decode`); the refactor
must not tempt anyone into returning parsed `Value`s. The tail-tolerant
snap-forward in `read_raw_from` is load-bearing for concurrent-append safety —
port it, don't reinvent it.

---

### 3. `include_terminal` is parked on a blocker that does not exist

**Severity:** Medium
**Type:** API design

**Why it matters.** `BusMessage::Inbox { include_terminal }` is accepted on the
wire, threaded through the handler signature, and then dropped
(`handle.rs:514-525`, parameter `_include_terminal`). The stated reason, repeated
in `ARCHITECTURE.md`, `README.md`, the MCP tool description, the `/famp-inbox`
slash command, and a project memory:

> *"would require the famp-bus actor to read famp-taskdir, which crosses the
> transport-vs-cli boundary."*

`famp-taskdir` is synchronous and tokio-free (deps: serde, toml, thiserror,
tempfile, uuid). And `famp-bus` **already does synchronous full-file disk reads
from inside the actor loop** — that is precisely what `BrokerEnv::drain_from`
is (`mailbox_env.rs:94-98`). `BrokerEnv = MailboxRead + LivenessProbe`
(`famp-bus/src/env.rs:6`) is the designated impure-adapter seam. Nobody added a
third capability to it, so a shipped protocol flag became a documented lie in
five places.

**Root problem.** The purity rule ("`famp-bus` does no I/O") was read as "no new
capabilities," when the codebase's own answer to I/O-in-the-actor is *add a
`BrokerEnv` supertrait*. The extension point exists and is unused.

**Recommended refactor direction.** Add `trait TaskStateRead { fn is_terminal(&self,
task_id: Uuid) -> bool; }` to `famp-bus`, make it a supertrait of `BrokerEnv`,
implement it in `mailbox_env.rs` over `famp-taskdir`, and consume it in the
§3.2 `DrainPolicy`. `famp-bus` gains **zero** new dependencies — the trait is
bus-defined, the impl lives in the binary crate exactly where `MailboxRead`'s
does.

**Why this is safe, and why it is not 999.1 again.** The obvious objection is
that a terminal filter reintroduces the filtered-await starvation of 999.1. It
does not: 999.1 stalls because a task filter is *impermanent* — an envelope that
does not match filter `T` today may be exactly what a later awaiter wants, so the
drain must not advance past it. **Terminal FSM states are absorbing**
(`famp-fsm`: `COMPLETED|FAILED|CANCELLED`, all terminal). An envelope for a
terminal task is unmatchable under `include_terminal: false` *forever*, which
means skip-and-advance is permanently valid — the same argument already written
into `awaiting.rs:~245` justifying the unconditional advance past self-authored
posts.

**Expected payoff.** A shipped flag stops being a no-op; five documentation
sites stop carrying a false rationale; and the codebase gains the pattern
("capability goes on `BrokerEnv`") that will be needed again the moment the v1.0
gateway wants task state at the bus boundary.

---

### 4. Inspect full-scans every mailbox on every call, under a 500 ms budget

**Severity:** Medium
**Type:** Architecture (scaling)

**Why it matters.** `read_message_snapshot` (`cli/broker/mod.rs:579-611`) calls
`famp_inbox::read::read_all` on **every registered identity's mailbox and every
`#channel.jsonl` on disk**, loading all of it into memory, on every
`InspectKind::Tasks` or `InspectKind::Messages` request. `read_mailbox_meta_for`
additionally `read_all`s each mailbox to count totals. This runs inside
`spawn_blocking` behind a **1-permit semaphore** and a time budget whose
overflow surfaces as `budget_exceeded`.

Given §3.1 (nothing is ever deleted), inspect cost grows monotonically with the
lifetime of the deployment. The observability tool degrades to a permanent
`budget_exceeded` precisely when mailboxes are large — i.e. exactly when the
operator needs it. The `budget_exceeded` wire shape was hardened only last
sprint (`inspect_budget_exceeded_payload`, kind-tagged enum fix), which suggests
it is already being hit.

**Root problem.** Metadata (count, last sender, last timestamp, unread) is
recomputed from the full log because it is stored nowhere. The lazy-walk
optimization at `build_inspect_ctx_blocking:520-536` (D-06 — only walk taskdir
for `Tasks`) treats the symptom.

**Recommended refactor direction.** Maintain per-mailbox metadata incrementally
where the append already happens (`Out::AppendMailbox`'s executor arm): a
sidecar `mailboxes/.<name>.meta` holding `{total, last_sender, last_ts, tail_offset}`,
written in the same atomic-replace style as `cursor_exec`. Inspect reads the
sidecar; `read_all` becomes a `--deep` fallback. This composes with §3.1 — the
sidecar is the natural home for the unified cursor too. **Do #1 first;** if
compaction lands, this becomes much less urgent, which is why it ranks 4th.

**Expected payoff.** Inspect latency becomes O(identities) instead of
O(total bytes ever sent). The `budget_exceeded` path stops being reachable in
normal operation.

---

### 5. MCP tool schemas and tool implementations have no enforced correspondence

**Severity:** Medium
**Type:** API design

**Why it matters.** The MCP surface is, per `CLAUDE.md`, the deployment target —
"the installed binary at `~/.cargo/bin/famp` is what every agent session reads."
It is also the surface declared to be the **stable contract across v0.8/v0.9/v1.0**.
Right now it advertises a capability it does not have, and nothing catches it.

**Evidence.** `cli/mcp/server.rs:72` declares, in the `famp_inbox` JSON schema:

```json
"action": { "type": "string", "enum": ["list","ack"], "description": "list=show messages, ack=mark as processed" },
"offset": { "type": "integer", "description": "Byte offset to ack up to (required for action=ack)" }
```

`cli/mcp/tools/inbox.rs::call` never reads `action` and never reads `offset` —
grep for `action` in that file returns only doc comments. `action: "ack"`
silently performs a `list`. Meanwhile `cli/inbox/ack.rs`'s doc comment claims it
is *"used by the MCP famp_inbox tool wrapper"*; it has zero callers outside CLI
dispatch. So the primary surface's only cursor-advance affordance is a no-op
that reports success — which is one of the mechanisms keeping the disk cursor
(§3.1, authority 4) permanently behind.

**Root problem.** Schemas are hand-written JSON literals in `server.rs`; tool
bodies hand-parse `Value`. Nothing — not the type system, not a test — relates
one to the other. Contrast this with `From<CliError> for ToolError`
(`tools/mod.rs`), which the last review's §4 correctly centralized: the error
projection has a single owner, the *input* projection has none.

**Recommended refactor direction.** Two options; the first is enough.

- **Cheap and sufficient:** wire `action: "ack"` through to
  `cli::inbox::ack::run_at_structured` (~10 lines; gives MCP its first
  cursor-advance path), and add a test that asserts every property named in a
  tool's `inputSchema` is read by that tool's `call`. A reflection test over the
  schema literal catches the whole class.
- **Structural:** give each tool a `#[derive(Deserialize)]` args struct with
  `deny_unknown_fields`, and generate the `inputSchema` from it (`schemars`).
  Schema drift becomes a compile error. Worth it only if new tools keep landing.

**Expected payoff.** The declared MCP contract becomes true. The class "tool
documents a field it ignores" stops being possible. And §3.1's disk cursor gains
its first legitimate writer from the surface that actually reads mail.

---

## 4. Cross-Cutting Patterns

- **Append-only-forever is an unexamined premise.** It is not written down as a
  decision anywhere; it is simply what happened when nobody wrote a delete path.
  Four cursor authorities, a manual `/famp-clear` skill, a 16 MiB registration
  cliff, an inspect budget, and a documented "double-print" context-cost pattern
  are all downstream of it. *When a system accretes coping mechanisms around one
  absence, name the absence.*
- **Locally-correct decisions that don't compose.** Each cursor authority has a
  decision record in a code comment; none references the others. The codebase's
  commenting discipline is genuinely excellent at recording *why this line* and
  has no mechanism for recording *why these three things together*. That is what
  `ARCHITECTURE.md` is for, and cursors aren't in it.
- **Framing math re-derived because the type won't carry it.** `DrainResult`
  drops per-line offsets; four consumers recompute `+1`. A `pub(crate)` const
  cannot cross the crate boundary where two of the copies live.
- **Coupling asserted in prose, enforced by nothing.** `mailbox_env.rs:159`
  ("Mirrors `famp_inbox::read::read_from`"), `mailbox.rs:20-24` ("mirrors
  famp-inbox's on-disk framing"), `inbox/ack.rs:32` ("used by the MCP wrapper" —
  it isn't), `server.rs:72` ("ack=mark as processed" — it doesn't). Four
  prose-level contracts; three are already false. Doc comments are where this
  codebase puts its invariants, and they have started to rot in exactly the
  places where two crates meet.
- **Tests can pin dead code.** `tests/mcp_error_kind_exhaustive.rs` constructs
  eight `CliError` variants that no production code can produce, in order to
  assert their error mapping. The test is why `cargo` never told anyone the
  federation CLI's errors outlived the federation CLI (commit `1935bef`).
- **An unused extension point invites deferral.** `BrokerEnv` was built as the
  impure seam, then `include_terminal` was parked rather than extend it. Seams
  that nobody has widened twice do not read as seams.

**Explicitly not findings.** The deferred v1.0 crates (`famp-transport`,
`-http`, `famp-keyring`) remain correctly parked; the spike-first decision of
2026-06-08 makes touching them pure churn. The `install`/`uninstall` symmetry is
fine — both are thin wrappers over shared `json_merge`/`toml_merge`, and
`uninstall_after_install_returns_to_clean_state` pins the round-trip. The
`CliError`/`BusErrorKind` duality that the last review called "transitional" has
in fact settled into a coherent design (the `BusError { kind, message }` funnel
plus the centralized `ToolError` map); leave it. 25 error enums across 15 crates
is one-per-crate and idiomatic, not sprawl.

---

## 5. Quick Wins

Each <30 min, zero behavioral risk.

1. **Delete 8 dead `CliError` variants** — `PortInUse`, `PeerNotFound`,
   `PeerDuplicate`, `PeerCardInvalid`, `PeerEndpointInvalid`, `PeerPubkeyInvalid`,
   `AlreadyInitialized`, `KeygenFailed`. Verified zero production construct
   sites; only `tests/mcp_error_kind_exhaustive.rs` and the uncompiled
   `tests/_deferred_v1/` reference them. Drop the matching test arms in the same
   commit.
2. **Wire MCP `action: "ack"`** to `cli::inbox::ack::run_at_structured`, or
   delete `action`/`offset` from the `famp_inbox` schema. Either honest outcome
   beats today's silent no-op. (§3.5)
3. **Promote `JSONL_RECORD_TERMINATOR_LEN` into `famp-inbox` as `pub`** and
   replace the three hardcoded `+ 1`s (`handle.rs:983`, `awaiting.rs:235`,
   `mailbox_env.rs`). Finishes the prior review's quick win #5, which was
   impossible while the const was `pub(crate)` in the wrong crate.
4. **Cap or loudly warn `decode_lines` on the register path.** One `.take(N)` or
   one `tracing::warn!` when `drained.lines.len()` is large converts the silent
   16 MiB registration cliff into an early signal. (Interim guard for §3.1.)
5. **Make `BrokerState::view()` call `identity::canonical_holder_id`** instead of
   re-implementing the `bind_as` holder lookup inline (`state.rs:~150-165`).
   The blocker is that the `identity::*` helpers take `&Broker<E>` when three of
   four only need `&BrokerState`; narrow the signatures and the duplication
   deletes itself.

---

## 6. Suggested Refactoring Roadmap

**Now — quick wins + the one that unblocks everything.**
Land quick wins 1–5 in any order (independent, mechanical). Then §3.2, the
drain-walk extraction, in the three-step sequence given: widen `DrainResult`,
migrate consumers one at a time, extract `walk`/`DrainPolicy`. This is the
keystone: it produces the single place where offsets advance, without which §3.1
is a six-site change and §3.3 has nowhere to plug in.

**Next — in parallel, once §3.2 is green.**
§3.3 (`TaskStateRead` on `BrokerEnv`, consumed as a `DrainPolicy` axis) and §3.5
(schema/impl correspondence test). Independent surfaces, no shared files.
§3.3 also updates the five documentation sites carrying the false blocker.

**Before touching §3.1 — instrument.**
Add a test that asserts `inspect identities`' `unread` equals the count a
subsequent `famp_inbox` actually returns. **It will fail today.** That failing
test is the specification for §3.1 and the acceptance criterion for 999.11.
Also extend `prop04_drain_completeness.rs` to cover the `Inbox`-then-`Await`
interleaving on a shared channel — per the 2026-06-19 lesson, per-handler tests
did not catch the cursor-share bug, only cross-handler interleaving did.

**Then — §3.1, as a design doc first.**
Retention (compaction below `min(cursors)`) before position (the unified map).
This is 999.11. The offset-stability question — what happens to a live client's
held `next_offset` after compaction — is the one thing in this review that must
be *decided*, not merely refactored. `famp-inbox/read.rs:97`'s snap-forward will
mask a stale offset rather than reject it; that is the trap.

**Wait — §3.4.**
Incremental mailbox metadata is real but its urgency collapses if compaction
lands. Revisit after §3.1.

**Don't.**
Touch the parked v1.0 federation crates. Touch the `CliError`/`BusErrorKind`
split. Re-split `handle.rs` on churn alone — the seam is right; the residual
smear is cursor *lifecycle* (`register` seeds, `join` seeds, `leave`/`disconnect`
drop, `inbox`/`await` advance), and §3.1 will extract a `broker/cursors.rs`
sibling to `awaiting.rs` as a natural byproduct. Don't do it before then.

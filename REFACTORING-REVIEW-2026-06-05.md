# Repo Refactoring Review — FAMP

_Generated 2026-06-05 · scope: full Rust workspace (15 crates, ~26k LOC src)_

## 1. Executive Summary

The codebase is **healthy and well-tested**, with a clean primitive→bus→surface
layering and a disciplined wire protocol. Design debt is concentrated, not
diffuse, and almost all of it traces to **two structural forms**:

1. **Handler accretion** — request-surface logic piled into single files until
   they became god modules. `famp-bus/src/broker/handle.rs` (2333 LOC, 19
   commits in last 200) and `famp-inspect-server/src/lib.rs` (1130 LOC) are the
   two centers of gravity.
2. **Envelope-as-raw-JSON** — a typed `famp-envelope` crate exists, but the read
   path treats envelopes as untyped `serde_json::Value` and pokes individual
   fields with `.get("from").and_then(as_str)` in **22 sites across 9 files**.
   The field-name strings and parsing decisions are duplicated everywhere a
   message is inspected.

The four highest-leverage opportunities: **(1)** split `handle.rs` into
dispatch + await + identity modules, **(2)** introduce a typed pre-verification
envelope view to collapse the 22 raw-JSON poke sites, **(3)** decompose
`inspect-server`, **(4)** factor the MCP tool boilerplate. None require a
rewrite; all are incremental. The deferred v1.0 federation crates are
**deliberately parked, not dead** — left as-is (see §5).

The problem is **modularity + data-model**, not architecture. The layering is
right; the largest files just outgrew their boundaries.

## 2. Mental Model of the Codebase

**Layered, mostly as documented in `ARCHITECTURE.md`:**

- **Protocol primitives** (transport-neutral, reused across v0.9 + v1.0):
  `famp-canonical` (RFC 8785 JCS), `famp-crypto` (Ed25519), `famp-core`
  (identity/principal), `famp-fsm` (5-state task FSM), `famp-envelope` (signed
  envelope + structural decode).
- **Local bus** (`famp-bus`): a **pure-actor** UDS broker — `handle.rs` is a
  synchronous `Msg → Vec<Out>` reducer over `BrokerState`, no tokio inside the
  core. `proto.rs` is the wire enum; `mailbox.rs` is a read-only drain trait +
  in-memory test double; `famp-inbox` is the durable on-disk JSONL layer.
- **CLI/MCP surface** (`famp`, 13.8k LOC): clap CLI + an MCP stdio server. MCP
  tools **delegate** to CLI `run_at_structured` entry points (good — minimal
  business-logic duplication between the two surfaces).
- **Inspect subsystem** (`famp-inspect-proto/-server/-client`): read-only
  introspection, carried as **opaque JSON over the bus** (not a parallel wire
  protocol — `BusReply::InspectOk { payload: Value }`).
- **Deferred v1.0 federation** (`famp-transport`, `famp-transport-http`,
  `famp-keyring`): parked; zero live callers in the v0.x path.

**Historical pivot (visible in the code):** v0.8 was HTTPS federation daemons
(`FAMP_HOME`, TOFU-pinned peers, per-message Ed25519). v0.9 pivoted to a
local-first UDS bus and **dropped crypto on the local path**. Confirmed in
`handle.rs`: the drain calls the one-arg `AnyBusEnvelope::decode(line)`
(`bus.rs:49`, structural decode, **no verifier**) — not the two-arg
signature-verifying variant. The transport/keyring crates and the envelope's
verifying decode path are fossils of the pre-pivot world, intentionally kept
for the v1.0 gateway.

## 3. Highest-Impact Refactoring Opportunities

### 1. Decompose `handle.rs` — the broker god module

**Severity:** High
**Type:** Modularity

**Why it matters:** At 2333 LOC and 19 commits in the last 200, this is the
single most-touched, highest-fan-in file in the repo. Every new bus feature
lands here, and it now mixes ~7 unrelated reasons to change. The await
subsystem alone is 9 interdependent functions (~450 LOC) welded to the dispatch
loop. Cognitive load per change is high and the blast radius of any edit is the
whole broker.

**Evidence:** `crates/famp-bus/src/broker/handle.rs`
- Dispatch: `handle()`/`handle_wire()` (~13–103)
- 14 message handlers: hello, register, send, send_agent, send_channel, inbox,
  await_envelope, join, leave, sessions, whoami, set_listen, disconnect, tick
  (~114–820)
- **Await subsystem** (9 fns, ~544–1075): `await_envelope`, `resolve_await_owner`,
  `await_mailboxes`, `await_offset`, `set_await_offset`, `drain_await_batch`,
  `await_reply_for_mailbox`, `waiting_clients_for_name`, `filter_matches`
- Identity resolution (5+ call sites): `registered_name`, `effective_identity`,
  `proxy_holder_alive`, `canonical_holder_id`, `resolve_op_identity` (~856–938)
- Codec/validation: `encode_envelope`, `decode_lines`, `task_id_from` (~1131–1178)
- ~300 LOC of embedded tests (~1190–1499)

**Root problem:** The file is the broker's entire request surface in one
compilation unit. The await feature and identity resolution are cohesive
sub-domains that never got their own module.

**Recommended refactor direction:** Keep `handle.rs` as the thin dispatch +
simple handlers. Extract:
- `broker/await.rs` — the 9 await functions + their tests (~450 LOC)
- `broker/identity.rs` — the 5 resolution helpers, exposed as
  `identity::resolve_op_identity(state, client)`, deduping the 5 inline call
  sites and their copy-pasted `NotRegistered` error path.
- Move embedded tests into `broker/handle/tests.rs` or `tests/`.
This drops `handle.rs` to ~1500 LOC of genuinely-related dispatch logic.

**Expected payoff:** Three modules with one reason to change each; await and
identity become independently testable; new-message edits stop touching await
internals. Smaller diff scope per future feature.

**Migration strategy:** Pure mechanical move — these are free functions over
`&BrokerState`/`&mut BrokerState`, no trait surface to break. Extract one module
at a time, `cargo nextest run -p famp-bus` between each (note: use plain
`cargo test`, not `nextest list` — see project memory on the nextest list hang).
Zero behavior change; the actor signature is unchanged.

**Risks / cautions:** The await functions share private state-mutation helpers;
extract those alongside or keep them `pub(super)`. Don't refactor logic while
moving — move first, verify green, then simplify in a separate commit.

---

### 2. Introduce a typed envelope view — kill the raw-JSON poking

**Severity:** High
**Type:** Data model

**Why it matters:** `famp-envelope` defines typed envelopes, yet the read path
never uses them. Instead, **22 sites across 9 files** extract fields with raw
`value.get("from").and_then(serde_json::Value::as_str)`. The field name
`"from"`/`"to"`/`"task_id"`/`"body"` and its parse rule are re-encoded at every
site. Add a field, rename one, or change a parse rule, and you must find all 22.
This is duplicated decision-making about the message's data shape — the most
breadth of any single issue in the repo.

**Evidence:** raw-poke sites in `register.rs`, `mcp/tools/send.rs`,
`mcp/tools/verify.rs`, `await_cmd/poll.rs`, `cli/broker/mod.rs`,
`mcp/tools/await_.rs`, `mcp/tools/inbox.rs`, `famp-envelope/src/peek.rs`, and
**19 occurrences inside `famp-inspect-server/src/lib.rs`** alone
(`message_row()` ~407–438, `envelope_task_id()` ~443–473). `peek.rs` is a
half-step: a typed `peek_sender() -> Principal` accessor — but it extracts only
the `from` field and is used only by tests/runtime glue.

**Root problem:** There's no typed accessor for the **pre-verification** read
path. The full `AnySignedEnvelope::decode` requires a `TrustedVerifyingKey`
*up-front* (you need the sender to look up the key — the two-phase decode that
`peek.rs` documents), so consumers that only want to *read* fields before/without
verification fall back to raw JSON.

**Recommended refactor direction:** Promote `peek.rs` into a small typed
**structural view** — e.g. `EnvelopeView<'a>` (or `peek_envelope(bytes) ->
EnvelopeHeader`) in `famp-envelope` that strict-parses once and exposes
`from()`, `to()`, `task_id()`, `kind()`, `body()` as typed accessors, **no
signature verification**. Migrate the 22 sites (inspect-server first — biggest
cluster) to it. The field-name strings and parse rules live in exactly one
place.

**Expected payoff:** Single source of truth for envelope field access; renaming
a wire field becomes a one-file change; inspect-server's `message_row`/
`envelope_task_id` collapse to typed calls; new inspectors can't silently
misspell a field name.

**Migration strategy:** Additive — build the view, migrate call sites
incrementally (each is a local change), delete `peek_sender` once `from()`
subsumes it. Property-test the view against the existing raw extractors on a
corpus of real envelopes before flipping each site.

**Risks / cautions:** Must stay a **pre-verification structural** view — do NOT
"just decode the full verified envelope everywhere," because verification needs
the keyring lookup the local path deliberately skips. Keep byte-exactness: the
view parses, it never re-encodes (matches `famp-inbox`'s "bytes-signed =
bytes-stored" invariant).

---

### 3. Decompose `famp-inspect-server/src/lib.rs`

**Severity:** Medium
**Type:** Modularity

**Why it matters:** 1130 LOC, 13 commits in last 200 — the second center of
gravity. One `lib.rs` mixes the RPC dispatcher with five independent handlers
(broker, identities, tasks, messages, waiters) plus ad-hoc JSON parsers. Each
new inspect kind grows the file; the handlers have nothing to do with each other.

**Evidence:** `crates/famp-inspect-server/src/lib.rs` — `dispatch()` (~94–111),
`inspect_broker` (~113), `inspect_identities` (~123), `inspect_waiters` (~150),
`inspect_tasks`/`inspect_tasks_by_id` (~168–353, the heavy one), `inspect_messages`/
`message_row` (~357–439), and parser utilities `envelope_task_id`/`derive_fsm_state`/
`parse_rfc3339_to_epoch` (~443–511).

**Root problem:** Same handler-accretion pattern as `handle.rs` (§1), in the
inspect crate. (The duplicated JSON-poking *within* this file is the
inspect-server manifestation of finding §2 — fix it there, don't double-count.)

**Recommended refactor direction:** One module per inspect kind
(`server/tasks.rs`, `server/messages.rs`, …) behind the existing `dispatch()`.
The `tasks` handler (~185 LOC of FSM-state aggregation) most needs its own home.
The parser utilities move into the §2 typed view.

**Expected payoff:** Each inspect kind independently testable; `tasks` FSM logic
isolated from message scanning; adding an inspect kind is a new file, not a
bigger one.

---

### 4. Factor the MCP tool boilerplate

**Severity:** Medium
**Type:** Modularity / Cleanup

**Why it matters:** The 12 files in `cli/mcp/tools/` repeat the same three-part
shape — required-field parsing, `act_as: session::active_identity().await`
injection, and an identical `match CliError { … }` → `ToolError` mapping arm
block (~250 LOC of duplication). Every new tool copy-pastes it; every change to
error mapping touches all 12.

**Evidence:** the `match run_at_structured(...) { Ok =>…, Err(BusError) =>…,
Err(NotRegisteredHint) =>…, Err(BrokerUnreachable) =>…, Err(e) =>… }` block is
byte-identical in `join.rs:66`, `leave.rs:48`, `send.rs:88`, `inbox.rs:119`,
`await_.rs:94` (send adds one `SendArgsInvalid` arm). Field-parse and
`active_identity` injection repeat across the same set.

**Root problem:** No shared bridge between "an MCP tool" and "a CLI
`run_at_structured` entry point." Each tool re-implements the adapter.

**Recommended refactor direction:** A single `call_cli_tool` helper (or
`CliToolBridge` trait) owning the `CliError → ToolError` mapping and identity
injection, so each tool is just `parse args → call → shape output`. Centralizing
the error map also kills the drift risk between tools.

**Expected payoff:** ~250 LOC removed; error-mapping changes become one-file;
new tools are ~15 LOC. **Caveat:** keep this only if the team prefers it — Rust
shops often tolerate explicit handler boilerplate over a macro/trait
indirection. If the error map is the only real pain, extract *just* that mapping
fn and leave the parse/inject inline.

## 4. Cross-Cutting Patterns

- **Handler accretion** (root of §1 + §3): the request-surface files
  (`handle.rs`, `inspect-server/lib.rs`) grew one handler at a time and never
  got sub-module boundaries. A "one module per handler kind" convention would
  prevent recurrence.
- **Envelope-as-untyped-JSON** (§2): the most pervasive single decision-duplication.
- **Identity resolution duplication:** the canonical-holder / proxy-liveness
  lookup is inlined in 4–5 spots in `handle.rs` with subtly different filtering
  — a latent inconsistency risk; §1's `identity.rs` extraction fixes it.
- **Error-construction inconsistency (low):** `handle.rs` builds error replies
  three ways (the `err()` helper, direct `BusReply::Err{…}`, and
  `Out::Reply(…, HelloErr)`); message strings are scattered literals. The dual
  `CliError`/`BusErrorKind` taxonomies are **intentional and transitional**
  (retire per the stated Plan 02-09 once tools are rewired) — note, don't
  refactor yet.
- **Format coupling via comment, not type:** `famp-bus/mailbox.rs`'s
  `InMemoryMailbox` hand-mirrors `famp-inbox/read.rs`'s JSONL byte/offset
  accounting (`line.len() + 1`), enforced only by a comment citing line numbers.
  If the disk framing changes, the test double silently drifts.

## 5. On the deferred v1.0 federation crates (not a finding)

`famp-transport` (291), `famp-transport-http` (1008), `famp-keyring` (278) have
zero live callers and are imported as `use famp_transport as _;` to suppress
warnings. This *looks* like dead code but is **deliberately parked** federation
infrastructure (CLAUDE.md: "v1.0 federation internals — don't conflate them with
the primitive layer"), un-parked at the named v1.0 trigger. Parked crates that
compile and stay out of the way produce ~zero ongoing friction, and a
"clean-parking" refactor would be churn against a decision that reverses on
un-park. **Leave the abstraction alone.** Only the two zero-risk hygiene moves
in Quick Wins apply.

## 6. Quick Wins (<30 min each, zero behavior change)

1. **Remove fossil field `ClientState.bus_proto`** (`famp-bus/src/broker/state.rs:10`)
   — written at Hello, validated once, never read again; marked `#[allow(dead_code)]`.
2. **Drop redundant `client` from `ParkedAwait`** (`state.rs:57`) — it duplicates
   the `pending_awaits` map key; store `(AwaitFilter, Instant)`.
3. **Move `handle.rs`'s ~300 LOC of embedded tests** into `broker/handle/tests.rs`
   — shrinks the god module immediately, no logic change.
4. **`famp-transport` → `[dev-dependencies]`** *if* a grep confirms it's
   test-only (it is, per the import audit) — removes a false prod-dependency
   edge from the binary today, no parking-strategy change.
5. **Replace the mailbox offset-mirroring comment with a shared const/helper** —
   extract the `+1` newline accounting both `famp-inbox/read.rs` and
   `InMemoryMailbox` depend on, so format changes can't silently desync the test
   double.

## 7. Suggested Refactoring Roadmap

1. **First, in parallel (independent, mechanical):** §1 `handle.rs` module split
   and §3 `inspect-server` decomposition — both pure code moves, no shared
   surface, do them in separate worktrees.
2. **Then §2 (typed envelope view)** — build `EnvelopeView` in `famp-envelope`,
   property-test it against the existing raw extractors, migrate
   **inspect-server first** (19 of 22 sites; lands cleanly after §3's
   decomposition), then the CLI/MCP sites.
3. **§4 (MCP boilerplate)** last and only if the team wants it — lowest leverage,
   most a matter of taste.
4. **Before §1/§2 touch await/envelope logic:** add a few integration tests over
   the await-drain and message-inspect paths if coverage there is thin, so the
   moves are verifiably behavior-preserving.

Quick wins 1–5 can land anytime; do #3 (move tests) as the first commit of §1.

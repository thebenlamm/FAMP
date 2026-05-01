# BUS-11 — Bus ↔ Federation Translation Table

**Status:** diagnostic doc, pre-Phase-03 (v0.9). Companion to BUS-11 invariant
recorded in `crates/famp-bus/src/lib.rs` and the type-level enforcement in
`crates/famp-envelope/src/bus.rs`.

**Purpose:** map every shape that flows on the local bus to its eventual
federation counterpart (and vice versa) so that when the v1.0 federation
gateway lands, the translation surface is already known. The act of writing
the table is itself the diagnostic — see *Failure-Mode Probes* at the bottom.

**Source of record for the v0.9 wrapper convention this table documents:**
`crates/famp/src/cli/send/mod.rs::build_envelope_value` (the only constructor
of bus-side wrapped envelopes).

---

## The Wrapper Convention (v0.9, what this table maps)

The bus path is non-spec-bearing (BUS-11). v0.9 carries DMs, deliveries, and
channel posts as **unsigned `audit_log` envelopes** with a `famp.send.*`
event prefix and a mode-tagged inner payload under `body.details`:

```json
{
  "famp": "0.5.2",
  "class": "audit_log",
  "scope": "standalone",
  "id": "<uuidv7>",
  "from": "agent:local.bus/<sender>",
  "to":   "agent:local.bus/<recipient-or-channel>",
  "authority": "advisory",
  "ts": "<rfc3339>",
  "body": {
    "event": "famp.send.<mode>",
    "details": {
      "mode": "new_task" | "deliver" | "deliver_terminal" | "channel_post",
      "summary"?: "<from --new-task>",
      "task"?:    "<uuid>",
      "body"?:    "<freeform>",
      "terminal"?: true,
      "more_coming"?: true
    }
  }
}
```

This is bus-only. Federation envelopes never carry a `famp.send.*` event.
Real `audit_log` envelopes (non-`famp.send.*` event) keep their actual
audit-event semantics on both surfaces.

---

## Bus → Federation (PROMOTE direction)

The gateway reads `class: audit_log` + `body.event` to disambiguate.

| # | Bus shape (current) | Federation shape (v1.0 target) | Mapping rule | Notes |
|---|---------------------|-------------------------------|--------------|-------|
| 1 | `audit_log` + `event="famp.send.new_task"` + `body.details = {mode:"new_task", summary?, body?, more_coming?}` | `Request` envelope (`scope=standalone`); `body.scope.instructions = body`; `body.scope.more_coming = more_coming` (omit when false); `body.natural_language_summary = summary`; `body.bounds = <gateway default profile>` | PROMOTE. Documented translation rule, not invented semantics. | The bus has no `bounds` concept; gateway supplies a default-profile `Bounds` (≥2 keys per §9.3). Default profile is gateway-policy, written down. **D1 below.** |
| 2 | `audit_log` + `event="famp.send.deliver"` + `body.details = {mode:"deliver", task, body?}` | `Deliver` envelope (`scope=task`, `task_id = task` from header rewrite); `body.interim = true`; `body.natural_language_summary = body` (or `body.result = {"text": body}` per gateway policy) | PROMOTE. | `interim=true` because the bus mode `deliver` is non-terminal by definition (the user used `--task` without `--terminal`). |
| 3 | `audit_log` + `event="famp.send.deliver_terminal"` + `body.details = {mode:"deliver_terminal", task, body?, terminal:true}` | `Deliver` envelope (`scope=task`, `task_id = task`); `body.interim = false`; envelope `terminal_status = "completed"`; `body.provenance = <gateway-synthesized>` | PROMOTE. **Two synthesized fields. D2 + D3 below.** | The bus has no failure path: `terminal=true` carries no disposition signal. Gateway defaults to `completed`. Gateway synthesizes `provenance` from its own signing identity (the gateway IS the prov surface for bus-originated terminals). |
| 4 | `audit_log` + `event="famp.send.channel_post"` + `body.details = {mode:"channel_post", body?}` | NO FEDERATION COUNTERPART | DROP at gateway. | Channels are bus-local in v0.9. The gateway MUST refuse to forward `famp.send.channel_post` outbound and MUST log the drop. v1.0+ may introduce a federation channel concept; that's a v1.0 design call, not a translation rule. |
| 5 | `audit_log` with `event` NOT prefixed `famp.send.` (real audit event) | `audit_log` envelope unchanged (signed by sender) | PASS-THROUGH (with signing). | Gateway re-signs the body with its own key; the audit event itself is unmodified. |

### Documented synthesis points (D1–D3)

- **D1 (bounds default profile):** gateway provides a `Bounds` value with at
  least two fields set per §9.3. Recommended profile: `hop_limit: 3`,
  `recursion_depth: 3` (purely local-origin tasks; conservative). Documented
  in gateway config.
- **D2 (terminal_status default = `completed`):** bus `terminal=true` is a
  single-bit success signal. Gateway maps to `completed`. **The bus protocol
  has no path to express `failed` or `cancelled` terminal deliveries today.**
  If a bus user needs to express failure, they use `famp send` with a
  prose body explaining the failure; the gateway still maps to `completed`.
  This is a *deliberate v0.9 narrowness*, not a translation defect.
- **D3 (provenance synthesis):** gateway synthesizes `body.provenance` for
  bus-originated terminals using its own identity (gateway is the boundary
  prov surface; the bus origin is unsigned by BUS-11). Documented as
  gateway-as-prov.

None of D1–D3 require the gateway to invent **semantics** absent from the
bus payload — all three are explicit, written translation conventions.

---

## Federation → Bus (DEMOTE direction)

`AnyBusEnvelope` already supports typed `Request`/`Commit`/`Deliver`/`Ack`/
`Control` natively (`crates/famp-envelope/src/bus.rs:93-100`). The gateway
delivers federation envelopes onto the bus as their typed bus variant —
**no audit_log wrapper on inbound**.

| # | Federation shape | Bus shape | Mapping rule | Notes |
|---|------------------|-----------|--------------|-------|
| 6 | `request` (signed) | `BusEnvelope<RequestBody>` (typed, unsigned) | TYPED PASS-THROUGH. Strip `signature`; preserve all body fields verbatim. | BUS-11 requires unsigned on bus. The gateway's signature verification happens *before* placement on the bus. |
| 7 | `commit` (signed) | `BusEnvelope<CommitBody>` | Same. | |
| 8 | `deliver` (signed, any `terminal_status`) | `BusEnvelope<DeliverBody>` | Same. Failure-path deliveries (`terminal_status=failed` with `error_detail`) cross *into* the bus cleanly — the asymmetry from D2 is one-way. | |
| 9 | `ack` (signed) | `BusEnvelope<AckBody>` | Same. | |
| 10 | `control` (signed) | `BusEnvelope<ControlBody>` | Same. | |
| 11 | `audit_log` (signed, non-`famp.send.*` event) | `BusEnvelope<AuditLogBody>` | Same. **MUST distinguish from wrapped DM-as-audit_log** — discrimination is by `event` prefix. | |

The fed→bus direction has **no synthesized fields** — every federation
envelope projects into a typed bus envelope with one operation (signature
strip + class preservation).

---

## Untranslatable / Asymmetric Cases (called out explicitly)

| Case | Direction | Behavior | Why |
|------|-----------|----------|-----|
| Bus channel post | bus→fed | DROPPED at gateway | Channels are v0.9-bus-local by design. |
| Federation `request`/`commit`/`control` | fed→bus | Typed pass-through (lines 6, 7, 10) | These have no v0.9 bus *originator*, but the bus **can carry them inbound** because `AnyBusEnvelope` has the variants. |
| Bus terminal-failure / terminal-cancel | bus→fed | Not expressible | Bus protocol carries `terminal:true` only. Failure must come from federation side. |
| Federation `ack` | fed→bus | Typed pass-through | The bus does not currently *originate* `ack` (bus DM model is fire-and-forget per BUS-11), but inbound `ack` from federation projects cleanly. |

**No case requires the gateway to invent semantics not present in the
payload it is translating.** All gaps are documented translation rules
(D1–D3) or documented drops (channel post).

---

## Failure-Mode Probes (the architect's two triggers)

The architect named two failure modes that, if observed, force Option 2
(introduce `MessageClass::BusDm` via atomic v0.5.3 bump) back onto the table.

### FM-1: gateway must invent semantics not in the bus payload

**Status: NOT TRIGGERED.**

Walking the table:

- Lines 1–3 require gateway-supplied **defaults** (D1 bounds, D2
  terminal_status, D3 provenance). A documented default value is not
  invented semantics — it's a written translation rule, replaceable by
  bus-payload extension when needed.
- Line 4 (channel post) is an explicit drop, not a translation. No
  invention.
- Line 5 (real audit_log) is pass-through; no invention.
- Lines 6–11 (fed→bus) are typed projections with zero synthesized fields.

The call is close on D2 (terminal_status default). Surfacing it for the
original window's judgment: *if* the bus needing to express failed
terminals becomes a real workflow before v1.0, that pressure should
re-open Option 2 — not because the gateway invents (it doesn't), but
because the bus protocol becomes structurally lossy. Today no such
workflow exists; AUDIT-04 / Phase 03 plans don't introduce one.

### FM-2: a v0.9-internal consumer needs to filter audit_log from DMs without inspecting `event`

**Status: NOT TRIGGERED.**

Phase 03 scope per `.planning/ROADMAP.md`:
- `famp install-claude-code` (writes MCP config + slash command files)
- 7 slash commands (`/famp-register`, `/famp-join`, `/famp-leave`,
  `/famp-msg`, `/famp-channel`, `/famp-who`, `/famp-inbox`) — pure MCP-tool
  forwarders
- README 12-line / 30-second Quick Start gate
- `~/.claude/hooks.json` Edit-event runner reading
  `~/.famp-local/hooks.tsv` and dispatching `famp send` (HOOK-04b)

None of these read inbox content and need to discriminate audit_log from
wrapped DMs. The hook runner *produces* `famp send` calls; it doesn't
consume the bus stream. `famp inbox list` already discriminates by
inspecting `body.event` (the existing convention).

If a future phase introduces a bus-stream consumer that needs class-only
discrimination (e.g., a Claude Code log-viewer that filters real audit
events from DMs), FM-2 fires and Option 2 returns.

---

## Sunset Clause (v1.0 trigger)

If, at federation gateway implementation, **any** row in this table
cannot be expressed as a translation rule without hand-waving — meaning
the gateway must invent semantics not present in the bus payload — this
convention is reopened and `MessageClass::BusDm` (or `LocalEnvelope`)
returns via atomic v0.5.x bump.

The forensic indicator is that the gateway implementation needs to
*read* fields it has no source for, not that it needs to *default*
fields it has documented defaults for.

---

## Drift Invariant (for Phase 03 / v0.9 onward)

Any new federation `MessageClass` MUST add a row to this table.

Any new bus-only `event` prefix (anything beyond `famp.send.{new_task,
deliver, deliver_terminal, channel_post}`) MUST document its gateway
behavior here in the same commit that introduces it.

Any second construction site for wrapped-`audit_log` DMs (besides
`build_envelope_value`) MUST be rejected at review unless this table is
amended to acknowledge it.

---

## References

- BUS-11 invariant: `crates/famp-bus/src/lib.rs:11-13`,
  `crates/famp-envelope/src/bus.rs:3-6`
- Type-level enforcement: `crates/famp-envelope/src/bus.rs:16-30`
  (compile_fail gates 1 & 2)
- Wrapper construction site: `crates/famp-envelope/src/cli/send/mod.rs::build_envelope_value`
- Atomic bump precedent: commit `9ca6e13` (Phase 1 plan 01-03)
- Architect counsel (refined verdict, Option 1+):
  `~/.famp/mailboxes/FAMP.jsonl` (second message)
- Local-first bus design spec:
  `docs/superpowers/specs/2026-04-17-local-first-bus-design.md`

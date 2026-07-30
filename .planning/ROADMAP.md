# Roadmap: FAMP

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

## Milestones

- ✅ **v0.5.1 Spec Fork** — Phases 0–1 (shipped 2026-04-13). Interop contract locked; FAMP-v0.5.1-spec.md authoritative. See [milestones/v0.5.1-ROADMAP.md](milestones/v0.5.1-ROADMAP.md).
- ✅ **v0.6 Foundation Crates** — Phases 1–3 (shipped 2026-04-13). Substrate shipped: `famp-canonical`, `famp-crypto`, `famp-core`. 25/25 requirements satisfied, 112/112 tests green. See [milestones/v0.6-ROADMAP.md](milestones/v0.6-ROADMAP.md).
- ✅ **v0.7 Personal Runtime** — Phases 1–4 (shipped 2026-04-14). Minimal usable library on two transports. 4/4 phases, 15/15 plans, 253/253 tests green.
- ✅ **v0.8 Usable from Claude Code** — Phases 1–4 + v0.8.x bridge (shipped 2026-04-26). CLI + daemon + inbox + MCP server + session-bound identity (`famp_register`/`famp_whoami`, `FAMP_LOCAL_ROOT`-only). 5/5 phases, 18/18 plans, 39/39 requirements (37 + 2 bridge), 419/419 tests green. See [milestones/v0.8-ROADMAP.md](milestones/v0.8-ROADMAP.md) · [milestones/v0.8-MILESTONE-AUDIT.md](milestones/v0.8-MILESTONE-AUDIT.md).
- ✅ **v0.9 Local-First Bus** — Phases 1–4 + close-fix Phase 5 (shipped 2026-05-04). UDS-backed broker replacing the per-identity TLS listener mesh; zero crypto on the local path; IRC-style channels; durable per-name mailboxes; 8-tool stable MCP surface. 5/5 phases, 35 plans, **85/85 requirements**, audit `passed`. Federation internals (`famp-transport-http`, `famp-keyring`) preserved in CI via library-API `e2e_two_daemons`; escape-hatch tag `v0.8.1-federation-preserved`. See [milestones/v0.9-ROADMAP.md](milestones/v0.9-ROADMAP.md) · [milestones/v0.9-REQUIREMENTS.md](milestones/v0.9-REQUIREMENTS.md) · [milestones/v0.9-MILESTONE-AUDIT.md](milestones/v0.9-MILESTONE-AUDIT.md).
- ✅ **v0.10 Inspector & Observability** — Phases 1–3 (shipped 2026-05-11). Read-only inspector RPC on the v0.9 broker UDS + `famp inspect` CLI subcommand. Closes the conversation-state opacity gap that produced three recurring v0.9 incidents (orphan socket-holder vs stale PID file, task FSM invisibility, stale-mailbox relays). 26/26 requirements, audit `passed`. See [milestones/v0.10-ROADMAP.md](milestones/v0.10-ROADMAP.md) · [milestones/v0.10-REQUIREMENTS.md](milestones/v0.10-REQUIREMENTS.md) · [milestones/v0.10-MILESTONE-AUDIT.md](milestones/v0.10-MILESTONE-AUDIT.md).
- ✅ **v0.11 Broker Daemon & Cross-Tool Bootstrap** — Phases 4–6 (shipped 2026-06-06). Service-managed daemon (`famp daemon install`) restores the broker-presence guarantee that `56b2293` (correctly) removed; EPERM sandbox diagnostics + daemon install/status/uninstall/restart lifecycle + version-skew detection + daemon-first cross-platform README. 15/15 requirements, audit waived (Phase 6 human-verify E2E). See [milestones/v0.11-ROADMAP.md](milestones/v0.11-ROADMAP.md) · [milestones/v0.11-REQUIREMENTS.md](milestones/v0.11-REQUIREMENTS.md).
- ✅ **v1.0 Federation Profile — Gateway Core** — Phases 7–12 (shipped 2026-07-29, tagged `v1.0.0` at `5edff41`). Gate A fired (Ben's sustained cross-machine use); this milestone closes it: an agent on one of Ben's machines exchanges a signed FAMP envelope with an agent on a second machine he controls, bidirectionally and reliably, over a network he fully controls (direct or a VPN he already runs — no public relay, no cross-person trust). Resolves the broker-liveness fork (same-host `kill(pid,0)` reaping a naively-proxied remote principal), ships `famp-gateway` (Layer 2) wrapping the preserved `famp-transport-http` + `famp-keyring`, signed cross-host envelopes (INV-10 + forward-compat fields), two-machine TOFU key bootstrap, and retires the ~27 parked federation tests (triaged 27/27 RETIRE, with the real Phase 9 E2E pinned into CI in their place). Gate B (conformance vector pack, 2nd implementer) stays event-driven and out of this milestone's scope. **Delivered:** 6 phases (7–12), 29 plans, 29/29 requirements, 106 commits over 7 days; scope grew past the planned Phases 7–10 by two phases — Phase 11 (the Gate A dogfood found no shipping client could address a remote principal, plus 8 setup-guide defects and a `from`-forgery hole) and Phase 12 (design review C's §16 nine-item release checklist). UAT-01 proven live macOS ↔ Linux, terminal COMPLETED on both hosts. See [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md) · [milestones/v1.0-REQUIREMENTS.md](milestones/v1.0-REQUIREMENTS.md).

## Phases

Full phase details (goals, dependencies, success criteria, per-plan lists) live in the
per-milestone archives under [`milestones/`](milestones/) — one `v<X.Y>-ROADMAP.md` per
shipped milestone. This file stays constant-size: collapsed history here, active work
expanded, backlog at the bottom.

<details>
<summary>✅ v1.0 Federation Profile — Gateway Core (Phases 7–12) — SHIPPED 2026-07-29, tagged <code>v1.0.0</code> at <code>5edff41</code></summary>

- [x] Phase 7: Broker-Liveness Fork + Gateway Skeleton (3/3 plans) — completed 2026-07-23
- [x] Phase 8: Signed Cross-Host Envelope + Trust Bootstrap (4/4 plans) — completed 2026-07-23
- [x] Phase 9: End-to-End Cross-Host Delivery (5/5 plans) — completed 2026-07-27
- [x] Phase 10: Test Reactivation + Setup Docs (3/3 plans) — completed 2026-07-27 (`10-VERIFICATION.md` `human_needed` on DOC-04's unassisted-follower clause only; superseded by Phase 11's UAT-01 PASS)
- [x] Phase 11: Shipping-Client Remote Addressing + Setup Hardening (8/8 plans) — completed 2026-07-29
- [x] Phase 12: v1.0.0 Release Gate (5/5 plans) — completed 2026-07-29

Details: [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md) · [milestones/v1.0-REQUIREMENTS.md](milestones/v1.0-REQUIREMENTS.md) · phase artifacts under [milestones/v1.0-phases/](milestones/v1.0-phases/)

</details>

<details>
<summary>✅ v0.11 Broker Daemon & Cross-Tool Bootstrap (Phases 4–6) — SHIPPED 2026-06-06</summary>

- [x] Phase 4: Broker Lifecycle & Bootstrap Diagnostics (3/3 plans) — completed 2026-06-04
- [x] Phase 5: Daemon Service Management & Version Safety (5/5 plans) — completed 2026-06-04
- [x] Phase 6: Onboarding & Cross-Platform Docs (3/3 plans) — completed 2026-06-06

Details: [milestones/v0.11-ROADMAP.md](milestones/v0.11-ROADMAP.md) · [milestones/v0.11-REQUIREMENTS.md](milestones/v0.11-REQUIREMENTS.md)

</details>

<details>
<summary>✅ v0.10 Inspector &amp; Observability (Phases 1–3) — SHIPPED 2026-05-11</summary>

- [x] Phase 1: Broker Diagnosis & Identity Inspection (4/4 plans) — completed 2026-05-10
- [x] Phase 2: Task FSM & Message Visibility (3/3 plans) — completed 2026-05-10
- [x] Phase 3: Load Verification & Integration Hardening (3/3 plans) — completed 2026-05-11

Details: [milestones/v0.10-ROADMAP.md](milestones/v0.10-ROADMAP.md) · [milestones/v0.10-REQUIREMENTS.md](milestones/v0.10-REQUIREMENTS.md)

</details>

<details>
<summary>✅ v0.5.1 → v0.9 (spec fork, foundation crates, personal runtime, Claude Code integration, local-first bus) — SHIPPED 2026-04-13 → 2026-05-04</summary>

Phase numbering reset per milestone through v0.9; each milestone's phases, plans, and
success criteria are preserved in its own archive:

- ✅ v0.9 Local-First Bus — Phases 1–4 + close-fix Phase 5 (35 plans) — [archive](milestones/v0.9-ROADMAP.md) · [reqs](milestones/v0.9-REQUIREMENTS.md) · [audit](milestones/v0.9-MILESTONE-AUDIT.md)
- ✅ v0.8 Usable from Claude Code — Phases 1–4 + v0.8.x bridge (18 plans) — [archive](milestones/v0.8-ROADMAP.md) · [reqs](milestones/v0.8-REQUIREMENTS.md) · [audit](milestones/v0.8-MILESTONE-AUDIT.md)
- ✅ v0.7 Personal Runtime — Phases 1–4 (15 plans) — [archive](milestones/v0.7-ROADMAP.md) · [reqs](milestones/v0.7-REQUIREMENTS.md) · [audit](milestones/v0.7-MILESTONE-AUDIT.md)
- ✅ v0.6 Foundation Crates — Phases 1–3 (9 plans) — [archive](milestones/v0.6-ROADMAP.md) · [reqs](milestones/v0.6-REQUIREMENTS.md) · [audit](milestones/v0.6-MILESTONE-AUDIT.md)
- ✅ v0.5.1 Spec Fork — Phases 0–1 (9 plans) — [archive](milestones/v0.5.1-ROADMAP.md) · [reqs](milestones/v0.5.1-REQUIREMENTS.md) · [audit](milestones/v0.5.1-MILESTONE-AUDIT.md)

</details>

### 📋 Next milestone — not yet defined

No active milestone. Run `/gsd-new-milestone` to open the next one (questioning →
research → requirements → roadmap). Two named gates remain open and event-driven, and
neither is scheduled:

- **Gate B — conformance vector pack.** Fires when a second implementer commits to
  interop. Independent of version number; ships at whatever tag is current. Draft plan:
  `.planning/WRAP-V0-5-1-PLAN.md`; SEED-001 is its RFC 8785 gate (already green in CI).
- **v1.1 sketch — open-internet federation.** Public reachability (relay / NAT
  traversal), cross-person trust bootstrap, signed peer directory, and protocol-grade
  ingress (freshness / replay-cache enforcement, audience binding, DoS ordering,
  revocation). Explicitly deferred out of v1.0 per `milestones/v1.0-REQUIREMENTS.md`.

The 2026-06-08 mesh-VPN "Future Milestone Sketch" that lived in this file is superseded
and preserved in [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md): the tailnet is
no longer the trust boundary, because the shipped gateway verifies Ed25519/INV-10 at the
boundary itself.
## Progress Table

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Canonical JSON Foundations | v0.6 | 3/3 | Complete | 2026-04-13 |
| 2. Crypto Foundations | v0.6 | 3/3 | Complete | 2026-04-13 |
| 3. Core Types & Invariants | v0.6 | 2/2 | Complete | 2026-04-13 |
| 1. Minimal Signed Envelope | v0.7 | 3/3 | Complete | 2026-04-13 |
| 2. Minimal Task Lifecycle | v0.7 | 3/3 | Complete | 2026-04-13 |
| 3. MemoryTransport + TOFU Keyring | v0.7 | 4/4 | Complete | 2026-04-13 |
| 4. Minimal HTTP Transport | v0.7 | 5/5 | Complete | 2026-04-14 |
| 1. Identity & CLI Foundation | v0.8 | 3/3 | Complete | 2026-04-14 |
| 2. Daemon & Inbox | v0.8 | 3/3 | Complete | 2026-04-14 |
| 3. Conversation CLI | v0.8 | 4/4 | Complete | 2026-04-14 |
| 4. MCP Server & Same-Laptop E2E | v0.8 | 3/3 | Complete | 2026-04-15 |
| 1. `famp-bus` library + audit-log MessageClass | v0.9 | 3/3 | Complete | 2026-04-28 |
| 2. UDS wire + CLI + MV-MCP rewire + hook subcommand | v0.9 | 14/14 | Complete | 2026-04-30 |
| 3. Claude Code integration polish | v0.9 | 6/6 | Complete | 2026-05-03 |
| 4. Federation CLI unwire + federation-CI preservation | v0.9 | 8/8 | Complete | 2026-05-04 |
| 5. v0.9 Milestone Close — CC-07 + HOOK-04b + verification backfill | v0.9 | 5/5 | Complete   | 2026-06-04 |
| 1. Broker Diagnosis & Identity Inspection | v0.10 | 4/4 | Complete | 2026-05-10 |
| 2. Task FSM & Message Visibility | v0.10 | 3/3 | Complete | 2026-05-10 |
| 3. Load Verification & Integration Hardening | v0.10 | 3/3 | Complete | 2026-05-11 |
| 4. Broker Lifecycle & Bootstrap Diagnostics | v0.11 | 3/3 | Complete | 2026-06-04 |
| 5. Daemon Service Management & Version Safety | v0.11 | 5/5 | Complete | 2026-06-04 |
| 6. Onboarding & Cross-Platform Docs | v0.11 | 3/3 | Complete | 2026-06-06 |
| 7. Broker-Liveness Fork + Gateway Skeleton | v1.0 | 3/3 | Complete    | 2026-07-23 |
| 8. Signed Cross-Host Envelope + Trust Bootstrap | v1.0 | 4/4 | Complete    | 2026-07-23 |
| 9. End-to-End Cross-Host Delivery | v1.0 | 5/5 | Complete   | 2026-07-27 |
| 10. Test Reactivation + Setup Docs | v1.0 | 3/3 | Complete   | 2026-07-27 |
| 11. Shipping-Client Remote Addressing + Setup Hardening | v1.0 | 8/8 | Complete | 2026-07-29 |
| 12. v1.0.0 Release Gate | v1.0 | 5/5 | Complete | 2026-07-29 |

## Backlog

### Phase 999.1: `famp await` crash safety — cursor advance vs flush ordering (BACKLOG)

**Goal:** [Captured for future planning]
**Requirements:** TBD
**Plans:** 0 plans

**Context:** Surfaced 2026-04-24 while wiring a Claude Code Stop hook that blocks on `famp await --timeout 23h`. Open question: if the `famp await` process is SIGKILL'd (or its parent dies) after the inbox cursor has advanced but before stdout is flushed/consumed by the caller, is the entry lost? Verification test: run `famp await` in a subshell, SIGKILL immediately after a peer sends, then check whether `famp inbox list` still shows the entry. If lost, cursor should only advance after successful flush/ack. Low urgency (single-consumer listeners rarely crash mid-flush) but a real correctness concern for the protocol layer.

Plans:

- [ ] TBD (promote with /gsd:review-backlog when ready)

### Phase 999.2: Multi-listener lock semantics — concurrent `famp await` consumers (BACKLOG)

**Goal:** [Captured for future planning]
**Requirements:** TBD
**Plans:** 0 plans

**Context:** Surfaced 2026-04-24 during adversarial review of the Stop hook listener. If two processes (e.g., two Claude Code windows sharing the same cwd + `.famp-listen` sentinel, or just two shells) both call `famp await` against the same `FAMP_HOME`, what happens? Expected: serialize cleanly via `inbox.lock` so exactly one consumer gets each new entry; the other blocks and awaits the next. Feared: cursor race where both processes read the same entry (duplicate delivery) or one deadlocks. Test plan: spawn two concurrent `famp await` processes against the same FAMP_HOME, have a peer send one envelope, verify exactly one consumer receives it and the other continues blocking. Low near-term priority (single-listener is the current usage pattern) but important before encouraging multi-listener workflows.

Plans:

- [ ] TBD (promote with /gsd:review-backlog when ready)

### Phase 999.3: `heartbeat` envelope class — work-in-progress visibility (BACKLOG)

**Goal:** Define and ship a low-bandwidth `heartbeat` envelope class so a long-running worker can periodically signal "still alive, working on `<one-liner>`" without the originator having to poll. Eliminates the failure mode where 8–15 minute silent gaps in a multi-agent task look indistinguishable from a crashed daemon.

**Requirements:** TBD
**Plans:** 0 plans

**Context:** Surfaced 2026-04-25 during the first 3-agent pressure test. Symptom: agent-a starved 21 minutes watching agent-b silently work on a pressure-tested artifact, then the operator intervened thinking it was stuck. Today there is no protocol-level signal between "actively working" and "crashed mid-task." Proposal: new envelope class `heartbeat` carrying `{ task_id, working_on: <≤120 char string>, ts }`; sender emits at most every N minutes (default 5) or on demand from a hypothetical `famp_status` MCP tool; receiver-side, the originator's `famp_await` surfaces "agent-b heartbeat at HH:MM, working on: ..." rather than rendering silence as suspicious. Sized as substrate work because it touches `famp-envelope` (new MessageClass) and `famp-fsm` (heartbeat is non-state-advancing — does not consume a slot in the 5-state FSM, but the inbox surface treats it like a deliver).

Plans:

- [ ] TBD (promote with /gsd:review-backlog when ready)

### Phase 999.4: `user_attention` envelope class — human-in-loop primitive (BACKLOG)

**Goal:** Define and ship a `user_attention` envelope class so a worker can explicitly mark a task as "blocked pending human input" — distinct from `REQUESTED`, `COMMITTED`, or any of the three terminal states. The inbox surface and orchestrator must render this as a first-class human-action signal, not just another deliver.

**Requirements:** TBD
**Plans:** 0 plans

**Context:** Surfaced 2026-04-25 during the same 3-agent pressure test. Symptom: agent-c (a reviewer-role agent on call) said "this needs the operator" during round-2 escalation; agent-b had no FAMP-native primitive to forward the blocked-on-human state to agent-a (the orchestrator) in a way that would surface differently from a normal reply. Workaround used: a prose-tagged deliver, indistinguishable from any other reply. Proposal: new envelope class `user_attention` carrying `{ task_id, reason: <markdown blob explaining what input is needed>, suggested_actions?: Vec<string> }`; receiver-side, `famp_inbox list` and `famp_await` MUST flag these distinctly (e.g., a separate column or icon). Open design question: does this advance the FSM (new state `BLOCKED_HUMAN`?) or is it a non-state-advancing signal layered on COMMITTED? Likely the latter — keeps the 5-state FSM intact and matches the heartbeat (999.3) pattern.

Plans:

- [ ] TBD (promote with /gsd:review-backlog when ready)

### Phase 999.5: Spec-by-path tracking — `~/Workspace/...` paths in messages (BACKLOG, deferred to v1.0)

**Goal:** Track the spec-by-path gap explicitly so it isn't forgotten before v1.0. The gap is already covered structurally by the v1.0 federation gateway design — this entry exists so there is a discoverable link from the pressure-test findings to the federation work, and so v1.0 planning explicitly verifies the gap is closed.

**Requirements:** TBD
**Plans:** 0 plans

**Context:** Surfaced 2026-04-25 during the first 3-agent pressure test. Symptom: agent-b sent absolute filesystem paths (e.g. `~/Workspace/FAMP/...`, `~/Workspace/<other-project>/...`) inside envelope bodies because the protocol has no native way to address a spec/artifact by content-id or by federation-resolvable URL. Today this works only because all three agents are co-resident on the same Mac with the same `$HOME`. The moment any agent runs cross-host, every such reference is dead. v0.9 (local-first bus, in design at `docs/superpowers/specs/2026-04-17-local-first-bus-design.md`) does NOT address this — it's a same-host design. v1.0's federation gateway is the right home for content-addressable refs (or signed-URL refs) because that's the layer where cross-host trust + transport already exists. **Action for v1.0 planning:** when scoping the federation gateway, include an explicit requirement that an envelope can carry a portable artifact reference (sha256-id or signed URL) and the receiver can dereference it without trusting the sender's filesystem. **Status (2026-07-23):** not picked up by the v1.0 Gateway Core roadmap (Phases 7–10) — those phases carry direct filesystem-independent principal addressing (name/principal, not path) but no portable content-addressable artifact reference. Remains open for v1.1+.

Plans:

- [ ] TBD — to be folded into v1.0 federation gateway scope, NOT promoted independently. (Surface during /gsd:new-milestone for v1.0.)

### Phase 999.7: Broker inspect ingress prioritization (BACKLOG)

**Goal:** Prevent saturated inspect RPC traffic from monopolizing the broker's shared ingress queue before the inspect semaphore is reached.
**Requirements:** TBD
**Plans:** 0 plans

**Context:** Captured 2026-05-13 after adversarial review of the `inspect_load_does_not_starve_bus_messages` flake fix. The current mitigation bounds inspect filesystem dispatch and removes unbounded shed-path reply tasks, but all client frames still share `broker_rx`: inspect `Hello`, `Inspect`, and disconnect frames can fill or monopolize ingress before `Out::InspectRequest` reaches the semaphore. Future planning should evaluate splitting inspect ingress from ordinary bus ingress, classifying inspect frames before the shared broker actor queue, or giving ordinary bus traffic priority/budgeted draining so live `Send`/`Inbox` traffic cannot be delayed by saturated inspect connection churn.

Plans:

- [ ] TBD (promote with /gsd:review-backlog when ready)

### Phase 999.8: Audit-log FSM state handling for inspect tasks (BACKLOG)

**Goal:** Teach `famp inspect tasks` and message metadata to derive stable FSM states from v0.9/v0.10 `audit_log` send envelopes, not only canonical `request|commit|deliver|control` classes.
**Requirements:** TBD
**Plans:** 0 plans

**Context:** Captured 2026-05-13 during adversarial review of inspect load-test hardening. `derive_fsm_state` currently maps canonical envelope classes explicitly, but current local bus sends are encoded as `class: "audit_log"` with `body.event` and `body.details.mode`. Mailbox-only task rows and message metadata can therefore surface `UNKNOWN` for valid local bus task traffic. Future work should add explicit audit-log send-mode handling, e.g. `famp.send.new_task` / `mode: new_task` -> `REQUESTED`, `mode: deliver` -> `COMMITTED`, terminal deliver modes -> `COMPLETED|FAILED|CANCELLED`, with focused unit tests for task rows and message rows.

Plans:

- [ ] TBD (promote with /gsd:review-backlog when ready)

---
*Roadmap updated: 2026-06-03 — v0.11 Broker Daemon & Cross-Tool Bootstrap roadmap created. Three phases (4–6) covering 15/15 requirements: Phase 4 (BLC-01, BLC-02, BOOT-01 — broker lifecycle flag + sandbox diagnostics), Phase 5 (DAEMON-01..06, BOOT-02, VER-01, VER-02 — daemon service lifecycle + version safety), Phase 6 (DOC-01..03 — onboarding docs + cross-platform boundary). Phase 5 guardian plist-review gate is a blocking pre-load requirement. Phase dirs: `.planning/phases/04-*`, `05-*`, `06-*`. Prior milestone: v0.10 Inspector & Observability shipped 2026-05-11 (3/3 phases, 10/10 plans, 26/26 requirements). v0.10 Inspector & Observability recut after matt-essentialist + zed-velocity-engineer review. Three-phase structure: Phase 1 (Broker Diagnosis & Identity Inspection — closes orphan-listener incident class end-to-end, 16 reqs), Phase 2 (Task FSM & Message Visibility — I/O-bound handlers + the budget/cancel reqs that finally have something real to enforce against, 9 reqs), Phase 3 (Load Verification & Integration Hardening, 1 req). 26/26 v1 requirements mapped. Original cut (RPC-foundation-with-stub-handlers in Phase 1, all CLI in Phase 2) rejected as yak-shaving — Phase 1 success criteria around budget+cancel were testing synthetic test-only handlers, not the inspector's real work surface. INSP-RPC-02 reworded from runtime property test to compile-time `&BrokerState` signature + workspace dep-graph gate (`just check-inspect-readonly`). Phase numbering reset to Phase 1 per FAMP convention (v0.7/v0.8/v0.9 each reset; v0.10 follows). Independent of v1.0 federation gates (Gate A: Ben symmetric cross-machine; Gate B: 2nd implementer interop) which were unwelded 2026-05-09 per `docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`. v0.9 Local-First Bus shipped 2026-05-04; v0.8 shipped 2026-04-26; v0.7 shipped 2026-04-14; v0.6 + v0.5.1 shipped 2026-04-13.*

*Roadmap updated: 2026-07-23 — v1.0 Federation Profile — Gateway Core roadmap created. Four phases (7–10), continuing sequential numbering from v0.11's Phase 6 (not reset), covering 13/13 v1 requirements from the 2026-07-23 REQUIREMENTS.md (supersedes the 2026-06-08 mesh-VPN Gate A draft). Foundation-first ordering, each phase gating the next: Phase 7 (LIVE-01, LIVE-02, GW-04 — resolves the broker-liveness fork with the Design-A local-proxy recommendation and stands up the `famp-gateway` skeleton; the spine every later phase depends on), Phase 8 (WIRE-01, WIRE-02, TRUST-01, TRUST-02 — signed cross-host envelope + two-machine TOFU key bootstrap, reusing `famp-crypto`/`famp-canonical`/`famp-keyring` without rebuilding them), Phase 9 (GW-01, GW-02, GW-03 — full bidirectional request→commit→deliver→ack cycle across two machines, proving Phases 7+8 compose), Phase 10 (TEST-01, TEST-02, DOC-04 — reactivates the ~27 parked `crates/famp/tests/_deferred_v1/` tests, lands a live two-process E2E in `just ci`, ships the two-machine setup guide). Scope is deliberately narrow: own-two-machines only (direct or Ben-controlled VPN), no public relay, no cross-person trust, no signed directory, no capability/approval plane — all deferred to v1.1/v2.0 per REQUIREMENTS.md v2 Requirements. Gate B (conformance vector pack) stays independent and out of this milestone. Phase dirs: `.planning/phases/07-*` through `10-*` (to be created at plan-phase time).*

*Roadmap updated: 2026-07-29 — **v1.0 Federation Profile — Gateway Core CLOSED and archived**; `v1.0.0` tagged and pushed at `5edff41`. Shipped 6 phases (7–12), 29 plans, 29/29 requirements — two more phases than the four planned at roadmap creation, both discovery-driven: Phase 11 (the Gate A dogfood found no shipping client could address a remote principal, plus 8 setup-guide defects and a sender-`from` forgery hole) and Phase 12 (design review C's §16 nine-item release checklist). Phase details and the superseded 2026-06-08 mesh-VPN Gate A sketch moved to `milestones/v1.0-ROADMAP.md`; this file collapsed to milestone groupings per the constant-size convention. Backlog (999.x) preserved verbatim below — its phase dirs were restored to `.planning/phases/` after the archiver swept them in with v1.0's. Closeout was `override_closeout`: 42 pre-existing open artifacts acknowledged and deferred, none a v1.0 requirement gap (see STATE.md § Deferred Items). Next milestone not yet defined; Gate B (conformance vector pack) and the v1.1 open-internet sketch both remain event-driven.*

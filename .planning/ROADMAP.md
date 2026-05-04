# Roadmap: FAMP

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

## Milestones

- ✅ **v0.5.1 Spec Fork** — Phases 0–1 (shipped 2026-04-13). Interop contract locked; FAMP-v0.5.1-spec.md authoritative. See [milestones/v0.5.1-ROADMAP.md](milestones/v0.5.1-ROADMAP.md).
- ✅ **v0.6 Foundation Crates** — Phases 1–3 (shipped 2026-04-13). Substrate shipped: `famp-canonical`, `famp-crypto`, `famp-core`. 25/25 requirements satisfied, 112/112 tests green. See [milestones/v0.6-ROADMAP.md](milestones/v0.6-ROADMAP.md).
- ✅ **v0.7 Personal Runtime** — Phases 1–4 (shipped 2026-04-14). Minimal usable library on two transports. 4/4 phases, 15/15 plans, 253/253 tests green.
- ✅ **v0.8 Usable from Claude Code** — Phases 1–4 + v0.8.x bridge (shipped 2026-04-26). CLI + daemon + inbox + MCP server + session-bound identity (`famp_register`/`famp_whoami`, `FAMP_LOCAL_ROOT`-only). 5/5 phases, 18/18 plans, 39/39 requirements (37 + 2 bridge), 419/419 tests green. See [milestones/v0.8-ROADMAP.md](milestones/v0.8-ROADMAP.md) · [milestones/v0.8-MILESTONE-AUDIT.md](milestones/v0.8-MILESTONE-AUDIT.md).
- ✅ **v0.9 Local-First Bus** — Phases 1–4 + close-fix Phase 5 (shipped 2026-05-04). UDS-backed broker replacing the per-identity TLS listener mesh; zero crypto on the local path; IRC-style channels; durable per-name mailboxes; 8-tool stable MCP surface. 5/5 phases, 35 plans, **85/85 requirements**, audit `passed`. Federation internals (`famp-transport-http`, `famp-keyring`) preserved in CI via library-API `e2e_two_daemons`; escape-hatch tag `v0.8.1-federation-preserved`. See [milestones/v0.9-ROADMAP.md](milestones/v0.9-ROADMAP.md) · [milestones/v0.9-REQUIREMENTS.md](milestones/v0.9-REQUIREMENTS.md) · [milestones/v0.9-MILESTONE-AUDIT.md](milestones/v0.9-MILESTONE-AUDIT.md).
- 📋 **v1.0 Federation Profile** — trigger-gated; fires when Sofer (or named equivalent) runs FAMP from a different machine and exchanges a signed envelope. **4-week clock starts at v0.9.0 (2026-05-04).** Cross-host FAMP-over-HTTPS via a `famp-gateway` wrapping the local bus. Agent Cards, causality & replay defense, negotiation, delegation, provenance, extensions, adversarial conformance + Level 2/3 badges + conformance vector pack.

## Phases

*No active phases — between milestones. Next milestone (v1.0) is trigger-gated; run `/gsd-new-milestone v1.0` once the trigger fires.*

<details>
<summary>✅ v0.5.1 Spec Fork (Phases 0–1) — SHIPPED 2026-04-13</summary>

- [x] Phase 0: Toolchain & Workspace Scaffold — completed 2026-04-13
- [x] Phase 1: Spec Fork (FAMP-v0.5.1) — completed 2026-04-13

Archive: [milestones/v0.5.1-ROADMAP.md](milestones/v0.5.1-ROADMAP.md)

</details>

<details>
<summary>✅ v0.6 Foundation Crates (Phases 1–3) — SHIPPED 2026-04-13</summary>

- [x] Phase 1: Canonical JSON Foundations (3/3 plans) — completed 2026-04-13 — SEED-001 resolved, RFC 8785 gate 12/12 green
- [x] Phase 2: Crypto Foundations (4/4 plans) — completed 2026-04-13 — Ed25519 `verify_strict`, §7.1c worked example byte-exact, NIST KATs green
- [x] Phase 3: Core Types & Invariants (2/2 plans) — completed 2026-04-13 — Principal/Instance, UUIDv7 IDs, ArtifactId, 15-category ProtocolErrorKind, AuthorityScope, INV-1..11

Archive: [milestones/v0.6-ROADMAP.md](milestones/v0.6-ROADMAP.md) · Phases: [milestones/v0.6-phases/](milestones/v0.6-phases/)

</details>

<details>
<summary>✅ v0.7 Personal Runtime (Phases 1–4) — SHIPPED 2026-04-14</summary>

- [x] Phase 1: Minimal Signed Envelope (3/3 plans) — completed 2026-04-13 — INV-10 mandatory-signature enforcement, 5 shipped message classes
- [x] Phase 2: Minimal Task Lifecycle (3/3 plans) — completed 2026-04-13 — 5-state TaskFsm, proptest transition legality, compiler-checked terminals
- [x] Phase 3: MemoryTransport + TOFU Keyring + Same-Process Example (4/4 plans) — completed 2026-04-13 — `personal_two_agents` example, 3 adversarial cases on MemoryTransport
- [x] Phase 4: Minimal HTTP Transport + Cross-Machine Example (5/5 plans) — completed 2026-04-14 — axum + rustls + reqwest, `cross_machine_two_agents` example, 3 adversarial cases × 2 transports

Archive: [milestones/v0.7-ROADMAP.md](milestones/v0.7-ROADMAP.md) · Audit: [milestones/v0.7-MILESTONE-AUDIT.md](milestones/v0.7-MILESTONE-AUDIT.md) · Requirements: [milestones/v0.7-REQUIREMENTS.md](milestones/v0.7-REQUIREMENTS.md)

</details>

<details>
<summary>✅ v0.8 Usable from Claude Code (Phases 1–4 + v0.8.x bridge) — SHIPPED 2026-04-26</summary>

- [x] Phase 1: Identity & CLI Foundation (3/3 plans) — completed 2026-04-14 — `famp init`, persistent Ed25519 + TLS, FAMP_HOME override
- [x] Phase 2: Daemon & Inbox (3/3 plans) — completed 2026-04-14 — `famp listen`, durable JSONL inbox with fsync, graceful shutdown
- [x] Phase 3: Conversation CLI (4/4 plans) — completed 2026-04-14 — `famp send/await/inbox/peer add`, task records, TLS TOFU
- [x] Phase 4: MCP Server & Same-Laptop E2E (3/3 plans) — completed 2026-04-15 — `famp mcp` stdio server, E2E-01 automated test, E2E-02 smoke test PASSED
- [x] v0.8.x bridge: Session-bound MCP identity (5/5 plans) — completed 2026-04-26 — `famp_register`/`famp_whoami`, `FAMP_LOCAL_ROOT`-only, pre-registration `not_registered` gating, B-strict variant, two-MCP-server E2E, `await_cmd` FSM advance fix

Archive: [milestones/v0.8-ROADMAP.md](milestones/v0.8-ROADMAP.md) · Requirements: [milestones/v0.8-REQUIREMENTS.md](milestones/v0.8-REQUIREMENTS.md) · Audit: [milestones/v0.8-MILESTONE-AUDIT.md](milestones/v0.8-MILESTONE-AUDIT.md) · Phases: [milestones/v0.8-phases/](milestones/v0.8-phases/)

</details>

<details>
<summary>✅ v0.9 Local-First Bus (Phases 1–4 + close-fix Phase 5) — SHIPPED 2026-05-04</summary>

- [x] Phase 1: `famp-bus` library + audit-log MessageClass (3/3 plans) — completed 2026-04-28 — pure state machine, codec, types, four TDD gates, proptest coverage, atomic v0.5.2 constant bump
- [x] Phase 2: UDS wire + CLI + MV-MCP rewire + hook subcommand (14/14 plans) — completed 2026-04-30 — `famp broker`, top-level CLI surface, MCP rewired to bus (8 tools), `famp-local hook add`, integration tests
- [x] Phase 3: Claude Code integration polish (6/6 plans) — completed 2026-05-03 — `famp install-claude-code`, slash commands, 12-line / 30-second README acceptance gate, Codex parity
- [x] Phase 4: Federation CLI unwire + federation-CI preservation (8/8 plans) — completed 2026-05-04 — top-level CLI removals, `e2e_two_daemons` library-API refactor, `v0.8.1-federation-preserved` tag, migration doc
- [x] Phase 5: Milestone close — CC-07 fix + HOOK-04b path parity + Phase 3 verification backfill (4/4 plans) — completed 2026-05-04 — closes gaps from v0.9-MILESTONE-AUDIT.md (CC-07 BROKEN→satisfied via `famp_peers` projection; HOOK-04b PARTIAL→fully wired via `FAMP_LOCAL_ROOT` parameterization; retroactive `03-VERIFICATION.md`; REQUIREMENTS sweep)

Archive: [milestones/v0.9-ROADMAP.md](milestones/v0.9-ROADMAP.md) · Requirements: [milestones/v0.9-REQUIREMENTS.md](milestones/v0.9-REQUIREMENTS.md) · Audit: [milestones/v0.9-MILESTONE-AUDIT.md](milestones/v0.9-MILESTONE-AUDIT.md)

</details>

<details>
<summary>v0.8 Phase Details (archived)</summary>

See [milestones/v0.8-phases/](milestones/v0.8-phases/) for full plan and summary files.

</details>

<details>
<summary>v0.7 Phase Details (archived)</summary>

### Phase 1: Minimal Signed Envelope
**Goal:** `famp-envelope` encodes, decodes, and signature-verifies every message class the Personal Runtime actually emits, and rejects anything else at the type level.
**Depends on:** v0.6 substrate (`famp-canonical`, `famp-crypto`, `famp-core`)
**Requirements:** ENV-01, ENV-02, ENV-03, ENV-06, ENV-07, ENV-09 (narrowed), ENV-10, ENV-12 (cancel-only), ENV-14, ENV-15 (10 requirements)
**Plans:** 3/3 plans complete
- [x] 01-01-PLAN.md — Crate scaffold + primitive types (class/scope/version/timestamp) + error skeleton + §7.1c vector 0 fixtures on disk
- [x] 01-02-PLAN.md — Sealed BodySchema trait + five shipped body types with ENV-09 and ENV-12 narrowings enforced at the type level
- [x] 01-03-PLAN.md — Type-state UnsignedEnvelope/SignedEnvelope + decode pipeline + AnySignedEnvelope dispatch + vector 0 byte-exact regression + full adversarial + proptest suite

### Phase 2: Minimal Task Lifecycle
**Goal:** The 5-state task FSM (`REQUESTED → COMMITTED → {COMPLETED | FAILED | CANCELLED}`) is compiler-checked and every illegal transition is unreachable, not merely rejected at runtime.
**Depends on:** Phase 1
**Requirements:** FSM-02 (narrowed), FSM-03, FSM-04, FSM-05, FSM-08 (5 requirements)
**Plans:** 3/3 plans complete
- [x] 02-01-PLAN.md — Lift MessageClass + TerminalStatus into famp-core (layering prerequisite for famp-fsm)
- [x] 02-02-PLAN.md — famp-fsm TaskState/TaskFsm engine + deterministic fixture tests (FSM-02, FSM-04, FSM-05)
- [x] 02-03-PLAN.md — FSM-03 consumer stub + FSM-08 proptest Cartesian legality matrix

### Phase 3: MemoryTransport + TOFU Keyring + Same-Process Example
**Goal:** A single developer runs `request → commit → deliver → ack` end-to-end in one binary, signatures verified against a local-file TOFU keyring, and the three adversarial cases fail closed on `MemoryTransport`.
**Depends on:** Phase 2
**Requirements:** TRANS-01, TRANS-02, KEY-01, KEY-02, KEY-03, EX-01, CONF-03, CONF-05, CONF-06, CONF-07 (10 requirements)
**Plans:** 4 plans
- [x] 03-01-PLAN.md — famp-transport: Transport trait + MemoryTransport + test-util feature (TRANS-01, TRANS-02)
- [x] 03-02-PLAN.md — famp-keyring: Keyring + file format + TOFU + --peer flag + round-trip fixture (KEY-01, KEY-02, KEY-03)
- [x] 03-03-PLAN.md — Runtime glue in crates/famp/src/runtime/: RuntimeError + peek_sender + canonical pre-check + recipient cross-check + envelope→FSM adapter
- [x] 03-04-PLAN.md — personal_two_agents example + subprocess test + CONF-05/06/07 adversarial tests + REQUIREMENTS.md KEY-01 D-A1 fix (EX-01, CONF-03, CONF-05, CONF-06, CONF-07)

### Phase 4: Minimal HTTP Transport + Cross-Machine Example
**Goal:** The same signed cycle runs across two processes over HTTPS, bootstrapped from the same TOFU keyring, and the Phase 3 adversarial matrix is extended to `HttpTransport` — no new conformance categories are introduced.
**Depends on:** Phase 3
**Requirements:** TRANS-03, TRANS-04, TRANS-06, TRANS-07, TRANS-09, EX-02, CONF-04 (7 requirements)
**Plans:** 5 plans
- [x] 04-01-PLAN.md — famp-transport-http skeleton: deps + error enums (MiddlewareError, HttpTransportError) + lift peek_sender into famp-envelope
- [x] 04-02-PLAN.md — Server side: build_router + FampSigVerifyLayer (two-phase decode) + RequestBodyLimitLayer + sentinel layering tests (TRANS-04, TRANS-07, TRANS-09 partial)
- [x] 04-03-PLAN.md — Client side: HttpTransport (native AFIT) + tls.rs PEM/rustls helpers + CI no-openssl gate (TRANS-03, TRANS-06)
- [x] 04-04-PLAN.md — cross_machine_two_agents example + fixture certs + subprocess CONF-04 test + same-process safety net (EX-02, CONF-04)
- [x] 04-05-PLAN.md — Promote tests/adversarial.rs to directory module + HTTP adapter + 3 sentinel-checked HTTP rows reusing CONF-07 fixture byte-identically (TRANS-09 complete; CONF-05/06/07 HTTP rows)

</details>

## Future Milestone Sketch (v1.0 Federation Profile)

**Trigger (set 2026-04-27 in v0.9 prep sprint T7; clock started 2026-05-04 at v0.9.0):** Sofer (or named equivalent) runs FAMP from a different machine and exchanges a signed envelope. 4-week clock; if untriggered by 2026-06-01, federation framing is reconsidered. Conformance vector pack ships at the same trigger.

Rough ordering (not committed):

- **v1.0 Identity & Cards** — Agent Card format, federation credential, capability declaration, pluggable trust store, `.well-known` distribution (TRANS-05), SPEC-04..06. Also introduces `famp-gateway` bridging the v0.9 local bus to remote FAMP-over-HTTPS.
- **v1.1 Causality & Replay Defense** — freshness windows, bounded replay cache, idempotency-key scoping, supersession, cancellation-safe send path (TRANS-08), SPEC-07/08
- **v1.2 Negotiation & Commitment** — propose/counter-propose, round limits, capability snapshot binding, conversation FSM
- **v1.3 Delegation** — assist / subtask / transfer forms, transfer timeout, delegation ceiling
- **v1.4 Provenance** — graph, canonicalization, redaction, signed terminal reports
- **v1.5 Extensions** — critical/non-critical registry, INV-9 fail-closed
- **v1.6 Adversarial Conformance + Level 2/3 Badges** — full CONF matrix, stateright model checking, conformance-badge automation

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
| 5. v0.9 Milestone Close — CC-07 + HOOK-04b + verification backfill | v0.9 | 4/4 | Complete | 2026-05-04 |

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

**Context:** Surfaced 2026-04-25 during the first 3-agent pressure test. Symptom: agent-b sent absolute filesystem paths (e.g. `~/Workspace/FAMP/...`, `~/Workspace/<other-project>/...`) inside envelope bodies because the protocol has no native way to address a spec/artifact by content-id or by federation-resolvable URL. Today this works only because all three agents are co-resident on the same Mac with the same `$HOME`. The moment any agent runs cross-host, every such reference is dead. v0.9 (local-first bus, in design at `docs/superpowers/specs/2026-04-17-local-first-bus-design.md`) does NOT address this — it's a same-host design. v1.0's federation gateway is the right home for content-addressable refs (or signed-URL refs) because that's the layer where cross-host trust + transport already exists. **Action for v1.0 planning:** when scoping the federation gateway, include an explicit requirement that an envelope can carry a portable artifact reference (sha256-id or signed URL) and the receiver can dereference it without trusting the sender's filesystem.

Plans:
- [ ] TBD — to be folded into v1.0 federation gateway scope, NOT promoted independently. (Surface during /gsd:new-milestone for v1.0.)

### Phase 999.6: `update_zprofile_init` should sandbox on non-default `FAMP_LOCAL_ROOT` (BACKLOG)

**Goal:** Make `docs/history/v0.9-prep-sprint/famp-local/famp-local`'s `update_zprofile_init` a no-op when `FAMP_LOCAL_ROOT` is not the default (`$HOME/.famp-local`), so test rigs and verification matrices can run `famp-local wire` against a sandboxed state root without contaminating the user's real `~/.zprofile` login hook.

**Requirements:** TBD
**Plans:** 0 plans

**Context:** Surfaced 2026-04-26/27 during v0.9 prep sprint T3 (`260426-s2j`, `identity-of` subcommand) and again during T3.5 (`260426-stp`, `validate_identity_name` drift fix). The T3 verification matrix ran `cmd_wire` with `FAMP_LOCAL_ROOT=$(mktemp -d)` expecting full state isolation, but `cmd_wire` calls `update_zprofile_init "$mesh"` which writes to `~/.zprofile` unconditionally — the executor had to manually restore the user's real 10-agent login hook. T3.5 worked around the issue by also sandboxing `$HOME`, but the underlying script bug stays in `docs/history/v0.9-prep-sprint/famp-local/famp-local`. Proposal: add `[ "$STATE_ROOT" = "$HOME/.famp-local" ] || return 0` (or equivalent) at the top of `update_zprofile_init` so it only writes the login hook for the default state root. Optional refinement: a `FAMP_LOCAL_NO_ZPROFILE=1` opt-out env var for cases where the user wants a sandboxed root *and* a login-hook write (CI smoke tests of full `cmd_wire`). ~5-minute fix, shell-only, low blast radius. Files a real bug Sofer or any second tester would hit if they ran the script's own verification matrix.

Plans:
- [ ] TBD (promote with /gsd:review-backlog when ready)

---
*Roadmap updated: 2026-05-04 — v0.9 Local-First Bus shipped (5 phases, 35 plans, 85/85 reqs, audit `passed`). Active phases section emptied; v0.9 collapsed into <details> alongside v0.5.1/v0.6/v0.7/v0.8. v1.0 trigger 4-week clock starts now (2026-06-01 expiration). Phase numbering remains milestone-local (FAMP convention; v0.7 reset to Phase 1, v0.8 reset to Phase 1, v0.9 reset to Phase 1; v1.0 will reset to Phase 1).*

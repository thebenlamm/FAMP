# Roadmap: FAMP

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

## Milestones

- ✅ **v0.5.1 Spec Fork** — Phases 0–1 (shipped 2026-04-13). Interop contract locked; FAMP-v0.5.1-spec.md authoritative. See [milestones/v0.5.1-ROADMAP.md](milestones/v0.5.1-ROADMAP.md).
- ✅ **v0.6 Foundation Crates** — Phases 1–3 (shipped 2026-04-13). Substrate shipped: `famp-canonical`, `famp-crypto`, `famp-core`. 25/25 requirements satisfied, 112/112 tests green. See [milestones/v0.6-ROADMAP.md](milestones/v0.6-ROADMAP.md).
- ✅ **v0.7 Personal Runtime** — Phases 1–4 (shipped 2026-04-14). Minimal usable library on two transports. 4/4 phases, 15/15 plans, 253/253 tests green.
- ✅ **v0.8 Usable from Claude Code** — Phases 1–4 + v0.8.x bridge (shipped 2026-04-26). CLI + daemon + inbox + MCP server + session-bound identity (`famp_register`/`famp_whoami`, `FAMP_LOCAL_ROOT`-only). 5/5 phases, 18/18 plans, 39/39 requirements (37 + 2 bridge), 419/419 tests green. See [milestones/v0.8-ROADMAP.md](milestones/v0.8-ROADMAP.md) · [milestones/v0.8-MILESTONE-AUDIT.md](milestones/v0.8-MILESTONE-AUDIT.md).
- ✅ **v0.9 Local-First Bus** — Phases 1–4 + close-fix Phase 5 (shipped 2026-05-04). UDS-backed broker replacing the per-identity TLS listener mesh; zero crypto on the local path; IRC-style channels; durable per-name mailboxes; 8-tool stable MCP surface. 5/5 phases, 35 plans, **85/85 requirements**, audit `passed`. Federation internals (`famp-transport-http`, `famp-keyring`) preserved in CI via library-API `e2e_two_daemons`; escape-hatch tag `v0.8.1-federation-preserved`. See [milestones/v0.9-ROADMAP.md](milestones/v0.9-ROADMAP.md) · [milestones/v0.9-REQUIREMENTS.md](milestones/v0.9-REQUIREMENTS.md) · [milestones/v0.9-MILESTONE-AUDIT.md](milestones/v0.9-MILESTONE-AUDIT.md).
- 🚧 **v0.10 Inspector & Observability** — Phases 1–3 (in progress, opened 2026-05-10). Read-only inspector RPC on the v0.9 broker UDS + `famp inspect` CLI subcommand. Closes the conversation-state opacity gap that produced three recurring v0.9 incidents (orphan socket-holder vs stale PID file, task FSM invisibility, stale-mailbox relays). Independent of the v1.0 federation gate. See **Phases** + **Phase Details** below.
- 📋 **v1.0 Federation Profile** — trigger-gated; **two independent ship gates** per [`docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`](../docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md). **Gate A (Gateway):** Ben's sustained symmetric cross-machine FAMP use (~2 weeks) → unlocks `famp-gateway`, reactivates `crates/famp/tests/_deferred_v1/`, tags `v1.0.0`. **Gate B (Conformance):** a 2nd implementer commits to interop → unlocks the conformance vector pack at whatever release tag is current. 4-week clock retired; both gates event-driven.

## Phases

- [x] **Phase 1: Broker Diagnosis & Identity Inspection** — completed 2026-05-10 — `famp.inspect.*` namespace mounted on broker UDS, all three crates (`-proto`, `-client`, `-server`) shipped, `famp inspect broker` and `famp inspect identities` end-to-end (RPC + CLI). Closes the orphan-listener incident class in one merge.
- [ ] **Phase 2: Task FSM & Message Visibility** — `famp inspect tasks` and `famp inspect messages` end-to-end (RPC + CLI). I/O-bound handlers (taskdir + mailbox file walks) gain the 500 ms latency budget and cancellable-handler discipline that the in-memory Phase 1 handlers don't need.
- [ ] **Phase 3: Load Verification & Integration Hardening** — load test proving inspect-call traffic cannot starve bus message throughput (INSP-RPC-05); end-to-end orphan-listener scenario re-exercises Phase 1's `inspect broker` under integration conditions; doc + migration notes.

## Phase Details

### Phase 1: Broker Diagnosis & Identity Inspection
**Goal:** Operator runs `famp inspect broker` and `famp inspect identities` against the v0.9 broker and gets the conversation state needed to retire the orphan-listener incident class — including the load-bearing dead-broker diagnosis (`famp inspect broker` is the one command that must work even when the broker is dead). All three inspector crates (`famp-inspect-proto`, `famp-inspect-client`, `famp-inspect-server`) ship in this phase under workspace dependency-version discipline.
**Depends on:** v0.9 broker (`famp-bus`, `~/.famp/bus.sock`)
**Requirements:** INSP-BROKER-01..04, INSP-IDENT-01..03, INSP-RPC-01, INSP-RPC-02, INSP-CRATE-01, INSP-CRATE-02, INSP-CRATE-03, INSP-CLI-01, INSP-CLI-02, INSP-CLI-03, INSP-CLI-04 (16 requirements)
**Success Criteria** (what must be TRUE):
  1. `famp inspect broker` against a running broker prints `state: HEALTHY` plus pid, socket path, started-at, and build version on a single human-readable line; against a non-running broker it prints exactly one of `DOWN_CLEAN | STALE_SOCKET | ORPHAN_HOLDER | PERMISSION_DENIED` plus the evidence row used to decide. Detection is connect-handshake-based (no PID file): `DOWN_CLEAN` = no socket file; `STALE_SOCKET` = socket file present but `connect()` returns `ECONNREFUSED`; `ORPHAN_HOLDER` = `connect()` succeeds but the listener rejects FAMP's `Hello` (wrong `bus_proto` or non-bus reply); `PERMISSION_DENIED` = `EACCES`. Exit 0 only on `HEALTHY`; exit 1 with diagnosis on stdout for the four down-states.
  2. `famp inspect identities` lists every registered session identity with name, listen-mode, registered-at, last-activity, cwd, and mailbox unread/total/last-sender/last-received-at — and contains zero "double-print" / "received vs surfaced" counters (deferred per Out of Scope; the surface deliberately does not attempt the wrong instrument).
  3. The broker accepts `BusMessage::Inspect { kind, ... }` frames on the existing UDS socket (no separate inspector socket); the dispatch path is read-only by construction — handler signatures take `&BrokerState` (not `&mut`), and `just check-inspect-readonly` fails CI if `famp-inspect-server` transitively imports any mailbox-write, taskdir-write, or broker `&mut self` mutation surface.
  4. Every `famp inspect <subcommand>` accepts `--json` emitting a stable documented JSON shape; default output is fixed-width column-aligned with explicit headers (no Rust `Debug` format); when the broker is not running, `famp inspect identities` exits 1 with stderr `"error: broker not running at <socket-path>"` while `famp inspect broker` continues to work against the dead broker per success criterion 1.
  5. `just check-no-io-in-inspect-proto` (parallel to `check-no-tokio-in-bus`) fails compilation if `famp-inspect-proto` acquires a tokio / axum / reqwest / clap dependency; `cargo tree -p famp-inspect-client` contains no `clap` dependency (linkable by future SPA / `famp doctor` consumers); `cargo tree` shows `famp-inspect-server` linked to the same `famp-canonical`, `famp-envelope`, `famp-fsm` versions as the broker (no Cargo-resolved version skew).
**Plans:** 4 plans
Plans:
- [x] 01-01-PLAN.md — Wave 0: Proto types + state extensions + Wave-0 test scaffolds (famp-inspect-proto crate, BusMessage::Inspect variant, BrokerState::new with started_at, Register cwd/listen extension)
- [x] 01-02-PLAN.md — Wave 1: famp-inspect-server (tokio-free, &BrokerStateView handlers) + famp-inspect-client (UDS, no clap, peer_pid)
- [x] 01-03-PLAN.md — Wave 2: Broker dispatch arm (BusMessage::Inspect → famp-inspect-server) + CLI subcommand scaffolding
- [x] 01-04-PLAN.md — Wave 3: CLI rendering (HEALTHY + 4 down-states + table) + integration tests + 3 just check-* recipes wired into ci:

### Phase 2: Task FSM & Message Visibility
**Goal:** Operator runs `famp inspect tasks` and `famp inspect messages` and gets the FSM and envelope-metadata visibility that v0.9's task-FSM-invisibility and stale-mailbox-relay incidents asked for. This is the phase where the I/O-bound handlers land — taskdir file walks for tasks, mailbox file reads for messages — so it's also the phase where the 500 ms latency budget (INSP-RPC-03) and cancellable-handler discipline (INSP-RPC-04) gain real handlers to enforce against (Phase 1's pure in-memory handlers had nothing to budget or cancel; the budget would have been theater).
**Depends on:** Phase 1
**Requirements:** INSP-TASK-01..04, INSP-MSG-01..03, INSP-RPC-03, INSP-RPC-04 (9 requirements)
**Success Criteria** (what must be TRUE):
  1. `famp inspect tasks` groups by task_id with FSM state + envelope count + last-transition-age, surfaces `task_id == 0` rows in a top-level `--orphans` bucket above the per-task groups, supports `--id <task_id>` for the envelope chain summary and `--id <task_id> --full` whose output piped through `jq` reproduces the canonical JCS bytes that fed each envelope's signature input.
  2. `famp inspect messages --to <name>` returns envelope metadata only — sender, recipient, task_id, MessageClass, FSM state, timestamp, body byte length, body sha256 prefix (first 12 hex chars) — never message bodies; `--tail N` limits to the most-recent N envelopes (default 50).
  3. An I/O-bound inspect handler exceeding the 500 ms default latency budget is dropped at the tokio wrapper layer with a `BudgetExceeded` reply; concurrent bus message delivery on the same socket is unaffected (no queue stall). The budget enforces in `crates/famp/src/cli/broker/` (the tokio wrapper), not inside `famp-bus` (which stays tokio-free per the existing CI gate).
  4. A test issues 1000 concurrent `famp inspect tasks` and `famp inspect messages` calls and cancels them mid-flight; all 1000 close cleanly with no leaked file descriptors, mailbox locks, or in-flight allocations. Verified via `lsof` snapshot before/after the test plus an explicit allocation tracker.
**Plans:** 3 plans
Plans:
- [x] 02-01-PLAN.md — Wave 1: Proto enum reply types + famp-inspect-server TaskSnapshot/MessageSnapshot + sync handlers (D-01/D-02 wire commitment)
- [x] 02-02-PLAN.md — Wave 2: Broker executor spawn_blocking + timeout(500ms) wrapper + lazy taskdir/mailbox pre-read + max_blocking_threads(1024)
- [x] 02-03-PLAN.md — Wave 3: famp inspect tasks/messages CLI + integration tests + 1000-cancel test + nextest.toml serialization

### Phase 3: Load Verification & Integration Hardening
**Goal:** Prove under integration-grade conditions that (a) inspect-call pressure does not starve bus message throughput and (b) the dead-broker diagnosis path actually disambiguates the orphan-socket-holder failure class that produced the v0.9 incident, then ship the docs.
**Depends on:** Phase 2
**Requirements:** INSP-RPC-05 (1 requirement; load test owns this. Phase 1's INSP-BROKER-02..04 + INSP-CLI-04 are re-exercised under E2E integration conditions but ownership stays in Phase 1.)
**Success Criteria** (what must be TRUE):
  1. A sustained load test runs concurrent `famp.inspect.*` calls at saturating rate alongside live bus message traffic; bus message throughput under inspect pressure stays within an explicit, committed percentage of unloaded throughput (target threshold set during plan-phase). No starvation.
  2. An end-to-end orphan-listener scenario test reproduces the v0.9 incident class (a non-FAMP process holds `~/.famp/bus.sock`); `famp inspect broker` correctly reports state `ORPHAN_HOLDER` with the holder PID in the evidence row, exit code 1, diagnosis on stdout — verifying INSP-BROKER-02/03/04 + INSP-CLI-04 ride the full integration path, not just unit tests.
  3. `docs/MIGRATION-v0.9-to-v0.10.md` (or the v0.10 release-notes section of the README) names the new `famp inspect` surface, the four down-state values from `famp inspect broker`, the `--json` shape commitment, and explicitly calls out the read-only discipline + the deferred items (no `--body`, no doctor, no SPA, no double-print counter).
**Plans:** 3 plans
Plans:
- [x] 03-01-PLAN.md — Wave 1: INSP-RPC-05 load test (`inspect_load_test.rs`) + nextest.toml `inspect-subprocess` filter extension
- [x] 03-02-PLAN.md — Wave 1 (parallel): v0.9-incident-class label on existing orphan E2E test + `docs/MIGRATION-v0.9-to-v0.10.md` migration doc
- [ ] 03-03-PLAN.md — Wave 2 (gap closure): saturated direct inspect RPC no-starvation proof for GAP-03-01

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

**Trigger (re-framed 2026-05-09 to two independent ship gates per [`docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`](../docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md)):**

- **Gate A (Gateway):** Ben sustains symmetric cross-machine FAMP use (laptop ↔ home dev server, two equal agents) for ~2 weeks → unlocks `famp-gateway`, reactivates `crates/famp/tests/_deferred_v1/`, tags `v1.0.0`. Activated by Ben's own use case.
- **Gate B (Conformance):** A 2nd implementer commits to interop and exercises the wire format against their own code lineage → unlocks the conformance vector pack at whatever release tag is current. The "Sofer or named equivalent" framing survives only as Gate B's activation condition.

The original 4-week clock has been retired; both gates are event-driven. Conformance vector pack (drafted as `WRAP-V0-5-1-PLAN.md`) ships with Gate B.

Rough ordering inside v1.0+ (not committed):

- **v1.0 Identity & Cards** — Agent Card format, federation credential, capability declaration, pluggable trust store, `.well-known` distribution (TRANS-05), SPEC-04..06. Also introduces `famp-gateway` bridging the v0.9 local bus to remote FAMP-over-HTTPS (Gate A).
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
| 1. Broker Diagnosis & Identity Inspection | v0.10 | 4/4 | Complete | 2026-05-10 |
| 2. Task FSM & Message Visibility | v0.10 | 0/3 | Ready to execute | — |
| 3. Load Verification & Integration Hardening | v0.10 | 2/3 | Gap closure planned | — |

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
*Roadmap updated: 2026-05-10 — v0.10 Inspector & Observability recut after matt-essentialist + zed-velocity-engineer review. Three-phase structure: Phase 1 (Broker Diagnosis & Identity Inspection — closes orphan-listener incident class end-to-end, 16 reqs), Phase 2 (Task FSM & Message Visibility — I/O-bound handlers + the budget/cancel reqs that finally have something real to enforce against, 9 reqs), Phase 3 (Load Verification & Integration Hardening, 1 req). 26/26 v1 requirements mapped. Original cut (RPC-foundation-with-stub-handlers in Phase 1, all CLI in Phase 2) rejected as yak-shaving — Phase 1 success criteria around budget+cancel were testing synthetic test-only handlers, not the inspector's real work surface. INSP-RPC-02 reworded from runtime property test to compile-time `&BrokerState` signature + workspace dep-graph gate (`just check-inspect-readonly`). Phase numbering reset to Phase 1 per FAMP convention (v0.7/v0.8/v0.9 each reset; v0.10 follows). Independent of v1.0 federation gates (Gate A: Ben symmetric cross-machine; Gate B: 2nd implementer interop) which were unwelded 2026-05-09 per `docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`. v0.9 Local-First Bus shipped 2026-05-04; v0.8 shipped 2026-04-26; v0.7 shipped 2026-04-14; v0.6 + v0.5.1 shipped 2026-04-13.*

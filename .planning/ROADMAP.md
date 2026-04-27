# Roadmap: FAMP

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

## Milestones

- ✅ **v0.5.1 Spec Fork** — Phases 0–1 (shipped 2026-04-13). Interop contract locked; FAMP-v0.5.1-spec.md authoritative. See [milestones/v0.5.1-ROADMAP.md](milestones/v0.5.1-ROADMAP.md).
- ✅ **v0.6 Foundation Crates** — Phases 1–3 (shipped 2026-04-13). Substrate shipped: `famp-canonical`, `famp-crypto`, `famp-core`. 25/25 requirements satisfied, 112/112 tests green. See [milestones/v0.6-ROADMAP.md](milestones/v0.6-ROADMAP.md).
- ✅ **v0.7 Personal Runtime** — Phases 1–4 (shipped 2026-04-14). Minimal usable library on two transports. 4/4 phases, 15/15 plans, 253/253 tests green.
- ✅ **v0.8 Usable from Claude Code** — Phases 1–4 + v0.8.x bridge (shipped 2026-04-26). CLI + daemon + inbox + MCP server + session-bound identity (`famp_register`/`famp_whoami`, `FAMP_LOCAL_ROOT`-only). 5/5 phases, 18/18 plans, 39/39 requirements (37 + 2 bridge), 419/419 tests green. See [milestones/v0.8-ROADMAP.md](milestones/v0.8-ROADMAP.md) · [milestones/v0.8-MILESTONE-AUDIT.md](milestones/v0.8-MILESTONE-AUDIT.md).
- 📋 **v0.9 Local-First Bus** *(active 2026-04-27)* — UDS-backed broker replacing the per-identity TLS listener mesh for same-host agents. Zero crypto on the local path; IRC-style channels; durable per-name mailboxes; stable MCP tool surface carried forward to v1.0. **4 phases, 84 requirements** locked to the [design spec phasing](../docs/superpowers/specs/2026-04-17-local-first-bus-design.md): (1) `famp-bus` library + audit-log MessageClass v0.5.2 atomic constant bump, (2) UDS wire + CLI + MV-MCP rewire + `famp-local hook add`, (3) Claude Code integration polish (12-line / 30-second README acceptance gate), (4) federation CLI unwire + `e2e_two_daemons` library-API refactor + `v0.8.1-federation-preserved` tag.
- 📋 **v1.0 Federation Profile** — trigger-gated; fires when Sofer (or named equivalent) runs FAMP from a different machine and exchanges a signed envelope. 4-week clock starts at v0.9.0. Cross-host FAMP-over-HTTPS via a `famp-gateway` wrapping the local bus. Agent Cards, causality & replay defense, negotiation, delegation, provenance, extensions, adversarial conformance + Level 2/3 badges + conformance vector pack.

## Phases

- [ ] **v0.9 Phase 1: `famp-bus` library + audit-log MessageClass** — pure state machine, codec, types, four TDD gates, proptest coverage, atomic v0.5.2 constant bump
- [ ] **v0.9 Phase 2: UDS wire + CLI + MV-MCP rewire + hook subcommand** — `famp broker`, top-level CLI surface, MCP rewired to bus, `famp-local hook add`, integration tests
- [ ] **v0.9 Phase 3: Claude Code integration polish** — `famp install-claude-code`, slash commands, 12-line / 30-second README Quick Start
- [ ] **v0.9 Phase 4: Federation CLI unwire + federation-CI preservation** — top-level CLI removals, `e2e_two_daemons` library-API refactor, `v0.8.1-federation-preserved` tag, migration doc

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

---

<details>
<summary>v0.9 Local-First Bus (active) — Phase Details</summary>

### Phase 1: `famp-bus` library + audit-log MessageClass (v0.5.2 atomic bump)
**Goal:** Ship the protocol-primitive substrate for the local bus — pure state machine, types, codec, in-memory mailbox, four RED-first TDD gates, full proptest coverage — and atomically close the v0.5.1→v0.5.2 spec-vs-constant lag T5 intentionally introduced. Library only: no UDS, no tokio in broker core, no I/O.
**Depends on:** v0.8 substrate (`famp-canonical`, `famp-crypto`, `famp-core`, `famp-fsm`, `famp-envelope` unchanged)
**Requirements:** BUS-01..11 (11), TDD-01..04 (4), PROP-01..05 (5), AUDIT-01..06 (6), CARRY-03, CARRY-04 (28 requirements total)
**Success Criteria** (what must be TRUE):
  1. `cargo test -p famp-bus` is fully green including the four RED-first TDD gates (codec fuzz, drain cursor atomicity, PID reuse race, EOF cleanup mid-await) and all five proptest properties (DM fan-in ordering, channel fan-out, join/leave idempotency, drain completeness, PID-table uniqueness) — verifiable from a fresh checkout.
  2. The pure broker state machine `Broker::handle(from: ClientId, msg: BusMessage) -> Vec<Out>` exercises every `BusMessage` and `BusReply` variant under proptest with zero `tokio`, zero I/O, and zero `RwLock` / `Mutex<HashMap>` on broker state — provable by `cargo tree -p famp-bus` showing no `tokio` dependency in the core module.
  3. The single commit that adds `MessageClass::AuditLog` enum variant + dispatch + `event`/`subject`/`details` body validation MUST also bump `FAMP_SPEC_VERSION` from `"0.5.1"` → `"0.5.2"` in `crates/famp-envelope/src/version.rs`, and the doc-comment lag note T5 deliberately introduced is removed in the same commit. A signed envelope produced after Phase 1 declares conformance to `"0.5.2"` and a receiver MUST store the `audit_log` envelope and MUST NOT emit `ack` (Δ31 normative).
  4. `just ci` is unaffected — every conformance gate that was green at v0.8 close (RFC 8785 byte-exact, §7.1c worked example, RFC 8032 KATs, NIST FIPS 180-2 KATs) is still green; no envelope-type, canonical-JSON, or crypto regression.
  5. Carry-forward debt addressed: TD-4 (broker auto-creates `REQUESTED` task record on inbound request, eliminating the v0.8 receiver-side test seed) is naturally absorbed by the broker state-machine design; TD-7 (Nyquist `VALIDATION.md` for v0.8 phases 02-04 + bridge) is either backfilled in this phase or formally deferred per project policy.
**Plans:** 3 plans
- [x] 01-01-PLAN.md — `famp-bus` crate scaffold + types + codec + InMemoryMailbox + four TDD-RED gates (TDD-01 GREEN; TDD-02/03/04 RED-first scaffolds)
- [ ] 01-02-PLAN.md — Pure `Broker` actor + dispatch + TDD-02/03/04 GREEN + five proptest properties PROP-01..05 GREEN
- [ ] 01-03-PLAN.md — Atomic v0.5.2 bump (single commit): `MessageClass::AuditLog` + body schema + `Relation::Audits` + `AnySignedEnvelope::AuditLog` dispatch + `BusEnvelope<B>` sibling type (BUS-11) + `AnyBusEnvelope` 6-arm dispatch + `UnexpectedSignature` + `FAMP_SPEC_VERSION` flip + T5 doc-comment removal + vector_1 fixture + `just check-spec-version-coherence` CI guard

### Phase 2: UDS wire + CLI + MV-MCP rewire + `famp-local hook add`
**Goal:** Wrap the Phase 1 library in a real wire and a real CLI so a developer can `famp register alice &; famp register bob &; famp send --to bob "hi"` from two terminals on one laptop with no MCP plumbing yet. Rewire `famp mcp` to the bus (drops TLS / `reqwest`), expose the eight-tool stable surface, and ship Sofer's biggest leverage gap as a declarative `famp-local hook add` subcommand.
**Depends on:** Phase 1
**Requirements:** BROKER-01..05 (5), CLI-01..11 (11), MCP-01..10 (10), HOOK-01..04 (4), TEST-01..05 (5), CARRY-02 (36 requirements total)
**Success Criteria** (what must be TRUE):
  1. Shell-level usability works end-to-end: two terminals running `famp register alice` and `famp register bob` (with the broker auto-spawned via `posix_spawn` + `setsid` on first invocation) can exchange `famp send --to <name>` DMs and `famp send --channel <#name>` channel messages, with `famp inbox list`, `famp await`, `famp join`, `famp leave`, `famp sessions`, `famp whoami` all observable from the user's shell.
  2. Single-broker exclusion is provable at the OS level: two near-simultaneous `famp register` invocations produce exactly one surviving broker (TEST-04), `kill -9` mid-`Send` followed by client reconnect recovers the mailbox without loss (TEST-03), 5-minute idle timer triggers a clean shutdown that fsyncs mailbox handles and unlinks `bus.sock`, and a startup warning fires when `~/.famp/` is detected on NFS.
  3. `famp mcp` connects to the UDS bus instead of TLS — `cargo tree -p famp` shows `reqwest` and `rustls` are no longer reached from the MCP startup path; the eight-tool surface (`famp_register`, `famp_send`, `famp_inbox`, `famp_await`, `famp_peers`, `famp_join`, `famp_leave`, `famp_whoami`) round-trips through the MCP E2E harness with two stdio processes scripted from both sides (TEST-05), bus-side equivalent of v0.8's `e2e_two_daemons`. The MCP error-mapping layer is exhaustive `match` over `BusErrorKind` with no wildcard — adding a `BusErrorKind` variant fails compile until MCP error mapping handles it.
  4. `famp-local hook add --on Edit:<glob> --to <peer-or-#channel>` declaratively wires hooks (replacing hand-written bash scripts), persists to `~/.famp-local/hooks.tsv`, and round-trips through `famp-local hook list` + `famp-local hook remove <id>`.
  5. INBOX-01 wording (carry-forward TD-3) is rewritten to match the raw-bytes-per-line implementation (or a structured wrapper added) alongside the CLI inbox rework; `just ci` is full green at every commit.
**Plans:** TBD

### Phase 3: Claude Code integration polish
**Goal:** Make the user-facing onboarding hit the milestone acceptance gate — two Claude Code windows exchange a message in **≤12 lines of instruction and ≤30 seconds elapsed** on a fresh macOS install. This phase is the gate; if the gate fails, the design is too heavy and must be revisited before v0.9.0 tags.
**Depends on:** Phase 2
**Requirements:** CC-01..10 (10 requirements total)
**Success Criteria** (what must be TRUE):
  1. `famp install-claude-code` writes a user-scope MCP config to `~/.claude.json` (or invokes `claude mcp add`) and drops slash-command markdown files into `~/.claude/commands/` — verifiable by inspecting the resulting files and round-tripping through Claude Code without any further manual edits.
  2. The seven slash commands (`/famp-register`, `/famp-join`, `/famp-leave`, `/famp-msg`, `/famp-channel`, `/famp-who`, `/famp-inbox`) each invoke the corresponding MCP tool with the right argument shape — `/famp-register alice` calls `famp_register(name="alice")`, `/famp-msg bob "ship it"` calls `famp_send(to={kind:"agent",name:"bob"}, new_task="ship it")`, etc.
  3. The README Quick Start passes the **12-line / 30-second acceptance test** on a clean macOS install: `brew install famp && famp install-claude-code` followed by two Claude Code windows registering as different identities and exchanging a message — completed in ≤12 user-visible lines and ≤30 seconds wall-clock.
  4. Onboarding doc (`docs/ONBOARDING.md` or equivalent) ships as part of this phase and walks a new user from zero install to first cross-window message; ready to ship at v0.9.0 tag.
**Plans:** TBD
**UI hint**: yes

### Phase 4: Federation CLI unwire + federation-CI preservation
**Goal:** Remove federation-grade plumbing from the user-facing CLI (no more `famp setup` / `famp listen` / `famp init` / `famp peer add` / `famp peer import` / old TLS-form `famp send`), relabel `famp-transport-http` + `famp-keyring` as "v1.0 federation internals" — and **preserve** them in CI so they don't mummify before the v1.0 federation gateway lands. This is the plumb-line-2 commitment against the Architect's "local-case black hole" risk.
**Depends on:** Phase 3
**Requirements:** FED-01..06 (6), MIGRATE-01..04 (4), TEST-06, CARRY-01 (12 requirements total)
**Success Criteria** (what must be TRUE):
  1. `cargo tree` shows the federation crates (`famp-transport-http`, `famp-keyring`) are consumed only by the refactored `e2e_two_daemons` integration test — no top-level CLI subcommand reaches them. The six removed CLI verbs (`famp setup`, `famp listen`, `famp init`, `famp peer add`, `famp peer import`, old TLS-form `famp send`) are gone from `famp --help` output.
  2. `e2e_two_daemons` is refactored to target `famp-transport-http`'s library API directly — instantiates two server instances in-process, exchanges a full signed `request → commit → deliver → ack` cycle over real HTTPS, verifies canonical JSON + Ed25519 end-to-end — and runs green in `just ci` on every commit (FED-04, plumb-line-2 commitment). Conformance gates (RFC 8785, §7.1c) continue running unchanged on every CI run (TEST-06).
  3. Tag `v0.8.1-federation-preserved` is cut on the commit BEFORE Phase 4 deletions land, providing an escape hatch for federation-needed users; `v0.9.0` tag is cut at the end of Phase 4 with `just ci` fully green and `cargo tree -i openssl` empty.
  4. `docs/MIGRATION-v0.8-to-v0.9.md` ships with the CLI mapping table (`famp setup` → `famp register`, `famp listen` → gone, `famp peer add` → gone, etc.), `.mcp.json` cleanup instructions, and `famp install-claude-code` auto-update guidance; `README.md`, `CLAUDE.md`, `.planning/MILESTONES.md` updated so local-first is the headline and federation is the v1.0 promise; `scripts/famp-local` (prep-sprint scaffolding) marked deprecated.
  5. `[[profile.default.test-groups]]` is pinned for listen-subprocess tests (max-threads = 4, carry-forward TD-1) before listen subprocess tests proliferate further — addressed alongside the `e2e_two_daemons` refactor.
**Plans:** TBD

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
**Success Criteria** (what must be TRUE):
  1. `famp-envelope` round-trips every shipped message class (`request`, `commit`, `deliver`, `ack`, `control/cancel`) through `famp-canonical` and `famp-crypto::verify_strict` under a `proptest` generator, byte-exact both directions.
  2. Decoding an envelope without a signature returns a typed `ProtocolError` (INV-10 enforced) — unsigned messages are unreachable from any downstream consumer, not merely logged.
  3. Every body struct uses `#[serde(deny_unknown_fields)]`; adding an unknown key at any depth produces a decode error in a committed fixture test.
  4. `ENV-12 (cancel-only)` is enforced at the type level: there is no constructor or deserialize path that yields a `control` body with `supersede` or `close`; the wider v0.6-catalog form is explicitly gated out for v0.7.
  5. `ENV-09 (narrowed)` contains no capability-snapshot binding; the commit body schema compiles and round-trips without any reference to Agent Cards, and this omission is documented inline with a pointer to v0.8.
**Plans:** 3/3 plans complete
- [x] 01-01-PLAN.md — Crate scaffold + primitive types (class/scope/version/timestamp) + error skeleton + §7.1c vector 0 fixtures on disk
- [x] 01-02-PLAN.md — Sealed BodySchema trait + five shipped body types with ENV-09 and ENV-12 narrowings enforced at the type level
- [x] 01-03-PLAN.md — Type-state UnsignedEnvelope/SignedEnvelope + decode pipeline + AnySignedEnvelope dispatch + vector 0 byte-exact regression + full adversarial + proptest suite

### Phase 2: Minimal Task Lifecycle
**Goal:** The 5-state task FSM (`REQUESTED → COMMITTED → {COMPLETED | FAILED | CANCELLED}`) is compiler-checked and every illegal transition is unreachable, not merely rejected at runtime.
**Depends on:** Phase 1
**Requirements:** FSM-02 (narrowed), FSM-03, FSM-04, FSM-05, FSM-08 (5 requirements)
**Success Criteria** (what must be TRUE):
  1. `TaskFsm` exposes exactly 5 states (1 initial + 1 intermediate + 3 terminals); adding or removing a variant causes a hard compile error in a downstream consumer stub under `#![deny(unreachable_patterns)]` (INV-5, FSM-03).
  2. `FSM-02 (narrowed)` is enforced: no `REJECTED`, no `EXPIRED`, no timeout-driven transitions exist in the public API. The wider v0.6-catalog form is gated out for v0.7.
  3. `proptest` transition-legality tests enumerate the full `(class, relation, terminal_status, current_state)` tuple space and assert: every legal tuple is accepted, every illegal tuple is rejected with a typed error, zero panics.
  4. FSM state types are fully owned (no lifetimes, no `&str`/`&[u8]` in the public enum), so state can be moved across threads and stored without borrow gymnastics.
**Plans:** 3/3 plans complete
- [x] 02-01-PLAN.md — Lift MessageClass + TerminalStatus into famp-core (layering prerequisite for famp-fsm)
- [x] 02-02-PLAN.md — famp-fsm TaskState/TaskFsm engine + deterministic fixture tests (FSM-02, FSM-04, FSM-05)
- [x] 02-03-PLAN.md — FSM-03 consumer stub + FSM-08 proptest Cartesian legality matrix

### Phase 3: MemoryTransport + TOFU Keyring + Same-Process Example
**Goal:** A single developer runs `request → commit → deliver → ack` end-to-end in one binary, signatures verified against a local-file TOFU keyring, and the three adversarial cases fail closed on `MemoryTransport`.
**Depends on:** Phase 2
**Requirements:** TRANS-01, TRANS-02, KEY-01, KEY-02, KEY-03, EX-01, CONF-03, CONF-05, CONF-06, CONF-07 (10 requirements)
**Success Criteria** (what must be TRUE):
  1. `famp-transport` exposes a `Transport` trait (async send + incoming stream); `MemoryTransport` is an in-process implementation (~50 LoC, no network, no TLS) usable as a `dev-dependency` from any crate that needs an end-to-end fixture.
  2. The TOFU keyring is a local-file `HashMap<Principal, VerifyingKey>` where principal = raw 32-byte Ed25519 pubkey. It loads from a one-line-per-principal file (base64url-unpadded) **or** from `--peer <principal>:<pubkey>` CLI flags. There is no Agent Card, no federation credential, no pluggable trust store — that scope is v0.8.
  3. `cargo run --example personal_two_agents` exits `0` and prints a typed conversation trace containing, in order, a signed `request`, `commit`, `deliver`, and `ack` over `MemoryTransport` (CONF-03).
  4. The three adversarial cases (CONF-05 unsigned / CONF-06 wrong-key / CONF-07 canonical divergence) each fail closed with a distinct typed `ProtocolError` when injected into `MemoryTransport`; no panics, no silent drops, no generic `Error::Other`.
  5. The keyring file format is round-trip tested (load → save → load produces byte-identical bytes) and committed as a fixture.
**Plans:** 4 plans
- [x] 03-01-PLAN.md — famp-transport: Transport trait + MemoryTransport + test-util feature (TRANS-01, TRANS-02)
- [x] 03-02-PLAN.md — famp-keyring: Keyring + file format + TOFU + --peer flag + round-trip fixture (KEY-01, KEY-02, KEY-03)
- [x] 03-03-PLAN.md — Runtime glue in crates/famp/src/runtime/: RuntimeError + peek_sender + canonical pre-check + recipient cross-check + envelope→FSM adapter
- [x] 03-04-PLAN.md — personal_two_agents example + subprocess test + CONF-05/06/07 adversarial tests + REQUIREMENTS.md KEY-01 D-A1 fix (EX-01, CONF-03, CONF-05, CONF-06, CONF-07)

### Phase 4: Minimal HTTP Transport + Cross-Machine Example
**Goal:** The same signed cycle runs across two processes over HTTPS, bootstrapped from the same TOFU keyring, and the Phase 3 adversarial matrix is extended to `HttpTransport` — no new conformance categories are introduced.
**Depends on:** Phase 3
**Requirements:** TRANS-03, TRANS-04, TRANS-06, TRANS-07, TRANS-09, EX-02, CONF-04 (7 requirements)
**Success Criteria** (what must be TRUE):
  1. `famp-transport-http` exposes an axum `POST /famp/v0.5.1/inbox` endpoint per principal and a `reqwest` client send path, both running on `rustls` via `rustls-platform-verifier` (no OpenSSL), with a 1 MB request-body limit enforced as a `tower` layer (TRANS-03/04/06/07).
  2. Signature verification runs as HTTP middleware **before** routing (TRANS-09): unsigned or wrong-key requests are rejected at the tower layer and never reach handler code — verified by a test that asserts the handler closure is not entered on an adversarial case.
  3. Running `cross_machine_two_agents` as a server in one shell and a client in another completes a signed `request → commit → deliver → ack` cycle over real HTTPS, with both ends loading the other's pubkey from a local keyring file or `--peer` flag (CONF-04, EX-02). Exit code `0`.
  4. **The existing Phase 3 adversarial matrix is extended to `HttpTransport`.** The same three cases (unsigned / wrong-key / canonical divergence) that passed on `MemoryTransport` now also fail closed on HTTP with the same typed errors — six test rows total across the two transports, derivative of CONF-05/06/07, no new CONF-0x requirements introduced in this phase.
  5. `.well-known` Agent Card distribution (TRANS-05) and the cancellation-safe spawn-channel send path (TRANS-08) are explicitly absent; the crate compiles and the examples run without them, and their omission is documented inline with a pointer to v0.8+.
**Plans:** 5 plans
- [x] 04-01-PLAN.md — famp-transport-http skeleton: deps + error enums (MiddlewareError, HttpTransportError) + lift peek_sender into famp-envelope
- [x] 04-02-PLAN.md — Server side: build_router + FampSigVerifyLayer (two-phase decode) + RequestBodyLimitLayer + sentinel layering tests (TRANS-04, TRANS-07, TRANS-09 partial)
- [x] 04-03-PLAN.md — Client side: HttpTransport (native AFIT) + tls.rs PEM/rustls helpers + CI no-openssl gate (TRANS-03, TRANS-06)
- [x] 04-04-PLAN.md — cross_machine_two_agents example + fixture certs + subprocess CONF-04 test + same-process safety net (EX-02, CONF-04)
- [x] 04-05-PLAN.md — Promote tests/adversarial.rs to directory module + HTTP adapter + 3 sentinel-checked HTTP rows reusing CONF-07 fixture byte-identically (TRANS-09 complete; CONF-05/06/07 HTTP rows)

</details>

## Future Milestone Sketch (v1.0 Federation Profile)

**Trigger (set 2026-04-27 in v0.9 prep sprint T7):** Sofer (or named equivalent) runs FAMP from a different machine and exchanges a signed envelope. 4-week clock starts at v0.9.0; if untriggered, federation framing is reconsidered. Conformance vector pack ships at the same trigger.

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
| 1. `famp-bus` library + audit-log MessageClass | v0.9 | 0/3 | Not started | — |
| 2. UDS wire + CLI + MV-MCP rewire + hook subcommand | v0.9 | 0/0 | Not started | — |
| 3. Claude Code integration polish | v0.9 | 0/0 | Not started | — |
| 4. Federation CLI unwire + federation-CI preservation | v0.9 | 0/0 | Not started | — |

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

**Goal:** Make `scripts/famp-local`'s `update_zprofile_init` a no-op when `FAMP_LOCAL_ROOT` is not the default (`$HOME/.famp-local`), so test rigs and verification matrices can run `famp-local wire` against a sandboxed state root without contaminating the user's real `~/.zprofile` login hook.

**Requirements:** TBD
**Plans:** 0 plans

**Context:** Surfaced 2026-04-26/27 during v0.9 prep sprint T3 (`260426-s2j`, `identity-of` subcommand) and again during T3.5 (`260426-stp`, `validate_identity_name` drift fix). The T3 verification matrix ran `cmd_wire` with `FAMP_LOCAL_ROOT=$(mktemp -d)` expecting full state isolation, but `cmd_wire` calls `update_zprofile_init "$mesh"` which writes to `~/.zprofile` unconditionally — the executor had to manually restore the user's real 10-agent login hook. T3.5 worked around the issue by also sandboxing `$HOME`, but the underlying script bug stays. Proposal: add `[ "$STATE_ROOT" = "$HOME/.famp-local" ] || return 0` (or equivalent) at the top of `update_zprofile_init` so it only writes the login hook for the default state root. Optional refinement: a `FAMP_LOCAL_NO_ZPROFILE=1` opt-out env var for cases where the user wants a sandboxed root *and* a login-hook write (CI smoke tests of full `cmd_wire`). ~5-minute fix, shell-only, low blast radius. Files a real bug Sofer or any second tester would hit if they ran the script's own verification matrix.

Plans:
- [ ] TBD (promote with /gsd:review-backlog when ready)

### Phase 1: Session-bound MCP identity

**Goal:** Two Claude Code (or Codex) windows opened in the same repo can register as different FAMP identities and exchange messages successfully, without per-window startup configuration encoding the identity. Delivers `famp_register` / `famp_whoami` MCP tools, pre-registration `not_registered` gating on the four messaging tools, removal of `FAMP_HOME` from the `famp mcp` startup path, `scripts/famp-local` migration (auto-rewrite-on-touch), and a two-MCP-server E2E test.
**Spec:** [`docs/superpowers/specs/2026-04-25-session-bound-identity-selection.md`](../docs/superpowers/specs/2026-04-25-session-bound-identity-selection.md) — pull-forward of the v0.9 `famp_register` / `famp_whoami` MCP contract onto the v0.8 substrate. Adopted variant: **B-strict** (no `legacy_famp_home` grace period; `FAMP_HOME` no longer pre-binds the MCP server; pre-registration tool calls return typed `not_registered`; `famp-local wire`/`mcp-add` stop emitting `FAMP_HOME` and auto-rewrite existing entries on re-touch).
**Requirements:** MCP-07..16, E2E-04 (numbering continues v0.8 MCP-01..06 / E2E-01..03).
**Depends on:** v0.8 milestone (shipped 2026-04-15) — `famp mcp` server, `famp-inbox` PID+liveness lock, `famp-local` wrapper.
**Plans:** 5/5 complete (2026-04-26) — 14 commits, 419/419 tests green (+53 new), clippy clean, B-strict invariants verified.

Plans:
- [x] 01-01-PLAN.md — CliError variants (NotRegistered/UnknownIdentity/InvalidIdentityName) + exhaustive mcp_error_kind arms + cli::mcp::session module skeleton — completed 2026-04-26
- [x] 01-02-PLAN.md — Internal refactor: server::run drops home arg, dispatch_tool reads session::current(), four messaging tools take &IdentityBinding, NotRegistered gating with pinned hint string — completed 2026-04-26
- [x] 01-03-PLAN.md — famp_register + famp_whoami tool implementations, six-tool tool_descriptors(), dispatch routes register/whoami pre-binding; mcp::mod::run reads FAMP_LOCAL_ROOT only (FAMP_HOME removed from MCP startup path) — completed 2026-04-26
- [x] 01-04-PLAN.md — scripts/famp-local migration: cmd_wire/cmd_mcp_add stop emitting FAMP_HOME; cmd_wire auto-rewrites legacy .mcp.json files in place idempotently; Rust integration test driving bash — completed 2026-04-26
- [x] 01-05-PLAN.md — Two-MCP-server E2E test (full request→commit→deliver→terminal cycle through two windows registered as different identities); README onboarding rewrite with v0.9 sunset callout; bonus fix: await_cmd/mod.rs terminal-deliver FSM advance — completed 2026-04-26

---
*Roadmap updated: 2026-04-27 — v0.9 Local-First Bus phase structure locked. 4 phases, 84 requirements, 100% coverage, milestone-local phase numbering (v0.9 resets to Phase 1 per FAMP convention; v0.7 reset to Phase 1, v0.8 reset to Phase 1). Phase shape locked by [`docs/superpowers/specs/2026-04-17-local-first-bus-design.md`](../docs/superpowers/specs/2026-04-17-local-first-bus-design.md) "Phasing" section + [`.planning/V0-9-PREP-SPRINT.md`](V0-9-PREP-SPRINT.md) T9. v0.5.1/v0.6/v0.7/v0.8 entries above are LOCKED — completed-milestone history.*

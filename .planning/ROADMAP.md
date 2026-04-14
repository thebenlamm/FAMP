# Roadmap: FAMP

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

## Milestones

- ✅ **v0.5.1 Spec Fork** — Phases 0–1 (shipped 2026-04-13). Interop contract locked; FAMP-v0.5.1-spec.md authoritative. See [milestones/v0.5.1-ROADMAP.md](milestones/v0.5.1-ROADMAP.md).
- ✅ **v0.6 Foundation Crates** — Phases 1–3 (shipped 2026-04-13). Substrate shipped: `famp-canonical`, `famp-crypto`, `famp-core`. 25/25 requirements satisfied, 112/112 tests green. See [milestones/v0.6-ROADMAP.md](milestones/v0.6-ROADMAP.md).
- ✅ **v0.7 Personal Runtime** — Phases 1–4 (shipped 2026-04-14). Minimal usable library on two transports. 4/4 phases, 15/15 plans, 253/253 tests green.
- 🔨 **v0.8 Usable from Claude Code** — Phases 1–4 (in progress). Turn v0.7's proven substrate into a `famp` CLI + persistent-state daemon + file inbox + MCP server so two Claude Code sessions can drive two local agents through one long task.
- 📋 **v0.9+ Federation Profile** — Identity & Cards, Causality, Negotiation, Delegation, Provenance, Extensions, Adversarial Conformance.

## Phases

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

### 🔨 v0.8 Usable from Claude Code (Current)

- [ ] **Phase 1: Identity & CLI Foundation** - One-line description: `famp init` creates persistent identity in `~/.famp/`; the CLI binary replaces the 8-line placeholder with real subcommand dispatch.
- [ ] **Phase 2: Daemon & Inbox** - `famp listen` wraps the v0.7 HTTP transport with a durable JSONL inbox; inbound signed messages land on disk with fsync guarantees.
- [ ] **Phase 3: Conversation CLI** - `famp send`, `famp await`, `famp inbox`, and `famp peer add` deliver the one-long-task conversation shape through the CLI; local task records persist across restarts.
- [ ] **Phase 4: MCP Server & Same-Laptop E2E** - `famp mcp` exposes Claude Code tools over stdio; two live Claude Code sessions drive two daemons through ≥4 deliver exchanges and close the task.

---

## Milestone v0.8: Usable from Claude Code

### Phase Details

### Phase 1: Identity & CLI Foundation
**Goal**: A developer can run `famp init` on a fresh laptop and get a fully wired persistent identity — Ed25519 keypair, self-signed TLS cert, config, and peer list — ready to be used by every subsequent subcommand.
**Depends on**: v0.7 substrate (`famp-crypto`, `famp-core`, `famp-keyring`, `famp-transport-http`)
**Requirements**: CLI-01, CLI-07, IDENT-01, IDENT-02, IDENT-03, IDENT-04, IDENT-05, IDENT-06
**Success Criteria** (what must be TRUE):
  1. Running `famp init` on a machine with no `~/.famp/` directory produces: a `key.ed25519` (0600 permissions, 32 raw bytes), a `pub.ed25519`, a `tls.cert.pem` + `tls.key.pem` (self-signed via `rcgen`), a `config.toml` with `listen_addr = "127.0.0.1:8443"`, and an empty `peers.toml` — and exits 0.
  2. Running `famp init` a second time without `--force` exits non-zero with a human-readable error rather than silently overwriting the existing keypair.
  3. Setting `FAMP_HOME=/tmp/test-famp famp init` creates the identity directory at the override path, not at `~/.famp/`; every other subcommand reads from the same override.
  4. Running any subcommand against a missing or incomplete `FAMP_HOME` (e.g., missing `key.ed25519`) produces a typed error identifying exactly which file is absent or malformed, and exits non-zero.
  5. Private key bytes never appear in stdout, stderr, logs, or any error message emitted by the CLI.
**Plans**: 3 plans
- [x] 01-01-PLAN.md — CLI scaffold: deps, cli module tree, CliError, FAMP_HOME resolver, Config/Peers serde types, perms helpers (wave 1)
- [x] 01-02-PLAN.md — `famp init` impl: TLS generator, atomic --force replace, init::run_at, cli dispatcher, bin rewrite (wave 2)
- [x] 01-03-PLAN.md — Integration tests + load_identity + compile_fail doc-test on FampSigningKey (wave 3)

### Phase 2: Daemon & Inbox
**Goal**: A running `famp listen` process accepts inbound signed messages over HTTPS, persists each one durably to a JSONL inbox, and shuts down cleanly — all without any change to the v0.7 wire protocol or transport code.
**Depends on**: Phase 1
**Requirements**: CLI-02, DAEMON-01, DAEMON-02, DAEMON-03, DAEMON-04, DAEMON-05, INBOX-01, INBOX-02, INBOX-03, INBOX-04, INBOX-05
**Success Criteria** (what must be TRUE):
  1. `famp listen` starts, prints its bound address to stderr, and accepts `POST /famp/v0.5.1/inbox/{principal}` requests using the keypair and TLS cert from `~/.famp/` — no manual flag wiring required.
  2. A signed message sent to a running daemon appears as a well-formed JSONL line in `~/.famp/inbox.jsonl` within the same HTTP response cycle; a hard kill of the daemon immediately after a 200 response leaves that line intact on disk (fsync guarantee).
  3. Starting a second `famp listen` instance while one is already bound to the same port exits non-zero with a typed error rather than silently binding to a random port or hanging.
  4. Sending `SIGINT` or `SIGTERM` to a running daemon causes it to stop accepting new connections, flush any buffered inbox writes, and exit 0 within a few seconds.
  5. A truncated or malformed JSONL line in the inbox (simulating a mid-write crash) does not prevent subsequent `famp await` or `famp inbox` calls from reading the lines that follow it.
**Plans**: 3 plans
- [x] 02-01-PLAN.md — famp-inbox crate: durable JSONL append with fsync, tail-tolerant reader (wave 1)
- [x] 02-02-PLAN.md — famp listen command: tokio daemon, FampSigVerifyLayer reuse, inbox-append handler, graceful shutdown (wave 2)
- [x] 02-03-PLAN.md — Integration tests: smoke, SIGKILL durability, bind collision, SIGINT shutdown, truncated tail (wave 3)

### Phase 3: Conversation CLI
**Goal**: A developer can open a task, exchange multiple `deliver` messages within it across two terminal sessions, and close it with a terminal deliver — all through CLI commands — with task state persisted to disk and surviving daemon restarts.
**Depends on**: Phase 2
**Requirements**: CLI-03, CLI-04, CLI-05, CLI-06, CONV-01, CONV-02, CONV-03, CONV-04, CONV-05, INBOX-02, INBOX-03, INBOX-05
**Success Criteria** (what must be TRUE):
  1. `famp send --new-task "hello" --to alice` sends a signed `request` envelope to the named peer, creates `~/.famp/tasks/<uuid>.toml` with state `REQUESTED`, and prints the task-id to stdout.
  2. `famp send --task <id> --to alice` sends a `deliver` envelope within an existing task; the task record stays in its non-terminal state; calling this multiple times in sequence all succeed (the long-task shape works).
  3. `famp send --task <id> --terminal --to alice` sends the final `deliver`, the local task record transitions to `COMPLETED` via the v0.7 `famp-fsm` engine, and any subsequent `famp send --task <id>` call exits non-zero with a typed "task already terminal" error.
  4. `famp await --timeout 30s` blocks up to 30 seconds and returns structured output (task-id, from, message class, body) when a new inbox entry arrives; returns a typed timeout error if none arrives within the window.
  5. Task records survive a daemon restart: after `famp listen` is stopped and restarted, `famp send --task <id>` still finds the task record and sends correctly.
**Plans**: 4 plans
- [x] 03-01-PLAN.md — Storage primitives + REQUIREMENTS.md cleanup: famp-taskdir crate, InboxCursor, real PeerEntry schema, fix Phase 2 frontmatter labels (wave 1)
- [x] 03-02-PLAN.md — famp send (new-task / deliver / terminal modes) + famp peer add + TOFU TLS pinning + FSM glue (wave 2)
- [ ] 03-03-PLAN.md — famp await (poll-with-timeout) + famp inbox list/ack + read_from helper + duration parsing (wave 3)
- [ ] 03-04-PLAN.md — InboxLock advisory + 3 end-to-end conversation tests (full lifecycle, restart safety, lock contention) (wave 4)

### Phase 4: MCP Server & Same-Laptop E2E
**Goal**: Two Claude Code sessions on the same laptop — each pointing at its own `famp` daemon via an MCP server — can open one task, exchange ≥4 `deliver` messages driven by actual LLM conversation, and close the task with a terminal deliver that transitions COMPLETED on both sides.
**Depends on**: Phase 3
**Requirements**: MCP-01, MCP-02, MCP-03, MCP-04, MCP-05, MCP-06, E2E-01, E2E-02, E2E-03
**Success Criteria** (what must be TRUE):
  1. `famp mcp` runs as a stdio JSON-RPC server; adding it to `.mcp.json` as a local server and calling `famp_send` from Claude Code's tool panel opens a new task and returns the task-id without any shell access.
  2. All four MCP tools (`famp_send`, `famp_await`, `famp_inbox`, `famp_peers`) return structured responses; tool errors carry a `famp_error_kind` discriminator (e.g., `peer_not_found`, `task_terminal`, `timeout`) rather than an opaque `anyhow` string, so Claude Code can react to the error type.
  3. The automated integration test (`E2E-01`) — two `famp` daemons on different loopback ports, each with the other as a peer — runs the full `request → commit → deliver × N → terminal deliver → ack` lifecycle through the CLI under `cargo nextest` and exits 0.
  4. The manual witnessed smoke test (`E2E-02`) is completed: two live Claude Code sessions on the same laptop exchange ≥4 `deliver` messages driven by actual LLM conversation and close the task; the outcome is recorded in the phase verification document.
  5. `just ci` passes green with all 253 v0.7 tests still passing and `cargo tree -i openssl` returning empty (no new OpenSSL or native-tls dependencies introduced).
**Plans**: TBD
**UI hint**: yes

---

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

## Future Milestone Sketch (Federation Profile)

Rough ordering, not committed:

- **v0.9 Identity & Cards** — Agent Card format, federation credential, capability declaration, pluggable trust store, `.well-known` distribution (TRANS-05), SPEC-04..06
- **v0.10 Causality & Replay Defense** — freshness windows, bounded replay cache, idempotency-key scoping, supersession, cancellation-safe send path (TRANS-08), SPEC-07/08
- **v0.11 Negotiation & Commitment** — propose/counter-propose, round limits, capability snapshot binding, conversation FSM
- **v0.12 Delegation** — assist / subtask / transfer forms, transfer timeout, delegation ceiling
- **v0.13 Provenance** — graph, canonicalization, redaction, signed terminal reports
- **v0.14 Extensions** — critical/non-critical registry, INV-9 fail-closed
- **v0.15 Adversarial Conformance + Level 2/3 Badges** — full CONF matrix, stateright model checking, conformance-badge automation, `famp` CLI

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
| 1. Identity & CLI Foundation | v0.8 | 0/3 | Planned | - |
| 2. Daemon & Inbox | v0.8 | 0/3 | Planned | - |
| 3. Conversation CLI | v0.8 | 0/? | Not started | - |
| 4. MCP Server & Same-Laptop E2E | v0.8 | 0/? | Not started | - |

---
*Roadmap updated: 2026-04-14 — v0.8 Usable from Claude Code roadmap created (4 phases, 37 requirements, 100% coverage). v0.7 Personal Runtime shipped (4/4 phases, 15/15 plans, 253/253 tests).*

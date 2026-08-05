# FAMP — Federated Agent Messaging Protocol (Reference Implementation)

## What This Is

A Rust implementation of FAMP (Federated Agent Messaging Protocol), staged in **two profiles** so a single developer can get a usable library before the full federation-grade semantics are built out.

1. **Personal Profile (v0.6 + v0.7 + v0.8 + v0.9)** — the minimum usable library *and* the local-first bus, CLI, and Claude Code integration that make it actually usable from a terminal. v0.6 shipped byte-exact canonical JSON, Ed25519-signed envelopes with domain separation, and the core types. v0.7 shipped the five-state task lifecycle, `MemoryTransport` and a minimal HTTP transport. v0.8 wrapped that substrate in a federation-style CLI and MCP server. v0.9 replaced the same-host listener mesh with a UDS-backed local bus while preserving the federation internals for v1.0.

2. **Federation Profile (v1.0+)** — adds the semantics that matter at ecosystem scale: Agent Cards + federation credentials, negotiation/counter-proposal, the three delegation forms, provenance graphs, an extensions registry, the adversarial conformance matrix, and Level 2 + Level 3 conformance badges.

The signing substrate is the same in both profiles. Canonicalization, signing, and core types are done once and correctly in v0.6; Personal Profile exercises that substrate against a minimal runtime in v0.7; Federation Profile stacks ecosystem semantics on top without re-deriving the interop bytes.

## Core Value

**A byte-exact, signature-verifiable FAMP substrate a single developer can use from their own code today, and two independent parties can interop against later.** If canonicalization or signature verification disagrees, nothing else matters — so Personal Profile exercises the same signing contract Federation Profile will depend on.

## Current Milestone: v1.1 Open-Internet Federation

**Goal:** Two **different people** exchange signed FAMP envelopes over the open internet. v1.0 proved the gateway spine on two machines Ben controls, on a network he controls, with hand-copied keys. v1.1 removes all three of those crutches.

**Acceptance (an event, not a person — v1.0's lesson):** an agent on Ben's machine and an agent on a second person's machine, in different networks with **no shared VPN** and **no hand-copied keys**, exchange signed envelopes in **both** directions and both task FSMs reach a terminal state. That person follows a doc **unassisted** — Ben does not sit on a call with them.

**Target features:**
- **Public reachability** — relay and/or NAT traversal. The model is decided **first**, in a zero-code Phase-1 spike, because it carries real infra cost and ownership: self-hosted relay VM vs. hosted tunnel service vs. avoiding a relay entirely (direct-dial + port forward / an existing tunnel). Deliverable is a recorded decision with cost/month and named operator, not code.
- **Cross-person trust bootstrap.** The v1.0 mechanism (`famp peer export` → paste over Signal → `famp peer import`) is the thing that will actually fail with a real human. **This is the hard problem, not the transport.**
- **Prebuilt-binary distribution** (added 2026-08-02, Ben approved). Today's only install path is `cargo install famp` — a full Rust toolchain and 15 compiled crates. A fresh machine has neither, which makes the Human Acceptance Gate's fresh-machine validation unreachable as things stood. DIST-01..05: a tag-triggered release workflow, checksummed prebuilt binaries (macOS arm64/x86_64, Linux x86_64), one documented install command, docs leading with that path over `cargo install`.
- **Protocol-grade ingress at the boundary** — freshness / bounded replay cache, audience binding, DoS ordering, key revocation. All four were explicitly deferred out of v1.0 as open-internet concerns; this is where they come due. The v1.0 envelope already reserves the `nonce` and `expiry` fields these need.
- **Inbound message content is DATA, not instructions — BLOCKING SECURITY GATE**, settled before any outside person connects. Delivers machine-checkable, fail-closed provenance at every rendering surface (structural quarantine with untrusted-origin marking at the MCP + CLI output layer, adversarial corpus in CI), **FAMP-side and harness-agnostic**, not as a prompt convention and not in `~/.claude` wiring — a harness-layer boundary is untestable in FAMP's CI and silently fails to protect Codex/Grok/other clients. **This is the prerequisite for enforcement, not enforcement itself.** An independent adversarial review found the proposed harness-level tool-gating mechanism has structural bypasses (non-MCP render paths, provenance laundering across one unhooked peer); Ben resolved this 2026-08-02 to Option B — provenance + honest docs now, no tool-gating in v1.1. See REQUIREMENTS.md's resolved scope decision and `docs/QUARANTINE.md`.
- **SEED-002 — harness-adapter push notification** (`famp watch --notify`, replacing the `famp await` long-poll + `.famp-listen` sentinel + global Stop-hook trick). Promoted into v1.1 scope: a stranger's agent waking reliably on inbound messages is part of the unassisted-follower experience, and the blocking Stop-hook convention is the brittlest part of onboarding someone new.

**Explicitly NOT in scope:**
- **The FAMP-Sec capability / approval / tool-admission plane** (v2.0+, demand-gated). v1.1 stays conversation-only — no remote-triggered tools.
- **Gate B's conformance vector pack** — independent and event-driven; it fires when a second implementer commits to interop, not on this milestone's schedule.

**Constraints:**
- **Layer 0 primitives stay untouched** — `famp-canonical`, `famp-crypto`, `famp-core`, `famp-envelope`, `famp-fsm`. The nonce/expiry/capability/approval fields v1.0 reserved are already there — **use them**; do not add a second signature or a parallel envelope type.
- **The human acceptance gate is scheduled EARLY in the phase order, not last.** v1.0's single most important finding (no shipping client could address a remote principal) arrived at the final human gate after four phases of green CI. A second person is lined up, so the real-person gate can sit at Phase 2–3.

**Status (2026-07-30): milestone opened. Phase numbering continues from v1.0's Phase 12 → v1.1 starts at Phase 13.**

---

## Shipped Milestone: v1.0 Federation Profile — Gateway Core — SHIPPED 2026-07-29

**Goal:** An agent on one of Ben's machines exchanges a signed FAMP envelope with an agent on a second machine he controls, bidirectionally and reliably, over a network he fully controls (direct or a VPN he already runs — **no public relay**). This is the thin cross-host slice that closes **Gate A** and tags `v1.0.0`.

**Delivered:** 6 phases (7–12), 29 plans, 29/29 requirements, 106 commits over 7 days. `v1.0.0` annotated and pushed at `5edff41` with 12/12 CI check-runs green at that exact SHA. Planned as Phases 7–10; two phases were added by discovery, not scope creep — Phase 11 (the Gate A dogfood found no *shipping* client could address a remote principal, plus 8 setup-guide defects and a sender-`from` forgery hole) and Phase 12 (design review C's §16 nine-item release checklist). See [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md) · [milestones/v1.0-REQUIREMENTS.md](milestones/v1.0-REQUIREMENTS.md) · [MILESTONES.md](MILESTONES.md).

**Why own-machines-first:** the gateway plus the broker-liveness fix are already one hard thing. Proving them with hand-copied keys and full network control means that when v1.1 adds public-internet reachability and cross-person trust, any failure is unambiguously in the *new* layer, not the spine. (Full arc: `~/.claude/plans/first-work-out-the-nested-star.md`.)

**Target features:**
- Resolve the **broker-liveness fork** — the current same-host `kill(pid,0)` reaps a naively-proxied cross-host principal. Design A (local-proxy: gateway backs each remote principal with a `bind_as` connection reporting its own live local PID; zero `famp-bus` change) is recommended; Design B (heartbeat/lease) is the fallback. **This decision gates the whole build.**
- Build **`famp-gateway`** (Layer 2) wrapping the preserved `famp-transport-http` + `famp-keyring`, bridging the local UDS bus to a remote gateway over signed HTTPS envelopes.
- **Reactivate** `crates/famp/tests/_deferred_v1/` (~27 parked federation tests) — triage, wire what still describes real behavior, retire the obsolete.
- **Full INV-10 envelope signing** on the cross-host path; extend `famp-envelope` with the forward-compat fields v1.1 needs (sender/receiver domain + key_id, nonce, expiry), capability/approval fields absent (omitted-when-empty).

**Explicitly NOT v1.0** (deferred to v1.1 / v2.0+ per the roadmap): public-internet relay, cross-person trust bootstrap, the signed peer directory, and the entire FAMP-Sec capability/approval/tool-admission plane.

**Status (2026-07-29): milestone goal MET; `v1.0.0` tagged, pushed, and archived.**
All four target features shipped — the broker-liveness fork resolved via Design A
(Phase 7), `famp-gateway` built (Phases 7–10), the deferred federation test debt
triaged and retired with the real E2E pinned into CI in its place (Phase 10), and
full INV-10 signing with forward-compat federation fields on the cross-host path.
Phase 11 closed the blocker Gate A exposed — a *shipping* client
(`famp send --to agent:<domain>/<name>`), not a hand-written injector, now drives
the cross-host cycle — and fixed two trust-boundary holes (broker-side `from`
forgery; gateway ingress/egress destination + metadata binding) **before** the
tag. Phase 12 closed design review C's §16 nine-item checklist, including an
independent adversarial source pass that found and fixed a real timestamp
defect and a silently-dropped route.

Proven live 2026-07-29 (UAT-01): macOS ↔ Linux over Tailscale, task
`019fab97-…` reached a terminal COMPLETED state on **both** hosts, bidirectional,
`sig_verified: true`. Record: `milestones/v1.0-phases/11-…/11-HUMAN-UAT.md`.

**One honest caveat carried forward:** Phase 10's `10-VERIFICATION.md` is still
`human_needed` and `10-HUMAN-UAT.md` `failed` on DOC-04's *unassisted-follower*
clause. That failure is what scoped Phase 11, whose UAT-01 re-ran the same Gate A
dogfood to a recorded PASS — superseded, not open. Left as-is rather than
back-dated, because Phase 11 is what closed it.

## Requirements

### Validated

- [x] Rust toolchain bootstrap (install rustup, pin toolchain, workspace scaffold) — *Validated in Phase 00: toolchain-workspace-scaffold*
- [x] Fork spec to `FAMP-v0.5.1-spec.md` and resolve identified ambiguities/bugs (canonical JSON, body schemas, state-machine holes) — *Validated in v0.5.1 milestone (Phase 01: spec-fork). 1038-line spec, 28 changelog entries, 21/21 spec-lint anchors green, worked Ed25519 example byte-exact from external reference per PITFALLS P10.*
- [x] `famp-canonical` — RFC 8785 JCS canonicalization with external-vector conformance gate — *Validated in Phase 01: canonical-json-foundations. 12/12 conformance tests green (Appendix B/C/E byte-exact, 100K cyberphone float corpus, UTF-16 supplementary, duplicate-key rejection). SEED-001 resolved: keep `serde_jcs 0.2.0`. CI gate + nightly 100M full-corpus workflow live; fallback plan on disk as insurance.*
- [x] `famp-crypto` — Ed25519 sign/verify with domain-separation prefix, `verify_strict`-only — *Validated in Phase 02: crypto-foundations. 7/7 truths verified. Ed25519 sign/verify with SPEC-03 domain-separation prefix, `verify_strict`-only (raw `verify` unreachable), weak-key rejection at ingress, base64url-unpadded strict codec, RFC 8032 KAT gate, §7.1c worked-example byte-exact interop gate, SHA-256 content-addressing via `sha2 0.11` (CRYPTO-07), constant-time verify via `subtle`. 24/24 nextest + clippy clean.*
- [x] `famp-core` — shared types, typed error enum, INV-1..11 scaffolding — *Validated in Phase 03: core-types-invariants. 10/10 must-haves verified. Principal/Instance identity, UUIDv7 ID newtypes, ArtifactId with `sha256:<hex>` invariant (CORE-01..03); 15-variant flat `ProtocolErrorKind` with wire-string round-trip and ProtocolError wrapper (CORE-04); `invariants::INV_1..INV_11` namespaced doc anchors (CORE-05); `AuthorityScope` 5-variant enum with hand-written 5×5 `satisfies()` truth table, no `Ord` derive (CORE-06); exhaustive consumer stub under `#![deny(unreachable_patterns)]` making new variants a hard compile error (SC #3/#5). 66/66 famp-core + 112/112 workspace nextest green.*

### Active — v1.1 Open-Internet Federation — IN PROGRESS ◆

Detailed requirements: see `.planning/REQUIREMENTS.md`. Eight requirement areas:

- [ ] **Public reachability model** — decided in a zero-code Phase-13 spike (cost/month + named operator recorded as a decision), then implemented.
- [ ] **Cross-person trust bootstrap** — replaces hand-copied `peer export`/`import`; must survive a real human who is not Ben.
- [ ] **Prebuilt-binary distribution** — tag-triggered release workflow, checksummed binaries, no-Rust-toolchain install (DIST-01..05).
- [ ] **Protocol-grade ingress** — freshness / bounded replay cache, audience binding, DoS ordering, key revocation.
- [x] **Auto-wake gate** — a remote-origin envelope never auto-wakes a parked `famp await`, enforced broker-side (QUAR-12..15). *Validated in Phase 19: Auto-Wake Gate.*
- [ ] **Inbound-content-is-DATA provenance** — FAMP-side structural quarantine, harness-agnostic, adversarial corpus in CI. Prerequisite for enforcement, not a steering boundary itself (Ben resolved to Option B, 2026-08-02 — see REQUIREMENTS.md). **Blocking before any outside person connects.**
- [ ] **SEED-002 push-notification harness adapter** — `famp watch --notify`, retiring the `famp await` poll + Stop-hook convention as the primary wake path.
- [ ] **Human acceptance gate (early)** — second person, own machine, own network, no shared VPN, no hand-copied keys, doc-only, bidirectional, both FSMs terminal.

### Active — Personal Profile (v0.6 + v0.7) — COMPLETE ✓

**v0.6 Foundation Crates — substrate: COMPLETE ✓**

**v0.7 Personal Runtime — minimal usable library: COMPLETE ✓**
- [x] `famp-envelope` — signed envelope with INV-10 enforcement; body schemas for `request`, `commit`, `deliver`, `ack`, `control/cancel` only — *Validated in Phase 01: minimal-signed-envelope. 5/5 must-haves verified, 73/73 nextest green, §7.1c vector-0 byte-exact on both canonical JSON (324 B) and Ed25519 signature (64 B). Sealed `BodySchema` trait + 5 body types, `UnsignedEnvelope`/`SignedEnvelope` type-state (INV-10 at the type level via compile_fail doctests), `deny_unknown_fields` at depth, ENV-12 cancel-only enforced as single-variant enum, ENV-09 narrowed (no `capability_snapshot`).*
- [x] Minimal task lifecycle FSM: `REQUESTED → COMMITTED → {COMPLETED | FAILED | CANCELLED}` (5 states, compiler-checked terminals) — *Validated in Phase 02: minimal-task-lifecycle. 4/4 must-haves verified. `famp-fsm` `TaskFsm` engine, FSM-03 compile-time exhaustiveness gate, FSM-08 2048-case proptest matrix.*
- [x] `famp-transport` trait + `MemoryTransport` (in-process) — *Validated in Phase 03. `Transport` trait async send + recv, in-process implementation under `crates/famp-transport/src/memory.rs`.*
- [x] Trust-on-first-use keyring — local `HashMap<Principal, VerifyingKey>`, principal = raw Ed25519 pubkey — *Validated in Phase 03. `famp-keyring` with file format, `--peer` flag, round-trip fixture.*
- [x] `famp-transport-http` — axum `POST /famp/v0.5.1/inbox/{principal}`, reqwest client, rustls (D-B5 full: platform verifier + extra anchor), 1 MB body limit, two-phase decode signature-verification middleware running BEFORE routing — *Validated in Phase 04. TRANS-03/04/06/07/09 satisfied; TRANS-05/08 explicitly deferred to v0.8+.*
- [x] `famp/examples/personal_two_agents.rs` — end-to-end signed cycle in one binary via MemoryTransport — *Validated in Phase 03 (CONF-03, EX-01).*
- [x] `famp/examples/cross_machine_two_agents.rs` — same flow over real HTTPS using fixture certs and TOFU keyring — *Validated in Phase 04 (CONF-04, EX-02). Same-process HTTPS test owns the CONF-04 gate; subprocess test #[ignore]'d due to bootstrap chicken-and-egg (deferred CLI flag).*
- [x] Adversarial matrix: 3 cases × 2 transports = 6 rows (CONF-05/06/07 across MemoryTransport + HttpTransport) — *Validated in Phases 03+04. Byte-identical CONF-07 fixture reused; HTTP rows include sentinel proof that handler closure is not entered.*

**v0.7 totals:** 4/4 phases, 15/15 plans, 32/32 requirements, 253/253 tests green, `cargo tree -i openssl` empty.

### Shipped — v1.0 Federation Profile — Gateway Core — COMPLETE ✓ (shipped 2026-07-29)

Closed **Gate A**: cross-host signed messaging between two machines Ben controls, driven by the shipping client. 6 phases (7–12), 29 plans, 29/29 requirements. Detailed requirements: see `.planning/milestones/v1.0-REQUIREMENTS.md`.

- [x] Phase 7: Broker-Liveness Fork + Gateway Skeleton — *Validated 2026-07-23. Design A landed with **zero `famp-bus` change**: `ProxiedPrincipal::register` backs each remote principal with a no-spawn UDS `Register` carrying the gateway's own PID, so the broker's unmodified `kill(pid,0)` sweep can't reap a cross-host holder. Proven by a unit test (N clients on one PID survive a live Tick, reap together), a real-process SIGKILL/reap subprocess test, and a cross-talk-isolation test against a genuine `famp-gateway` OS process. LIVE-01/02, GW-04.*
- [x] Phase 8: Signed Cross-Host Envelope + Trust Bootstrap — *Validated 2026-07-23. Seven optional omit-when-empty federation fields ride the one existing INV-10 signature — local-bus bytes byte-identical, no wire break. `FampSigningKey::generate()` (OsRng-only) + 16-char b64url `key_id()`; `verify_inbound` as the single pure trust decision routing exclusively through `TrustedVerifyingKey`/`verify_strict`; `famp peer export/import` mutual TOFU via a hand-copied 3-field blob backed by a new generate-once-and-persist gateway keypair at `~/.famp/gateway/identity.ed25519` (0600). WIRE-01/02, TRUST-01/02.*
- [x] Phase 9: End-to-End Cross-Host Delivery — *Validated 2026-07-27. `run_ingress` (axum + rustls; TLS is channel encryption only, `verify_inbound_any` remains the sole trust gate) + per-principal `run_egress` compose under one `tokio::select!`. The two-process loopback E2E closing `request → commit → deliver → ack` to a converged terminal FSM state on both sides caught a real BUS-11 ingress bug that had made **every** relayed envelope permanently unreadable on the receiving side. GW-01/02/03.*
- [x] Phase 10: Test Reactivation + Setup Docs — *Validated 2026-07-27 (`human_needed` on DOC-04's unassisted-follower clause only — superseded by Phase 11's UAT-01 PASS). All 27 parked `_deferred_v1/` tests triaged 27/27 RETIRE with a per-file rationale ledger (every one depended on a v0.9-deleted CLI symbol with no live rewrite target); two falsification-tested guards pin the real Phase 9 E2E into the default nextest set instead; `docs/GATEWAY-SETUP.md` gated by two accuracy tests that extract flags from the live CLI. TEST-01/02, DOC-04.*
- [x] Phase 11: Shipping-Client Remote Addressing + Setup Hardening — *Validated 2026-07-29, 7/7 must-haves. `famp send --to agent:<domain>/<name>` emits the domain-qualified envelope the gateway relays, class branched by send mode so the FSM can terminate through the CLI/MCP surface, `--to bob` byte-identical to before; one host-level federation authority (`--domain` > `FAMP_OWN_DOMAIN` > `$FAMP_HOME/own-domain`) makes the `from`-authority == pinned-label invariant structural. Broker binds envelope `from` to the authenticated identity (forgery rejected pre-insertion); gateway rejects — never signs — mismatched-authority egress, non-own-domain ingress, and client-supplied federation metadata; route map fail-closed on ambiguity. All 8 dogfood doc findings corrected. UAT-01 PASS live macOS ↔ Linux. ADDR-01..03, OBS-01, SEC-01..04, DOC-05, TEST-03, UAT-01.*
- [x] Phase 12: v1.0.0 Release Gate — *Validated 2026-07-30, 9/9 must-haves. Two-reviewer independent adversarial pass over shipped `v1.0.0-rc.1` found and fixed a real timestamp-validation defect in `federation_format_ok` and a silently-dropped gateway route; 10/10 findings dispositioned. Send-confirmation boundary pinned across guide/`--help`/README by one non-vacuous test. §16's proposed limitation wording re-evaluated against ADDR-02 and **dropped rather than shipped stale**. Version bumped atomically to `1.0.0` across 13 manifests + lock + banner; `v1.0.0` tagged at `5edff41` with 12/12 check-runs green re-queried at that SHA. REL-01..05.*

### Shipped — v0.11 Broker Daemon & Cross-Tool Bootstrap — COMPLETE ✓ (shipped 2026-06-06)

Restored guaranteed broker presence via a user-level service-managed daemon so FAMP works for both Claude Code and Codex without per-user broker babysitting. 3 phases (4–6), 11 plans, 15/15 requirements. Detailed requirements: see `.planning/milestones/v0.11-REQUIREMENTS.md`.

- [x] Phase 4: Broker Lifecycle & Bootstrap Diagnostics — *Validated 2026-06-04. `famp broker --no-idle-exit` ships with paused-time regression coverage; default 300s idle-exit behavior remains intact; SandboxEperm diagnostics surface the sandbox cause plus `famp daemon install` remedy through both CLI and MCP; deployed `~/.cargo/bin/famp` refreshed via `just install`; full Phase 04 gate passed outside the sandbox (lib 157/157, broker_lifecycle 6/6, broker_spawn_race 1/1). BLC-01/02, BOOT-01.*
- [x] Phase 5: Daemon Service Management & Version Safety — *Validated 2026-06-04. `famp daemon install/uninstall/status/restart` cross-platform lifecycle (launchd LaunchAgent macOS, systemd `--user` Linux), idempotent, on the guardian-approved plist shape; three-state `status` (exit 0/2/1); `restart` picks up a replaced binary via `kickstart -k`; install refuses inside a sandbox (BOOT-02); workspace unified to 0.11.0 with reconciled `-V`/banner/handshake and connect-time `ProtocolMismatch` skew detection. DAEMON-01..06, VER-01/02. DAEMON-06 Linux behavioral acceptance deferred to a Linux host (05-HUMAN-UAT.md).*
- [x] Phase 6: Onboarding & Cross-Platform Docs — *Validated 2026-06-06. README rewritten daemon-first: one-command `famp daemon install` quickstart, `famp broker --no-idle-exit` no-install bridge, explicit `## Platform support` boundary, five reconciled downstream sections, v0.9→v0.11 refresh. Verified live against the installed binary via a human-verify E2E (fresh-clone Claude+Codex delivery + full daemon lifecycle); accuracy gate caught and fixed a stale-binary idempotency failure and a `status` exit-code drift. DOC-01/02/03.*

### Shipped — v0.10 Inspector & Observability — COMPLETE ✓ (shipped 2026-05-11)

Read-only inspector surface on the v0.9 broker, consumed by a `famp inspect` CLI subcommand. Closes the conversation-state opacity gap that produced three recurring v0.9 incidents (orphan socket-holder vs stale PID file, task FSM invisibility, stale-mailbox relays). Independent of the v1.0 federation gate. Detailed requirements: see `.planning/REQUIREMENTS.md` v0.10 section.

- [x] Phase 1: Broker Diagnosis & Identity Inspection — *Validated 2026-05-10. `famp inspect broker` and `famp inspect identities` ship end-to-end over the v0.9 broker UDS; all three inspector crates are present; read-only, no-I/O proto, and dependency-version gates pass; `just test` passed 601/601 with 2 skipped.*
- [x] Phase 2: Task FSM & Message Visibility — *Validated 2026-05-10. `famp inspect tasks` and `famp inspect messages` end-to-end; 500ms latency budget (INSP-RPC-03) and cancellable-handler discipline (INSP-RPC-04) enforced; 1000-concurrent-cancel test passing. INSP-TASK-01..04, INSP-MSG-01..03, INSP-RPC-03, INSP-RPC-04 satisfied.*
- [x] Phase 3: Load Verification & Integration Hardening — *Validated 2026-05-11. GAP-03-01 closed: non-blocking bounded inspect dispatch (MAX_CONCURRENT_INSPECT_REQUESTS=1, Semaphore fast-shed) + saturated direct-RPC load test; ratio 0.82–1.01 vs prior 0.17; STARVATION_THRESHOLD=0.80 locked. Migration guide ships at `docs/MIGRATION-v0.9-to-v0.10.md`. INSP-RPC-05 satisfied.*

### Deferred — Federation Profile (v1.0+)

These are tracked in `REQUIREMENTS.md` but are **not v1-blocking**. They matter at ecosystem scale, not for a personally-usable library.

- **Agent Card + federation credential + trust registry** — Personal Profile uses a local pubkey keyring; Federation Profile adds the card format, self-signature resolution, capability declaration, and pluggable trust store.
- **`famp-causality` beyond `in_reply_to`** — freshness windows, bounded replay cache, supersession, idempotency-key scoping all defer.
- **Negotiation / counter-proposal / round limits** (`famp-protocol`) — Personal Profile uses direct `request → commit`; no `propose` body.
- **Three delegation forms** (`assist`, `subtask`, `transfer`) + transfer timeout + delegation ceiling — defer entire `famp-delegate` crate.
- **Provenance graph** (`famp-provenance`) — deterministic construction, redaction, signed terminal reports all defer.
- **Extensions registry** (`famp-extensions`) — critical/non-critical classification, INV-9 fail-closed. Defer.
- **HTTP transport — Agent-Card-aware pieces only.** Personal V1 ships a minimal HTTP binding (inbox endpoint, reqwest client, rustls, sig-verification middleware). Deferred: `.well-known` Agent Card distribution (TRANS-05), cancellation-safe spawn-channel send path (TRANS-08).
- **Adversarial conformance matrix** — replay, stale commit, canonical divergence, silent delegation, competing commits, round overflow, drop-at-every-await, key rotation. Personal Profile ships a minimal 3-case negative suite only.
- **`stateright` model checking** — defer; `proptest` transition-legality tests are sufficient for Personal Profile.
- **Level 2 (Conversational) + Level 3 (Task-capable) conformance badges** — defer to Federation Profile. Personal Profile is not a conformance-release target.
- **CLI (`famp keygen`, `famp envelope sign`, `famp serve`, …)** — library-first; CLI lands with Federation Profile.

### Out of Scope (permanent)

- **Python/TypeScript bindings in v1** — core must be proven first; bindings follow as separate milestone
- **Additional transports (libp2p, NATS, WebSocket)** — `Transport` trait leaves them open; not v1
- **Multi-party commitment profiles** — spec §23 Q1 explicitly defers; bilateral only
- **Cross-federation delegation** — spec §23 Q3; bilateral peering not defined
- **Streaming (token-by-token) deliver** — spec §23 Q2; `interim: true` deliveries sufficient
- **Economic/reputation/payment layers** — spec §21 exclusions stand
- **Agent lifecycle management** (start/stop/upgrade/monitor) — out of protocol scope per §21
- **Production deployment tooling** — library-first; ops concerns deferred

## Context

**Starting state:** Repository contains only `FAMP-v0.5-spec.md` (1178 lines, the protocol spec itself). No code, no git history, no Rust toolchain installed.

**Prior review findings (4 parallel review agents):** Loaded in conversation history. Key categories:

1. **Canonical JSON** is the #1 blocker — spec says "sorted keys, no whitespace" but doesn't reference RFC 8785 JCS. Without lockdown, two conformant implementations will produce different bytes and signature verification will fail. Must resolve in Phase 1 (spec fork).

2. **State-machine holes** — real spec bugs, not just under-specification:
   - §9.6 ack-disposition conflated with terminal-state crystallization
   - §7.3 "no body inspection" claim is false (`interim` flag, partial acceptance subset, control target live in body)
   - Transfer-timeout reversion vs. in-flight delegate commit race
   - EXPIRED vs. in-flight deliver not covered by delivery-wins default
   - INV-5 violated by competing-instance commits during resolution window
   - Conditional-lapse loses to delivery-wins (should win)
   - Negotiation round counting under supersession ambiguous
   - Capability snapshot binding contradicts card-version rule

3. **Under-specified body schemas** — `commit`, `propose`, `deliver`, `control` bodies undefined. Must write schemas before coding or implementations cannot interop.

4. **Security gaps** — no domain-separation prefix in signatures, no recipient binding, idempotency key collision surface, Agent Card self-signature is circular (needs federation credential), SHA-256 artifact encoding unspecified.

**Developer background:** User is new to Rust. Phase 0 covers toolchain install, workspace scaffold, and basic `cargo test` loop before any FAMP code is written.

**Why Rust:** Ed25519 and canonical JSON demand byte-exact behavior; `match` on enums makes INV-5 (single terminal state) and the task FSM compiler-checked in ways a Python/TS implementation can only approximate at runtime. One Rust core can later feed Python/TS/Go bindings via wasm-bindgen or PyO3.

## Constraints

- **Tech stack**: Rust (stable, latest). `ed25519-dalek` for signatures, `serde` + custom canonicalizer for RFC 8785 JCS, `proptest` + `stateright` for state-machine model checking, `axum` or `hyper` for HTTP transport reference.
- **Tech stack (deferred)**: No Python/TS bindings in v1; keep FFI surface clean but unwired.
- **Transport**: HTTP/1.1 + JSON over TLS as reference wire; in-process `MemoryTransport` for tests. Other transports live behind the `Transport` trait.
- **Conformance target**: Staged conformance is supported — each milestone tags conformance level achieved; the vector pack did NOT ship in v1.0 — it is gated on a second implementer committing to interop (Gate B, still open). (Revised 2026-04-27 in v0.9 prep sprint T6, re-confirmed 2026-07-29 at v1.0 close; see `.planning/V0-9-PREP-SPRINT.md` for context. Original constraint was "Level 2 + Level 3 in one milestone" — superseded by the local-first reframe and the absence of a named second implementer.)
- **Spec fidelity**: v0.5.2 is the authority for this implementation (the v0.5.1 fork amended with the `audit_log` `MessageClass`, which does not fire the task FSM, shipped alongside v0.9 Phase 1). All diffs from v0.5 documented with reviewer rationale.
- **Security**: Every message signed (INV-10); unsigned messages rejected. Ed25519 non-negotiable. Domain separation prefix added in v0.5.1 fork.
- **Developer onboarding**: Rust toolchain install is Phase 0; assume zero prior Rust experience.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Language: Rust | Compiler-checked invariants (INV-5 via enum `match`), byte-exact Ed25519 and canonical JSON, single core can feed future bindings | ✓ Good — CORE-05/06 exhaustive `match` under `#![deny(unreachable_patterns)]` validated the claim; `serde_jcs` + `ed25519-dalek` gave byte-exact on first try |
| **Personal Profile before Federation Profile** | Solo-dev usability is the near-term goal; federation-grade semantics are expensive and mostly matter at ecosystem scale. Ship the signing substrate + a minimal usable runtime first, then stack federation on top without changing the substrate. | ✓ Good — v0.6 substrate shipped in one day because scope was narrow; v0.7 runtime sits on top without substrate churn |
| ~~Ship Level 2 + Level 3 together as v1~~ | **Superseded 2026-04-12.** Rationale still valid for Federation Profile, but Personal Profile explicitly is not a conformance-release target. Level 2 + Level 3 badges now live in Federation Profile. | Superseded |
| Fork spec to v0.5.1 rather than write profile addendum | State-machine findings are real bugs not ambiguities; profile-that-contradicts-spec causes interop confusion | ✓ Good — v0.5.1 shipped; spec-lint anchors + FAMP_SPEC_VERSION constant make drift detectable |
| Both `MemoryTransport` and `HttpTransport` in v1 | Memory transport is ~50 lines; HTTP is the wire reference everyone points at | — Pending (v0.7) |
| **Keep `serde_jcs 0.2.0` rather than fork to `famp-canonical` (SEED-001)** | 12/12 RFC 8785 conformance gate green end-to-end (Appendix B/C/E, 100K float corpus, UTF-16 supplementary, NaN/Inf, duplicate-key). `ryu-js` number formatter proven correct. Fork would be ~500 LoC for zero measurable gain. Fallback plan on disk as insurance. | ✓ Good — decision recorded 2026-04-13 in `.planning/SEED-001.md` with cited evidence; nightly 100M-line full-corpus workflow re-validates on cron |
| **`verify_strict`-only public surface for Ed25519** | Raw `verify` tolerates non-canonical / small-subgroup signatures; unacceptable for protocol-level non-repudiation. Typing-out `verify` from the public API makes misuse unreachable, not just discouraged. | ✓ Good — README + wrapper audit landed in Plan 02-03; §7.1c worked example re-verifies every CI run |
| **Domain separation prefix prepended internally, never by callers** | Callers who assemble signing input by hand will eventually assemble it wrong (PITFALLS P10 worked example is the standing receipt). `famp-crypto::canonicalize_for_signature` is the only sanctioned path. | ✓ Good — §7.1c fixture byte-exact on first run against external Python reference |
| **Narrow, phase-appropriate error enums (not one god enum)** | Compiler-checked `match` over a 5-variant crypto error catches 100% of crypto failure modes; bolting the same enum onto canonical / envelope / transport would produce a 40-variant monster that matches nothing specific. | ✓ Good — pattern repeated in Plans 01-01 (D-16) and 02-01 with no regret |
| Ed25519 key encoding: raw 32-byte pub / 64-byte sig, unpadded base64url | Matches `ed25519-dalek` defaults; simplest interop contract; strict codec rejects padding and mixed alphabets | ✓ Good — base64 round-trip property tests green; strict decoder catches malformed fixtures |
| Artifact IDs: `sha256:<hex>` prefix scheme | SHA-256 is "RECOMMENDED" in spec; hex encoding is canonical and unambiguous; `famp-canonical`, `famp-crypto::sha256_artifact_id`, and `famp-core::ArtifactId` all agree on the exact string form | ✓ Good — NIST FIPS 180-2 KATs + cross-crate agreement test green |
| **15-category flat `ProtocolErrorKind` + exhaustive consumer stub under `#![deny(unreachable_patterns)]`** | Every downstream crate that adds a `_ => …` arm instead of exhaustively matching is a future interop bug. The consumer stub turns "forgot a new error category" into a hard compile error, not a runtime surprise. | ✓ Good — stub pattern ready to be re-used in `famp-envelope` |
| **`AuthorityScope` with hand-written 5×5 `satisfies()` truth table, no `Ord` derive** | Authority is a ladder, not a total order; deriving `Ord` would silently admit "commit_delegate > negotiate" comparisons that aren't meaningful. Hand-written table makes the spec §10 semantics reviewable. | ✓ Good — truth table committed; proptest round-trip + symmetry checks green |
| Test strategy: conformance vectors → FSM model checking → adversarial suite → two-node integration | Each layer catches a distinct failure class; vectors are the interop contract future implementations hold us to | ✓ Good so far — v0.6 exercised conformance vectors; FSM model checking and adversarial suite still ahead |
| **Restore broker presence via a service-managed daemon — do NOT revert `56b2293` (v0.11)** | The orphan-broker leak `56b2293` fixed was real (82 orphans / 4 days). Reverting it to get Codex working again would reintroduce the leak. A user-level launchd/systemd-`--user` daemon supplies presence without leaking — the principled restore. | ✓ Good — `famp daemon install` ships; broker mortality stays intact (BLC-02 regression guard green) |
| **Unconditional `KeepAlive=true` in the plist (v0.11)** | Gated/idle-aware KeepAlive needs either an idle-timeout flag or socket activation; neither exists yet (`--no-idle-exit` is the inverse). Unconditional KeepAlive is the honest interim shape; socket activation is explicitly deferred. | ✓ Good — guardian APPROVED the literal plist; byte-exact fixture gate in CI |
| **Fail loud on protocol-version skew at connect (v0.11)** | A long-lived daemon outlives client upgrades (`cargo install`/`brew upgrade`); a silent wire-version mismatch against a byte-exactness protocol is the exact failure the inspector exists to expose. `BusClientError::ProtocolMismatch` names `famp daemon restart`. Down-payment on v1.0 federation wire-version negotiation. | ✓ Good — VER-01 skew test green |
| **Unify workspace version to `0.11.0` as single source of truth (v0.11)** | `famp -V` reported `0.1.0` (unbumped crate) while the banner said `0.5.x` — untrustworthy for skew diagnosis on a long-lived daemon. Reconciled `-V` / banner / handshake to one version. | ✓ Good — VER-02; `-V`/banner/handshake agree |
| **Design A (local-proxy) for the broker-liveness fork — gateway registers each remote principal under its OWN pid (v1.0)** | The alternative (Design B heartbeat/lease) meant changing `famp-bus`, the one crate every other layer depends on, to solve a problem created *outside* it. Backing each proxied principal with a connection reporting the gateway's own live PID makes the broker's existing `kill(pid,0)` sweep correct by construction. | ✓ Good — zero `famp-bus` change; LIVE-01/02 + GW-04 verified 3/3 including real-process SIGKILL |
| **Federation fields ride the ONE existing INV-10 signature, omitted when empty (v1.0)** | A second signature or a separate federation envelope type would fork the canonicalization contract — the exact failure this project exists to prevent. Optional omit-when-empty fields keep local-bus bytes byte-identical while giving v1.1/v2.0 (nonce, expiry, capability, approval) their slots now. | ✓ Good — no wire break; local bus unaffected; v1.1 fields already reserved |
| **Retire the 27 parked `_deferred_v1/` tests rather than reactivate them (v1.0)** | Every one depended on a CLI symbol v0.9 Phase 4 deleted, with no live `famp-bus`/`famp-gateway` rewrite target. Reactivating would have meant inventing new tests wearing old names. Two guards pinning the *real* Phase 9 E2E into the default nextest set buy the actual protection. | ✓ Good — 27/27 RETIRE with a per-file rationale ledger; the E2E it protects is what caught the BUS-11 ingress bug |
| **Fix the sender-`from` forgery hole inside v1.0 rather than defer to v1.1** | A cross-host boundary that ships trusting a client-supplied `from` is a hole that gets designed around, not fixed later. The fix is small and local (broker binds `from` to the authenticated identity; gateway refuses to sign mismatched authority) and needs no local crypto and no BUS-11 reopen. | ✓ Good — SEC-01..04 shipped pre-tag; ingress destination binding + fail-closed route config landed with it |
| **Drop §16's proposed release-note limitation instead of shipping it (v1.0)** | The wording ("`famp send` … does not initiate or complete the task FSM") predated ADDR-02, which made remote sends typed and FSM-driving. A stale limitation in a `v1.0.0` tag body is worse than none — it misdescribes the product to the first outside reader. | ✓ Good — REL-05; tag body carries the accurate fire-and-forget wording, pinned by a regression test across three surfaces |
| **Two added phases (11, 12) instead of tagging `v1.0.0` at the end of Phase 10** | The Gate A dogfood proved the gateway worked but no *shipping client* could reach a remote principal — tagging there would have shipped a demo, not a product. Phase 12 likewise refused to tag on a checklist with three open items. | ✓ Good — cost ~2 days; caught a forgery hole, 8 doc defects, a timestamp defect, and a dropped route that all would have shipped |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd:transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

## Shipped Milestone: v0.11 Broker Daemon & Cross-Tool Bootstrap — SHIPPED 2026-06-06

**Goal:** Restore the guaranteed broker-presence that commit `56b2293` (correctly) removed — the principled way, via a service-managed daemon — so a fresh clone of FAMP works for **both** Claude Code and Codex with no per-user broker babysitting.

**Delivered:** 3 phases (4–6), 11 plans, 15/15 requirements. `famp daemon install` writes a user-level launchd/systemd-`--user` service that keeps one broker alive; `famp broker --no-idle-exit` is the no-install bridge; EPERM-on-bind now yields an actionable sandbox diagnostic instead of a swallowed errno; connect-time version-skew detection guards the long-lived daemon against an upgraded client; README rewritten daemon-first and verified live against the binary. DAEMON-06 Linux behavioral acceptance deferred to a Linux host. See [milestones/v0.11-ROADMAP.md](milestones/v0.11-ROADMAP.md) · [milestones/v0.11-REQUIREMENTS.md](milestones/v0.11-REQUIREMENTS.md).

**Target features:**
- `famp broker --no-idle-exit` — disable the 300s idle self-terminate (Arm 4, `crates/famp/src/cli/broker/mod.rs`). Hard prerequisite: without it any long-lived/daemon broker churns every 5 min, rebinds the socket, and orphans sessions.
- EPERM-on-bind → actionable error at `crates/famp/src/bus_client/spawn.rs:92` (currently swallowed by `let _ =`): "can't create a broker inside a sandbox; run `famp daemon install` from a normal shell." Extends the broker-unreachable disambiguation in commits `4da30a3`/`ebbf1d3`.
- `famp daemon install` — writes + loads the platform service: launchd plist (macOS) + systemd `--user` unit (Linux). Idempotent. Plist shape per guardian's APPROVE-WITH-CONDITIONS security review: `RunAtLoad=true`, `KeepAlive=true` (unconditional — gated KeepAlive fails because there is no idle-timeout flag and no socket activation), `ProcessType=Background`, `StandardOut/ErrorPath` → `~/.famp/broker.log`, **no** `EnvironmentVariables`/secrets, launches the broker with `--no-idle-exit`. Refuses to install if run inside a sandbox.
- Daemon lifecycle: `famp daemon uninstall | status | restart`. `status` doubles as the "is the daemon up" diagnostic.
- Version handshake on connect — the daemon is long-lived; on `cargo install`/`brew upgrade` the client updates while the running daemon stays on old wire logic. Exchange a protocol/build version at connect and **fail loud** on skew (a down-payment on v1.0 federation wire-version negotiation). Reconcile `famp -V` (reports `0.1.0`, unbumped crate version) vs the `0.5.x` banner.
- Stale-binary-path handling — the plist hardcodes `~/.cargo/bin/famp`; a moved/replaced binary silently breaks the daemon. Provide `restart` + a README "re-run after upgrade" line + detection.
- README quickstart: `famp daemon install` once → both Claude and Codex just connect, forever. Plus a zero-setup bridge line usable today (run `famp broker --no-idle-exit` in one unsandboxed terminal).
- Cross-platform honesty — macOS launchd + Linux systemd `--user`. Name the gap in the README (minimal distros / containers / WSL / headless without systemd `--user` / `loginctl enable-linger`); don't ship mac-only under a "works for both" banner.

**Out of scope (deferred / separate track):**
- launchd/systemd **socket activation** — the elegant answer (service manager holds the socket, starts the broker on first connect, zero idle residency), but needs fd-inheritance the binary doesn't support yet.
- **spawn-lock** for the cold-start unlink-race in `bind_exclusive`'s stale branch — independent cleanup; the daemon dissolves the race for daemon users. Ships on its own track.

**Why now (verified regression, not speculation):** FAMP is public and advertised as working for both Claude Code and Codex. Codex *used to* work; it regressed after `56b2293` (2026-05-12, "arm idle timer at startup to prevent orphan broker leak"). Before that fix brokers leaked — a spawned broker ran forever even with no clients (the commit msg cites 82 orphans over 4 days) — so a broker was essentially always up, and Codex (which can `connect()` but never `bind()` inside its seatbelt sandbox) always found one. The fix correctly made brokers mortal, which removed the accidental safety net and unmasked Codex's structural inability to bootstrap a broker. **Do not revert `56b2293`** — the leak was real. This milestone restores broker presence the principled way: a service-managed daemon, not a leak.

**Design context:** root cause verified via git archaeology (`56b2293`); guardian (security/system) gave APPROVE-WITH-CONDITIONS on the plist and wants to review the literal plist XML before it loads; scope converged by `matt-essentialist` (ownership-correction framing, version-skew + lifecycle gaps) and `zed-velocity-engineer` (smallest-shippable-cut triage, cross-platform/stale-binary risks). Slots ahead of the gated v1.0 Federation milestone.

---

## Shipped Milestone: v0.10 Inspector & Observability — SHIPPED 2026-05-11

**Goal:** Make FAMP conversation state legible without grep-and-guess via a read-only inspector RPC on the v0.9 broker UDS, consumed by a `famp inspect` CLI subcommand.

**Target features (single phase):**
- `famp inspect broker` — health + dead-broker diagnosis. Must work against a dead socket; outputs one of `HEALTHY` / `DOWN_CLEAN` / `STALE_PID` / `ORPHAN_HOLDER` / `PERMISSION_DENIED` plus the evidence row used to decide. This is the load-bearing v0.10 command — direct fix for the orphan-listener incident class.
- `famp inspect identities` — name, listen mode, cwd, mailbox unread/total, last-sender, last-received-at. No double-print counter (see Non-Goals).
- `famp inspect tasks [--id <task_id>] [--orphans] [--full]` — FSM-grouped task list with envelope chain summary. `task_id == 0` rows go in a top-level `--orphans` bucket, recency-sorted. Summary fields: `envelope_id, sender, recipient, fsm_transition, timestamp, sig_verified, envelope_count, last_transition_age`. `--full` emits the canonical JCS (RFC 8785) form so output round-trips through `jq` and reproduces the signature input byte-for-byte.
- `famp inspect messages --to <name> [--tail N]` — envelope metadata only (no body). Surfaces `byte_len` + `sha256_prefix` instead of body content; sidesteps privacy entirely.
- `--json` on every command (machine-readable consumption path; future SPA / `famp doctor` / external tooling consume the same surface).

**Crate split:**
- `famp-inspect-proto` — RPC request/response types, no I/O. Shared canonicalization/envelope/FSM crate dependencies match the broker (single source of truth — separate-binary path was rejected because it would re-introduce version-skew failure modes against a byte-exactness protocol).
- `famp-inspect-client` — UDS client. Future SPA / `famp doctor` / external tooling depend on this without pulling in the CLI.
- `famp-inspect-server` — handlers mounted by the broker. Same UDS, separate `famp.inspect.*` namespace, bounded latency budget + cancellable handlers (must not back up the message path under load).

**Non-goals (v0.10):**
- No mutation. No replay, no force-FSM-transition, no inbox doctoring. CLI subcommands like `famp doctor` come later, gated on what the inspector reveals as actually needed.
- No browser SPA. No SSE. Punted to v0.10.x or later, only if ~2 weeks of CLI use proves the read-only surface is insufficient.
- No federation view. Inspector observes one node — the local broker. v1.0 gateway can expose its own surface later.
- No remote access. UDS local-trust only.
- No `--body` fetch in v0.10. Body fetch overlaps with reading mailbox files directly during incidents; ship in v0.10.x only if usage signals reach for it.
- No double-print counter. The double-print failure mode (a docs/CLAUDE-CODE-CONTEXT-GUIDE.md-described class where wake-up notification + inbox fetch each carry the message body, doubling token cost) is not observable at the broker. Right instrument is per-message token attribution at the model boundary, or a static audit of the `famp_await` notification payload. Both are separate investigations from the inspector. A broker-side "delivery count" was considered and rejected — wrong-by-construction counters outlive the diagnostics that retire them.

**Why now (between milestones):**
v0.9 shipped a working broker but conversation state stayed opaque — three incidents in v0.9 (orphan PID file masking a delivery bug, task_id zeros invisible until grepped, stale mailboxes relayed because there was no easy way to see latest) cost real time. The v1.0 federation gate is event-driven (Sofer cross-machine) and could fire any time or none. The inspector is independent of that gate, addresses real recurring pain, and is the right place to land observability before v1.0 layers federation complexity on top.

**Design context:** Brainstorm + adversarial review by `matt-essentialist` (reframe: "the inspector RPC is the product, the dashboard is a demo of it") and `hamming-research-scientist` (rejected the double-print counter as wrong-instrument; flagged the dead-broker-diagnosis command as load-bearing). Decisions locked in conversation prior to milestone open.

<details>
<summary>Superseded draft: v1.0 Federation Profile — Gate A (Gateway), 2026-06-08 mesh-VPN framing</summary>

**Superseded 2026-07-23** by the own-machines-first scope at the top of this file, and shipped under that scope on 2026-07-29. Two premises here did not survive: (1) *reachability = mesh VPN* — the tailnet is no longer the trust boundary, because the gateway verifies Ed25519/INV-10 at the boundary itself (auto-memory `project_v10_reachability_meshvpn`, updated 2026-07-23); (2) *add the friend after the own-machines proof* — cross-person trust moved out of v1.0 entirely and into v1.1. Kept for the design rationale only.

**Goal:** Ben's Claude Code on one host exchanges signed FAMP envelopes with another host's Claude Code, over a mesh VPN, via a new `famp-gateway` (Layer 2) wrapping the preserved `famp-transport-http` + `famp-keyring`.

**Gate A fired 2026-06-08** by the friend-to-friend ask (Ben's Claude Code ↔ his friend's Claude Code) — the activation signal the design named; the ~2-week dogfood was proof-of-need, not a technical prereq (see `docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`).

**Target features (thin vertical slice):**
- `famp-gateway` crate (Layer 2) — bridges the local UDS bus to remote FAMP-over-HTTPS using the preserved transport; re-introduces the signed-envelope path (Ed25519/INV-10, already built) on the cross-host hop.
- **Reachability = mesh VPN** (Tailscale/WireGuard): the gateway HTTPS listener binds to a tailnet IP, TOFU-pinned peer. FAMP does **not** build NAT traversal / relay / STUN / TURN (decision 2026-06-08; auto-memory `project_v10_reachability_meshvpn`).
- **Two-host trust bootstrap:** `peer_export` → out-of-band channel (e.g. Signal) → `peer_import` + TOFU pin. Ship rough; bootstrap-UX polish defers to v1.1.
- Reactivate the ~27 deferred federation tests in `crates/famp/tests/_deferred_v1/`.

**Sequence (hard ordering):** prove gateway on Ben's **own laptop ↔ home machine first** (full network control, keys copied directly, tight debug loop), *then* add the friend. Don't debug gateway + NAT + cross-person trust simultaneously across two people's schedules.

**Precise scope delta vs the planned Gate A** (own two machines): internet/NAT reachability + two-human out-of-band key exchange. NOT reachability/trust in general — Gate A already scoped bidirectional reachability + mutual trust.

**Deferred out of this milestone:** Agent Cards + federation credentials, `.well-known` card distribution, negotiation/counter-proposal, delegation forms, provenance graph, extensions registry. SEED-002 (push-notification harness adapter) deferred — orthogonal to the federation transport. **Gate B (Conformance)** stays event-driven: a 2nd implementer commits to interop → conformance vector pack (`WRAP-V0-5-1-PLAN.md`; SEED-001 is its serde_jcs gate) ships at whatever tag is current. Sofer is the natural Gate-B candidate.

**References:** `docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`; auto-memories `project_v10_trigger`, `project_v10_reachability_meshvpn`; escape-hatch tag `v0.8.1-federation-preserved`.

</details>

## Last Milestone: v0.9 Local-First Bus — SHIPPED 2026-05-04

**Goal:** Replace the per-identity TLS listener mesh used for same-host agents with a UDS-backed broker. Zero crypto on the local path; IRC-style channels; durable per-name mailboxes; stable MCP tool surface carried forward to v1.0. Federation primitives (Agent Cards, delegation, provenance, cross-host trust) move to v1.0 as a `famp-gateway` wrapping the local bus.

**Status:** shipped 2026-05-04. Opened 2026-04-27 via `/gsd-new-milestone v0.9` after the 3-day v0.8→v0.9 prep sprint (T1-T9, see `.planning/V0-9-PREP-SPRINT.md`). Design authority: [`docs/superpowers/specs/2026-04-17-local-first-bus-design.md`](../docs/superpowers/specs/2026-04-17-local-first-bus-design.md) (506 lines, both `zed-velocity-engineer` and `the-architect` reviewed). Architecture summary: [ARCHITECTURE.md](../ARCHITECTURE.md).

**Closed:** 5 phases (1+2+3+4 + close-fix Phase 5), 35 plans, 85/85 requirements, audit `passed`, 193 commits over 8 days (2026-04-27 → 2026-05-04). 12-line/30-second README acceptance gate met. See [milestones/v0.9-ROADMAP.md](milestones/v0.9-ROADMAP.md), [milestones/v0.9-REQUIREMENTS.md](milestones/v0.9-REQUIREMENTS.md), [milestones/v0.9-MILESTONE-AUDIT.md](milestones/v0.9-MILESTONE-AUDIT.md).

**Acceptance criterion:** Two Claude Code windows exchange a message in **≤12 lines of README and ≤30 seconds**. If Phase 3 cannot land this, design is too heavy and must be revisited before v0.9.0 tags.

**Phase 04 completion note:** The user-facing federation CLI (`init`, `setup`, `listen`, `peer`, and TLS-form `send`) is removed. `famp-transport-http` and `famp-keyring` remain as v1.0 federation internals, direct library-level `e2e_two_daemons` tests stay green in CI, and the escape-hatch tag `v0.8.1-federation-preserved` points to the pre-deletion commit.

**Target features — 4 phases:**

1. **`famp-bus` library + audit-log MessageClass.** New crate with `BusMessage`/`BusReply`/`Target`/`BusErrorKind`/`Delivered`/`SessionRow` types, length-prefixed canonical-JSON codec, pure broker state-machine (no I/O, no tokio), in-memory mailbox for tests. Four Phase-1 TDD gates written first: codec fuzz, drain cursor atomicity, PID reuse race, EOF cleanup mid-await. Proptest coverage: DM fan-in ordering, channel fan-out, join/leave idempotency, drain completeness, PID-table uniqueness. **Plus the v0.5.2 audit-log MessageClass** (T5 spec amendment, lagged constant). The commit that adds `MessageClass::AuditLog` enum variant + dispatch + body validation MUST also bump `FAMP_SPEC_VERSION` `"0.5.1"` → `"0.5.2"` in `crates/famp-envelope/src/version.rs` — atomic; bumping in a separate commit either lies (if before impl) or strands impl as v0.5.1-tagged (if after).

2. **UDS wire + CLI + MV-MCP rewire + hook subcommand.** `famp broker` subcommand wrapping `famp-bus::Broker` with `posix_spawn`+`setsid` lifecycle, `bind()`-exclusion (socket IS the lock), 5-minute idle exit. Top-level CLI: `register`, `send`, `inbox`, `await`, `join`, `leave`, `sessions`, `whoami`. On-disk mailbox impl reusing `famp-inbox` format. `famp mcp` rewired to bus (drops TLS/reqwest), exposes 8 tools (`famp_register`, `famp_send`, `famp_inbox`, `famp_await`, `famp_peers`, `famp_join`, `famp_leave`, `famp_whoami`). Integration tests (2-client DM, 3-client channel fan-out, broker-crash recovery), MCP E2E harness. **Plus `famp-local hook add` subcommand** (Sofer-driven scope addition) — declarative `--on Edit:<glob> --to <peer>` wiring; replaces hand-written bash wrappers.

3. **Claude Code integration polish.** `famp install-claude-code` writes user-scope MCP config + drops slash-command files to `~/.claude/commands/` (`/famp-register`, `/famp-join`, `/famp-leave`, `/famp-msg`, `/famp-channel`, `/famp-who`, `/famp-inbox`). README Quick Start rewrite hitting the 12-line/30s gate. Onboarding doc. (Boundary doc — "When NOT to Use FAMP" — already shipped 2026-04-27 via prep sprint T4.)

4. **Federation CLI unwire + federation-CI preservation.** Remove `famp setup`, `famp listen`, `famp init`, `famp peer add/import`, old `famp send` from top-level CLI. Move `famp-transport-http` + `famp-keyring` under "v1.0 federation internals" label in workspace `Cargo.toml`. **Hard requirement:** refactor `e2e_two_daemons` to target `famp-transport-http` library API directly (no deleted-CLI dependency), green in `just ci` on every commit. Tag `v0.8.1-federation-preserved` on the commit BEFORE Phase 4 deletions land. Write `docs/MIGRATION-v0.8-to-v0.9.md`. This is the **plumb-line-2 commitment** against mummification — Architect's "local-case black hole" risk.

**Carry-forward from v0.8:**
- The six-tool MCP surface (`famp_register`, `famp_whoami`, `famp_send`, `famp_await`, `famp_inbox`, `famp_peers`) is the stable v1.0 contract — broker swap must not break it. Phase 2 adds `famp_join` and `famp_leave` for IRC-style channels.
- Session-bound identity contract (`FAMP_LOCAL_ROOT` only at MCP startup, `IdentityBinding` per session) — superseded on the bus path: `~/.famp/` becomes the sole local root; identity now per-process via `famp register <name>` against the broker.
- v0.8 per-identity HTTPS daemons remain the federation transport for v1.0 (consumed only by the refactored `e2e_two_daemons` test in v0.9; user-facing federation lands in v1.0 wrapped by `famp-gateway`).

**Open product questions to resolve before v0.9.0 tag:**
- Product name decision (Option Z: split — FAMP stays the protocol/library, local-first product gets new brand). Binary stays `famp` through v0.9.x for continuity.
- Slash-command naming bikeshed (`/famp-msg` vs `/famp-send` vs `/famp-dm`) — defer to Phase 3.

**Open watch-items from v0.8 audit (carry-forward debt to address inside v0.9 phases):**
- TD-1: pin `[[profile.default.test-groups]]` for listen-subprocess tests (max-threads = 4) before listen subprocess tests proliferate further. Address in Phase 4 alongside `e2e_two_daemons` refactor.
- TD-3: rewrite REQUIREMENTS.md INBOX-01 to match raw-bytes-per-line implementation OR add structured wrapper. Address in Phase 2 alongside CLI inbox rework.
- TD-4: broker should auto-create REQUESTED record on inbound request, eliminating receiver-side test seed. Naturally absorbed by Phase 1 broker state-machine design.
- TD-7: backfill Nyquist VALIDATION.md for v0.8 phases 02-04 + bridge, OR formally defer per project policy. Address inside Phase 1's TDD-gates pass.

**Risk tracking:**
- **Local-case black hole** (Architect): if v0.9 is too satisfying, v1.0 federation never ships. Mitigated by Phase 4 federation-CI requirement + the v1.0 trigger named above (Sofer-from-different-machine, 4-week clock at v0.9.0).
- **Broker exclusion via `bind()` on NFS-mounted home dirs** — document `~/.famp/` must be local FS; add a startup check that warns otherwise. Phase 2 surface.
- **Channel mailbox unbounded growth** — add `famp mailbox rotate` in v0.9.1 before any user complains. Acceptable for v0.9 because interactive developer usage won't hit the limit for weeks.

## Previous Milestone: v0.8 Usable from Claude Code — SHIPPED 2026-04-26

**Goal:** Turn v0.7's proven substrate into something Ben can actually use — two Claude Code sessions on the same laptop, each driving a `famp` agent via MCP tools, coordinating on one long task.

**Target features:**
- `famp` CLI with subcommands (`init`, `listen`, `send`, `await`, `peer add`, `inbox`) — replacing today's 8-line placeholder binary
- Persistent identity at `~/.famp/` (Ed25519 keypair, self-signed TLS cert, config, peer list) with one-time `famp init`
- Background listener daemon that holds the v0.7 HttpTransport open and writes inbound messages to a file-based inbox
- Block-with-timeout `famp await` for turn-based inbox semantics (Claude Code can't be interrupted by incoming network messages — pull model required)
- MCP server exposing `famp_send` / `famp_await` / `famp_inbox` (and peer management) as Claude Code tools
- **One-long-task conversation shape** — open a task, exchange many `deliver` messages back and forth within that one task, close with a terminal deliver. Exercises the v0.7 FSM without adding new message classes or changing the spec.
- Same-laptop end-to-end: two Claude Code sessions, two daemons on different loopback ports, full round-trip via MCP tool calls

**Explicitly NOT in v0.8:**
- Agent Cards, federation credential, pluggable trust store, `.well-known` distribution — all defer to Federation Profile (v1.0+)
- New message classes, new FSM states, or any v0.5.1 spec changes — v0.8 is pure implementation on top of v0.7
- Cross-machine deployment (laptop ↔ EC2 via Tailscale) — same-laptop E2E is the v0.8 gate; the CLI and daemon are designed to work across hostnames, but the Tailscale bring-up and cross-machine smoke test are deferred to v0.9 or an informal post-v0.8 exercise
- Group / multi-party chat — not in the v0.5.1 spec (bilateral only); requires a real protocol extension, not a milestone
- crates.io publishing, public distribution, framework / abstraction work — library is still pre-release, not a published crate

**Success shape:**
1. `famp init` on a fresh laptop creates `~/.famp/` with keypair, cert, config, and empty peer list.
2. Two shells each run `famp listen` on different loopback ports; each has `peer add`-ed the other.
3. Two Claude Code sessions — each pointed at its own `famp` daemon via an MCP server — open a single task, exchange ≥4 `deliver` messages back and forth (driven by actual LLM conversation, not scripted), and close with a terminal deliver. The task closes COMPLETED on both sides.
4. `just ci` green; no regression in the 253 v0.7 tests.

## Earlier Milestone: v0.7 Personal Runtime — SHIPPED 2026-04-14

**Goal:** A single developer can run the same signed `request → commit → deliver` cycle **two ways**: (a) in one binary via `MemoryTransport`, and (b) across two machines / two processes via a minimal HTTP binding, with trust bootstrapped from a local keyring file. This is the finish line for "something I can use myself."

**Target crates / deliverables:**
- `famp-envelope` — signed envelope with mandatory-signature enforcement; body schemas for **only** `request`, `commit`, `deliver`, `ack`, `control/cancel`. Negotiation, delegation, announce, describe bodies explicitly omitted.
- Minimal task FSM — 5 states (`REQUESTED → COMMITTED → {COMPLETED | FAILED | CANCELLED}`; 1 initial + 1 intermediate + 3 terminals), compiler-checked terminals, no `stateright` model check (defer), no timeouts.
- `famp-transport` trait + `MemoryTransport` (in-process, ~50 LoC).
- `famp-transport-http` **minimal subset**: axum `POST /famp/v0.5.1/inbox` endpoint, `reqwest` client send, rustls TLS, 1 MB body-size limit, signature-verification middleware running **before** routing. No `.well-known` Agent Card distribution (TRANS-05), no cancellation-safe spawn-channel send (TRANS-08) — both defer to Federation Profile.
- Trust-on-first-use keyring — local `HashMap<Principal, VerifyingKey>`; principal = raw Ed25519 pubkey, bootstrapped from a keyring file or CLI flags. No Agent Card, no federation credential, no trust registry.
- `famp/examples/personal_two_agents.rs` — end-to-end happy path in one binary via `MemoryTransport`, printing a typed trace.
- `famp/examples/cross_machine_two_agents.rs` — same flow across two processes via HTTP. Both ends load the other's pubkey from a local file.
- Negative tests run against **both** transports: unsigned rejected, wrong-key rejected, canonical divergence detected. Three cases × two transports, not eighteen.

**Explicitly NOT in v0.7:** Agent Card, federation credential, trust registry, `.well-known` distribution, negotiation/counter-proposal, three delegation forms, provenance graph, extensions registry, `stateright` model checking, adversarial conformance matrix, Level 2/3 badges, CLI, cancellation-safe send path. All move to Federation Profile milestones v0.8+.

**Success shape:** `cargo run --example personal_two_agents` prints a signed conversation trace and exits 0; running `cross_machine_two_agents` server in one shell and client in another completes the same cycle over HTTPS; the three negative tests fail closed with typed errors on both transports; `just ci` green.

**Phase numbering:** reset to Phase 1 (milestone-local numbering; v0.6 ended at Phase 3 but phase numbers are not continuous across milestones).

## Current State

**v1.1 progress (2026-08-05):** Phase 19 Auto-Wake Gate is complete and independently verified 10/10. The broker admits only Local-origin records to parked Await, preserves Gateway/Unknown records for explicit Inbox reads, and the pairing artifact carries the QUAR-15 consent warning. Current work advances to Phase 20, the Human Acceptance Gate.

**Shipped:**
- **v0.5.1 Spec Fork** (2026-04-13) — interop contract locked.
- **v0.6 Foundation Crates** (2026-04-13) — substrate: `famp-canonical`, `famp-crypto`, `famp-core`. 25/25 requirements, 112/112 tests.
- **v0.7 Personal Runtime** (2026-04-14) — minimal usable library on two transports. `famp-envelope`, `famp-fsm`, `famp-transport` + `MemoryTransport`, `famp-keyring` (TOFU), `famp-transport-http` (axum + rustls + reqwest, signature-verification middleware, 1 MB body cap, D-B5 full `rustls-platform-verifier` + extra anchor), two finish-line examples (`personal_two_agents`, `cross_machine_two_agents`), 3×2 adversarial matrix with sentinel proofs. 32/32 requirements, 253/253 tests, `cargo tree -i openssl` empty.
- **v0.8 Usable from Claude Code** (2026-04-26) — `famp` CLI (`init`, `listen`, `send`, `await`, `peer add`, `inbox`), persistent identity at `~/.famp/`, fsync-durable file-based inbox daemon, advisory inbox lock, block-with-timeout `famp await`, `famp mcp` stdio JSON-RPC server with six-tool surface, session-bound identity (`FAMP_LOCAL_ROOT`-only, B-strict variant), one-long-task conversation shape, same-laptop E2E. 5 phases (4 archived + v0.8.x bridge), 18 plans, 39/39 requirements, 419/419 tests green.
- **v0.9 Local-First Bus** (2026-05-04) — UDS broker at `~/.famp/bus.sock` replacing per-identity TLS listener mesh. New `famp-bus` Layer-1 substrate (tokio-free pure broker, length-prefixed canonical-JSON codec, four TDD gates GREEN, five proptest properties GREEN). Atomic v0.5.1 → v0.5.2 spec bump landing `MessageClass::AuditLog` + `Relation::Audits` + `BusEnvelope<B>` sibling type + `AnyBusEnvelope` 6-arm dispatch. New 8-verb top-level CLI (`register`, `send`, `inbox`, `await`, `join`, `leave`, `sessions`, `whoami`) and 8-tool stable MCP surface; `famp install-claude-code` + 7 slash commands; `famp-local hook add` declarative wiring with Stop-hook execution runner. Federation CLI hard-deleted (`famp setup/listen/init/peer add/peer import` removed); `famp-transport-http` + `famp-keyring` preserved as v1.0 internals via library-API `e2e_two_daemons` test that runs in CI every commit. Tag `v0.8.1-federation-preserved` cut as escape hatch. Migration guide ships at `docs/MIGRATION-v0.8-to-v0.9.md`. 5 phases, 35 plans, 85/85 requirements, audit `passed`. 12-line/30-second README acceptance gate met.
- **v0.10 Inspector & Observability** (2026-05-11) — read-only `famp.inspect.*` RPC on the v0.9 broker UDS + `famp inspect broker/identities/tasks/messages` CLI; three inspector crates (`-proto` no-I/O, `-client` no-clap, `-server` version-aligned); compile-time + dep-graph read-only discipline; 500 ms latency budget + cancellable handlers; no-starvation load test. Closes the orphan-listener / task-FSM-invisibility / stale-mailbox incident classes. 3 phases, 10 plans, 26/26 requirements, audit `passed`.
- **v0.11 Broker Daemon & Cross-Tool Bootstrap** (2026-06-06) — user-level service-managed daemon (`famp daemon install/uninstall/status/restart`; launchd + systemd `--user`) restoring guaranteed broker presence so a fresh clone works for both Claude Code and Codex; `famp broker --no-idle-exit` flag + no-install bridge; actionable EPERM-on-bind sandbox diagnostics (CLI + MCP); connect-time version-skew detection; workspace version unified to 0.11.0; daemon-first README verified live against the binary. 3 phases (4–6), 11 plans, 15/15 requirements. DAEMON-06 Linux behavioral acceptance deferred to a Linux host.
- **v1.0 Federation Profile — Gateway Core** (2026-07-29, tagged `v1.0.0` at `5edff41`) — **Gate A closed.** `famp-gateway` (Layer 2) proxies remote principals onto the local UDS bus over signed HTTPS: Design-A liveness fix with zero `famp-bus` change, 7 forward-compat federation fields under the one existing INV-10 signature, two-machine TOFU via `famp peer export/import`, and `famp send --to agent:<domain>/<name>` as the shipping remote-addressing surface with the class branched by send mode so the FSM terminates through the CLI/MCP. Trust boundary hardened pre-tag: broker binds envelope `from` to the authenticated identity, gateway rejects non-own-domain ingress / mismatched-authority egress / client-supplied federation metadata, route config fail-closed on ambiguity. Proven live macOS ↔ Linux over Tailscale (UAT-01), terminal COMPLETED on both hosts. 6 phases (7–12), 29 plans, 29/29 requirements, 12/12 CI check-runs green at the tagged SHA.

**Codebase state at v1.0.0:** Rust workspace at version `1.0.0`; 16 crates across three layers — Layer 0 protocol primitives (`famp-canonical`, `famp-crypto`, `famp-core`, `famp-envelope`, `famp-fsm`), Layer 1 local bus + inspector (`famp-bus`, `famp-inbox`, `famp-inspect-{proto,client,server}`), Layer 2 federation (`famp-gateway`, `famp-transport`, `famp-transport-http`, `famp-keyring`), plus the `famp` binary. Spec authority `v0.5.2`. ~973 workspace tests green; `cargo tree -i openssl` still empty. Stable 12-tool MCP surface.

**Next:** planning open. Two named non-blocking gates remain, both event-driven:
- **Gate B — conformance vector pack.** Fires when a second implementer commits to interop (`WRAP-V0-5-1-PLAN.md`; SEED-001 is its RFC 8785 gate, already green). Independent of any version number — ships at whatever tag is current. The first foreign-implementation contact (Grok, 2026-06-11) exchanged messages on the bus but did not commit to interop.
- **v1.1 sketch — open-internet federation.** Public reachability (relay / NAT traversal), cross-person trust bootstrap, a signed peer directory, and protocol-grade ingress (freshness/replay-cache enforcement, audience binding, DoS ordering, revocation). The v1.0 own-machines proof is deliberately the floor this builds on: any v1.1 failure is unambiguously in the new layer, not the spine.

Also open and unscoped: the FAMP-Sec capability/approval/tool-admission plane (v2.0+, demand-gated — reviewed draft 0.2 exists), and the `999.x` backlog (await crash-safety, multi-listener lock semantics, `heartbeat`/`user_attention` envelope classes, spec-by-path tracking, broker-owned delivery position).

**Usable-from-Claude-Code finish line ✓✓:** Two Claude Code windows registering as different identities and exchanging a message is now reachable in **≤12 lines / ≤30 seconds** via `cargo install famp && famp install-claude-code` — no per-identity TLS certs, no peer cards, no `FAMP_HOME` juggling. MCP surface grew from 8 tools (v0.9) to 12 (current); the contract is stable across v0.8 → v0.9 → v1.0, the count is not.

---
*Last updated: 2026-08-05 after completing Phase 19 Auto-Wake Gate. Broker-enforced Local-only Await behavior and QUAR-15 pairing consent are independently verified; Phase 20 Human Acceptance Gate is next.*

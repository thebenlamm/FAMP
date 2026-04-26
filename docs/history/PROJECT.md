# FAMP — Federated Agent Messaging Protocol (Reference Implementation)

## What This Is

A Rust implementation of FAMP (Federated Agent Messaging Protocol) v0.5.1, staged in **two profiles** so a single developer can get a usable library before the full federation-grade semantics are built out.

1. **Personal Profile (v0.6 + v0.7 + v0.8)** — the minimum usable library *and* the daemon, CLI, and Claude Code integration that make it actually usable from a terminal. v0.6 shipped byte-exact canonical JSON, Ed25519-signed envelopes with domain separation, and the core types. v0.7 shipped the five-state task lifecycle, `MemoryTransport` and a minimal HTTP transport, and two examples proving the substrate. v0.8 wraps that substrate in a `famp` CLI, a persistent listener daemon with a file-based inbox, and an MCP server so two Claude Code sessions can drive two local agents through a long-running task.

2. **Federation Profile (v0.9+)** — adds the semantics that matter at ecosystem scale: Agent Cards + federation credentials, negotiation/counter-proposal, the three delegation forms, provenance graphs, an extensions registry, the adversarial conformance matrix, and Level 2 + Level 3 conformance badges.

The signing substrate is the same in both profiles. Canonicalization, signing, and core types are done once and correctly in v0.6; Personal Profile exercises that substrate against a minimal runtime in v0.7; Federation Profile stacks ecosystem semantics on top without re-deriving the interop bytes.

## Core Value

**A byte-exact, signature-verifiable FAMP substrate a single developer can use from their own code today, and two independent parties can interop against later.** If canonicalization or signature verification disagrees, nothing else matters — so Personal Profile exercises the same signing contract Federation Profile will depend on.

## Requirements

### Validated

- [x] Rust toolchain bootstrap (install rustup, pin toolchain, workspace scaffold) — *Validated in Phase 00: toolchain-workspace-scaffold*
- [x] Fork spec to `FAMP-v0.5.1-spec.md` and resolve identified ambiguities/bugs (canonical JSON, body schemas, state-machine holes) — *Validated in v0.5.1 milestone (Phase 01: spec-fork). 1038-line spec, 28 changelog entries, 21/21 spec-lint anchors green, worked Ed25519 example byte-exact from external reference per PITFALLS P10.*
- [x] `famp-canonical` — RFC 8785 JCS canonicalization with external-vector conformance gate — *Validated in Phase 01: canonical-json-foundations. 12/12 conformance tests green (Appendix B/C/E byte-exact, 100K cyberphone float corpus, UTF-16 supplementary, duplicate-key rejection). SEED-001 resolved: keep `serde_jcs 0.2.0`. CI gate + nightly 100M full-corpus workflow live; fallback plan on disk as insurance.*
- [x] `famp-crypto` — Ed25519 sign/verify with domain-separation prefix, `verify_strict`-only — *Validated in Phase 02: crypto-foundations. 7/7 truths verified. Ed25519 sign/verify with SPEC-03 domain-separation prefix, `verify_strict`-only (raw `verify` unreachable), weak-key rejection at ingress, base64url-unpadded strict codec, RFC 8032 KAT gate, §7.1c worked-example byte-exact interop gate, SHA-256 content-addressing via `sha2 0.11` (CRYPTO-07), constant-time verify via `subtle`. 24/24 nextest + clippy clean.*
- [x] `famp-core` — shared types, typed error enum, INV-1..11 scaffolding — *Validated in Phase 03: core-types-invariants. 10/10 must-haves verified. Principal/Instance identity, UUIDv7 ID newtypes, ArtifactId with `sha256:<hex>` invariant (CORE-01..03); 15-variant flat `ProtocolErrorKind` with wire-string round-trip and ProtocolError wrapper (CORE-04); `invariants::INV_1..INV_11` namespaced doc anchors (CORE-05); `AuthorityScope` 5-variant enum with hand-written 5×5 `satisfies()` truth table, no `Ord` derive (CORE-06); exhaustive consumer stub under `#![deny(unreachable_patterns)]` making new variants a hard compile error (SC #3/#5). 66/66 famp-core + 112/112 workspace nextest green.*

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

### Deferred — Federation Profile (v0.8+)

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
- **Conformance target**: Level 2 + Level 3 in one milestone. Level 1-only is explicitly not a release target.
- **Spec fidelity**: v0.5.1 fork is the authority for this implementation. All diffs from v0.5 documented with reviewer rationale.
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

## Current Milestone: v0.8 Usable from Claude Code

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
- Agent Cards, federation credential, pluggable trust store, `.well-known` distribution — all defer to Federation Profile (v0.9+)
- New message classes, new FSM states, or any v0.5.1 spec changes — v0.8 is pure implementation on top of v0.7
- Cross-machine deployment (laptop ↔ EC2 via Tailscale) — same-laptop E2E is the v0.8 gate; the CLI and daemon are designed to work across hostnames, but the Tailscale bring-up and cross-machine smoke test are deferred to v0.9 or an informal post-v0.8 exercise
- Group / multi-party chat — not in the v0.5.1 spec (bilateral only); requires a real protocol extension, not a milestone
- crates.io publishing, public distribution, framework / abstraction work — library is still pre-release, not a published crate

**Success shape:**
1. `famp init` on a fresh laptop creates `~/.famp/` with keypair, cert, config, and empty peer list.
2. Two shells each run `famp listen` on different loopback ports; each has `peer add`-ed the other.
3. Two Claude Code sessions — each pointed at its own `famp` daemon via an MCP server — open a single task, exchange ≥4 `deliver` messages back and forth (driven by actual LLM conversation, not scripted), and close with a terminal deliver. The task closes COMPLETED on both sides.
4. `just ci` green; no regression in the 253 v0.7 tests.

## Previous Milestone: v0.7 Personal Runtime — SHIPPED 2026-04-14

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

**Shipped:**
- **v0.5.1 Spec Fork** (2026-04-13) — interop contract locked.
- **v0.6 Foundation Crates** (2026-04-13) — substrate: `famp-canonical`, `famp-crypto`, `famp-core`. 25/25 requirements, 112/112 tests.
- **v0.7 Personal Runtime** (2026-04-14) — minimal usable library on two transports. `famp-envelope`, `famp-fsm`, `famp-transport` + `MemoryTransport`, `famp-keyring` (TOFU), `famp-transport-http` (axum + rustls + reqwest, signature-verification middleware, 1 MB body cap, D-B5 full `rustls-platform-verifier` + extra anchor), two finish-line examples (`personal_two_agents`, `cross_machine_two_agents`), 3×2 adversarial matrix with sentinel proofs. 32/32 requirements, 253/253 tests, `cargo tree -i openssl` empty.

**Next:** v0.8 Usable from Claude Code — `famp` CLI (`init`, `listen`, `send`, `await`, `peer add`, `inbox`), persistent identity at `~/.famp/`, file-based inbox daemon, block-with-timeout await, MCP server for Claude Code, one-long-task conversation shape, same-laptop E2E with two live Claude Code sessions driving two local daemons. Still Personal Profile — Federation Profile (Agent Cards, federation credential, negotiation, delegation, provenance) defers to v0.9+.

**Personal Profile finish line ✓:** Both finish-line examples build clean. `cargo nextest run --workspace` → 253/253. CONF-04 happy path runs over real rustls TLS. The 3 adversarial cases (unsigned / wrong-key / canonical divergence) fail closed on both `MemoryTransport` and `HttpTransport` with distinct typed errors.

---
*Last updated: 2026-04-14 — v0.8 Usable from Claude Code milestone opened. v0.7 Personal Runtime shipped: 4/4 phases, 15/15 plans, 32/32 requirements, 253/253 tests green.*

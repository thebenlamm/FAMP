# Milestones

## v0.9 Local-First Bus (Shipped: 2026-05-04)

**Phases completed:** 5 phases (1 + 2 + 3 + 4 + close-fix Phase 5), 35 plans
**Timeline:** 2026-04-27 → 2026-05-04 (8 days, 193 commits)
**Test footprint:** workspace green; `cargo tree -i openssl` empty for the user-facing local path
**Requirements:** 85/85 satisfied (BUS-01..11, TDD-01..04, PROP-01..05, AUDIT-01..06, BROKER-01..05, CLI-01..11, MCP-01..10, HOOK-01..04a/04b, TEST-01..06, CC-01..10, FED-01..06, MIGRATE-01..04, CARRY-01..04)

**Delivered:** Two Claude Code windows on the same Mac exchange a signed-on-the-bus message in **≤12 lines of README and ≤30 seconds wall-clock** via `cargo install famp && famp install-claude-code` — no per-identity TLS certs, no peer cards, no `FAMP_HOME` juggling. Federation-grade primitives stay in the workspace as v1.0 internals (`famp-transport-http`, `famp-keyring`), exercised by a refactored `e2e_two_daemons` library-API test that runs in CI on every commit.

**Key accomplishments:**

- **`famp-bus` Layer 1 substrate** (Phase 1) — pure-state broker `Broker::handle(BrokerInput, Instant) -> Vec<Out>` with zero `tokio` and zero I/O in core; nine `BusMessage` / eleven `BusReply` variants byte-exact through `famp-canonical`; length-prefixed canonical-JSON codec (4-byte BE, 16 MiB cap); four RED-first TDD gates (codec fuzz, drain cursor atomicity, PID reuse race, EOF cleanup mid-await) all GREEN; five proptest properties (DM fan-in ordering, channel fan-out, join/leave idempotency, drain completeness, PID-table uniqueness) all GREEN. `just check-no-tokio-in-bus` permanent CI gate.
- **Atomic v0.5.1 → v0.5.2 spec bump** (Phase 1) — single commit `9ca6e13` lands `MessageClass::AuditLog` + `Relation::Audits` + `AuditLogBody` + `BusEnvelope<B>` (BUS-11 sibling type with private inner + 2 `compile_fail` doctests) + `AnyBusEnvelope` 6-arm dispatch + `EnvelopeDecodeError::UnexpectedSignature` + `FAMP_SPEC_VERSION = "0.5.2"` + T5 lag-block deletion + `vector_1` worked example + `just check-spec-version-coherence` CI guard. AUDIT-05 atomic-bump invariant honored end-to-end.
- **UDS broker daemon + 8-verb CLI** (Phase 2) — `famp broker` UDS daemon at `~/.famp/bus.sock` with `posix_spawn`+`setsid` auto-spawn (no double-fork), `bind()`-IS-the-lock single-broker exclusion, 5-minute idle exit with fsync+unlink, NFS-mount startup warning. User-facing `famp register | send | inbox | await | join | leave | sessions | whoami` rewires the v0.8 surface onto the bus; `~/.famp/mailboxes/<name>.jsonl` reuses `famp-inbox` JSONL with atomic temp-file+rename cursor advance. 14 plans across 7 waves; `kill -9` mid-Send recovery, two-near-simultaneous-register race, and bus-side MCP E2E (TEST-05) all green.
- **MCP rewire to bus + `famp-local hook add` declarative wiring** (Phase 2) — `famp mcp` drops `reqwest` + `rustls` from the startup path; `cargo tree -p famp` shows zero TLS reach for MCP. Eight stable tools (`famp_register`, `famp_send`, `famp_inbox`, `famp_await`, `famp_peers`, `famp_join`, `famp_leave`, `famp_whoami`) round-trip via stdio; MCP error-mapping is exhaustive `match` over `BusErrorKind` (no wildcard — adding a variant fails compile). `famp-local hook add --on Edit:<glob> --to <peer-or-#channel>` registers TSV rows to `~/.famp-local/hooks.tsv`; `list`/`remove` round-trip.
- **Claude Code integration polish** (Phase 3) — `famp install-claude-code` writes user-scope MCP config to `~/.claude.json` and drops 7 slash-command markdown files (`/famp-register`, `/famp-join`, `/famp-leave`, `/famp-send`, `/famp-channel`, `/famp-who`, `/famp-inbox`) into `~/.claude/commands/`. README Quick Start passes the **12-line / 30-second acceptance test** on a fresh macOS install. Codex parity ships as MCP-only install/uninstall via TOML structural merge. HOOK-04b execution runner registers a `hooks.Stop` entry pointing at `~/.famp/hook-runner.sh` (sourced from `crates/famp/assets/hook-runner.sh`, parameterized on `${FAMP_LOCAL_ROOT:-$HOME/.famp-local}`).
- **Federation CLI unwire + plumb-line-2 preservation** (Phase 4) — six federation CLI verbs deleted (`famp setup`, `famp listen`, `famp init`, `famp peer add`, `famp peer import`, old TLS-form `famp send`); `famp-transport-http` + `famp-keyring` relabeled "v1.0 federation internals" in workspace `Cargo.toml`. `e2e_two_daemons` refactored to library-API direct instantiation (full signed `request → commit → deliver → ack` over real HTTPS in-process), runs green in `just ci` every commit (FED-04 plumb-line-2 commitment against mummification). Tag `v0.8.1-federation-preserved` cut at `debed78` as escape hatch BEFORE deletions land. `docs/MIGRATION-v0.8-to-v0.9.md` ships table-first; `~27` federation-coupled tests parked under `crates/famp/tests/_deferred_v1/` for v1.0 reactivation.
- **Milestone-close fixes** (Phase 5, 2026-05-04) — `/famp-who [#channel?]` rewritten to call only `famp_peers` with client-side channel projection (CC-07 BROKEN → satisfied; keeps MCP surface stable at 8 tools); `crates/famp/assets/hook-runner.sh` parameterized to honor `FAMP_LOCAL_ROOT` (HOOK-04b PARTIAL → fully wired); retroactive `03-VERIFICATION.md` covering CC-01..10 + post-fix HOOK-04b; REQUIREMENTS.md sweep flipped 36 Phase-2 traceability rows Pending → Complete.

**Design authority:** [`docs/superpowers/specs/2026-04-17-local-first-bus-design.md`](../docs/superpowers/specs/2026-04-17-local-first-bus-design.md).

**v1.0 ship gates (unwelded 2026-05-09):** the single fused v1.0 trigger named at v0.9 close has been split into two independent ship gates per [`docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`](../docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md). **Gate A (Gateway gate):** Ben sustains symmetric cross-machine FAMP use (laptop ↔ home dev server, two equal agents) for ~2 weeks → unlocks `famp-gateway`, reactivates `crates/famp/tests/_deferred_v1/`, tags `v1.0.0` (no `-rc`). **Gate B (Conformance gate):** a 2nd implementer commits to interop and exercises the wire format against their own code lineage → unlocks the conformance vector pack at whatever release tag is current. The 4-week clock has been retired (both gates are event-driven; the clock was anti-mummification insurance for the fused trigger and is unnecessary once the gates are independent). The "Sofer or named equivalent" framing survives only as Gate B's activation condition; Gate A is activated by Ben's own use case.

**Audit:** `passed` (85/85 reqs, 5/5 phases verified, 6/6 flows wired) — see [milestones/v0.9-MILESTONE-AUDIT.md](milestones/v0.9-MILESTONE-AUDIT.md). Tech debt deferred per audit (8 pre-existing TLS-loopback test timeouts on macOS; WR-06 env-var test races; 6 IN-* info-level review findings; minor doc stale references) documented in audit `tech_debt` block.

**Known deferred items at close:** 33 (30 orphan quick_task slugs from federation-era + v0.9 prep-sprint drift + 2 v1.0-gated dormant seeds + 1 UAT header status drift; see STATE.md `## Deferred Items`).

---

## v0.8 Usable from Claude Code (Shipped: 2026-04-26)

**Phases completed:** 5 phases (4 archived 2026-04-15 + 1 bridge phase 2026-04-26), 18 plans, 419/419 tests green

**Delivered:** Two Claude Code sessions on the same laptop can each drive a `famp` agent via MCP tools, register as different identities at runtime, and exchange a long task — proven end-to-end by `crates/famp/tests/mcp_session_bound_e2e.rs`.

**Key accomplishments:**

- **Persistent on-disk identity** (Phase 1) — `famp init` produces Ed25519 keypair (0600), self-signed TLS cert via rcgen, `config.toml`, empty `peers.toml`. `FAMP_HOME` override drives every subcommand.
- **`famp listen` daemon + durable JSONL inbox** (Phase 2) — axum + rustls server reusing v0.7's `FampSigVerifyLayer` byte-for-byte; inbox writes are fsync-sealed before HTTP 200; tail-tolerant reader survives crash mid-write; SIGINT/SIGTERM clean shutdown; bind-collision returns typed `PortInUse`.
- **One-long-task conversation CLI** (Phase 3) — `famp send/await/inbox/peer add` over the v0.7 FSM unmodified; task records survive daemon restarts; advisory `inbox.lock` prevents double-consumption; v0.5.1 envelope schemas unchanged (CONV-05 checkpoint proves v0.7 was expressive enough).
- **`famp mcp` stdio JSON-RPC server** (Phase 4) — hand-rolled Content-Length framing, four tools (`famp_send`/`famp_await`/`famp_inbox`/`famp_peers`), exhaustive `CliError::mcp_error_kind()` (28 variants, no wildcard) so misuse is structurally categorizable. Multi-entry keyring + auto-commit handler enable two-daemon flows.
- **Session-bound MCP identity (v0.8.x bridge)** (2026-04-26) — `famp_register` / `famp_whoami` tools added; `famp mcp` stops reading `FAMP_HOME` at startup (reads `FAMP_LOCAL_ROOT` only); pre-registration `not_registered` gating on the four messaging tools; B-strict variant (no `legacy_famp_home` grace period); archived `docs/history/v0.9-prep-sprint/famp-local/famp-local` auto-rewrites legacy `.mcp.json` files in place; two-MCP-server E2E test locks the user-visible promise. Pull-forward of v0.9 MCP contract onto the v0.8 substrate.
- **No openssl, no native-tls** — `cargo tree -i openssl` empty across the entire 11-crate workspace. 419/419 tests green; clippy clean (`-D warnings`).

**Audit:** `tech_debt` — see [milestones/v0.8-MILESTONE-AUDIT.md](milestones/v0.8-MILESTONE-AUDIT.md). Seven items deferred to v0.9 (TD-1 nextest parallelism pin; TD-3 INBOX-01 wording; TD-4 receiver-side task seed; TD-7 Nyquist validation backfill; cosmetic items TD-2/5/6 closed inline at audit).

**Known deferred items at close:** 23 (22 quick_task index drift + 1 stale SEED-001 marker; see STATE.md `## Deferred Items`).

---


## v0.8 Usable from Claude Code (Shipped: 2026-04-15)

**Phases completed:** 4 phases, 13 plans
**Timeline:** 2026-04-14 → 2026-04-15 (2-day execution)
**Test footprint:** 355/355 workspace tests green; `just ci` clean; `cargo tree -i openssl` empty
**Requirements:** 37/37 satisfied (CLI-01..07, IDENT-01..06, DAEMON-01..05, INBOX-01..05, CONV-01..05, MCP-01..06, E2E-01..03)

**Key accomplishments:**

- **`famp init` creates persistent identity** — Ed25519 keypair (0600 permissions), self-signed TLS cert via `rcgen`, `config.toml` + `peers.toml`. `FAMP_HOME` env var override for test isolation.
- **`famp listen` daemon** — wraps v0.7 `famp-transport-http` with durable JSONL inbox (fsync-before-200), SIGINT/SIGTERM graceful shutdown, single-instance port guard, auto-commit handler for inbound requests.
- **`famp-inbox` crate** — append-only JSONL with atomic fsync, tail-tolerant reader (survives mid-write crash), advisory lock for concurrent access, cursor-based read tracking.
- **Conversation CLI** — `famp send --new-task/--task/--terminal`, `famp await --timeout`, `famp inbox`, `famp peer add`. Task records persist in `~/.famp/tasks/` and survive daemon restarts. TLS TOFU pinning on first contact.
- **`famp mcp` stdio server** — JSON-RPC over stdin/stdout with 4 tools (`famp_send`, `famp_await`, `famp_inbox`, `famp_peers`). Exhaustive `CliError::mcp_error_kind()` mapping (28 variants, no wildcard).
- **E2E-01 automated test** — two-daemon harness with mutual peer registration, full `request → auto-commit → 4 delivers → terminal → COMPLETED` lifecycle under `cargo nextest`.
- **E2E-02 manual smoke test PASSED** — CLI-based test (5 delivers exchanged, task COMPLETED). MCP server works but Claude Code integration needs debugging. Inbox artifacts archived.

---

## v0.7 Personal Runtime (Shipped: 2026-04-14)

**Phases completed:** 4 phases, 15 plans, 18 tasks

**Key accomplishments:**

- 19 integration tests
- MessageClass and TerminalStatus lifted from famp-envelope to famp-core via backward-compatible re-exports, unblocking famp-fsm from any famp-envelope dependency (D-D1)
- 5-state TaskFsm engine with single-function transition table (5 legal arrows), terminal immutability enforcement, and 12 deterministic fixture tests covering all v0.7 happy paths plus 60-combo terminal rejection matrix
- Consumer stub under `#![deny(unreachable_patterns)]` proves variant-change safety at compile time; proptest matrix runs 2048 cases over the full 5×5×4 Cartesian product with an independent oracle, zero panics, and exact error-field assertions
- Crate skeleton
- Cargo wiring (`crates/famp/Cargo.toml`)
- `crates/famp/examples/personal_two_agents.rs`
- Task 1 — cycle_driver extraction + deps + fixtures
- 1. [Rule 3 - Blocking] reqwest `rustls-no-provider` feature fails at runtime

---

## Milestone Plan (adopted 2026-04-12)

FAMP v1 is staged across **two profiles**:

### Personal Profile — "library a solo dev can use today"

- **v0.6 Foundation Crates** *(shipped 2026-04-13)* — substrate. Byte-exact canonical JSON (RFC 8785), Ed25519 sign/verify with domain separation, compiler-checked core types. Crates: `famp-canonical`, `famp-crypto`, `famp-core`.
- **v0.7 Personal Runtime** *(shipped 2026-04-14)* — minimal usable library **on two transports**. Signed envelope with 5 message classes (`request`, `commit`, `deliver`, `ack`, `control/cancel`), 5-state task FSM (REQUESTED → COMMITTED → {COMPLETED | FAILED | CANCELLED}), `MemoryTransport` (same-process) + minimal HTTP transport (cross-machine), trust-on-first-use keyring bootstrapped from a local file. ~18 requirements across 4 phases. **Finish line: the same signed request/commit/deliver cycle runs two ways — `cargo run --example personal_two_agents` in one binary, and `cargo run --example cross_machine_two_agents` split across two shells/machines — and three negative tests (unsigned, wrong-key, canonical divergence) fail closed on both transports.**

### Federation Profile — "ecosystem-grade reference implementation"

Deferred to v0.8+. Rough milestone sketch (not yet committed):

- **v0.8 Identity & Cards** — Agent Card format, federation credential, capability declaration, pluggable trust store, `.well-known` card distribution
- **v0.9 Causality & Replay Defense** — freshness windows, bounded replay cache, idempotency-key scoping, supersession, cancellation-safe send path
- **v0.10 Negotiation & Commitment** — propose/counter-propose, round limits, capability snapshot binding, conversation FSM
- **v0.11 Delegation** — assist / subtask / transfer forms, transfer timeout, delegation ceiling
- **v0.12 Provenance** — graph construction, canonical serialization, redaction, signed terminal reports
- **v0.13 Extensions** — critical/non-critical registry, INV-9 fail-closed
- **v0.14 Adversarial Conformance + Level 2/3 Badges** — full CONF-* matrix, stateright model checking, automated conformance-badge runner, `famp` CLI

**Continuity guarantee:** the signing substrate from v0.6 is the same in both profiles. Personal Profile consumers simply don't reach for Federation Profile crates; Federation Profile work stacks on top without changing the canonical-JSON or Ed25519 contract. Nothing shipped in v0.6 or v0.7 needs to be re-derived when the federation semantics come online.

**Non-goal:** Personal Profile is not a conformance-release target. Level 2 + Level 3 conformance badges are a Federation Profile deliverable.

---

## v0.6 Foundation Crates (Shipped: 2026-04-13)

**Phases completed:** 3 phases, 9 plans, 16 tasks
**Timeline:** 2026-04-12 → 2026-04-13 (single-day execution)
**Crates shipped:** `famp-canonical`, `famp-crypto`, `famp-core`
**Test footprint:** 112/112 workspace tests green; `just ci` clean
**Requirements:** 25/25 satisfied (CANON-01..07, SPEC-02/03/18/19, CRYPTO-01..08, CORE-01..06)

**Key accomplishments:**

- **RFC 8785 canonical JSON byte-exact.** `famp-canonical` wraps `serde_jcs 0.2.0` behind a stable `Canonicalize` trait with the SEED-001 conformance gate wired into CI as a blocking pre-requisite. 12/12 gate green: Appendix B/C/E byte-exact, 100K cyberphone float corpus, UTF-16 supplementary-plane key sort, NaN/Infinity rejection, duplicate-key rejection. Nightly 100M-line full-corpus workflow armed with SHA-256 integrity check. 357-LoC from-scratch fallback plan committed on disk as insurance.
- **SEED-001 decision recorded with cited evidence** (`.planning/SEED-001.md`): keep `serde_jcs` — `ryu-js` number formatter proven correct against RFC 8785 Appendix B + cyberphone corpus; no fork needed.
- **Ed25519 signing primitives with hard strictness guarantees.** `famp-crypto` exposes only `verify_strict` (raw `verify` unreachable from public API), rejects weak / small-subgroup public keys at ingress via `TrustedVerifyingKey` newtype with committed must-reject fixtures, and prepends the SPEC-03 domain-separation prefix internally so callers can never assemble signing input by hand.
- **Worked Ed25519 example from PITFALLS P10 verifies byte-exact in Rust.** `§7.1c` fixture committed verbatim from external Python `jcs 0.2.1` + `cryptography 46.0.7`; blocking `test-crypto` CI job re-runs it on every push. RFC 8032 Ed25519 KATs also wired as a hard gate.
- **SHA-256 content-addressing (CRYPTO-07) closed via Plan 02-04.** `sha256_artifact_id` + `sha256_digest` backed by `sha2 0.11.0`, gated by NIST FIPS 180-2 Known Answer Tests. Identifier form `sha256:<hex>` consistent across `famp-canonical` and `famp-core::ArtifactId`.
- **Compiler-checked core types (`famp-core`).** `Principal`/`Instance` identity with wire-string round-trip; distinct UUIDv7 `MessageId`/`ConversationId`/`TaskId`/`CommitmentId` newtypes that cannot be accidentally swapped at call sites; `ArtifactId` with `sha256:<hex>` invariant enforced at parse time.
- **15-category `ProtocolErrorKind` + `AuthorityScope` ladder + INV-1..INV-11 anchors.** Flat enum covers all §15.1 wire categories with round-trip string codec; 5-variant authority ladder with hand-written 5×5 `satisfies()` truth table (no `Ord` derive); invariant constants namespaced in `famp_core::invariants`. Exhaustive consumer stub under `#![deny(unreachable_patterns)]` makes any new enum variant a hard compile error in every downstream crate.
- **CI-parity tech debt closed same day:** rustfmt drift fixed, `famp-canonical` integration-test clippy hygiene sweep (file-level allow for `unused_crate_dependencies` / `unwrap_used` / `expect_used` / `pedantic`), carried TODO from Plan 01-02 retired. `just ci` runs clean end-to-end.

---

## v0.5.1 Spec Fork (Shipped: 2026-04-13)

**Phases completed:** 2 phases, 9 plans, 15 tasks

**Key accomplishments:**

- rust-toolchain.toml pinning Rust 1.87.0 with rustfmt + clippy, dual Apache-2.0/MIT license files, .gitignore, docs/ placeholder, and copy-pasteable bootstrap README.
- 13-crate Cargo workspace with [workspace.dependencies] pinning all 16 protocol-stack crates, strict clippy deny-all lints, and green cargo build + test on empty stubs.
- Justfile + nextest two-profile config + 6-job GitHub Actions workflow establishing a CI-parity gate where `just ci` green locally implies green CI on push.
- FAMP-v0.5.1-spec.md stub at repo root with FAMP_SPEC_VERSION = "0.5.1" constant, plus scripts/spec-lint.sh ripgrep anchor lint wired into `just ci` as a mandatory gate.
- Fixed SPEC-01-FULL counter regex in `scripts/spec-lint.sh`.

---

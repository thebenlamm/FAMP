# Milestones

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

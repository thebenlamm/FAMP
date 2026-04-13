# Milestones

## Milestone Plan (adopted 2026-04-12)

FAMP v1 is staged across **two profiles**:

### Personal Profile — "library a solo dev can use today"

- **v0.6 Foundation Crates** *(shipped 2026-04-13)* — substrate. Byte-exact canonical JSON (RFC 8785), Ed25519 sign/verify with domain separation, compiler-checked core types. Crates: `famp-canonical`, `famp-crypto`, `famp-core`.
- **v0.7 Personal Runtime** *(next)* — minimal usable library **on two transports**. Signed envelope with 5 message classes (`request`, `commit`, `deliver`, `ack`, `control/cancel`), 4-state task FSM, `MemoryTransport` (same-process) + minimal HTTP transport (cross-machine), trust-on-first-use keyring bootstrapped from a local file. ~18 requirements across 4 phases. **Finish line: the same signed request/commit/deliver cycle runs two ways — `cargo run --example personal_two_agents` in one binary, and `cargo run --example cross_machine_two_agents` split across two shells/machines — and three negative tests (unsigned, wrong-key, canonical divergence) fail closed on both transports.**

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

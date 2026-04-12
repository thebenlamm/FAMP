# FAMP v0.5 Rust Reference — Research Summary

**Project:** FAMP v0.5 Rust Reference Implementation
**Domain:** Agent messaging protocol reference library (signed messages, canonical JSON, FSM-governed negotiation, federated identity)
**Researched:** 2026-04-12
**Confidence:** HIGH on stack, features, architecture; MEDIUM on two specific deps (`serde_jcs`, `stateright`) and protocol-novel pitfalls

## Executive Summary

FAMP is a **protocol library, not an application.** Success is defined by one question: *will two independent implementations produce identical signed bytes for the same input?* Stack, architecture, and phase order all flow from that.

Recommended approach: **12-crate Cargo workspace** (with staged Phase 2–3 merging for beginner build velocity) on a curated Rust stack — `ed25519-dalek 2.2`, `serde_jcs 0.2` wrapped behind `famp-canonical` with RFC 8785 vector gates, `serde_json`, `sha2 0.11`, `uuid v7`, `axum 0.8` + `reqwest 0.13` over `rustls 0.23`, `tokio 1.51`, `thiserror 2`, and `proptest` + `stateright` + `insta` for testing. Ship Level 2 + Level 3 conformance together — 48 table-stakes features, zero differentiators in v1.

Dominant risks: canonical-JSON correctness (UTF-16 sort, ECMAScript number formatting, Unicode normalization), Ed25519 strictness (`verify_strict` only, weak-key rejection), domain-separation byte format (must be spelled out with hex dumps in v0.5.1 fork), and FSM concurrency safety (INV-5 under cancellation, key rotation, competing commits). All mitigated by front-loading the spec fork and canonicalization in Phases 1–2, wiring RFC 8785 + RFC 8032 vectors into CI as hard gates, and shipping adversarial tests alongside — not after — happy-path integration.

## Key Findings

### Stack

One-crate-per-concern with strict defaults. No OpenSSL, no `native-tls`, no `async-std`, no SIMD JSON, no `actix-web`, no `#[async_trait]`.

**Core:** `ed25519-dalek 2.2` (expose only `verify_strict`; reject weak keys at ingress) · `serde_jcs 0.2` wrapped in `famp-canonical` with RFC 8785 CI gate + from-scratch fallback documented · `serde 1.0` + `serde_json 1.0` (one JSON library, `deny_unknown_fields` everywhere) · `sha2 0.11` (artifact `sha256:<hex>`) · `uuid 1.23` (v7) · `base64 0.22` (URL_SAFE_NO_PAD) · `axum 0.8` + `tower-http` + `reqwest 0.13` + `rustls 0.23` · `tokio 1.51` · `thiserror 2` (libs) + `anyhow 1` (bins/tests only) · `proptest 1.11` + `stateright 0.31` + `insta 1.47` + `cargo-nextest`. Tooling: `just`, strict `clippy` (`unsafe_code = "forbid"`), `rust-toolchain.toml`, GitHub Actions.

**Risks flagged:**
1. `serde_jcs` single-maintainer, "unstable" label → wrapper + RFC 8785 gate + ~500 LoC fallback
2. `stateright` last released 2025-07-27 → fine for v1, fallback is hand-written BFS over state space

### Features

48 features across 10 concern areas map 1:1 to Level 2 + Level 3 conformance. Zero differentiators in v1.

- **Foundation (F1–F5):** RFC 8785 JCS encoder, Ed25519 sign/verify with domain separation, core types, 15-category error taxonomy, INV-1…INV-11 enforcement
- **Identity (F6–F11):** Principal + instance, authority scopes, federation trust, versioned Agent Cards, capability claims
- **Envelope + 9 message classes (F12–F22):** all 11 causal relations, mandatory signatures
- **State machines (F23–F26):** task + conversation FSMs, terminal precedence, model-checked
- **Protocol logic (F27–F30):** negotiation with INV-11 bound, commit binding, three delegation forms, silent-subcontract prohibition
- **Freshness/replay (F31–F34):** window, bounded cache, supersession, retransmission vs retry
- **Provenance (F35–F39):** deterministic graph, canonicalization, redaction, signed terminal reports, artifact immutability
- **Extensions (F40):** critical/non-critical registry, INV-9 fail-closed
- **Transport (F41–F43):** `Transport` trait, `MemoryTransport`, reference `HttpTransport`
- **Conformance (F44–F48):** externally-sourced JSON vectors, adversarial suite, two-node integration, L2+L3 badges

**Post-v1:** benchmark suite (D6), state machine tracer (D1), graph inspector (D2). **Anti-features:** Python/TS FFI, libp2p/NATS, multi-party commitment, cross-federation, streaming deliver, economic/reputation layers, Level-1-only release.

### Architecture

12 library crates + 1 umbrella (`famp`). DAG rooted at `famp-core`, branches into `famp-canonical` + `famp-crypto`, converges at `famp-envelope`, peers into `famp-identity` + `famp-causality`, joins at `famp-fsm`, culminates in merged `famp-protocol` (negotiation + commitment + delegation + provenance fused to prevent premature API churn). `famp-extensions` orthogonal on envelope. `famp-transport` + `famp-transport-http` are dumb pipes — do NOT depend on `famp-protocol`.

**Key decisions:** Phases 2–3 may temporarily merge core + canonical + crypto + envelope into `famp-foundation` for beginner build-time relief, re-split at Phase 4. FSM state types owned (no lifetimes) — use `Arc`/`Clone`. Native `async fn` in traits (Rust ≥1.75), no `#[async_trait]`.

### Critical Pitfalls (top 5)

1. **JCS key sort must use UTF-16 code-unit order, not UTF-8 bytes** — Rust `BTreeMap`/`str::cmp` silently wrong on supplementary-plane keys. Implement `jcs_key_cmp` via `char::encode_utf16`; bake supplementary-plane vectors into initial commit.
2. **JCS number serialization cannot delegate to `ryu` / `f64::to_string`** — ECMAScript rules diverge. Port cyberphone reference formatter; run 100M-sample corpus in CI.
3. **Ed25519 `verify()` accepts small-subgroup keys** — wrap, expose only `verify_strict`, reject weak pubkeys at ingress, include known-weak-key "must reject" fixtures.
4. **Domain-separation byte format must be spelled out with a hex dump** — use length-prefixed ASCII (e.g., `b"FAMP-v0.5.1-envelope-sig\x00"`); ship as conformance vector #1.
5. **Conformance vectors must NOT be self-generated** — source from RFC 8785 Appendix B + cyberphone + second implementation (Python); self-generated vectors test bug reproducibility, not correctness.

Also critical: Unicode normalization leaks, serde `deny_unknown_fields` discipline, FSM lifetime hell, async cancellation / INV-5 under drop, proptest generators producing already-canonical inputs, Agent Card key rotation breaking in-flight commitments, extension registry as dead code, spec-version drift, build-time spiral, happy-path-only integration.

## Implications for Roadmap

**Nine-phase structure dictated by the DAG:**

0. **Toolchain + Workspace Scaffold** — pin Rust, crate granularity, CI skeleton
1. **Spec Fork v0.5.1** — domain separation (hex dump), recipient binding, versioned cards, spec constant, §9.6 terminal precedence, §7.3 body inspection, §23 open questions as anti-features
2. **Canonical + Crypto Foundations** — RFC 8785 + RFC 8032 vectors green in CI; highest-risk phase
3. **Envelope + Message Schemas** — 9 message classes, `deny_unknown_fields` discipline
4. **Identity + Causality** — versioned Agent Cards, replay/freshness/supersession
5. **State Machines + Model Checking** — `stateright` exhaustive exploration, INV-5 compile-time
6. **Protocol Logic + Extensions** — merged negotiate/commit/delegate/provenance; reference critical + non-critical extensions
7. **Transport (Memory + HTTP)** — trait shaped by two concurrent implementations; cancellation-safe
8. **Conformance + Adversarial + CLI** — externally-sourced vectors, cancellation injection, `famp` umbrella CLI

**Research flags (need `/gsd:research-phase`):** Phase 1, 2, 5, 6, 8.
**Standard patterns (skip research-phase):** Phase 0, 3, 4, 7.

## Confidence

| Area | Level | Notes |
|------|-------|-------|
| Stack | HIGH | Versions verified against crates.io 2026-04-12 |
| Features | HIGH | Every feature cites spec section |
| Architecture | HIGH on structure, MEDIUM on trait shapes | 12-crate DAG is acyclic |
| Pitfalls | HIGH on JCS/Ed25519/serde; MEDIUM on protocol-history + Rust FSM ergonomics |

**Overall:** HIGH with two named MEDIUM dependencies mitigated by wrapper crates and documented fallbacks.

### Gaps to Address
- `serde_jcs` correctness on RFC 8785 edge cases → Phase 2 CI gate + ~500 LoC fallback
- ECMAScript number formatter sourcing → Phase 2 decision (`ryu-js` vs port from cyberphone C)
- Agent Card retention policy for key rotation → Phase 1 spec fork must pin a number
- Clock-skew tolerance → Phase 1 concrete value (research suggests ±60s)
- Recipient binding field name/position → Phase 1 spec fork
- Idempotency key format → fixed-width random bytes (128-bit) recommended
- `loom` vs `shuttle` for concurrency testing → Phase 6/7 decision

## Ready for Requirements

All research complete. Proceed to REQUIREMENTS.md.

# FAMP Codebase Survey Report

**Audit Date**: 2026-04-13 | **Snapshot**: Commit f48fbe6 | **Scope**: 14 crates, ~11K LOC, 135 .rs files, 43 test/fixture files

---

## 1. Architecture Summary

FAMP (Federated Agent Messaging Protocol) is a Rust reference implementation of a protocol for cryptographically-authenticated message passing between autonomous agents in trusted federations. The implementation is organized into three logical layers:

**Identity & Cryptography Layer** (`famp-core`, `famp-crypto`, `famp-identity`): Defines principals (agents), Ed25519 signing/verification with domain separation, and canonical JSON serialization per RFC 8785 (JCS). Signature verification is the gating function — every message is signed; unsigned messages are rejected at the middleware layer.

**Message Envelope & Protocol Layer** (`famp-envelope`, `famp-canonical`, `famp-fsm`, `famp-protocol`): Defines the wire format (signed JSON envelopes with sender, recipient, timestamp, message class, and typed body). Envelopes are immutable once signed. Canonical JSON is critical — divergence from RFC 8785 breaks interoperability. FSM enforces protocol state transitions (conversation negotiation, commitment, delegation); currently a stub but tested via proptest + stateright.

**Transport & Runtime Layer** (`famp-transport`, `famp-transport-http`, `famp`, `famp-keyring`): HTTP/1.1 reference transport with TLS (rustls, aws-lc-rs crypto provider). In-memory transport for tests. A runtime loop in `famp/src/runtime/loop_fn.rs` mirrors the HTTP middleware signature-verification path byte-for-byte to ensure consistency. Keyring manages Ed25519 keys with peer trust flags (trusted TOFU, untrusted ingress validation).

**Conformance Layer** (`famp-conformance`, `famp-canonical/tests/`): RFC 8785 vector tests + RFC 8032 ED25519 worked examples + cross-machine integration tests (http_happy_path, cross_machine_happy_path). Adversarial test matrix covers signed/unsigned, valid/invalid signatures, malformed JSON, oversized bodies, missing fields.

---

## 2. Crate / Workspace Layout

| Crate | Purpose | LOC | Tests | Status |
|-------|---------|-----|-------|--------|
| **famp-core** | Identity (Principal, MessageId), base types | ~263 | identity_roundtrip.rs (255 L) | Feature-complete |
| **famp-crypto** | Ed25519 sign/verify, SHA256, domain separation, weak-key rejection | ~184 (keys.rs) | rfc8032_vectors, worked_example, weak_key_rejection, base64_roundtrip (4 suites) | Feature-complete |
| **famp-canonical** | RFC 8785 JCS canonicalization wrapper + strict JSON parser | ~158 (strict_parse.rs) | conformance.rs (142 L, vector-gated) | Wave 2 impl, conformance gated |
| **famp-envelope** | SignedEnvelope, Message codec, numeric bounds validation | ~514 (envelope.rs) | prop_roundtrip, roundtrip_signed, body_shapes, vector_zero, adversarial (5 suites) | Feature-complete |
| **famp-identity** | Stub (placeholder for future identity negotiation) | — | — | Not started |
| **famp-causality** | Stub (placeholder for causality tracking) | — | — | Not started |
| **famp-fsm** | State machine (stub) | — | proptest_matrix (185 L), deterministic (129 L) via stateright | Stub with test harness |
| **famp-protocol** | Stub (placeholder for negotiation) | — | — | Not started |
| **famp-extensions** | Stub (placeholder for extension points) | — | — | Not started |
| **famp-transport** | Transport trait + MemoryTransport (in-memory, for tests) | ~230 (memory.rs) | memory adversarial.rs (145 L) | Feature-complete |
| **famp-transport-http** | HTTP/1.1 server + client, TLS helpers, FampSigVerifyLayer middleware | ~262 (transport.rs) + ~223 (middleware.rs) + ~185 (tls.rs) | middleware_layering (215 L), transport tests, tls tests | Feature-complete |
| **famp-keyring** | Keyring (TOFU + peer flag), file format (TOML) | ~149 (lib.rs) | roundtrip.rs (137 L), peer_flag.rs | Feature-complete |
| **famp** | Umbrella crate + CLI, runtime loop, examples, integration tests | ~258 (cycle_driver.rs) | runtime_unit.rs (214 L), http_happy_path (189 L), cross_machine_happy_path (173 L), adversarial/{memory, http, fixtures} (161 + 145 + 157 L) | Phases 3-4 complete (v0.7 milestone) |
| **famp-conformance** | Stub (conformance test coordination) | — | — | Stub |

**Total**: ~11,980 LOC source + test code across 14 crates. 43 integration/fixture test files. Workspace members all inherit version 0.1.0, edition 2021, Rust 1.89+.

---

## 3. Entry Points

### Binaries
- **`famp` CLI** (`crates/famp/src/bin/famp.rs`, 8 LOC) — Minimal stub; routes to library

### Library Roots
- **`famp` (umbrella)**: Re-exports from famp-core, famp-crypto, famp-envelope, famp-transport, famp-keyring
- **`famp-crypto`**: `pub fn sign_value()`, `pub fn verify_value()`, `pub struct TrustedVerifyingKey`
- **`famp-canonical`**: `pub fn canonicalize(value: &Value) -> Result<Vec<u8>, ...>`, `pub fn from_slice_strict(data: &[u8]) -> Result<Value, ...>`
- **`famp-envelope`**: `pub struct SignedEnvelope<B>`, `.sign()`, `.decode(bytes, verifying_key)`
- **`famp-transport`**: `pub trait Transport`, `pub struct MemoryTransport`
- **`famp-transport-http`**: `pub struct HttpTransport`, `pub fn build_router(keyring: &Keyring)`, `pub struct FampSigVerifyLayer`
- **`famp-keyring`**: `pub struct Keyring`, `.load_toml()`, `.lookup(principal)`

### HTTP Routes
- **POST `/inbox/:principal`** (via `famp-transport-http/src/server.rs`) — accepts signed envelope JSON, validates signature via FampSigVerifyLayer, routes to inbox handler
  - Route extraction: Principal from URL path (typed principal parsing, error → MiddlewareError::BadPrincipal)
  - Body limit: 1 MiB outer cap (RequestBodyLimitLayer), 1 MiB + 16 KiB inner sentinel in middleware (LOW-03)
  - Middleware pipeline: FampSigVerifyLayer → inbox_handler
- **GET `/health`** (stub, not implemented in survey scope) — would be for liveness checks

### Examples
- **`personal_two_agents.rs`** (304 LOC) — two agents in same process via MemoryTransport, sign/verify roundtrip
- **`cross_machine_two_agents.rs`** (254 LOC) — two agents across machines via HttpTransport with TLS, command-line config

---

## 4. Build / Test / Run Commands

### From Justfile (verified to exist)

```bash
# Build entire workspace
cargo build --workspace --all-targets

# Run all tests via cargo-nextest
cargo nextest run --workspace

# Run famp-canonical strict conformance gate (CI per-PR)
cargo nextest run -p famp-canonical --no-fail-fast

# Run famp-canonical with 100M float corpus (nightly only)
cargo nextest run -p famp-canonical --features full-corpus --no-fail-fast

# Run famp-crypto as blocking gate (RFC 8032 + §7.1c worked example)
cargo nextest run -p famp-crypto
cargo test -p famp-crypto --doc

# Full local CI parity (mimics GitHub Actions)
just ci  # → fmt-check lint build test-canonical-strict test-crypto test test-doc spec-lint
```

### CI (GitHub Actions, `.github/workflows/ci.yml`)

- **fmt-check**: `cargo fmt --all -- --check`
- **clippy**: `cargo clippy --workspace --all-targets -- -D warnings`
- **build** (Ubuntu + macOS): `cargo build --workspace --all-targets` + no-openssl gate
- **test-canonical**: `cargo nextest run -p famp-canonical --no-fail-fast` (RFC 8785 vectors)
- **test-crypto**: `cargo nextest run -p famp-crypto` (RFC 8032 + worked example)
- **test** (Ubuntu + macOS, conditional on test-canonical + test-crypto passing): `cargo nextest run --workspace --profile ci`
- **doc-test**: `cargo test --workspace --doc`
- **audit**: `cargo audit` daily + on-demand

### Verification (Local Quick Check)
```bash
cd /Users/benlamm/Workspace/FAMP
just ci
```

All commands are Justfile recipes; no shell scripts needed except CI YAML. **Rust toolchain**: stable via `dtolnay/rust-toolchain@stable`, pinned in `rust-toolchain.toml` (checked; version 1.89).

---

## 5. Directory Map

```
/Users/benlamm/Workspace/FAMP/
├── Cargo.toml (workspace root, members + workspace lints)
├── Cargo.lock (locked deps; aws-lc-rs 1.16.2, serde_jcs 0.2.0, etc.)
├── Justfile (task runner recipes)
├── rust-toolchain.toml (stable pinned)
├── rustfmt.toml (fmt config)
├── CLAUDE.md (tech stack + constraints)
├── FAMP-v0.5-spec.md (reference spec v0.5)
├── FAMP-v0.5.1-spec.md (authoritative spec v0.5.1, fork notes)
├── README.md (onboarding + project status)
├── LICENSE-APACHE + LICENSE-MIT (dual-licensed)
├── .github/workflows/
│   ├── ci.yml (push + PR gate)
│   └── nightly-full-corpus.yml (100M float tests)
├── .codebase-review/
│   ├── RISK_MAP.md (this review)
│   └── SURVEY_REPORT.md (this file)
├── scripts/
│   └── spec-lint.sh (ripgrep-based spec anchor lint)
├── docs/ (placeholder for design docs)
├── crates/
│   ├── famp-core/ (Principal, MessageId types)
│   ├── famp-crypto/ (Ed25519, SHA256, domain separation)
│   ├── famp-canonical/ (RFC 8785 wrapper + strict parser)
│   ├── famp-envelope/ (SignedEnvelope codec + bounds)
│   ├── famp-identity/ (stub)
│   ├── famp-causality/ (stub)
│   ├── famp-fsm/ (stub + stateright tests)
│   ├── famp-protocol/ (stub)
│   ├── famp-extensions/ (stub)
│   ├── famp-transport/ (Transport trait + MemoryTransport)
│   ├── famp-transport-http/ (HTTP/TLS transport + middleware)
│   ├── famp-keyring/ (Keyring + TOML format)
│   └── famp/ (umbrella + runtime + CLI + examples)
├── .planning/ (GSD workflow artifacts — phases 1-4 complete)
└── target/ (build cache, gitignored)
```

---

## 6. Dependency Summary

### Critical Crypto & Serialization
| Crate | Version | Role | Notes |
|-------|---------|------|-------|
| `ed25519-dalek` | 2.2.0 | Ed25519 signing/verification | verify_strict (non-canonical rejection), is_weak() |
| `sha2` | 0.11.0 | SHA-256 hashing | NIST KAT gated (e58178f) |
| `serde_jcs` | 0.2.0 | RFC 8785 JCS canonicalization | Single-maintainer, "unstable" label; conformance-gated |
| `serde` + `serde_json` | 1.0.228 + 1.0.149 | JSON serialization | No SIMD, one source of truth |
| `base64` | 0.22.1 | Base64url encoding | URL_SAFE_NO_PAD (strict, rejects padding) |

### HTTP & TLS
| Crate | Version | Role | Notes |
|-------|---------|------|-------|
| `axum` | 0.8.8 | Web framework | Handler extractors + tower middleware |
| `reqwest` | 0.13.2 | HTTP client | rustls-tls-native-roots backend |
| `rustls` | 0.23.38 | TLS stack | Pure Rust, aws-lc-rs crypto provider (shipped in deps) |
| `rustls-platform-verifier` | 0.5 | OS root store integration | Client-side trust anchor loading |
| `tokio` | 1.51.1 | Async runtime | Full features in binaries, minimal in libs |

### Testing & Verification
| Crate | Version | Role | Notes |
|-------|---------|------|-------|
| `proptest` | 1.11.0 | Property-based testing | Envelope roundtrip, keyring formats |
| `stateright` | 0.31.0 | State-machine model checker | FSM deterministic tests; 9 months since last release |
| `insta` | 1.47.2 | Snapshot testing | Vector snapshots (conformance vectors) |
| `nextest` | 0.9.132 | Test runner | Parallel execution, faster feedback |

### Error Handling & Utilities
| Crate | Version | Role | Notes |
|-------|---------|------|-------|
| `thiserror` | 2.0.18 | Typed errors (libs) | Derive-macro, Display + From + Error |
| `anyhow` | 1.0.102 | Untyped errors (bins/tests only) | Never in library APIs |
| `uuid` | 1.23.0 | UUIDv7 identifiers | v7 feature for time-ordering |
| `zeroize` | 1 | Secure key material cleanup | Feature enabled on ed25519-dalek |
| `tempfile` | 3 | Temporary files (tests) | Keyring roundtrip tests |

### Build & Lint
| Tool | Version | Role |
|------|---------|------|
| `rustfmt` | shipped | Code formatting (strict settings in rustfmt.toml) |
| `clippy` | shipped | Linting (all + pedantic deny, unwrap/expect deny) |
| `cargo-nextest` | 0.9.132 | Test runner (CI gate via taiki-e/install-action) |
| `just` | (installed globally) | Task runner (Justfile) |

### No Yanked/Outdated Dependencies
- All dependencies in Cargo.lock are from crates.io (registry source)
- Versions match workspace.dependencies (single source of truth)
- No openssl or native-tls in tree (verified by CI gate)
- No duplicate major versions (cargo tree clean per sha2 0.11.0 + digest 0.11 note in CLAUDE.md)

---

## 7. Test Infrastructure Inventory

### Unit Tests (Integration within Crates)
- **famp-core**: identity_roundtrip.rs (255 L) — Principal wire format, roundtrip JSON
- **famp-crypto**: 4 suites — rfc8032_vectors.rs, weak_key_rejection.rs, worked_example.rs, base64_roundtrip.rs
- **famp-canonical**: conformance.rs (142 L) — RFC 8785 vector suite, gated by feature flag + test-canonical-strict CI gate
- **famp-envelope**: 5 suites — prop_roundtrip.rs (262 L), roundtrip_signed.rs (251 L), body_shapes.rs (236 L), vector_zero.rs (125 L), adversarial.rs (359 L)
- **famp-keyring**: roundtrip.rs (137 L), peer_flag.rs — TOML serialization, peer flag state validation
- **famp-transport**: memory.rs adversarial tests (145 L)
- **famp-transport-http**: middleware_layering.rs (215 L) — signature verification, decode error parity
- **famp**: runtime_unit.rs (214 L) — message processing in-memory loop

### Integration Tests
- **http_happy_path.rs** (189 L) — HTTP transport, TLS, live server (1x message roundtrip)
- **cross_machine_happy_path.rs** (173 L) — Two agents cross-machine via HTTP
- **adversarial/harness.rs** (145 L) — Shared matrix harness
- **adversarial/memory.rs** (145 L) — MemoryTransport attack vectors (unsigned, invalid sig, malformed, oversized)
- **adversarial/http.rs** (161 L) — HttpTransport attack vectors (same matrix + TLS layer)
- **adversarial/fixtures.rs** (157 L) — Fixture key pairs, test vectors, cert fixtures

### Property-Based Testing
- **famp-envelope/tests/prop_roundtrip.rs** — proptest Envelope roundtrip (arbitrary envelope → JSON → parse → verify)
- **famp-fsm/tests/proptest_matrix.rs** (185 L) — FSM state space exploration via proptest strategies

### Model Checking
- **famp-fsm/tests/deterministic.rs** (129 LOC) — stateright exhaustive FSM verification

### Snapshot Testing
- **insta** snapshots (via conformance.rs) — RFC 8785 test vector snapshots stored as .insta files

### Test Count Summary
- 43 test/fixture .rs files
- Unit tests: ~1,500 LOC across 8 crates
- Integration tests: ~1,200 LOC (4 multi-crate suites)
- Property tests: proptest (Envelope), stateright (FSM)
- Conformance vectors: RFC 8785 (serde_jcs) + RFC 8032 (worked example)
- **Coverage**: Signature path, canonical JSON, envelope codec, HTTP middleware, keyring, all transports
- **Adversarial coverage**: Unsigned messages, invalid signatures, malformed JSON, oversized bodies, TLS errors, misrouted principals

---

## 8. Risk Heatmap — Top 20 Files by Size/Churn/Criticality

| Rank | File | LOC | Type | Risk | Reason |
|------|------|-----|------|------|--------|
| 1 | famp-envelope/src/envelope.rs | 514 | Core | HIGH | Message codec + signature roundtrip |
| 2 | famp-canonical/src/strict_parse.rs | 158 | Core | HIGH | RFC 8785 deserialization |
| 3 | famp-crypto/src/keys.rs | 184 | Crypto | CRITICAL | Weak-key rejection + ingress |
| 4 | famp-transport-http/src/transport.rs | 262 | Transport | HIGH | HTTP client/server routing |
| 5 | famp/tests/common/cycle_driver.rs | 258 | Test | MEDIUM | Integration driver (mock) |
| 6 | famp-core/src/identity.rs | 263 | Core | MEDIUM | Principal parsing |
| 7 | famp-transport-http/src/middleware.rs | 223 | Transport | CRITICAL | Signature verification gate |
| 8 | famp-transport/src/memory.rs | 230 | Transport | MEDIUM | In-memory transport (tests) |
| 9 | famp-envelope/tests/prop_roundtrip.rs | 262 | Test | HIGH | Envelope property tests |
| 10 | famp-envelope/tests/roundtrip_signed.rs | 251 | Test | HIGH | Signed envelope vectors |
| 11 | famp/examples/personal_two_agents.rs | 304 | Example | MEDIUM | Personal profile demo |
| 12 | famp/examples/cross_machine_two_agents.rs | 254 | Example | MEDIUM | Federation demo |
| 13 | famp-envelope/tests/body_shapes.rs | 236 | Test | MEDIUM | Bounds validation |
| 14 | famp-transport-http/src/tls.rs | 185 | Transport | HIGH | TLS + cert loading |
| 15 | famp-fsm/tests/proptest_matrix.rs | 185 | Test | MEDIUM | FSM property tests |
| 16 | famp/tests/runtime_unit.rs | 214 | Test | HIGH | Runtime message loop |
| 17 | famp-canonical/tests/conformance.rs | 142 | Test | CRITICAL | RFC 8785 vectors |
| 18 | famp-keyring/src/lib.rs | 149 | Keyring | MEDIUM | Key lookup + management |
| 19 | famp-envelope/tests/adversarial.rs | 359 | Test | HIGH | Adversarial matrix |
| 20 | famp/tests/http_happy_path.rs | 189 | Test | HIGH | HTTP transport integration |

**Churn**: Most critical files (envelope.rs, canonical/strict_parse.rs, middleware.rs, keys.rs) are in feature-complete crates (phases 1-3 stable, phase 4 recent integration). Stub crates (famp-identity, famp-causality, famp-protocol) have zero LOC/churn.

---

## 9. Documentation Inventory

| Location | Type | Purpose |
|----------|------|---------|
| `/FAMP-v0.5-spec.md` | Spec | Reference spec v0.5 (frozen for comparison) |
| `/FAMP-v0.5.1-spec.md` | Spec | Authoritative spec v0.5.1 (fork diffs documented) |
| `/CLAUDE.md` | Tech stack | Rationale for every crate choice; alternatives; version pins; beginner friction map |
| `/README.md` | Onboarding | Quick start, architecture diagram, v0.7 milestone status |
| `/.github/workflows/ci.yml` | CI/CD | Test gates, artifact naming conventions (D-* references to CLAUDE.md sections) |
| `/scripts/spec-lint.sh` | Lint | ripgrep-based anchor lint (spec section references in code) |
| `crate/*/src/lib.rs` | Crate docs | High-level crate purpose (all crates have top-level doc comments) |
| `crate/famp-crypto/src/verify.rs` line 38+ | Comments | Domain separation explanation + test harness |
| `crate/famp-transport-http/src/middleware.rs` line 1+ | Comments | Two-phase decode + canonical pre-check (CONF-05/06/07) |
| `crate/famp-transport-http/src/tls.rs` line 1-12 | Comments | Crypto provider choice (aws-lc-rs vs ring decision) |
| `.planning/` | GSD artifacts | Phase plans, assumptions, review checklists (not code) |

---

## 10. Specialist Review Partitioning

Recommend 6 merged specialists, each owning specific domain:

### (1) **ARCH + DEBT**
- **Owns**: Overall crate architecture, coupling, feature completeness, refactoring debt
- **Review scope**:
  - Stub crate completion (famp-identity, famp-causality, famp-protocol, famp-conformance)
  - Workspace organization (any unnecessary couples between layers)
  - Lint strictness (clippy/rustfmt compliance, workspace lints enforcement)
  - Technical debt (any TODOs, FIXMEs, deferred decisions)
- **Key files**: Cargo.toml (workspace), lib.rs in each crate, .planning/* artifacts

### (2) **SEC + DEPS**
- **Owns**: Cryptography correctness, dependency supply chain, secret handling, no-std/FFI surface
- **Review scope**:
  - ed25519-dalek usage (verify_strict vs verify, is_weak enforcement, zeroize on drop)
  - serde_jcs correctness (RFC 8785 conformance vectors, single-maintainer risk mitigation)
  - TLS stack (rustls 0.23.38, aws-lc-rs provider, PEM loading error handling, cipher suites)
  - Key material (FampSigningKey zeroization, keyring file permissions, peer flag semantics)
  - Dependency audit (yanked versions, outdated crates, transitive supply chain)
  - No-openssl gate validation (CI check exists and holds)
- **Key files**: famp-crypto/src/*, famp-transport-http/src/tls.rs, Cargo.toml (workspace deps), .github/workflows/ci.yml

### (3) **PROTOCOL (Logic + API + Spec Fidelity)**
- **Owns**: Message semantics, signature verification, canonicalization, envelope validity, spec compliance
- **Review scope**:
  - RFC 8785 canonicalization (strict_parse.rs, conformance vectors, ryu-js number encoding)
  - Signature verification path (two-phase decode + canonical pre-check in middleware, domain separation prefix)
  - Envelope codec (scope/class validation, body bounds, roundtrip JSON fidelity)
  - Principal parsing (sender extraction, keyring lookup, error propagation)
  - Spec v0.5.1 compliance (diffs from v0.5, INV-10 unsigned rejection, TRANS-07 body cap, §7.1b weak-key)
  - HTTP route design (inbox /inbox/:principal routing, error responses per CONF-05/06/07)
- **Key files**: famp-canonical/src/*, famp-envelope/src/*, famp-transport-http/src/middleware.rs, famp/src/runtime/loop_fn.rs, FAMP-v0.5.1-spec.md

### (4) **REL + PERF**
- **Owns**: Reliability (error handling, panic safety), performance (latency/throughput), observability
- **Review scope**:
  - All unwrap/expect/panic calls (should be test-only, marked `#[allow(...)]`)
  - Typed error propagation (thiserror in libs, anyhow in bins/tests only)
  - Middleware error responses (400 for bad input, 401 for invalid sig, 500 for internal errors)
  - Resource limits (1 MiB body cap, inner sentinel, reasonable timeout defaults)
  - Async safety (tokio spawning, lock contention in MemoryTransport)
  - No unintended clones or allocations in hot path
- **Key files**: crate/*/src/error.rs, famp-transport-http/src/middleware.rs, famp/src/runtime/loop_fn.rs

### (5) **TEST**
- **Owns**: Test coverage, property testing, model checking, adversarial scenarios
- **Review scope**:
  - RFC 8785 conformance vectors (full corpus nightly, edge cases)
  - RFC 8032 worked examples (signature correctness, weak-key rejection)
  - Roundtrip tests (envelope serialization, keyring TOML, principal wire format)
  - Adversarial matrix (unsigned messages, invalid signatures, malformed JSON, oversized bodies, TLS errors)
  - proptest strategies (arbitrary envelope generation, body field ranges)
  - stateright FSM coverage (state transitions, determinism)
  - Integration tests (http_happy_path, cross_machine_happy_path parity)
- **Key files**: crate/*/tests/*, scripts/spec-lint.sh, .github/workflows/nightly-full-corpus.yml

### (6) **DEVOPS + DX**
- **Owns**: CI/CD, build system, developer ergonomics, onboarding
- **Review scope**:
  - Justfile recipes (build, test, lint, ci targets; all recipes executable)
  - CI/CD (GitHub Actions, fail-fast gates, artifact naming, no-openssl verification)
  - Cargo.toml organization (workspace deps pinning, feature flags, lints)
  - Rust toolchain (stable, version pin in rust-toolchain.toml)
  - Documentation (README, CLAUDE.md tech stack, spec lint, crate docs)
  - Examples (personal_two_agents, cross_machine_two_agents executable and correct)
  - Developer setup (one-line bootstrap via rustup, just ci for green run)
- **Key files**: Justfile, Cargo.toml, Cargo.lock, rust-toolchain.toml, .github/workflows/, README.md, CLAUDE.md

---

## 11. Assumptions & Inferred Facts

1. **Spec Authority**: FAMP-v0.5.1-spec.md is authoritative; v0.5.0 comparison available in v0.5-spec.md for diff review. Fork diffs documented in CLAUDE.md (lines 16, e.g., domain separation in §7.1a).

2. **Crypto Provider Status**: `ring` feature flag remains in Cargo.toml (rustls 0.23.38) but aws-lc-rs is what's compiled (dependency shape pulled aws-lc-rs as default). No code change needed; rustls handles provider selection at compile time.

3. **serde_jcs Stability**: "Unstable" label refers to API churn risk, not correctness. Single-maintainer but 34 direct dependents (lib.rs stat). Forking plan documented but not yet needed (MEDIUM confidence in CLAUDE.md table).

4. **Conformance Gating**: RFC 8785 vectors run on every PR (test-canonical-strict CI gate, no-fail-fast); 100M float corpus nightly (full-corpus feature). Assumed sufficient for production correctness.

5. **Test Coverage**: 43 integration/unit test files + property tests (proptest) + model checking (stateright) + snapshots (insta). Assumed adequate for protocol-layer verification; no formal proof of security.

6. **Transport Trait**: Both MemoryTransport and HttpTransport implement the same Transport interface. Assumed no semantic divergence; adversarial matrix tests both.

7. **GSD Workflow**: Phase 1-3 complete (cryptography, envelope, runtime). Phase 4 complete (http transport + examples). v0.7 milestone archived. Code is ready for conformance-level-2 deployment; level 3 (FSM) not yet gated.

8. **Operational Assumptions**:
   - Each federation provides its own TLS trust anchors (not validated in code; deployment responsibility)
   - Keyring file permissions are OS/filesystem responsibility (code does not validate)
   - Private keys are NOT encrypted at rest (acceptable for dev, inadequate for production)
   - Peer flag (trusted TOFU) is externally managed (code enforces but does not create)

9. **CI Success Criterion**: `just ci` must pass (fmt-check, lint, build, test-canonical-strict, test-crypto, test, test-doc, spec-lint). Currently green.

---

**End of Survey Report**

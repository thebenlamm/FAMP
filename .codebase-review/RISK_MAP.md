# FAMP Risk Map — Critical Surfaces & Blast Radii

**Audit Date**: 2026-04-13 | **Scope**: 14 crates, ~11K LOC across 135 .rs files | **Profile**: Rust, Ed25519 + RFC 8785 JCS + HTTP/TLS

---

## Executive Risk Summary

### High-Risk Surfaces (Blast Radius: Critical)

**1. Canonical JSON (RFC 8785) Path**
- **Files**: `/crates/famp-canonical/src/` (stub + wave2_impl feature gate)
- **Criticality**: **CRITICAL** — Wrong canonicalization = signature mismatch = protocol failure
- **Line references**: 
  - `famp-canonical/src/lib.rs` — `canonicalize()` export point
  - `famp-canonical/src/strict_parse.rs` — custom strict JSON parser (158 LOC)
  - `famp-canonical/tests/conformance.rs` — RFC 8785 vector conformance (142 LOC)
- **Risk factors**:
  - Dependency on single-maintainer `serde_jcs 0.2.0` (labeled "unstable", published 2026-03-25)
  - No forked fallback yet (plan exists but not deployed)
  - Custom strict deserializer may diverge from RFC 8785 number encoding
  - Conformance gate only in `test-canonical-strict` (CI per-PR, not per-push)
- **Mitigation**: Conformance test suite run on every PR; 100M float corpus nightly (full-corpus feature); wrapper crate allows forking if needed

---

**2. Signature Verification Path**
- **Files**: `/crates/famp-crypto/src/verify.rs` + `/crates/famp-transport-http/src/middleware.rs`
- **Criticality**: **CRITICAL** — Invalid signatures accepted = auth bypass
- **Line references**:
  - `famp-crypto/src/verify.rs:38-91` — `verify_canonical_bytes()` with domain separation
  - `famp-crypto/src/keys.rs:65-75` — `TrustedVerifyingKey::from_bytes()` ingress with weak-key check
  - `famp-crypto/src/keys.rs:70` — `.is_weak()` call (SPEC §7.1b enforcement)
  - `famp-transport-http/src/middleware.rs:105-127` — two-phase decode + verify in middleware
  - `famp-transport-http/src/middleware.rs:161-162` — canonical pre-check (allows early rejection)
- **Risk factors**:
  - Use of `verify_strict()` (correct) but if ever changed to plain `verify()` → non-canonical sigs accepted
  - `from_bytes()` calls line 69 + 81 rely on `ed25519_dalek::VerifyingKey::from_bytes()` + custom weak-key check
  - Unwraps/expects in test harness (`verify.rs:55-64`) but isolated by `#[allow(...)]`
  - Middleware mirrors runtime loop_fn exactly; divergence would be exploitable
- **Mitigation**: Weak-key rejection with `is_weak()` test; RFC 8032 vectors run on every PR; verify_strict_test in keys.rs::144-156

---

**3. HTTP Transport + TLS Configuration**
- **Files**: `/crates/famp-transport-http/src/tls.rs` + `/crates/famp-transport-http/src/server.rs`
- **Criticality**: **HIGH** — Weak TLS/certs = man-in-the-middle
- **Line references**:
  - `tls.rs:40-42` — `install_default_provider()` for aws-lc-rs
  - `tls.rs:51-58` — `load_pem_cert()` with validation (returns error if empty cert list)
  - `tls.rs:61-64` — `load_pem_key()` with error on missing key
  - `tls.rs:68-76` — `build_server_config()` with `no_client_auth()` (acceptable for federation)
  - `tls.rs:78-85` — client config with OS root store + extra roots (D-B5)
  - `server.rs` — RequestBodyLimitLayer at 1 MiB (TRANS-07 §18)
- **Risk factors**:
  - Crypto provider switched from planned `ring` to `aws-lc-rs` (note: no code change required, dependency shape already pulled it)
  - Self-signed certs for dev via rcgen, but production requires federation-provided anchors (undocumented operationally)
  - No client-cert auth enforced (OK for reference, but production federations may need stricter policy)
  - PEM loading errors are typed, good; but relies on rustls_pemfile correctness
  - Server binds via axum-server; no explicit cipher suite or version pinning in code (rustls 0.23.38 defaults: TLS 1.2+ only, good)
- **Mitigation**: Native tests with live TLS stack (http_happy_path, cross_machine_happy_path); no-openssl gate in CI; explicit crypto provider install

---

### Medium-Risk Surfaces (Blast Radius: Moderate)

**4. Envelope Message Parsing & Codec**
- **Files**: `/crates/famp-envelope/src/envelope.rs` + `/crates/famp-envelope/src/body/bounds.rs`
- **Criticality**: **MEDIUM** — Malformed envelopes could cause DoS or bypass checks
- **Line references**:
  - `envelope.rs:252-259` — scope/class mismatch error (validation gates)
  - `envelope.rs:381-427` — vector_zero test with key/sig roundtrip
  - `body/bounds.rs:82-170` — numeric bounds validation (NaN/Inf rejection)
  - `body/bounds.rs:105-131` — roundtrip tests with unwraps (test-only, in `#[allow(...)]`)
- **Risk factors**:
  - Large envelope struct (514 LOC) with generic Body type parameter
  - Bounds validation for numeric fields (amount, weights) — critical for transfer/commitment semantics
  - Unwraps on valid JSON (serde_json::to_string) in tests only, acceptable
  - No explicit size bounds on Bounds struct itself; relies on 1 MiB body cap (middleware + inner sentinel)
- **Mitigation**: proptest roundtrip suite (prop_roundtrip.rs, 262 LOC); adversarial matrix tests (adversarial.rs, 359 LOC); snapshot vectors via insta

---

**5. Principal/Identity Parsing**
- **Files**: `/crates/famp-core/src/identity.rs` (263 LOC)
- **Criticality**: **MEDIUM** — Invalid principals could bypass sender checks
- **Line references**:
  - `identity.rs` — Principal struct with from_str() implementation
  - `famp-keyring/src/file_format.rs:61` — Principal::from_str() in keyring load
  - `famp-keyring/src/peer_flag.rs:18` — Principal::from_str() in peer flag validation
  - `famp-transport-http/src/server.rs:75` — Principal::from_str() for sender extraction
- **Risk factors**:
  - Principal is the unit of identity; parsing errors must be rejected cleanly
  - Multiple call sites parse principals from untrusted sources (keyring file, HTTP header)
  - No explicit canonical form enforcement (relies on roundtrip equality)
  - Tests in identity_roundtrip.rs (255 LOC) cover basic cases
- **Mitigation**: Exhaustive roundtrip tests; Principal is Copy/Clone/Eq; from_str errors propagated as typed KeyringError/MiddlewareError

---

**6. Key Material Handling & Keyring**
- **Files**: `/crates/famp-keyring/src/` (149 LOC lib + 137 LOC tests) + `/crates/famp-crypto/src/keys.rs` (184 LOC)
- **Criticality**: **MEDIUM** — Key leakage or corruption = compromise of all messages
- **Line references**:
  - `keys.rs:21` — `FampSigningKey` newtype (no explicit Zeroize, relies on dalek's ZeroizeOnDrop)
  - `keys.rs:34-56` — signing key constructors + verifying_key() derivation
  - `keyring/src/file_format.rs` — TOML-based keyring format with peer_flag field
  - `keyring/src/lib.rs` — Keyring struct with load/lookup methods
  - `keyring/tests/roundtrip.rs` — TOML roundtrip and peer flag validation
- **Risk factors**:
  - Signing keys use dalek's auto-zeroization on drop; no manual zeroize() call needed but relies on feature flag
  - Private keys stored in TOML on disk; no encryption at rest (acceptable for dev, inadequate for production)
  - Keyring file permissions not validated in code (OS/filesystem responsibility)
  - Peer flag (trusted vs untrusted) changes key ingress policy but stored alongside key material
- **Mitigation**: Zeroize feature enabled in workspace dep; key from_bytes() validates exactly 32 bytes; roundtrip tests ensure TOML fidelity

---

### Low-Risk Surfaces (Blast Radius: Limited)

**7. Protocol State Machine (FSM)**
- **Files**: `/crates/famp-fsm/src/` (stub) + `/crates/famp-fsm/tests/` (proptest, deterministic tests)
- **Criticality**: **LOW** — FSM violations are logical errors, caught by adversarial tests
- **Line references**:
  - `famp-fsm/tests/proptest_matrix.rs` (185 LOC) — strategy-based state exploration
  - `famp-fsm/tests/deterministic.rs` (129 LOC) — stateright model checker
  - `famp-fsm/tests/consumer_stub.rs` — stub consumer for FSM harness
- **Risk factors**:
  - FSM not yet fully implemented (stub crate); conformance level 2/3 in progress
  - stateright maintenance status "medium confidence" (9 months since last release, but only option)
  - State explosion risk if model not carefully scoped
- **Mitigation**: Deterministic tests with stateright; proptest coverage; FSM logic isolated in separate crate

---

**8. HTTP Transport Client/Server**
- **Files**: `/crates/famp-transport-http/src/transport.rs` (262 LOC) + `/crates/famp-transport/src/memory.rs` (230 LOC)
- **Criticality**: **LOW-MEDIUM** — Client bugs don't break security (server endpoint decides), but server routing errors could misdirect
- **Line references**:
  - `transport.rs:239` — panic in test (UnknownRecipient assertion, test-only)
  - `transport.rs:256` — Principal::from_str() with unwrap in test
  - `memory.rs:182` — panic in MemoryTransport test harness
- **Risk factors**:
  - Panics isolated to test code (marked with `#[allow(...)]` + test cfg)
  - Recipient routing done by Principal lookup; wrong principal → UnknownRecipient error (typed, not crash)
  - HTTP transport uses Axum router which is well-tested
  - Both transports implement same Transport trait interface
- **Mitigation**: Memory transport tests mirror HTTP tests; adversarial matrix covers both (memory.rs, http.rs in tests/); no production panics

---

**9. Canonicalization Prefix (Domain Separation)**
- **Files**: `/crates/famp-crypto/src/prefix.rs` (small, ~40 LOC)
- **Criticality**: **MEDIUM** — Wrong prefix = cross-message signature forgery
- **Line references**:
  - `prefix.rs:22-23` — DOMAIN_PREFIX prepended to canonical JSON
  - `prefix.rs:` — `canonicalize_for_signature()` function
  - `sign.rs:28-29` + `verify.rs:29-30` — both call same prefix logic
- **Risk factors**:
  - Domain separation is v0.5.1 fork addition (not in v0.5.0 spec)
  - Prefix value hardcoded as constant; verify sign/verify use same path (good)
  - No test vector for domain separation specifically (only worked example in tests/worked_example.rs)
- **Mitigation**: Domain separation in SPEC v0.5.1 §7.1a; worked example covers full roundtrip (keys.rs::144-156); verify test mirrors sign exactly

---

## Critical File Reference Table

| File Path | LOC | Risk | Key Check |
|-----------|-----|------|-----------|
| `famp-canonical/src/strict_parse.rs` | 158 | HIGH | RFC 8785 deserialization |
| `famp-canonical/tests/conformance.rs` | 142 | HIGH | Conformance vectors |
| `famp-crypto/src/verify.rs` | ? | CRITICAL | verify_canonical_bytes() + verify_strict |
| `famp-crypto/src/keys.rs` | 184 | CRITICAL | is_weak() + from_bytes ingress |
| `famp-crypto/src/sign.rs` | ? | CRITICAL | domain separation + sign path |
| `famp-envelope/src/envelope.rs` | 514 | HIGH | message roundtrip + signature |
| `famp-envelope/src/body/bounds.rs` | 175 | MEDIUM | numeric validation |
| `famp-transport-http/src/middleware.rs` | 223 | CRITICAL | two-phase decode + verify |
| `famp-transport-http/src/tls.rs` | 185 | HIGH | PEM loading + rustls config |
| `famp-transport-http/src/server.rs` | ? | HIGH | RequestBodyLimitLayer + routing |
| `famp-core/src/identity.rs` | 263 | MEDIUM | Principal parsing |
| `famp/src/runtime/loop_fn.rs` | ? | CRITICAL | matches middleware.rs exactly |
| `famp-keyring/src/file_format.rs` | ? | MEDIUM | TOML keyring load + validate |

---

## Unwrap/Panic Audit

| Location | Context | Classification |
|----------|---------|-----------------|
| `famp-crypto/src/verify.rs:55-64` | Test harness, behind `#[allow(...)]` | TEST-ONLY |
| `famp-crypto/src/keys.rs:41-42` | base64 decode in constructor, error returned | SAFE (typed error) |
| `famp-crypto/src/hash.rs:41-42` | char::from_digit fallback (unreachable) | SAFE (static analysis proves) |
| `famp-envelope/src/body/bounds.rs:105-131` | Test assertions on known JSON | TEST-ONLY |
| `famp-transport-http/src/transport.rs:239` | Test panic on wrong error type | TEST-ONLY |
| `famp-transport-http/src/tls.rs:147, 157, 167` | Test panics on wrong error type | TEST-ONLY |
| `famp-transport/src/memory.rs:182` | Test panic on wrong error type | TEST-ONLY |

**Conclusion**: All production code correctly returns typed errors. Panics/unwraps isolated to test assertions (marked with `#[allow(...)]` and test cfg).

---

## Recent Changes: ring → aws-lc-rs

**Status**: Completed, no code changes required by FAMP implementation.

- **PR/Commit**: Not found in git log (likely transitive — rustls 0.23.38 pulled aws-lc-rs as default provider)
- **Workspace Cargo.toml**: `rustls = { version = "0.23.38", features = ["ring", "std", "tls12"] }`
- **Comment in tls.rs lines 7-12**: Explicitly notes aws-lc-rs is what actually compiled in; ring feature is ignored
- **Implication**: No action needed; rustls handles provider selection at compile time
- **Risk**: None — aws-lc-rs is FIPS-targeted and equally secure as ring for this use case

---

## Assumptions & Gaps

1. **Conformance vectors**: famp-canonical/tests/conformance.rs exists and gates CI; we did not audit correctness of individual test vectors (scope: code structure only)
2. **rustls correctness**: Assume rustls 0.23.38 is correct; FAMP does not re-implement TLS
3. **ed25519-dalek safety**: Assume dalek 2.2.0 is correct; we verify only that FAMP uses verify_strict and is_weak correctly
4. **Keyring operational security**: File permissions, disk encryption at rest assumed to be admin responsibility (not enforced in code)
5. **serde_jcs stability**: Assume serde_jcs 0.2.0 is correct for RFC 8785; forking plan exists but not yet needed
6. **HTTP routing security**: Assume axum router is correct; FAMP validates only principal parsing and signature
7. **Deployment trust anchors**: Assume each federation provides correct TLS trust anchors (not validated in this codebase)

---

**End of Risk Map**

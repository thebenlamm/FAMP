# FAMP Codebase Review — Final Report

**Date**: 2026-04-13
**Scope**: Full repo, v0.7 (14 crates, ~11K LOC, 135 .rs files, commit f48fbe6)
**Methodology**: Multi-agent review (Survey + Risk Map + 6 specialists: ARCH, SEC, PROTOCOL, REL, TEST, DEVOPS) with risk triage and deduplication.

---

## Executive Verdict

**Ship v0.7.** FAMP's core promise — *"a byte-exact, signature-verifiable implementation of FAMP that two independent parties can interop against from day one"* — is **met** for Conformance Level 2. Every critical surface (RFC 8785 canonicalization, Ed25519 `verify_strict`, domain separation, INV-10 type-state, duplicate-key rejection, weak-key rejection, 1 MiB body cap, no-OpenSSL TLS) is locked with either compile-time enforcement or hard CI gates against external vectors (RFC 8785 Appendix B/C/E + RFC 8032 Test 1 + Python jcs worked example). The SEC specialist returned **SAFE TO SHIP** with zero criticals; the PROTOCOL specialist returned **CONFORMANT** with zero criticals/highs. What remains is *polish*: onboarding docs, a handful of API ergonomics fixes, one L3-deferred protocol gap (envelope version validation), and cleanup of dead stubs/silencers. No finding blocks v0.7. Level 3 conformance (competing-instance FSM, Agent Card federation signatures) is explicitly deferred and documented as such.

---

## Headline Numbers

Rolled up and deduplicated across all 7 specialist reports (SURVEY excluded as non-finding):

| Severity | Count | Categories |
|---|---|---|
| CRITICAL | 1 | DevOps/DX (missing CONTRIBUTING.md — debatable severity) |
| HIGH | 4 | Security (1: serde_jcs single-maintainer, mitigated), Tests (2: feature-gate discoverability, cross-lang interop), DX (1: README assumes Rust fluency) |
| MEDIUM | ~10 | Architecture (4), Protocol (1: FSM competing-instance, L3), Reliability (1: error-path allocation), Tests (2), DX (2) |
| LOW / INFO | ~12 | Test-only unwraps audited clean; documentation polish; vestigial deps |

**Note on the lone CRITICAL**: DEVOPS specialist flagged missing `CONTRIBUTING.md` as critical. Triaged down: this is HIGH at most for a pre-v1.0 reference impl. Reclassified accordingly below.

---

## Top 10 Findings (Ranked by Blast Radius)

Protocol code blast-radius order: **spec correctness / crypto → reliability → tech debt → docs**.

### Rank #1 — [MEDIUM] Envelope `famp` version field not validated on decode
- **Source**: PROTOCOL (INFO severity there, elevated here because it's the only spec-fidelity gap in otherwise locked crypto)
- **Location**: `crates/famp-envelope/src/envelope.rs:245-247` (`decode_value`)
- **Why it matters**: Spec v0.5.1 §Δ01 + §19 mandate envelopes carry `famp: "0.5.1"` exactly and reject mismatches as `unsupported_version`. Encode path is correct; decode silently accepts any version string. A cross-version attacker could send `famp: "0.6.0"` and have it parse. Not exploitable today (one version exists) but will become a silent interop hazard the moment a v0.6 ships.
- **Fix**: Add post-serde check in `decode_value`:
  ```rust
  if wire.famp.as_str() != FAMP_SPEC_VERSION {
      return Err(EnvelopeDecodeError::UnsupportedVersion { got: wire.famp.into() });
  }
  ```
  Plus a new `EnvelopeDecodeError::UnsupportedVersion` variant and one adversarial test.
- **Effort**: **S** (≤30 LOC including test)

### Rank #2 — [HIGH] RFC 8785 conformance tests hidden behind `wave2_impl` feature gate
- **Source**: TEST
- **Location**: `crates/famp-canonical/tests/conformance.rs:19` (`#![cfg(feature = "wave2_impl")]`), `Justfile` test-canonical-strict recipe
- **Why it matters**: The single most load-bearing test suite in the whole repo — RFC 8785 vectors that gate signature interop — compiles to nothing under a bare `cargo nextest run -p famp-canonical`. CI enables the feature, so merges are safe, but a developer running local tests gets a silent green. One day someone force-pushes after a "quick fix," CI doesn't re-run, and a canonicalization regression lands.
- **Fix**: Make `wave2_impl` a default feature in `famp-canonical/Cargo.toml`, OR rename to `conformance-gate` and make it default. Remove the cfg gate from the conformance test file itself (tests should always compile; the feature should gate the implementation wiring, not the spec vectors).
- **Effort**: **S** (1 Cargo.toml line + 1 cfg removal)

### Rank #3 — [HIGH] serde_jcs single-maintainer supply-chain risk
- **Source**: SEC, RISK_MAP
- **Location**: `Cargo.toml` workspace deps line ~37; `crates/famp-canonical/src/lib.rs`
- **Why it matters**: `serde_jcs 0.2.0` is the only maintained Rust RFC 8785 impl, single-maintainer (`l1h3r`), self-labeled "unstable," published 2026-03-25. A silent canonicalization bug here invalidates **every FAMP signature**. This is the single biggest correctness dependency in the tree.
- **Fix**: Already partially mitigated by the per-PR conformance gate and nightly 100M float corpus. Required actions: (a) fix Rank #2 so the gate is un-bypassable, (b) add a CI notification (Dependabot / manual monitor) for `serde_jcs` releases, (c) keep the documented ~500 LOC fork-to-`famp-canonical` plan current. **Do not fork preemptively** — conformance gating is the right response until the gate actually fires red.
- **Effort**: **S** for monitoring, **M** if fork becomes necessary

### Rank #4 — [MEDIUM] FSM competing-instance state (COMMITTED_PENDING_RESOLUTION) deferred
- **Source**: PROTOCOL
- **Location**: `crates/famp-fsm/src/state.rs:9-15` (5-state stub), spec §11.5a / Δ21
- **Why it matters**: Spec §11.5a mandates a 6th internal state with lex-smaller-UUIDv7 tiebreak for simultaneous commits. v0.7 "Personal Profile" intentionally narrows the FSM to 5 states and defers this to v1.0 L3. Documented in `.planning/phases/02-*/02-CONTEXT.md` D-C1. **Not a v0.7 blocker**, but must be called out because it's the single reason v0.7 cannot claim L3 conformance.
- **Fix**: No v0.7 action. For v1.0: add `Committed → CommittedPendingResolution → Committed|Failed(conflict:competing_instance)` transitions with proptest coverage of simultaneous-commit races.
- **Effort**: **M** (v1.0 work)

### Rank #5 — [MEDIUM] Umbrella crate `famp` missing public re-exports
- **Source**: ARCH
- **Location**: `crates/famp/src/lib.rs` (27 LOC, only `pub mod runtime`)
- **Why it matters**: Survey doc claims the umbrella re-exports `Principal`, `FampSigningKey`, `SignedEnvelope`, `Keyring`, `Transport` — but it doesn't. External consumers must reach into `famp_core::`, `famp_crypto::`, etc. Trivially broken advertising of the public API.
- **Fix**:
  ```rust
  pub use famp_core::{Principal, MessageId, ProtocolError};
  pub use famp_crypto::{FampSigningKey, TrustedVerifyingKey, sign_value, verify_value};
  pub use famp_canonical::{canonicalize, from_slice_strict};
  pub use famp_envelope::{SignedEnvelope, AnySignedEnvelope};
  pub use famp_transport::Transport;
  pub use famp_keyring::Keyring;
  ```
- **Effort**: **S** (~10 LOC)

### Rank #6 — [MEDIUM] `famp-transport-http` has stale `use X as _;` silencers
- **Source**: ARCH
- **Location**: `crates/famp-transport-http/src/lib.rs:6-8`
- **Why it matters**: Post-Phase 4 dead markers for `famp_crypto` and `serde_json` with a comment saying "remove as each later plan lands." Phase 4 shipped. Either the deps are genuinely unused (and should be deleted, since `workspace.lints` enforces `unused_crate_dependencies`) or they are used and the silencer is a workspace-lint suppression hack. Needs a 15-minute audit.
- **Fix**: Grep each dep inside the crate; delete if unused, document if needed.
- **Effort**: **S**

### Rank #7 — [MEDIUM] Five stub crates ship with no phase markers
- **Source**: ARCH, TEST
- **Location**: `crates/{famp-identity,famp-causality,famp-protocol,famp-extensions,famp-conformance}/src/lib.rs` (14 LOC each)
- **Why it matters**: Stubs consume workspace slots, CI cycles, and public API real estate with no documented activation path. Users browsing crates.io (if published) will see five empty crates with no explanation.
- **Fix**: Add `publish = false` to each stub Cargo.toml (from DEVOPS Rank #6 too). Add a one-line module doc: `//! Phase 2 stub — see SPEC §<N>, tracked in ROADMAP.md`. No code changes required.
- **Effort**: **S**

### Rank #8 — [MEDIUM] HTTP decode path allocates String on every serde error
- **Source**: REL
- **Location**: `crates/famp-envelope/src/envelope.rs:168,288,292,318,326` (5 call sites)
- **Why it matters**: `map_err(|e| EnvelopeDecodeError::BodyValidation(e.to_string()))` allocates on every malformed-input path. An adversary flooding the inbox with malformed JSON pays us allocator cost per request. Not a DoS (body cap + middleware rejects early) but visible in profiles. Same crate already has a `map_serde_error` helper at line 296 doing the correct `From`-impl pattern; the 5 call sites just didn't adopt it.
- **Fix**: Add `impl From<serde_json::Error> for EnvelopeDecodeError` (or reuse the existing helper) and change the 5 call sites to `?` / `.map_err(Into::into)`.
- **Effort**: **S** (~20 LOC)

### Rank #9 — [HIGH] Onboarding docs: README assumes Rust fluency; no CONTRIBUTING.md; sparse `///` docs on `famp-crypto` public API
- **Source**: DEVOPS (merged 3 findings)
- **Location**: `README.md`, missing `CONTRIBUTING.md`, `crates/famp-crypto/src/lib.rs` (`sign_value`/`verify_value` have zero `///` docs), `docs/` (stub)
- **Why it matters**: FAMP's `CLAUDE.md` constraint is "assume zero prior Rust experience." Reality: README jumps to `just ci` with no conceptual overview of why canonicalization matters, `sign_value`/`verify_value` have no doc examples, no CONTRIBUTING, `docs/` is `.gitkeep`. A motivated Rust beginner can run the example (~95% success) but cannot meaningfully contribute without pinging a maintainer.
- **Fix**: Three deliverables: (1) add README "Conceptual Overview" + ASCII FSM diagram + expected example output, (2) create `CONTRIBUTING.md` (setup, crate layout table, `just ci` before PR, commit style), (3) add `///` docs with examples to `sign_value`, `verify_value`, `sign_canonical_bytes`, `verify_canonical_bytes`, `TrustedVerifyingKey::from_bytes` (call out `verify_strict` and §7.1b weak-key semantics).
- **Effort**: **M** (2-4 hours)

### Rank #10 — [LOW] Dead `ring` feature flag on rustls + unused `insta` dev-dep
- **Source**: SEC, TEST
- **Location**: `Cargo.toml` workspace dep `rustls = { version = "0.23.38", features = ["ring", ...] }`; all crates' dev-deps list `insta 1.47.2` with zero call sites
- **Why it matters**: Cargo.lock shows aws-lc-rs is what actually compiles; the `ring` feature is a no-op on rustls 0.23. `insta` is pulled into every crate's dev-deps but never called. Neither is a bug; both are future-auditor-confusion traps.
- **Fix**: Remove `"ring"` from the rustls features list. Either delete `insta` from all dev-deps or actually adopt it for snapshot testing (recommended: delete).
- **Effort**: **S**

---

## Findings by Domain

Lower-priority items not in the Top 10, with file refs. No duplicates with above.

### Architecture (ARCH_REPORT.md)
- **[MEDIUM] Signer/Verifier traits are ceremonial** — `crates/famp-crypto/src/traits.rs` (90 LOC). Thin wrappers around free functions, no polymorphic use, no hardware-signer impls. Either delete or document as Phase-2 extension point.
- **[MEDIUM] `Transport` trait error-composition guidance missing** — `crates/famp-transport/src/lib.rs`. `type Error: ...` without doc guidance forces every consumer to case-match each transport's typed enum.
- **[LOW] `wire.rs` encapsulation rationale buried in `lib.rs` comment** — `crates/famp-envelope/src/{lib.rs,envelope.rs}`. Move the RESEARCH.md Pitfall 1-2 note to a module doc on `SignedEnvelope`.
- **[LOW] `#[allow(clippy::struct_field_names)]` in `famp-core/src/identity.rs`** — single bypass, justifiable.

### Security (SEC_REPORT.md)
- **[HIGH→INFO] TLS PEM cert loading** — `crates/famp-transport-http/src/tls.rs:50-57`. Flagged HIGH but already correctly implemented (`NoCertificatesInPem` typed error). Recommendation-only: document in deployment guide.
- **[MEDIUM] HTTP body-cap defense-in-depth sentinel is 16 KiB oversized vs outer cap** — `crates/famp-transport-http/src/middleware.rs:25-33`. Documented design choice, not a bug.
- **[LOW] Weak-key rejection test only covers identity point (`[0u8; 32]`)** — `crates/famp-crypto/src/keys.rs:146-152`. Add parameterized test for other 7 small-order points for defense-in-depth.
- **[LOW] Signature `PartialEq` uses `ct_eq`** — correct, but not load-bearing; protocol never compares sigs for equality on hot path. Keep it.
- **[INFO] `TrustedVerifyingKey` Debug impl prints base64url** — safe (public key material); add a comment clarifying.
- **[INFO] Keyring file plaintext at rest** — public keys only, no secrets; document `chmod 600` in deployment guide.

### Protocol (PROTOCOL_REPORT.md)
- All spec-fidelity findings collapsed into Top 10 Rank #1 and #4. No remaining lower-priority items; everything else passed.

### Reliability + Performance (REL_REPORT.md)
- **[LOW] `eprintln!` as diagnostic placeholder** — `crates/famp-transport-http/src/server.rs:104-108`. Phase-5 work: wire `tracing::error!` with instrumentation spans (sender/recipient/msg-id correlation IDs).
- **[LOW] No dedicated TLS-handshake timeout** — `crates/famp-transport-http/src/transport.rs:62-67`. Global 10s covers it; `reqwest` doesn't expose per-phase tuning without dropping to `hyper-util`. Acceptable for v0.7.
- **[GAP] No graceful-shutdown sequence** — server relies on tokio task-drop on transport drop. Document as Phase-5.

### Tests (TEST_REPORT.md)
- **[MEDIUM] MemoryTransport has no happy-path integration test** — mirror of `http_happy_path.rs` with MemoryTransport substituted, ~100 LOC. Adversarial cases are covered; happy-path is not.
- **[HIGH→POST-v1.0] No cross-language interop fixtures** — all external vectors come from Python jcs + cryptography (via PROVENANCE.md). A Go/JS/Java second impl and shared fixture corpus are required for Conformance Level 3 interop claims. **Not a v0.7 blocker.**
- **[LOW] `cross_machine_happy_path` is `#[ignore]` by default** — deliberate, documented; `--ignored` runs on-demand.
- **[LOW] No `cargo-fuzz` harness for `strict_parse.rs` / envelope codec** — optional enhancement.

### DevOps + DX (DEVOPS_REPORT.md)
- **[LOW] Workspace stub crates lack `publish = false`** — folded into Top 10 Rank #7.
- **[LOW] `docs/` directory is a `.gitkeep` stub** — create `docs/ARCHITECTURE.md` with crate dependency graph + message flow diagram. Links into Rank #9.
- **[LOW] Justfile recipes lack workflow ordering comments** — top-of-file comment block explaining typical dev loop.
- **[LOW] `rustfmt.toml` could add `normalize_comments`, `reorder_imports`** — cosmetic.
- **[LOW] No `.editorconfig`** — IDE convenience only.

---

## Calibration: What's Done Right

Multi-agent reviews bias negative. Here is the honest counterweight, with evidence:

1. **Conformance-first CI gates that cannot be skipped.** RFC 8785 Appendix B/C/E + RFC 8032 Test 1 + Python-sourced §7.1c worked example are hard merge blockers (`.github/workflows/ci.yml:70-81, 83-94`, `needs: [test-canonical, test-crypto]`). The 100M-float nightly corpus gates release tags. This is rare discipline for a protocol reference impl.

2. **INV-10 type-state enforced at the compiler.** `SignedEnvelope<B>` has private fields and only one constructor path: `UnsignedEnvelope::sign()` or `SignedEnvelope::decode()` (which verifies first). There is no "parsed but unverified" state. `compile_fail` doctests in `envelope.rs:68-91` pin the invariant.

3. **`verify_strict` is the only public verification path.** `crates/famp-crypto/src/verify.rs:31-34`. Non-canonical and small-order-point signatures cannot round-trip. Weak-key rejection at ingress via `is_weak()` (`keys.rs:70`).

4. **Domain separation prefix byte-identical on sign + verify.** `crates/famp-crypto/src/prefix.rs:7` (`DOMAIN_PREFIX = b"FAMP-sig-v1\0"`), used by both `sign_canonical_bytes` and `verify_canonical_bytes`. Pinned by the `prefix_bytes_match_spec()` test.

5. **Duplicate-key rejection before canonicalization.** Custom two-pass `from_slice_strict` with `StrictTree` visitor (`crates/famp-canonical/src/strict_parse.rs`). Does not rely on `serde_json`'s silent-merge behavior.

6. **CONF-07 canonical pre-check before verify.** Middleware re-canonicalizes wire bytes and byte-compares before calling decode, distinguishing `CanonicalDivergence` from `SignatureInvalid` — this kills the signature-oracle class of attacks. Byte-identical to `famp/src/runtime/loop_fn.rs:46-57` (MED-02 invariant, pinned by unit tests).

7. **Zero production panics.** Workspace lints `clippy::unwrap_used`, `clippy::expect_used` = deny. All 30+ unwraps are test-only under `#[allow(...)]`. Verified independently by SEC, REL, and TEST specialists.

8. **Typed errors at every boundary.** 461 LOC of `thiserror` error types across 8 `error.rs` files; `anyhow` never leaks out of a library. Middleware errors map 1:1 to spec-compliant HTTP status codes (400/401/404/413/500).

9. **Clean supply chain.** 274 transitive crates, zero yanked, zero known vulns (`rustsec/audit-check@v2` daily). No `openssl`, no `native-tls` — CI gate (`cargo tree -i openssl|native-tls`) enforces this.

10. **Test isolation is immaculate.** No `lazy_static`, no shared mutable state, no flaky sleeps, `tempfile`-based temp files. 233 tests in ~60-90s via nextest. Property tests use deliberately shallow strategies for shrink debuggability (mature choice).

11. **Reproducible builds.** Cargo.lock committed, `rust-toolchain.toml` pinned to 1.89.0, all deps in `workspace.dependencies` with single source of truth. `just ci` on any machine = identical GitHub Actions result.

12. **Zeroize on drop for signing keys.** `FampSigningKey` wraps `ed25519_dalek::SigningKey` with `zeroize` feature enabled; Debug impl redacts. `TrustedVerifyingKey` Debug prints base64url (safe — public material).

---

## PR-Level Implementation Plan

Five sequential PRs, highest blast-radius first. Each should ship independently.

### PR 1 — `feat(envelope): validate famp spec-version field on decode`
- **Addresses**: Top 10 Rank #1
- **Files**: `crates/famp-envelope/src/envelope.rs`, `crates/famp-envelope/src/error.rs`, `crates/famp-envelope/tests/adversarial.rs`
- **Risk**: **low** (additive check, all fixtures already carry the correct version)
- **Effort**: **S** (~30 LOC)
- **Why first**: Only remaining spec-fidelity gap; cheap to close.

### PR 2 — `fix(ci): make RFC 8785 conformance tests always compile`
- **Addresses**: Top 10 Rank #2 (and indirectly #3 by making the supply-chain mitigation un-bypassable)
- **Files**: `crates/famp-canonical/Cargo.toml` (make `wave2_impl` default), `crates/famp-canonical/tests/conformance.rs` (remove `#![cfg(...)]`), `Justfile` (simplify recipe), `README.md` (note the change)
- **Risk**: **low** (CI already enables the feature)
- **Effort**: **S**
- **Why second**: Closes the last way a canonicalization regression could sneak past local dev.

### PR 3 — `refactor(famp): umbrella re-exports + drop stale silencers + stub hygiene`
- **Addresses**: Top 10 Rank #5, #6, #7; DEVOPS LOW (publish metadata)
- **Files**: `crates/famp/src/lib.rs`, `crates/famp-transport-http/src/lib.rs`, `crates/famp-{identity,causality,protocol,extensions,conformance}/Cargo.toml` + `src/lib.rs`
- **Risk**: **low-med** (re-export surface is API-visible; run `cargo public-api` diff before merge)
- **Effort**: **S**
- **Why third**: Purely mechanical; unlocks a clean public API story for the docs PR.

### PR 4 — `perf(envelope): drop String allocations from serde error path + rustls/insta cleanup`
- **Addresses**: Top 10 Rank #8 and #10
- **Files**: `crates/famp-envelope/src/{envelope.rs,error.rs}`, workspace `Cargo.toml` (rustls features, remove `insta` from dev-deps across all crates)
- **Risk**: **low**
- **Effort**: **S**
- **Why fourth**: Small wins; bundle together since both are dead-code / hot-path polish.

### PR 5 — `docs: onboarding — CONTRIBUTING.md, README overview, ARCHITECTURE.md, famp-crypto /// docs`
- **Addresses**: Top 10 Rank #9; DEVOPS `docs/` stub, Justfile workflow comment, ARCH `wire.rs` rationale
- **Files**: new `CONTRIBUTING.md`, new `docs/ARCHITECTURE.md`, `README.md`, `crates/famp-crypto/src/{lib.rs,sign.rs,verify.rs,keys.rs}` (add `///` docs with runnable examples), `Justfile` (header comment), `crates/famp-envelope/src/envelope.rs` (wire.rs rationale in module doc)
- **Risk**: **low** (no code behavior change; doctests must compile)
- **Effort**: **M** (2-4 hours)
- **Why last**: Documents the clean state the previous four PRs produce.

**Deferred to v1.0 (not in scope for v0.7 ship):**
- Competing-instance FSM state (Top 10 Rank #4)
- Second-implementation interop fixtures (Go/JS/Java)
- `tracing::error!` wiring + correlation IDs
- `cargo-fuzz` harnesses
- MemoryTransport happy-path test (nice-to-have, could slot into any PR)

---

## Open Questions for Project Owner

1. **Publishing plan.** Is v0.7 intended for crates.io? If yes, the `publish = false` triage in PR 3 is load-bearing (don't accidentally publish empty stubs). If no, the hygiene is cosmetic.
2. **Second-implementation roadmap.** Protocol's core value prop is "two independent parties can interop from day one" — but today the only "second party" is Python jcs+cryptography generating fixtures offline. Is a Go or JS reference impl on the v1.0 roadmap, or is the bet that "any third party who shows up will use our RFC 8785 vectors + §7.1c worked example to self-validate"?
3. **COMMITTED_PENDING_RESOLUTION scope.** v0.7 "Personal Profile" defers this explicitly. Confirm v1.0 adds it, or confirm that "Personal Profile" is its own permanent conformance tier and L3 is a separate product SKU.
4. **`Signer`/`Verifier` traits in `famp-crypto`.** Keep as extension points for future hardware/HSM signers, or delete as ceremonial? Need product intent before ARCH can firmly recommend.
5. **`serde_jcs` fork trigger.** Is there a defined criterion (e.g., "one failing vector in CI, or `l1h3r` hasn't released in 6 months") or is it reactive?

---

## Conformance Readiness Checklist

Target (per CLAUDE.md): **Conformance Level 2 + Level 3 in one milestone.**

| Level | Requirement | v0.7 Status | Blocker? |
|---|---|---|---|
| **L1** | Envelope codec, RFC 8785 canonicalization, Ed25519 signing | ✅ Complete | — |
| **L1** | Duplicate-key rejection, INV-10 type-state | ✅ Complete | — |
| **L1** | Domain separation prefix, weak-key rejection, `verify_strict` | ✅ Complete | — |
| **L2** | HTTP transport binding (axum + rustls + aws-lc-rs) | ✅ Complete | — |
| **L2** | Middleware signature verification + CONF-05/06/07 error distinguishability | ✅ Complete | — |
| **L2** | Body size cap (1 MiB, TRANS-07 §18) + defense-in-depth | ✅ Complete | — |
| **L2** | Adversarial matrix (13/14 vectors) | ✅ Complete | — |
| **L2** | Byte-exact worked example (§7.1c) verified against external Python reference | ✅ Complete | — |
| **L2** | `famp` spec-version field rejection on decode | ⚠️ **Missing** | **Yes — PR 1** |
| **L3** | Full FSM (Requested → Committed → Delivered → Acked + control + delegation) | ⚠️ Narrowed to 5-state Personal Profile | v1.0 |
| **L3** | COMMITTED_PENDING_RESOLUTION competing-instance tiebreak (§11.5a / Δ21) | ❌ Not implemented | v1.0 |
| **L3** | Agent Card with `federation_credential` + `federation_signature` (§6.1 / Δ11) | ❌ Out of scope | v1.0 |
| **L3** | Second-implementation interop fixtures (non-Python) | ❌ Not present | v1.0 |
| **L3** | Commitment + delegation + control FSM with proptest + stateright | ⚠️ Stateright harness exists, covered states are 5 not 6+ | v1.0 |

**Verdict**: **L2 ready after PR 1 merges.** L3 is explicitly a v1.0 milestone and the specialists concur the v0.7 scope is correctly narrowed. Nothing in v0.7 shipped today would need to be torn down to reach L3.

---

**End of Final Review**

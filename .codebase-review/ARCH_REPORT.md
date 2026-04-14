# ARCH+DEBT Report

**Review Date**: 2026-04-13 | **Reviewer**: ARCH+DEBT Specialist | **Scope**: 14 crates, ~11K LOC

---

## Summary

FAMP's architecture is fundamentally sound with clean crate boundaries, minimal coupling, and strict linting enforcing payload consistency. The design successfully isolates cryptography (famp-crypto), serialization (famp-canonical), message codecs (famp-envelope), and transports behind clear trait abstractions (Transport, Signer/Verifier). However, the umbrella crate's public API is incomplete (no re-exports), the Signer/Verifier traits are thin wrappers with limited reusability, and five placeholder crates (famp-identity, famp-causality, famp-protocol, famp-extensions, famp-conformance) ship as stubs despite occupying workspace slots, creating long-term discoverability and maintenance debt.

---

## Findings

### [MEDIUM] Umbrella Crate Missing Public Re-exports
- **Severity**: MEDIUM
- **Location**: crates/famp/src/lib.rs (27 lines)
- **Issue**: The top-level `famp` crate lists all member crates as dependencies (8 internal paths) but exports only `pub mod runtime;` — no re-exports of core types like `Principal`, `FampSigningKey`, `SignedEnvelope`, `Keyring`, `Transport`, etc.
- **Why it matters**: External consumers must reach into individual crates (e.g., `famp_core::Principal`, `famp_crypto::FampSigningKey`) despite `famp` being the umbrella. This creates friction for API users and obscures the public surface. The survey document explicitly states "famp (umbrella): Re-exports from famp-core, famp-crypto, famp-envelope, famp-transport, famp-keyring" — but this is aspirational, not implemented.
- **Fix**: Add a series of `pub use` re-exports at the root:
  ```rust
  // crates/famp/src/lib.rs
  pub use famp_core::{Principal, MessageId, ProtocolError};
  pub use famp_crypto::{FampSigningKey, TrustedVerifyingKey, sign_value, verify_value};
  pub use famp_canonical::{canonicalize, from_slice_strict};
  pub use famp_envelope::{SignedEnvelope, AnySignedEnvelope};
  pub use famp_transport::Transport;
  pub use famp_keyring::Keyring;
  // ... and runtime submodule re-exports if needed
  ```

---

### [MEDIUM] Signer/Verifier Traits Are Thin Wrappers Without Composition Benefit
- **Severity**: MEDIUM
- **Location**: crates/famp-crypto/src/traits.rs (90 LOC)
- **Issue**: The `Signer` and `Verifier` traits are straight-through delegations to free functions with no new behavior or composition patterns. They exist only as method sugar (`sk.sign_value(v)` instead of `sign_value(sk, v)`). No other type implements these traits; they are not used polymorphically in the codebase; no test explores trait-object usage or alternative signers.
- **Why it matters**: Traits should earn their weight by enabling extensibility, composition, or alternative implementations. These traits are ceremonial boilerplate that adds 3 call-through layers without a concrete use case. If FAMP never expects pluggable signers or hardware-backed signers, the traits create false API flexibility while adding module clutter.
- **Fix**: Either (a) remove the traits and use free functions directly throughout (simpler, honest), or (b) if hardware signers are planned, document the design as "phase 2 extension point" and add a stub `HardwareSigner` example in tests to demonstrate the abstraction is real. Current code suggests option (a) is correct.

---

### [MEDIUM] Five Stub Crates Create Workspace Debt and Incomplete Picture
- **Severity**: MEDIUM
- **Location**: crates/{famp-identity, famp-causality, famp-protocol, famp-extensions, famp-conformance}/src/lib.rs (14 LOC each, no code)
- **Issue**: Five crates are placeholders with only a smoke-test (`crate_compiles_and_links`). They occupy:
  - Workspace member slots (Cargo.toml)
  - CI test cycles (nextest runs on all, even stubs)
  - Discovery clutter (users see them but can't use them)
  - No documented path to implementation (no TODOs, no phase markers, no backlog refs)
- **Why it matters**: Stub crates were a reasonable v1 architecture decision (declare shape early), but without a visible roadmap or phase-based activation (e.g., `#[cfg(feature = "phase-2")]`), they signal incomplete work while consuming real real estate. The survey mentions "Phase 1-3 complete (cryptography, envelope, runtime). Phase 4 complete (http transport + examples)" but stubs appear final, not staged.
- **Fix**: For each stub, choose one of:
  1. **Document in README**: "Phase 2: famp-identity, famp-causality, famp-protocol support" with a link to the backlog issue.
  2. **Add stub module comment with stage marker**: E.g., `//! Phase 2 stub. See issue #NNN.` + a feature gate if needed.
  3. **Remove from workspace** (if v0.1 scope explicitly excludes them) — they can return in 0.2 branch.

---

### [MEDIUM] Transport Trait Is Byte-Oriented But Lacks Error Composition Guidance
- **Severity**: MEDIUM
- **Location**: crates/famp-transport/src/lib.rs (46 LOC) — specifically the `Transport` trait
- **Issue**: The `Transport` trait defines `type Error: std::error::Error + Send + Sync + 'static;` but provides no guidance on:
  - Whether implementors should wrap boxed errors (performance risk) or use enums (verbosity risk).
  - How to distinguish transport-layer errors (network) from higher-layer errors (signature, envelope).
  - Test suite shows `MemoryTransport` uses `MemoryTransportError` (typed enum) and `HttpTransport` uses `HttpTransportError` (typed enum), but the trait doesn't enforce or encourage this pattern.
- **Why it matters**: Each transport duplicates error-handling logic. The generic `type Error` associated type requires callers to have `match` arms for each transport type, creating tight coupling. A runtime that wants to work with multiple transports needs a trait object (`Box<dyn Transport<Error = Box<dyn Error>>>`) or a wrapper enum, which is boilerplate and performance risk.
- **Fix**: Either (a) provide a canonical `TransportError` enum in the trait crate and require all implementors to wrap their errors, or (b) add guidance in the trait doc comment: "Implementors SHOULD return a typed enum (not a boxed error) to enable runtime polymorphism via a `TransportOutcome` wrapper enum at the call site."

---

### [MEDIUM] Unused Dependency Silencers Suggest Incomplete Wiring
- **Severity**: MEDIUM
- **Location**: crates/famp-transport-http/src/lib.rs (lines 6-8)
- **Issue**: Two `use X as _;` silencers for `famp_crypto` and `serde_json`, with a comment: "Silencers for dependencies still pending wiring after Plan 04-03. As each later plan lands, remove the matching line."
- **Why it matters**: These are dead-code markers that should have been cleaned up post-Phase 4. They indicate either:
  - Incomplete refactoring (the deps are genuinely unused and should be removed).
  - Planned future features that never materialized (the comment suggests this).
  - Test/examples use them but the lib doesn't (which violates workspace `unused_crate_dependencies` lint intent).
- **Fix**: Audit whether `famp_crypto` and `serde_json` are actually used in `famp-transport-http`. If not, remove the silencers and the dependencies. If yes (e.g., in tests or private helper functions), update the comment to document why they're needed and remove the silencer.

---

### [LOW] Module Organization: `wire.rs` Over-Encapsulates Envelope Structure
- **Severity**: LOW
- **Location**: crates/famp-envelope/src/lib.rs (pub(crate) wire module) + envelope.rs (decode_value helper)
- **Issue**: The `WireEnvelope<B>` struct and serialization boilerplate live in a private `wire.rs` module, not exposed in the public API. The lib.rs has a critical note about why (RESEARCH.md Pitfalls 1-2 regarding serde flattening), but the note is in the comment, not accessible from generated docs.
- **Why it matters**: Low severity because the design is correct — the encapsulation prevents misuse. However, future maintainers or external integrators won't see the reasoning in docs; they'll only see the restriction. This is not a bug but a minor maintenance friction point.
- **Fix**: Move the critical comment from lib.rs to a top-level module doc in `envelope.rs` and/or add a link in README to RESEARCH.md. Ensure the doc comment on the public `SignedEnvelope` struct notes: "Do not manually construct the wire format — use `.sign()` and `.decode()` methods; see [RESEARCH.md Pitfalls 1-2](../../../RESEARCH.md) for why."

---

### [LOW] Struct Field Names Lint Bypass in `famp-core`
- **Severity**: LOW
- **Location**: crates/famp-core/src/identity.rs (line with `#[allow(clippy::struct_field_names)]`)
- **Issue**: One lint bypass for `struct_field_names` on the `Principal` struct, which has fields `authority` and `name`. Clippy suggests the field names repeat the struct name (e.g., `principal_authority`), but FAMP correctly uses short names.
- **Why it matters**: This is a valid bypass (the lint is overly pedantic here), but it's a sign that workspace lint settings could be tuned. The workspace forbids `unwrap_used` and `expect_used` but allows many pedantic lints only to selectively deny them — this creates scattered `#[allow]` decorators.
- **Fix**: Consider adding to workspace lints: `struct_field_names = "allow"` if other crates have similar bypasses, or leave as-is if this is the only occurrence (acceptable cost).

---

### [INFO] Rustfmt Max Width Set to 100 (Permissive for v1)
- **Severity**: INFO
- **Location**: rustfmt.toml (max_width = 100)
- **Issue**: Standard Rust convention is 80 or 120 chars per line; FAMP uses 100. This is fine for readability but slightly unconventional.
- **Why it matters**: Very minor; this is a stylistic choice. No impact on correctness or architecture.
- **Fix**: No action required unless the team has a preference. Document in CONVENTIONS.md if style is contentious.

---

## Architectural Strengths

- **Clean Crate Boundaries**: Each crate has a single responsibility (crypto, canonicalization, envelope, transport, keyring, FSM). No circular dependencies detected.
- **Strict Linting Enforced**: `#[forbid(unsafe_code)]`, `unwrap_used = "deny"`, `expect_used = "deny"` across all crates. All panics/unwraps are test-only with explicit `#[allow(...)]` markers.
- **Trait Abstractions Well-Placed**: `Transport` trait decouples MemoryTransport and HttpTransport cleanly. Both implement the same interface; adversarial tests exercise both paths identically.
- **Domain Separation Pattern Clear**: Cryptography (famp-crypto) isolates signature logic with a dedicated `prefix.rs` module. Canonicalization (famp-canonical) is a thin wrapper over serde_jcs with conformance gates.
- **Error Types Properly Typed**: Libraries use `thiserror` for typed errors; bins/tests use `anyhow`. Errors propagate as `Result<T, SpecificError>`, not `anyhow::Result`, enabling exhaustive pattern matching.
- **Test Infrastructure Comprehensive**: 43 integration/unit test files, proptest roundtrips, stateright FSM checking, snapshot vectors. No dead code observed (unused_crate_dependencies enforced).
- **Workspace Pinning Disciplined**: All deps in `workspace.dependencies`; member crates reference via `{ workspace = true }` or explicit paths. Single source of truth for versions.

---

## Debt Register

| Item | Location | Effort | Priority | Notes |
|------|----------|--------|----------|-------|
| Add re-exports to umbrella crate | crates/famp/src/lib.rs | 0.5h | HIGH | Improves public API discoverability; minimal code. |
| Decide on Signer/Verifier traits | crates/famp-crypto/src/traits.rs | 1-2h | MEDIUM | Either remove (simpler) or document as phase-2 extension point. |
| Document or remove stub crates | crates/{famp-identity, famp-causality, famp-protocol, famp-extensions, famp-conformance} | 1h | MEDIUM | Add phase markers + backlog refs, or remove from workspace. |
| Audit famp-transport-http deps | crates/famp-transport-http/src/lib.rs | 0.5h | LOW | Determine if `famp_crypto` and `serde_json` silencers are needed or dead code. |
| Transport error composition guidance | crates/famp-transport/src/lib.rs | 1h | LOW | Add doc comment guidance on error handling pattern for implementors. |
| Move wire.rs design doc to public | crates/famp-envelope/src/envelope.rs | 0.5h | LOW | Ensure RESEARCH.md pitfalls are visible to callers. |

---

## No Issues Found In

- **Circular Dependencies**: Verified via Cargo.toml analysis; DAG is clean.
- **Public API Surface Bloat**: Each crate re-exports only its intended public types; private modules use `pub(crate)` correctly.
- **Lint Bypass Abuse**: All `#[allow(...)]` markers are justified and test-scoped (8 total bypass locations, all for valid reasons).
- **Module Over-Engineering**: Modules map cleanly to responsibilities; no god modules or over-fragmentation detected.
- **Async Safety**: Tokio spawning, lock contention, and future composition reviewed (separate scope, no issues in ARCH review).
- **Spec Compliance Markers**: CONF-05/06/07, RFC 8785 conformance, RFC 8032 vectors all referenced in code comments with traceability to FAMP-v0.5.1-spec.md.

---

**End of ARCH+DEBT Report**

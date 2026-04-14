---
phase: 260414-fjo-pr-4-architectural-cleanup
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/famp-crypto/src/lib.rs
  - crates/famp-crypto/src/traits.rs
  - crates/famp-crypto/README.md
  - Cargo.toml
  - CONTRIBUTING.md
  - crates/famp-identity/
  - crates/famp-causality/
  - crates/famp-protocol/
  - crates/famp-extensions/
  - crates/famp-conformance/
  - crates/famp/src/lib.rs
  - crates/famp/tests/umbrella_reexports.rs
autonomous: true
requirements:
  - PR4-CUT1-drop-signer-verifier
  - PR4-CUT2-remove-stub-crates
  - PR4-CUT3-umbrella-reexports

must_haves:
  truths:
    - "famp-crypto exposes only free functions for sign/verify; no Signer/Verifier traits remain"
    - "Workspace builds without the 5 stub crates; no references in active files"
    - "Callers can write `use famp::{Principal, SignedEnvelope, FampSigningKey, sign_value}` and it compiles"
    - "Each cut lands as its own atomic commit with full verification green"
  artifacts:
    - path: "crates/famp-crypto/src/lib.rs"
      provides: "Public surface without traits module"
      contains: "pub use sign::"
    - path: "Cargo.toml"
      provides: "workspace.members without 5 stub crates"
    - path: "crates/famp/src/lib.rs"
      provides: "Minimal protocol-surface re-exports"
      contains: "pub use famp_core::"
    - path: "crates/famp/tests/umbrella_reexports.rs"
      provides: "Compile-time proof that re-exports resolve"
  key_links:
    - from: "crates/famp/src/lib.rs"
      to: "famp_core, famp_envelope, famp_crypto, famp_canonical"
      via: "pub use"
      pattern: "pub use famp_(core|envelope|crypto|canonical)::"
---

<objective>
PR #4: three independent architectural cuts from the ARCH+DEBT review, landed as three atomic commits in a single plan.

1. Delete unused `Signer`/`Verifier` traits from `famp-crypto` (YAGNI, no polymorphic consumers).
2. Remove 5 stub crates (`famp-identity`, `famp-causality`, `famp-protocol`, `famp-extensions`, `famp-conformance`) from the workspace.
3. Add minimal umbrella re-exports on `famp` so callers can write `famp::Principal` instead of `famp_core::Principal`.

Purpose: shrink the surface area callers have to track, kill dead code that's silently lint-shielded, and make the canonical caller path obvious. Pre-1.0, single developer, no external consumers — semver breaks are free and there is no deprecation ceremony.

Output: three commits on main, `just ci` green after each, no changes to spec bytes, no changes to signing semantics, no new deps.
</objective>

<execution_context>
@~/.claude/get-shit-done/workflows/execute-plan.md
</execution_context>

<context>
@CLAUDE.md
@CONTRIBUTING.md
@Cargo.toml
@crates/famp-crypto/src/lib.rs
@crates/famp-crypto/src/traits.rs
@crates/famp-crypto/README.md
@crates/famp/src/lib.rs
@crates/famp-core/src/lib.rs
@crates/famp-envelope/src/lib.rs
@crates/famp-canonical/src/lib.rs

<grep_findings>
<!-- Planner pre-flight: Signer/Verifier references in active code -->
Direct trait references (all inside famp-crypto — will be removed or are ed25519-dalek imports, NOT FAMP traits):
- crates/famp-crypto/src/traits.rs — DELETE entirely
- crates/famp-crypto/src/lib.rs:68 `pub mod traits;`  — remove
- crates/famp-crypto/src/lib.rs:76 `pub use traits::{Signer, Verifier};` — remove
- crates/famp-crypto/README.md:84-85, 88 — trait-sugar section + "Explicitly NOT re-exported" note — edit
- crates/famp-crypto/src/sign.rs:11 `use ed25519_dalek::Signer as _;` — LEAVE (this is the dalek trait, not ours)
- crates/famp-crypto/tests/rfc8032_vectors.rs:23 `use ed25519_dalek::{..., Signer, ...};` — LEAVE (dalek trait)
- crates/famp-transport-http/src/tls.rs — matches `Verifier` substring inside rustls types, NOT our trait — LEAVE

No consumers of `famp_crypto::Signer` or `famp_crypto::Verifier` exist outside `traits.rs` itself.

<!-- Planner pre-flight: stub crate references in active files (.planning/ and .codebase-review/ excluded) -->
- Cargo.toml:8-12,16 — members list
- CONTRIBUTING.md:39 — repo layout bullet
- FAMP-v0.5.1-spec.md:968 — historical spec mention of Phase 8 `famp-conformance` — LEAVE (spec is authoritative history, not a workspace reference)
- README.md — NO matches
- Justfile — NO matches
- .github/workflows/ — NO matches
- crates/famp-canonical/docs/fallback.md — NO matches
- None of the 5 stub crates are listed under `[workspace.dependencies]` (verified: only deps on ed25519-dalek, serde, etc.)

<!-- Planner pre-flight: umbrella re-export target verification -->
famp_core public exports (lib.rs): Principal, Instance, MessageId, ProtocolError, ProtocolErrorKind, AuthorityScope, ArtifactId — all present.
famp_envelope public exports: UnsignedEnvelope, SignedEnvelope, AnySignedEnvelope, EnvelopeDecodeError, EnvelopeScope, MessageClass, TerminalStatus (re-exported), Timestamp, FAMP_SPEC_VERSION — all present.
famp_crypto public exports: FampSigningKey, TrustedVerifyingKey, FampSignature, CryptoError, DOMAIN_PREFIX, sign_value, verify_value, sign_canonical_bytes, verify_canonical_bytes — all present.
famp_canonical public exports: canonicalize, Canonicalize, from_slice_strict, from_str_strict, artifact_id_for_canonical_bytes, artifact_id_for_value, CanonicalError — note: NO `to_canonical_string` / `to_canonical_vec` — the primary entry point is `canonicalize`. Re-export `canonicalize`, `from_slice_strict`, `from_str_strict`, `CanonicalError` instead.
</grep_findings>

<interfaces>
Current famp-crypto public surface (after Task 1):
```rust
// crates/famp-crypto/src/lib.rs
pub mod error;
pub mod hash;
pub mod keys;
pub mod prefix;
pub mod sign;
pub mod verify;

pub use error::CryptoError;
pub use hash::{sha256_artifact_id, sha256_digest};
pub use keys::{FampSignature, FampSigningKey, TrustedVerifyingKey};
pub use prefix::{canonicalize_for_signature, DOMAIN_PREFIX};
pub use sign::{sign_canonical_bytes, sign_value};
pub use verify::{verify_canonical_bytes, verify_value};
```

Target famp umbrella surface (after Task 3):
```rust
// crates/famp/src/lib.rs — minimal protocol core re-exports
pub use famp_core::{
    ArtifactId, AuthorityScope, Instance, MessageId,
    Principal, ProtocolError, ProtocolErrorKind,
};
pub use famp_envelope::{
    AnySignedEnvelope, EnvelopeDecodeError, EnvelopeScope, MessageClass,
    SignedEnvelope, TerminalStatus, Timestamp, UnsignedEnvelope,
    FAMP_SPEC_VERSION,
};
pub use famp_crypto::{
    sign_canonical_bytes, sign_value, verify_canonical_bytes, verify_value,
    CryptoError, FampSignature, FampSigningKey, TrustedVerifyingKey,
    DOMAIN_PREFIX,
};
pub use famp_canonical::{
    canonicalize, from_slice_strict, from_str_strict, CanonicalError,
};
```
Note: `famp-canonical` does NOT expose `to_canonical_string`/`to_canonical_vec`. Use `canonicalize` (the `Canonicalize` trait entry point) instead. The executor MUST verify names via `cargo doc -p famp-canonical --no-deps --open` or grep before writing the re-export list, and adjust if `Canonicalize` trait export is also wanted.
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Delete Signer/Verifier traits from famp-crypto</name>
  <files>
    crates/famp-crypto/src/traits.rs (DELETE),
    crates/famp-crypto/src/lib.rs,
    crates/famp-crypto/README.md
  </files>
  <action>
    Delete the `Signer`/`Verifier` trait machinery from `famp-crypto`. These are ~90 LOC of thin sugar over the free functions in `sign.rs`/`verify.rs`, have zero polymorphic consumers anywhere in the workspace, and carry a rustdoc warning that says "do not depend on this trait." YAGNI cleanup — reintroduce only if a hardware-signer backend ever materializes. Pre-1.0, so no `#[deprecated]` stub, no re-export shim.

    Exact edits:

    1. `git rm crates/famp-crypto/src/traits.rs` — delete the file outright. Its two unit tests (`trait_sugar_matches_free_fn`, `trait_canonical_bytes_sugar_matches_free_fn`) go with it; they only test the trait delegation, and the same delegation is already exercised by the free-function tests in `sign.rs`/`verify.rs` and by RFC 8032 vectors.

    2. Edit `crates/famp-crypto/src/lib.rs`:
       - Remove line `pub mod traits;` (currently line 68).
       - Remove line `pub use traits::{Signer, Verifier};` (currently line 76).
       - Leave every other module and re-export untouched. Do NOT touch the `#[cfg(test)] use ... as _;` silencer block — that's out of scope per the planner's constraints.

    3. Edit `crates/famp-crypto/README.md`:
       - Remove the "Trait sugar (pure delegation to free functions)" subsection at lines 82-85 (the 4 lines listing `Signer` and `Verifier`).
       - Update the "Explicitly NOT re-exported" note at lines 87-91 to drop the mention of `ed25519_dalek::Signer`/`ed25519_dalek::Verifier` (those refer to the dalek traits; the note is about what FAMP deliberately hides, and it remains accurate without the trait-sugar framing). Rewrite it as: "`ed25519_dalek::VerifyingKey` is not re-exported. There is NO public path from this crate to `ed25519_dalek::VerifyingKey::verify` (non-strict); only `verify_strict` is reachable, and only via `TrustedVerifyingKey`, which cannot be constructed without passing ingress checks."

    4. Grep to confirm no collateral:
       ```bash
       rg -n 'famp_crypto::Signer|famp_crypto::Verifier|use famp_crypto::\{[^}]*Signer|use famp_crypto::\{[^}]*Verifier' crates/ examples/ tests/
       ```
       Must return zero matches. Hits on `ed25519_dalek::Signer` in `sign.rs:11` and `tests/rfc8032_vectors.rs:23` are NOT ours — leave them. Hits on `Verifier` in `famp-transport-http/src/tls.rs` are rustls types — leave them.

    5. Also grep `CONTRIBUTING.md` for any mention of the traits — if present, remove. (Planner pre-flight: none found; confirm anyway.)

    Do NOT touch `.planning/**` or `.codebase-review/**` — those are historical audit trails, and rewriting them destroys the record of why this cut was made.

    After edits, run the full verification gate below. Commit atomically.
  </action>
  <verify>
    <automated>cargo build --workspace --all-targets && cargo nextest run --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo doc --workspace --no-deps && rg -n 'famp_crypto::Signer|famp_crypto::Verifier' crates/ examples/ tests/ ; test $? -eq 1</automated>
  </verify>
  <done>
    - `crates/famp-crypto/src/traits.rs` no longer exists.
    - `crates/famp-crypto/src/lib.rs` has no `traits` module and no `Signer`/`Verifier` re-export.
    - `famp-crypto/README.md` trait-sugar section removed; "Explicitly NOT re-exported" note rewritten.
    - `just ci` (or the individual gates above) is green.
    - Commit landed with message `refactor(famp-crypto): drop unused Signer and Verifier traits` and a body explaining: YAGNI; no polymorphic consumers in the workspace; free functions in `sign.rs`/`verify.rs` remain the real API and are already the documented entry point in the crate-level `//!` rustdoc; pre-1.0, no deprecation ceremony; will reintroduce a proper extensibility contract if and when a hardware-signer or remote-signer backend lands.
  </done>
</task>

<task type="auto">
  <name>Task 2: Remove 5 stub crates from the workspace</name>
  <files>
    Cargo.toml,
    CONTRIBUTING.md,
    crates/famp-identity/ (DELETE),
    crates/famp-causality/ (DELETE),
    crates/famp-protocol/ (DELETE),
    crates/famp-extensions/ (DELETE),
    crates/famp-conformance/ (DELETE)
  </files>
  <action>
    The 5 crates (`famp-identity`, `famp-causality`, `famp-protocol`, `famp-extensions`, `famp-conformance`) are empty scaffolding from the v0.5.1 phase-0 workspace bootstrap. They contain no real code, are tracked in `.planning/` as deferred federation-profile work, and silently slow down every workspace build. Remove them. When the work is ready, reintroduce with `cargo new crates/famp-<name> --lib` — zero cost to start over from a clean slate.

    Exact edits:

    1. Delete the 5 crate directories, preserving git history:
       ```bash
       git rm -rf crates/famp-identity crates/famp-causality crates/famp-protocol crates/famp-extensions crates/famp-conformance
       ```

    2. Edit `Cargo.toml` `[workspace]` `members` array (currently lines 3-18). Remove these five lines:
       ```
         "crates/famp-identity",
         "crates/famp-causality",
         "crates/famp-protocol",
         "crates/famp-extensions",
         "crates/famp-conformance",
       ```
       Resulting `members` order should be: famp-core, famp-canonical, famp-crypto, famp-envelope, famp-fsm, famp-keyring, famp-transport, famp-transport-http, famp.

       Planner verified `[workspace.dependencies]` does NOT list any of the 5 crates — nothing to remove there. Double-check anyway.

    3. Edit `CONTRIBUTING.md` line 39 (the "Repo Layout" bullet that lists "`crates/famp-identity`, `crates/famp-causality`, `crates/famp-protocol`, `crates/famp-extensions`, `crates/famp-conformance` — deferred federation-profile scaffolding"). DELETE that line entirely — we no longer carry scaffolding in the tree. Do not replace with a "deferred crates" note; the `.planning/` tree is where deferred work lives.

    4. Grep the active tree to confirm zero residual references:
       ```bash
       rg -n 'famp-identity|famp-causality|famp-protocol|famp-extensions|famp-conformance|famp_identity|famp_causality|famp_protocol|famp_extensions|famp_conformance' Cargo.toml crates/ examples/ tests/ README.md CONTRIBUTING.md Justfile .github/
       ```
       Must return zero matches. Do NOT grep `.planning/`, `.codebase-review/`, or `FAMP-v0.5.1-spec.md` — those are historical audit trails and intentionally retain the old names. The spec mention at `FAMP-v0.5.1-spec.md:968` is a reference to a future `famp-conformance` phase and stays as-is (spec is authoritative).

    5. `Cargo.lock` will regenerate on the next `cargo` invocation — stage the regenerated lockfile with the commit. Do not edit it by hand.

    Verification gate must include a full workspace build + test + clippy + doc pass. If any crate outside the 5 deleted ones had a latent `use famp_identity as _;` or similar silencer, clippy `-D warnings` would fail and you must investigate rather than work around (planner pre-flight says none exist, but treat that as a hypothesis to test).

    Do NOT touch `.planning/**` or `.codebase-review/**`.
  </action>
  <verify>
    <automated>test ! -e crates/famp-identity && test ! -e crates/famp-causality && test ! -e crates/famp-protocol && test ! -e crates/famp-extensions && test ! -e crates/famp-conformance && cargo build --workspace --all-targets && cargo nextest run --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo doc --workspace --no-deps && ! rg -q 'famp-identity|famp-causality|famp-protocol|famp-extensions|famp-conformance|famp_identity|famp_causality|famp_protocol|famp_extensions|famp_conformance' Cargo.toml crates/ examples/ tests/ README.md CONTRIBUTING.md Justfile .github/</automated>
  </verify>
  <done>
    - 5 crate directories gone from `crates/`.
    - `Cargo.toml` workspace members list has exactly 9 entries (famp-core, famp-canonical, famp-crypto, famp-envelope, famp-fsm, famp-keyring, famp-transport, famp-transport-http, famp).
    - `CONTRIBUTING.md` repo-layout section no longer mentions the 5 stubs.
    - `Cargo.lock` regenerated.
    - `just ci` green.
    - Commit landed with message `refactor: remove unimplemented stub crates from workspace` and a body: lists the 5 crates, explains they had no implementations and were silently slowing workspace builds, notes that deferred federation-profile work is tracked under `.planning/` and will be reintroduced with `cargo new` when there's actual code to write, and clarifies this is a pre-1.0 structural cleanup not a semver event.
  </done>
</task>

<task type="auto">
  <name>Task 3: Add minimal umbrella re-exports to famp crate</name>
  <files>
    crates/famp/src/lib.rs,
    crates/famp/tests/umbrella_reexports.rs (NEW)
  </files>
  <action>
    Make `famp` the canonical one-stop import for callers who want the protocol core without tracking which member crate owns each type. Minimal surface only — protocol-critical types that a caller needs to construct, sign, verify, or parse an envelope. Body schemas, FSM types, transport types, and keyring types are NOT re-exported — those are either advanced/optional or have their own crate-level surface.

    Exact edits:

    1. Edit `crates/famp/src/lib.rs`. Keep the existing header (`#![forbid(unsafe_code)]`), the `use X as _;` silencer block (lines 9-25 — out of scope per planner), and `pub mod runtime;`. Add the umbrella re-exports after the silencer block and before `pub mod runtime;`.

    2. Augment the crate-level `//!` doc block at the top with one additional paragraph after the existing "FAMP top-level crate" sentence:
       ```
       //! # Public API
       //!
       //! This crate re-exports the minimal protocol surface from
       //! `famp-core`, `famp-envelope`, `famp-crypto`, and `famp-canonical`
       //! so callers can write `famp::Principal`, `famp::SignedEnvelope`,
       //! `famp::sign_value` without tracking which member crate owns each
       //! type. Rustdoc is preserved automatically by Rust's `pub use`.
       //!
       //! For advanced usage (body schemas, FSM, transport, keyring, raw
       //! canonical-JSON primitives beyond `canonicalize` / strict parse),
       //! import the member crates directly.
       ```

    3. Add the re-exports. Exact list (no more, no less):
       ```rust
       pub use famp_core::{
           ArtifactId, AuthorityScope, Instance, MessageId,
           Principal, ProtocolError, ProtocolErrorKind,
       };
       pub use famp_envelope::{
           AnySignedEnvelope, EnvelopeDecodeError, EnvelopeScope, MessageClass,
           SignedEnvelope, TerminalStatus, Timestamp, UnsignedEnvelope,
           FAMP_SPEC_VERSION,
       };
       pub use famp_crypto::{
           sign_canonical_bytes, sign_value, verify_canonical_bytes, verify_value,
           CryptoError, FampSignature, FampSigningKey, TrustedVerifyingKey,
           DOMAIN_PREFIX,
       };
       pub use famp_canonical::{
           canonicalize, from_slice_strict, from_str_strict, CanonicalError,
       };
       ```

       IMPORTANT: the planner pre-flight found `famp-canonical` does NOT export `to_canonical_string` or `to_canonical_vec`. The primary canonicalization entry point is the `canonicalize` free function (plus the `Canonicalize` trait). If the executor finds additional public helpers that belong in the minimal surface, add them — but err on the side of omission. Ben's rule: "Don't re-export every public item."

       `famp-crypto` will now expose `hash::{sha256_artifact_id, sha256_digest}`, `canonicalize_for_signature`, and `TrustedVerifyingKey` publicly — do NOT re-export the hash functions or `canonicalize_for_signature` at the umbrella level. They're implementation-detail entry points for callers who already reach into `famp_crypto` directly.

    4. Because `famp` already has `famp_crypto` in its `use ... as _;` silencer list, the `pub use famp_crypto::...` addition will make that silencer redundant for the non-test compile unit. Remove `use famp_crypto as _;` from line 14 — the real `pub use` now makes the dep "used." Leave every other silencer (`base64`, `ed25519_dalek`, `famp_transport`, `famp_transport_http`, `rand`, `tokio`, `url`, and the test-only ones) untouched.

       Similarly, verify after the edit: do `famp_core`, `famp_envelope`, `famp_canonical` need their silencers removed? Check `crates/famp/Cargo.toml` — if they're already listed as workspace dependencies for `famp`, the `pub use` now satisfies the lint. If they are NOT listed, add them to `[dependencies]` (workspace-inherited: `famp-core = { workspace = true }` pattern — if the pattern isn't workspace-dependency-based, use `path = "../famp-core"`). Planner note: executor must read `crates/famp/Cargo.toml` before writing the re-exports and add any missing path dependencies.

    5. Create `crates/famp/tests/umbrella_reexports.rs` as a compile-time smoke test. ~20 lines:
       ```rust
       //! Compile-time proof that the umbrella re-exports resolve.
       //! If this file fails to compile, PR #4 Task 3 regressed.

       use famp::{
           AnySignedEnvelope, ArtifactId, AuthorityScope, CanonicalError,
           CryptoError, DOMAIN_PREFIX, EnvelopeDecodeError, EnvelopeScope,
           FampSignature, FampSigningKey, Instance, MessageClass, MessageId,
           Principal, ProtocolError, ProtocolErrorKind, SignedEnvelope,
           TerminalStatus, Timestamp, TrustedVerifyingKey, UnsignedEnvelope,
           FAMP_SPEC_VERSION,
       };
       use famp::{canonicalize, from_slice_strict, from_str_strict};
       use famp::{sign_canonical_bytes, sign_value, verify_canonical_bytes, verify_value};

       #[test]
       fn reexports_compile_and_construct() {
           // Construct at least one re-exported type to prove it's real.
           let p: Principal = "agent:local/test".parse().expect("valid principal");
           let _ = p;
           // Touch the version constant so it's not dead code.
           assert!(!FAMP_SPEC_VERSION.is_empty());
           // Touch the domain prefix.
           assert_eq!(DOMAIN_PREFIX.len(), 12);
       }
       ```
       If `Principal::from_str` / `"...".parse()` signature differs from the above, adjust to match the actual `Principal` API (planner did not open `identity.rs`; executor verifies). The test must import every re-exported name and construct at least one.

    6. Verify `cargo doc -p famp --no-deps` generates clean HTML with all re-exports visible and their rustdoc inherited from the source crates.

    Do NOT add body schemas, FSM types, transport types, keyring types, or the long tail of `famp-canonical` / `famp-crypto` public items. Ben's rule: minimal surface, defer until a caller actually needs it.
  </action>
  <verify>
    <automated>cargo build --workspace --all-targets && cargo nextest run -p famp --test umbrella_reexports && cargo nextest run --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo doc -p famp --no-deps</automated>
  </verify>
  <done>
    - `crates/famp/src/lib.rs` contains the 4 `pub use` blocks above, the augmented `//!` doc explaining the umbrella role, and `use famp_crypto as _;` removed from the silencer block.
    - `crates/famp/Cargo.toml` has path/workspace dependencies on `famp-core`, `famp-envelope`, `famp-crypto`, `famp-canonical` if any were missing.
    - `crates/famp/tests/umbrella_reexports.rs` exists, imports every re-exported name, constructs at least one, and passes.
    - `cargo doc -p famp --no-deps` shows the re-exports with inherited rustdoc.
    - `just ci` green.
    - Commit landed with message `feat(famp): add minimal public API re-exports for protocol core` and a body explaining: the scope is deliberately minimal (identity, envelope, crypto entry points, strict-parse + canonicalize — no body schemas, no FSM, no transport, no keyring), the rationale (callers shouldn't have to know the internal crate structure for basic envelope construction/signing/verification), and the opt-out path (import member crates directly for advanced usage). Note the compile-time smoke test in `tests/umbrella_reexports.rs` as the regression gate.
  </done>
</task>

</tasks>

<verification>
Between tasks and at the end of the plan, the executor runs the full CI-parity gate. If any task's gate fails, STOP — do not roll forward to the next task. Fix in place or escalate.

Full gate:
```bash
just ci
# equivalent to:
# cargo fmt --all -- --check
# cargo build --workspace --all-targets
# cargo nextest run --workspace
# cargo clippy --workspace --all-targets -- -D warnings
# cargo doc --workspace --no-deps
# cargo audit
# just test-canonical-strict
# just spec-lint
```

Cross-task invariants:
- Workspace test count does not decrease (except for the 2 tests inside `traits.rs` which are removed with the file — net change is -2 tests from Task 1, then stable).
- No new dependencies added to any `Cargo.toml` (adding missing path deps for `famp` in Task 3 is scoped, not "new").
- Signing byte semantics unchanged: `DOMAIN_PREFIX` untouched, `verify_strict` untouched, canonicalization untouched. Spec compliance unchanged.
- `.planning/**` and `.codebase-review/**` are NOT modified by any task.
- Each task produces exactly ONE commit. Three tasks → three commits. No squashing.
</verification>

<success_criteria>
- Three atomic commits on `main` with messages:
  1. `refactor(famp-crypto): drop unused Signer and Verifier traits`
  2. `refactor: remove unimplemented stub crates from workspace`
  3. `feat(famp): add minimal public API re-exports for protocol core`
- `just ci` green after each commit (not just at the end).
- Workspace builds with 9 member crates instead of 14.
- `famp-crypto` public surface is free functions + key types only.
- `use famp::{Principal, SignedEnvelope, FampSigningKey, sign_value};` compiles from a fresh caller.
- `crates/famp/tests/umbrella_reexports.rs` passes and imports every re-exported name.
- Zero references to the 5 deleted crate names or to `famp_crypto::Signer` / `famp_crypto::Verifier` in active files (active = everything except `.planning/`, `.codebase-review/`, `FAMP-v0.5.1-spec.md`).
- No changes to spec, signing semantics, or canonicalization bytes.
</success_criteria>

<output>
After completion, create `.planning/quick/260414-fjo-pr-4-architectural-cleanup-drop-signer-v/260414-fjo-SUMMARY.md` with:
- The three commit SHAs and one-line summaries
- Workspace crate count before/after (14 → 9)
- famp-crypto LOC removed (~90 from traits.rs + README edits)
- Umbrella re-export count (24 names across 4 source crates)
- Any surprises found during grep (especially around silencer block interactions in Task 3)
- `just ci` timing delta before vs after (optional, only if easy to measure)
</output>

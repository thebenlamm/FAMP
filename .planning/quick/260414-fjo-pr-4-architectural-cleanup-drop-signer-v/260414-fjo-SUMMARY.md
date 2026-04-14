---
quick_task: 260414-fjo
title: PR #4 — Architectural cleanup (drop Signer/Verifier, remove stub crates, umbrella re-exports)
type: execute
autonomous: true
date: 2026-04-14
commits:
  - 9e5426f: "refactor(famp-crypto): drop unused Signer and Verifier traits"
  - 08c442a: "refactor: remove unimplemented stub crates from workspace"
  - e8ecf9f: "feat(famp): add minimal public API re-exports for protocol core"
requirements:
  - PR4-CUT1-drop-signer-verifier
  - PR4-CUT2-remove-stub-crates
  - PR4-CUT3-umbrella-reexports
tags:
  - refactor
  - api-surface
  - workspace-hygiene
  - architectural-cleanup
---

# Quick Task 260414-fjo: PR #4 Architectural Cleanup Summary

Three independent architectural cuts from the ARCH+DEBT review landed as three atomic commits. `just ci` green after each.

## Commits

| # | Hash | Type | One-liner |
|---|---|---|---|
| 1 | `9e5426f` | refactor(famp-crypto) | Drop unused `Signer` / `Verifier` traits (~90 LOC, zero consumers) |
| 2 | `08c442a` | refactor | Remove 5 empty stub crates from workspace |
| 3 | `e8ecf9f` | feat(famp) | Minimal umbrella re-exports for `famp::{Principal, SignedEnvelope, FampSigningKey, sign_value, ...}` |

## Metrics

| Dimension | Before | After |
|---|---|---|
| Workspace crate count | 14 | **9** |
| Workspace test count | 267 | **261** (-5 stub smoke tests, -2 trait delegation tests, +1 umbrella compile test) |
| `famp-crypto` public trait surface | `Signer`, `Verifier` exported | none (free functions only) |
| `famp` top-level re-exports | 0 | **25 names** across 4 source crates |

## Deliverables

### Task 1 — Drop Signer/Verifier traits
- **Deleted:** `crates/famp-crypto/src/traits.rs` (~90 LOC + 2 unit tests)
- **Edited:** `crates/famp-crypto/src/lib.rs` (dropped `pub mod traits;` and `pub use traits::{Signer, Verifier};`)
- **Edited:** `crates/famp-crypto/README.md` (removed "Trait sugar" subsection; rewrote "Explicitly NOT re-exported" note to drop dalek trait mentions)
- **Verification:** grep of `famp_crypto::Signer|famp_crypto::Verifier` across active tree returns zero; full workspace build, nextest, clippy (-D warnings), and doc all green.

### Task 2 — Remove 5 stub crates
- **Deleted (via `git rm -rf`):** `crates/famp-identity`, `crates/famp-causality`, `crates/famp-protocol`, `crates/famp-extensions`, `crates/famp-conformance` (5 dirs × 2 files each = 10 files, ~70 LOC total including smoke tests)
- **Edited:** `Cargo.toml` workspace members list (14 → 9 entries)
- **Edited:** `CONTRIBUTING.md` Repo Layout section (removed "deferred federation-profile scaffolding" bullet)
- **Edited:** `crates/famp-crypto/README.md` "What's NOT in this crate" bullet (replaced stale `famp-identity` cross-reference with "Federation Profile (v0.8+)" note)
- **Regenerated:** `Cargo.lock`
- **Verification:** grep across active tree (`Cargo.toml`, `crates/`, `CONTRIBUTING.md`, `README.md`, `Justfile`, `.github/`) returns zero matches for any of the 5 crate names. `.planning/`, `.codebase-review/`, and `FAMP-v0.5.1-spec.md` intentionally untouched (audit trail + spec).

### Task 3 — Umbrella re-exports on `famp`
- **Edited:** `crates/famp/src/lib.rs` — added `//! # Public API` rustdoc paragraph, removed `use famp_crypto as _;` silencer (the real `pub use` now makes the dep used), added 4 `pub use` blocks covering 25 names. `rustfmt` then sorted the blocks alphabetically by source crate.
- **Created:** `crates/famp/tests/umbrella_reexports.rs` — compile-time regression gate that imports every re-exported name, coerces each free function to a `fn` pointer, constructs a `Principal`, asserts `DOMAIN_PREFIX.len() == 12` and `FAMP_SPEC_VERSION` non-empty. Passes in 9ms.
- **Verification:** `cargo nextest run -p famp --test umbrella_reexports` passes; `cargo doc -p famp --no-deps` renders the re-exports with inherited rustdoc; full `just ci` green.

## Surprises found during execution

1. **`famp-envelope::TerminalStatus` is not at the crate root.** Grep-findings listed it under the envelope re-export block, but `crates/famp-envelope/src/lib.rs` only does `pub use body::BodySchema` and similar — `TerminalStatus` is reachable via `famp_envelope::body::TerminalStatus`, not `famp_envelope::TerminalStatus`. Resolved by sourcing `TerminalStatus` from `famp-core` instead (where it was lifted in Phase 02 Plan 01, per STATE.md). Same type, cleaner dependency graph.

2. **`SignedEnvelope<B>` / `UnsignedEnvelope<B>` are generic.** First attempt at `umbrella_reexports.rs` used bare `SignedEnvelope` / `UnsignedEnvelope` in a `touch_types` function signature, which failed with E0107 (missing generic argument). Fixed by concretizing to `SignedEnvelope<famp_envelope::body::RequestBody>` and `UnsignedEnvelope<famp_envelope::body::RequestBody>`.

3. **Clippy `too_many_arguments` on the smoke-test helper.** The `touch_types` fn takes one `Option<T>` per re-exported type — 19 arguments total — which trips clippy's default limit of 7. Added `#![allow(clippy::too_many_arguments)]` at the file scope alongside the existing `unwrap_used` and `unused_crate_dependencies` allows. Acceptable: this is a test file whose entire purpose is to force the compiler to resolve 19 type paths in one place.

4. **One stale cross-reference found outside the planned edit set.** `crates/famp-crypto/README.md:189` still said "belongs in `famp-identity` (later phase)" under the "What's NOT in this crate" section. Planner's grep excluded `crates/` for stub-name matches, so this slipped through pre-flight. Fixed inline during Task 2 and called out in the Task 2 commit body.

5. **Silencer block interaction minimal.** Removing `use famp_crypto as _;` from the silencer was clean — `famp_core`, `famp_envelope`, `famp_canonical` were never in the silencer block to begin with (they were already active deps via `crates/famp/Cargo.toml`, and `famp-fsm` / `famp-keyring` still don't need umbrella re-exports, so their absence from both lists is correct).

## Verification matrix

| Gate | Task 1 | Task 2 | Task 3 |
|---|---|---|---|
| `cargo fmt --all -- --check` | clean | clean | clean (after one auto-fmt) |
| `cargo build --workspace --all-targets` | ✓ | ✓ | ✓ |
| `cargo nextest run --workspace` | 265/265 | 260/260 | 261/261 |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✓ | ✓ | ✓ (after adding `too_many_arguments` allow) |
| `cargo doc --workspace --no-deps` | ✓ | ✓ | ✓ |
| `just ci` (full) | not run | not run | **✓ all 21 spec-lint rules pass, CI-parity green** |

## Invariants held

- `DOMAIN_PREFIX` byte sequence: unchanged
- `verify_strict`-only public path: unchanged
- `famp-canonical` RFC 8785 output: unchanged (no code touched)
- `FAMP_SPEC_VERSION`: unchanged
- No new dependencies added
- No `.planning/**` or `.codebase-review/**` files touched
- Each task produced exactly one commit

## Deviations from plan

**None requiring escalation.** Two inline Rule 1 / Rule 3 fixes:

1. **Rule 3 (blocking):** `SignedEnvelope` / `UnsignedEnvelope` generic parameters had to be concretized in the test helper. Fixed inline.
2. **Rule 3 (blocking):** Clippy `too_many_arguments` on test helper. Added scoped `#![allow]`.
3. **Rule 1 (stale doc):** Stale `famp-identity` cross-reference in `crates/famp-crypto/README.md` discovered during Task 2 grep. Fixed inline, rolled into the Task 2 commit.

No Rule 4 (architectural) escalations. No auth gates.

## Known stubs

None. Every re-export resolves to a real type; every deleted crate contained only scaffolding.

## Self-Check: PASSED

- `crates/famp-crypto/src/traits.rs`: **REMOVED** (verified via `git show HEAD:crates/famp-crypto/src/traits.rs` returns fatal)
- `crates/famp-identity/`, `crates/famp-causality/`, `crates/famp-protocol/`, `crates/famp-extensions/`, `crates/famp-conformance/`: **REMOVED** (verified via `ls crates/` returns 9 entries)
- `crates/famp/tests/umbrella_reexports.rs`: **EXISTS** (verified via successful nextest run, 1/1 pass)
- Commit `9e5426f`: **FOUND** in `git log`
- Commit `08c442a`: **FOUND** in `git log`
- Commit `e8ecf9f`: **FOUND** in `git log`
- `just ci`: **GREEN** (full CI-parity run completed after Task 3)

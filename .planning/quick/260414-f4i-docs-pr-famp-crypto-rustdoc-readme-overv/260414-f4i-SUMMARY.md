---
phase: 260414-f4i-docs
plan: 01
type: quick
completed: "2026-04-14"
requirements:
  - DEVOPS-DX-01
  - DEVOPS-DX-02
  - DEVOPS-DX-03
commits:
  - c0c5311  # docs(famp-crypto): rustdoc public API
  - 243fc19  # docs(readme): How FAMP Signs a Message
  - 1b432c5  # docs: CONTRIBUTING.md
---

# Quick Task 260414-f4i: PR #3 famp-crypto rustdoc + README overview + CONTRIBUTING.md

**One-liner:** Close the three HIGH docs findings from the codebase review so
future-Ben cannot silently break byte-exact interop by calling plain `verify`,
skipping `DOMAIN_PREFIX`, or canonicalizing at the wrong layer.

## Files Touched

### Task 1 — famp-crypto rustdoc (commit `c0c5311`)

| File | Lines added (approx) | What |
|---|---|---|
| `crates/famp-crypto/src/lib.rs` | +33 | Crate-level `//!` expanded: names the three invariants (DOMAIN_PREFIX, verify_strict, canonicalization-as-precondition). Quick-start doctest preserved verbatim. |
| `crates/famp-crypto/src/error.rs` | +27 | Per-variant `///` on `CryptoError` tying each variant to the invariant it protects; spec §7.1b citations. |
| `crates/famp-crypto/src/hash.rs` | +8 | `sha256_artifact_id` pitfall: 71-char wire string — never uppercase, never strip `sha256:` prefix. Spec §3.6a. |
| `crates/famp-crypto/src/keys.rs` | +47 | `FampSigningKey` / `TrustedVerifyingKey` / `FampSignature` Invariants + Pitfalls sections. "Trusted" called out as load-bearing. `[0u8; 32]` test-seed warning cross-refs the crate-level doctest. |
| `crates/famp-crypto/src/prefix.rs` | +30 | `DOMAIN_PREFIX` Invariants/Pitfalls/Spec, §7.1a + §Δ08. `canonicalize_for_signature` misnomer pitfall spelled out. |
| `crates/famp-crypto/src/sign.rs` | +19 | `sign_value` primary-entry-point framing, hot-loop pitfall directing callers to `sign_canonical_bytes`. `sign_canonical_bytes` precondition: MUST be `famp_canonical::canonicalize` output. |
| `crates/famp-crypto/src/verify.rs` | +28 | `verify_canonical_bytes` + `verify_value` explicit warning against `ed25519_dalek::VerifyingKey::verify`. Full non-repudiation failure-mode rationale lives on the `_canonical_bytes` function; the `_value` function cross-refs. |
| `crates/famp-crypto/src/traits.rs` | +18 | `Signer` / `Verifier` documented as placeholders, NOT stable extensibility contracts. Downstream crates told to use free functions. |

Total: 8 files, ~217 line insertions, 30 deletions (existing thin docs replaced).

### Task 2 — README.md (commit `243fc19`)

- `README.md`: +54 lines. New `## How FAMP Signs a Message` section inserted between `## Quick Start` and `## Daily Loop`. Covers canonicalization (RFC 8785), domain separation (`FAMP-sig-v1\0`, §7.1a/§Δ08), the 4-step sign-and-verify flow with the `verify_strict` warning, INV-10, and the 5-state task FSM as an ASCII diagram (REQUESTED → COMMITTED → {COMPLETED, FAILED, CANCELLED}). No existing content touched.

### Task 3 — CONTRIBUTING.md (commit `1b432c5`)

- `CONTRIBUTING.md`: +91 lines (new file). Sections: framing (v0.7 solo, external PRs from v1.0), Setup (pinned `1.89.0` toolchain, `cargo-nextest` + `just` bootstrap, `just ci`), Repo Layout (derived from current `Cargo.toml` workspace members, deferred crates called out), Test Gates table (every `just` target cross-checked against the actual Justfile), Commit Conventions (no `--no-verify`), Code Review (adversarial-review workflow per CLAUDE.md), Spec Fidelity (every signing/canonicalization/FSM change cites `FAMP-v0.5.1-spec.md`; deviations land as `Δ` notes first), and the "Do Not Touch Without a Spec Diff" list naming `DOMAIN_PREFIX`, `FAMP_SPEC_VERSION`, RFC 8785 canonicalization output, and `verify_strict` strictness.

## Verification

- `cargo doc -p famp-crypto --no-deps` — clean (no warnings, no broken intra-doc links)
- `cargo test -p famp-crypto --doc` — 1 passed (crate-level quick-start)
- `cargo clippy -p famp-crypto --all-targets -- -D warnings` — clean
- `cargo doc --workspace --no-deps` — clean
- `just ci` — **green** (fmt-check, lint, build, test-canonical-strict, test-crypto, test, test-doc, spec-lint all pass)

## Rustdoc items that resisted documentation

None fully resisted. One clippy surprise: `too_long_first_doc_paragraph` fired on the first drafts of the `Signer` / `Verifier` trait docs — pedantic clippy treats a multi-sentence opening as a single paragraph and caps its length. Split into `Abstraction placeholder for X.` + blank line + the caveat paragraph. Both trait docs now lead with a single sentence.

Public *methods* on the newtypes (`FampSigningKey::from_b64url`, etc.) were not given `///` docs — the plan's must-have was "every public item re-exported from `lib.rs`", which is the three newtypes, not their constructors. `missing_docs` is not denied at workspace level, so this does not block lints; adding method-level docs is a follow-up if the crate ever turns on `missing_docs`.

## Spec reference corrections

The plan text cited `§Δ01` for the `DOMAIN_PREFIX` addition. The actual delta anchor in `FAMP-v0.5.1-spec.md` line 1018 is `§Δ08` (`v0.5.1-Δ08 — §7.1a Domain separation`). Corrected everywhere — `prefix.rs`, README, and CONTRIBUTING all cite `§7.1a / §Δ08`. No other spec references needed correction; `§7.1a`, `§7.1b`, `§3.6a`, and `INV-10` all check out.

## Known Stubs

None. All edits are doc-only; no placeholder data, no TODOs, no unwired UI.

## Self-Check: PASSED

Files created/modified present:
- `crates/famp-crypto/src/lib.rs` — FOUND
- `crates/famp-crypto/src/error.rs` — FOUND
- `crates/famp-crypto/src/hash.rs` — FOUND
- `crates/famp-crypto/src/keys.rs` — FOUND
- `crates/famp-crypto/src/prefix.rs` — FOUND
- `crates/famp-crypto/src/sign.rs` — FOUND
- `crates/famp-crypto/src/verify.rs` — FOUND
- `crates/famp-crypto/src/traits.rs` — FOUND
- `README.md` — FOUND (with "How FAMP Signs a Message" section)
- `CONTRIBUTING.md` — FOUND (new)

Commits present on `main`:
- `c0c5311` — FOUND
- `243fc19` — FOUND
- `1b432c5` — FOUND

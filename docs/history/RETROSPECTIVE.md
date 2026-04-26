# FAMP — Living Retrospective

Cross-milestone lessons and patterns. Appended after each shipped milestone; cross-milestone trends live at the bottom.

---

## Milestone: v0.6 — Foundation Crates

**Shipped:** 2026-04-13
**Phases:** 3 | **Plans:** 9 | **Tasks:** 16 | **Commits:** 34
**Timeline:** 2026-04-12 → 2026-04-13 (substrate shipped in one execution day after half a day of planning/research)

### What Was Built

- `famp-canonical` — RFC 8785 JCS wrapper over `serde_jcs 0.2.0` with a 12-test conformance gate (Appendix B/C/E, 100K cyberphone float corpus, UTF-16 supplementary-plane sort, NaN/Infinity rejection, duplicate-key rejection) wired into CI as a blocking prerequisite; nightly 100M-line full-corpus workflow on cron; 357-LoC from-scratch fallback plan committed on disk.
- `famp-crypto` — Ed25519 `verify_strict`-only public surface, `TrustedVerifyingKey` ingress newtype that rejects weak / small-subgroup keys, SPEC-03 domain-separation prefix prepended internally via `canonicalize_for_signature`, RFC 8032 KAT gate, PITFALLS §7.1c worked-example byte-exact against external Python reference, `sha256_artifact_id` with NIST FIPS 180-2 KATs, constant-time `subtle::ConstantTimeEq` for signature equality.
- `famp-core` — `Principal`/`Instance` with wire-string round-trip, distinct UUIDv7 ID newtypes (`MessageId`/`ConversationId`/`TaskId`/`CommitmentId`), `ArtifactId` enforcing `sha256:<hex>` at parse time, 15-variant flat `ProtocolErrorKind` covering all §15.1 categories with `ProtocolError` wrapper, `invariants::INV_1..INV_11` public doc anchors, 5-variant `AuthorityScope` ladder with hand-written 5×5 `satisfies()` truth table (no `Ord` derive), exhaustive consumer stub under `#![deny(unreachable_patterns)]` turning any future enum extension into a compile error across downstream crates.

### What Worked

- **External vectors as hard CI gates, from day one.** RFC 8785 Appendix B and the cyberphone float corpus were committed *before* `famp-canonical` had a public API. RFC 8032 Ed25519 vectors were wired before `sign`/`verify` had implementations. PITFALLS §7.1c bytes came verbatim from the spec. Every interop-critical claim is a blocking test, not a TODO.
- **SEED-001 fallback plan written before the decision.** The 357-LoC RFC 8785 from-scratch fallback plan landed on disk in Plan 01-01, *before* Plan 01-03 ran the conformance gate and decided to keep `serde_jcs`. Having the escape hatch pre-committed made the "keep" decision low-risk rather than path-locked.
- **Narrow, phase-appropriate error enums.** Each crate got its own ~5-variant error enum with `thiserror`. Compiler-checked exhaustive `match` caught missing arms during refactors that a single god-enum would have hidden behind `_ => …` fallthrough.
- **Free-function-primary + trait-sugar pattern.** `sign_value` / `verify_canonical_bytes` / `canonicalize_for_signature` are the sanctioned callable surface; `Signer`/`Verifier` traits are thin sugar. Downstream crates can mock the traits for tests without losing the guarantee that the production path used the prefix.
- **Same-day tech-debt closure.** The audit surfaced two real CI-parity gaps (rustfmt drift + integration-test clippy hygiene) and both got closed inside the audit pass, not punted to v0.7. Result: `just ci` was actually green when the milestone archived, not "green once we clean up."
- **Verification gap closure via a dedicated plan, not an extension of the previous plan.** CRYPTO-07 (SHA-256 content-addressing) was flagged by `02-VERIFICATION.md` and closed by Plan 02-04, additive-only, leaving every other crypto source file untouched. Plan 02-04 → commit → verification → audit was a clean cycle.
- **Phase numbering reset to 1 for v0.6** — keeping v0.5.1's docs-only phases separately numbered would have blurred the "first code milestone" boundary in every future trace.

### What Was Inefficient

- **`gsd-tools summary-extract` pulled literal "One-liner:" placeholder text** from several Phase 2 SUMMARY.md files because the front-matter pattern didn't include a proper one-liner line, polluting the auto-generated MILESTONE entry. Had to hand-rewrite the v0.6 accomplishments section during archival. Fixable by: (a) a SUMMARY lint gate that rejects placeholder strings, or (b) a richer template that forces a one-sentence `deliverable:` field.
- **Planning artifacts lived in two places pre-archive.** `.planning/ROADMAP.md` and `.planning/v0.6-MILESTONE-AUDIT.md` both existed at repo root alongside `.planning/milestones/` until archival. Two sources of "current" added friction during the audit pass. Consider moving audit reports into `milestones/` as soon as they're written.
- **`famp-canonical` integration-test clippy hygiene** (`unused_crate_dependencies`, `unwrap_used`, `expect_used`) was a known carried TODO from Plan 01-02 and should have been closed in 01-03, not in the audit pass. Lesson: close phase-local hygiene before running the verification agent, not after.

### Patterns Established

- **External vectors as hard blocking CI gates** — applies to every interop-critical primitive: canonical JSON, Ed25519, SHA-256, (future) envelope schemas. No exceptions, no `continue-on-error`.
- **Fallback plan committed before the decision** — SEED-001 pattern. Any "use upstream crate X or fork?" decision gets a written fork plan on disk first, so the decision is technical not path-dependent.
- **Free-function-primary + trait-sugar** — public callable surface is free functions; traits are thin wrappers. Mocking for tests doesn't break the production invariant.
- **Narrow, phase-appropriate error enums** — not one god enum. ~5 variants per crate, each exhaustively matchable.
- **Exhaustive consumer stub under `#![deny(unreachable_patterns)]`** — for every wire-facing enum (`ProtocolErrorKind`, `AuthorityScope`, future message-type enum), commit a stub that matches every variant explicitly, so adding a variant is a compile error everywhere.
- **Verification gap closure as a dedicated additive plan** — don't extend the previous plan; write Plan N+1 that touches only the gap, and re-verify. Keeps provenance clean.
- **Hand-written truth tables for non-total-order relations** — `AuthorityScope::satisfies()` is a 5×5 hand-written table, not a derived `Ord`. When the relation isn't a total order, don't pretend it is.

### Key Lessons

1. **Byte-exact guarantees cost less than they look like when you front-load the vectors.** Ed25519 + RFC 8785 + SHA-256 all landed byte-exact on first implementation attempt because the vectors were already green-or-red on day one. The hard part was the decision (SEED-001), not the code.
2. **The milestone audit should be non-negotiable before `complete-milestone`, not after.** The v0.6 audit caught two real gaps (format drift, clippy hygiene) that `just ci` would have surfaced on the next CI push. Running the audit first turned a potentially embarrassing post-archive fix into a same-day tech-debt closure.
3. **Phase-local TODOs carried across phase boundaries cost more than they save.** The Plan 01-02 clippy hygiene TODO sat open through two full phases. Closing it in 01-03 would have cost 10 minutes; closing it in the audit cost context re-loading.
4. **SUMMARY.md front-matter is an API, not a log.** It's consumed by `gsd-tools summary-extract` during milestone archival. Missing or placeholder one-liners leak into MILESTONES.md. Treat the SUMMARY as a machine-readable artifact, not freeform notes.

### Cost Observations

- **Model mix:** dominated by Sonnet for per-plan execution; Opus used sparingly for architectural decisions (profile split, SEED-001 decision framing, audit verdict)
- **Session count:** substrate shipped across ~1 planning session + ~1 execution day; phase work ran in yolo mode with auto-advance
- **Notable efficiency:** single-day execution of 9 plans was possible because every plan had RESEARCH.md + VALIDATION.md + external vectors committed before execution started. The up-front cost was ~half a day of planning/research; the payoff was zero in-flight rework.

---

## Cross-Milestone Trends

### Patterns that have held across ≥2 milestones

- **External spec vectors committed verbatim, never self-generated** (PITFALLS P10) — established in v0.5.1 (spec-lint anchors), reinforced in v0.6 (RFC 8785, RFC 8032, §7.1c worked example, NIST FIPS 180-2 KATs). No v0.6 interop gate was generated by code-under-test.
- **`just ci` as a single blocking gate** — v0.5.1 established the CI-parity rule; v0.6 ran every phase under `just ci` end-to-end on closure and in the milestone audit.
- **Spec version pinning via a Rust constant** (`FAMP_SPEC_VERSION = "0.5.1"`) — makes substrate drift against the spec compile-detectable, not review-detectable.

### Open watch-items for v0.7

- How does the free-function-primary + trait-sugar pattern hold up under the `Transport` trait? Traits-as-sugar worked for `Signer`/`Verifier` because the primitive operations are pure; `Transport` has lifetime and async constraints that may push the trait back to primary.
- Does the narrow-error-enum rule survive `famp-envelope`, which has to translate canonical/crypto/core errors into envelope-level errors? Risk: the wrapping enum grows past ~10 variants and becomes the god enum we avoided in v0.6.
- Can `stateright` be avoided entirely for the 4-state task FSM (v0.7 Phase 2) if proptest transition-legality tests are thorough enough? Defer-until-proven-needed vs build-it-once-and-reuse.

---
*Living retrospective — appended per milestone, cross-milestone trends below the last entry.*

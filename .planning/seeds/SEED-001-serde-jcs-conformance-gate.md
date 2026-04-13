---
id: SEED-001
status: dormant
planted: 2026-04-13
planted_during: v0.5.1 Spec Fork (post-completion)
trigger_when: start of Phase 2 (Canonical + Crypto Foundations) — any milestone scoping `famp-canonical`, `famp-crypto`, RFC 8785, JCS, or Ed25519 signing
scope: Medium
---

# SEED-001: serde_jcs RFC 8785 conformance gate + fallback plan

## Why This Matters

**`serde_jcs 0.2.0` is the only serde-integrated RFC 8785 implementation in Rust, and it may be wrong.** The crate is self-labeled "unstable," single-maintainer (l1h3r), with known MEDIUM confidence in docs/STACK.md §2. It is the load-bearing foundation for every signature in FAMP: if its canonical JSON output diverges from RFC 8785 by a single byte, every signature we produce is non-conformant and every two-party interop test fails silently.

The spec fork (v0.5.1) locked canonical JSON to RFC 8785 verbatim — **which means we cannot ship a "close enough" canonicalizer.** Phase 2 is the moment this becomes real code. If we discover the break mid-Phase-2, we lose days to a forced pivot with no fallback plan queued.

This seed exists so Phase 2 starts with the pivot option already scoped.

## When to Surface

**Trigger:** Start of the milestone containing Phase 2 (Canonical + Crypto Foundations).

This seed should be presented during `/gsd:new-milestone` when the milestone scope matches any of these conditions:
- New milestone includes Phase 2 (Canonical + Crypto Foundations)
- Requirements list contains CANON-01..CANON-07 or CRYPTO-01..CRYPTO-08
- Any requirement references RFC 8785, JCS, canonical JSON, or `serde_jcs`
- Crate scope includes `famp-canonical` or `famp-crypto`

## Action When Triggered

1. **Front-load conformance** — The FIRST task in Phase 2 is a hard CI gate running the RFC 8785 Appendix B test vectors against `serde_jcs`. No signing code is written until this gate is green. A failed canonicalization byte at this stage would invalidate every signature downstream; discovering it later is catastrophic.

2. **Budget an explicit decision point** — After the gate runs, there is one branch:
   - **Green on all Appendix B vectors** → keep `serde_jcs` as the `famp-canonical` backing impl.
   - **Red on any vector** → fork to the fallback: ~500 LoC from-scratch JCS implementation in `famp-canonical`, wrapping `ryu-js` for number formatting (the hardest RFC 8785 §3.2.2.3 requirement, matching ECMAScript `Number.prototype.toString`). Do NOT attempt to fix `serde_jcs` upstream mid-phase — too slow, too risky, and the upstream maintenance cadence is unknown.
   - **Partial green (edge cases fail)** → same as red. Partial conformance is non-conformance for a byte-exact protocol.

3. **Cyberphone float corpus** — Separately, Phase 2 needs to run the cyberphone 100M-sample float corpus. Full run may be too slow for CI; budget a sampled subset (stratified across float categories) as the CI gate, with the full corpus behind a `--full-corpus` flag runnable locally. Decide the sampling strategy as part of the same decision point above.

4. **Duplicate-key rejection** — RFC 8785 §3.1 requires duplicate object keys to be rejected at parse. Verify `serde_jcs` enforces this (it may not, since `serde_json` silently dedupes by default). If it doesn't, the fallback must.

## Scope Estimate

**Medium** — A few phase tasks at the start of Phase 2:
- 1 task: wire RFC 8785 Appendix B vectors as a CI gate
- 1 task: run the gate, make the keep/fork decision
- 1 conditional task (only if fork): scaffold `famp-canonical` fallback against `ryu-js`
- 1 task: cyberphone sampling strategy + CI wiring
- 1 task: duplicate-key rejection test

If `serde_jcs` passes cleanly, Phase 2's canonical JSON work is ~1 day. If it fails, add ~3–5 days for the fallback.

## Breadcrumbs

Related context already in the codebase:

- **`docs/STACK.md`** — §2 labels `serde_jcs` as MEDIUM confidence, explicitly noting the "unstable" label, single-maintainer risk, and the ~500 LoC fallback plan. This is the canonical source.
- **`docs/PITFALLS.md`** — Canonical-JSON reviewer findings (cited throughout v0.5.1 spec fork changelog). Specifically P10: conformance vectors MUST come from an external reference implementation, not self-generated. This shapes how the CI gate is constructed.
- **`.planning/milestones/v0.5.1-phases/01-spec-fork-v0-5-1/01-RESEARCH.md`** §2 — Normative citations for RFC 8785 sections the canonicalizer must honor (§3.1 duplicate keys, §3.2.2.3 ECMAScript number format, §3.2.3 UTF-16 sort).
- **`FAMP-v0.5.1-spec.md`** §4a — The normative text that locks canonical JSON to RFC 8785. §4a.1/§4a.2 worked Examples A and B (ASCII mixed-case + U+1F600 emoji) were generated with Python `jcs 0.2.1` externally; these same bytes can serve as the FIRST Rust-side conformance check and a sanity test against `serde_jcs`.
- **`.planning/milestones/v0.5.1-phases/01-spec-fork-v0-5-1/01-06-SUMMARY.md`** — Documents the external Python `jcs 0.2.1` + `cryptography 46.0.7` toolchain used for §7.1c worked signature example. Phase 2 should reproduce these bytes in Rust as an additional cross-check.
- **`.planning/STATE.md`** → "Known Blockers" already flags "`serde_jcs` correctness unknown on RFC 8785 edge cases — fallback plan ready if CI gate fails." This seed is the persistent version of that reminder.

## Notes

- Roadmap's existing Phase 2 description already says "HIGHEST RISK" and flags the research-phase requirement. This seed is **not** a new concern — it's existing research hardened into an actionable trigger so nothing is lost between milestones.
- If the fallback path is taken, `famp-canonical`'s ~500 LoC becomes a project deliverable worth extracting as a standalone crate on crates.io later. That's a bonus, not a goal.
- The PITFALLS P10 rule (no self-generated conformance vectors) means Phase 2's gate vectors must come from: (a) RFC 8785 Appendix B verbatim, (b) cyberphone reference, (c) the Python `jcs` output already committed in §4a of the spec. Three independent external sources = trustworthy.

---
phase: 1
slug: canonical-json-foundations
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-12
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo-nextest 0.9.132` + `proptest 1.11` + `insta 1.47` |
| **Config file** | `crates/famp-canonical/Cargo.toml`, `.config/nextest.toml` (Wave 0 creates) |
| **Quick run command** | `just test-canonical` (→ `cargo nextest run -p famp-canonical`) |
| **Full suite command** | `just test` (→ `cargo nextest run --workspace`) |
| **Estimated runtime** | ~30 seconds (quick), ~90 seconds (full with proptest) |

---

## Sampling Rate

- **After every task commit:** Run `just test-canonical`
- **After every plan wave:** Run `just test`
- **Before `/gsd:verify-work`:** Full suite + RFC 8785 Appendix B vectors must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| TBD — populated by planner | | | | | | | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/famp-canonical/Cargo.toml` — crate scaffold
- [ ] `crates/famp-canonical/tests/rfc8785_vectors.rs` — RFC 8785 Appendix B vector harness
- [ ] `crates/famp-canonical/tests/fixtures/rfc8785/` — externally-sourced Appendix B vectors
- [ ] `crates/famp-canonical/tests/utf16_sort.rs` — supplementary-plane fixture stubs
- [ ] `crates/famp-canonical/tests/number_format.rs` — cyberphone es6testfile float corpus harness
- [ ] `crates/famp-canonical/tests/duplicate_keys.rs` — duplicate-key rejection stubs
- [ ] `crates/famp-canonical/tests/artifact_id.rs` — `sha256:<hex>` helper stubs
- [ ] `.config/nextest.toml` — nextest profile
- [ ] `Justfile` — `test-canonical`, `test`, `vectors` recipes

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| SEED-001 decision recorded | CANON-06 | Human rationale in ADR, not a test | Verify `crates/famp-canonical/docs/seed-001-decision.md` exists and is linked from STATE.md |
| Fallback plan document | CANON-07 | Narrative doc, not executable | Verify `crates/famp-canonical/docs/fallback.md` exists with RFC 8785 §3 key sort + number formatter + UTF-8 pass-through sections |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

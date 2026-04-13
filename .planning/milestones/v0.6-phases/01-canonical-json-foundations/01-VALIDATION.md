---
phase: 1
slug: canonical-json-foundations
status: planned
nyquist_compliant: true
wave_0_complete: true
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
| 01-01-T1 | 01-01 | 1 | SPEC-02 (deps) | build | `cargo build -p famp-canonical` | ❌ Wave 0 | ⬜ pending |
| 01-01-T2 | 01-01 | 1 | CANON-07 | manual | `test -f crates/famp-canonical/docs/fallback.md && wc -l` | ❌ Wave 0 | ⬜ pending |
| 01-01-T3 | 01-01 | 1 | CANON-02/03/04/05/06, SPEC-18 (harness) | build | `cargo build -p famp-canonical --tests` | ❌ Wave 0 | ⬜ pending |
| 01-02-T1 | 01-02 | 2 | CANON-01, CANON-06, SPEC-18 | unit | `cargo nextest run -p famp-canonical duplicate_keys artifact_id` | ❌ Wave 2 | ⬜ pending |
| 01-02-T2 | 01-02 | 2 | CANON-04 (fixtures), CANON-03 (corpus sample) | unit (compile) | `cargo build -p famp-canonical --tests` | ❌ Wave 2 | ⬜ pending |
| 01-03-T1 | 01-03 | 3 | CANON-02, CANON-03, CANON-04, CANON-05 | unit + manual ADR | `cargo nextest run -p famp-canonical && grep "Decision:" .planning/SEED-001.md` | ❌ Wave 3 | ⬜ pending |
| 01-03-T2 | 01-03 | 3 | CANON-02 (CI gate) | manual + recipe | `just test-canonical-strict` | ❌ Wave 3 | ⬜ pending |
| 01-03-T3 | 01-03 | 3 | Phase closeout | checkpoint | human-verify | ❌ Wave 3 | ⬜ pending |

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

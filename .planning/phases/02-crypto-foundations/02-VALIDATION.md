---
phase: 2
slug: crypto-foundations
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-13
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test + cargo nextest (Rust stable) |
| **Config file** | `Cargo.toml` (workspace), `crates/famp-crypto/Cargo.toml` |
| **Quick run command** | `cargo nextest run -p famp-crypto` |
| **Full suite command** | `cargo nextest run --workspace && cargo test --workspace --doc` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo nextest run -p famp-crypto`
- **After every plan wave:** Run `cargo nextest run --workspace && cargo test --workspace --doc`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| TBD | TBD | TBD | TBD | TBD | TBD | TBD | ⬜ pending |

*Populated by planner. Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/famp-crypto/Cargo.toml` — scaffold crate with `ed25519-dalek`, `sha2`, `base64`, `thiserror` deps
- [ ] `crates/famp-crypto/tests/vectors/` — directory for committed RFC 8032 + P10 fixtures
- [ ] `crates/famp-crypto/tests/common/mod.rs` — shared fixture loaders

*If existing infra covers: planner may remove and note "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Constant-time verification path | CRYPTO-08 | Timing-side-channel proofs require statistical analysis (`dudect`) outside normal test loop | Document rationale; optional `dudect`-style harness behind `--ignored` flag |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

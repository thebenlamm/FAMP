---
phase: 1
slug: identity-cli-foundation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-14
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo-nextest 0.9.x (unit + integration) |
| **Config file** | `Cargo.toml` workspace + `crates/famp/tests/` |
| **Quick run command** | `cargo nextest run -p famp` |
| **Full suite command** | `cargo nextest run --workspace` |
| **Estimated runtime** | ~30–60 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo nextest run -p famp`
- **After every plan wave:** Run `cargo nextest run --workspace`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

*Populated by planner — one row per task with its automated command and requirement mapping.*

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| _(filled by planner)_ |  |  |  |  |  |  |  |  | ⬜ |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*Derived from RESEARCH.md Wave 0 gap list — planner populates with exact paths.*

- [ ] `crates/famp/tests/` integration test files for IDENT-01..06, CLI-01, CLI-07
- [ ] Shared test helpers (tempdir FAMP_HOME fixture)
- [ ] Serial test harness for any env-var-sensitive tests

*Existing substrate (`famp-crypto`, `famp-keyring`, `famp-transport-http`) provides base crates; Phase 1 installs `famp` binary crate test scaffolding.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| _(none expected — all phase behaviors automatable)_ | | | |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

---
phase: 2
slug: minimal-task-lifecycle
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
| **Framework** | cargo-nextest 0.9.x + proptest 1.11 + rustc trybuild/compile-fail |
| **Config file** | `Cargo.toml` (workspace), `famp-task/Cargo.toml` |
| **Quick run command** | `cargo nextest run -p famp-task` |
| **Full suite command** | `just test` (cargo nextest + proptest + clippy -D warnings + trybuild) |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo nextest run -p famp-task`
- **After every plan wave:** Run `just test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| *(populated by planner)* | | | | | | | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `famp-task/tests/fsm_legality.rs` — proptest enumerating full `(class, relation, terminal_status, current_state)` tuple space
- [ ] `famp-task/tests/compile_fail/` — trybuild fixtures proving illegal transitions fail to compile (INV-5, FSM-03)
- [ ] `famp-task/tests/rejected_expired_absent.rs` — static assertions that `Rejected` / `Expired` variants do not exist (FSM-02 narrowed)
- [ ] proptest + trybuild dev-dependencies added to `famp-task/Cargo.toml`

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| *(none — all behaviors automatable via proptest + trybuild)* | | | |

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

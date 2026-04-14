---
phase: 2
slug: minimal-task-lifecycle
status: approved
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-13
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo-nextest 0.9.x + proptest 1.11 |
| **Config file** | `Cargo.toml` (workspace), `crates/famp-fsm/Cargo.toml` |
| **Quick run command** | `cargo nextest run -p famp-fsm` |
| **Full suite command** | `just test` (nextest + proptest + `cargo clippy -D warnings` + `cargo fmt --check`) |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo nextest run -p famp-fsm` (or scoped variant for the task's crate)
- **After every plan wave:** Run `just test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 02-01 | 1 | (infra) | unit | `cargo nextest run -p famp-core -p famp-envelope` | ✅ | ⬜ pending |
| 02-01-02 | 02-01 | 1 | (infra) | unit | `cargo nextest run -p famp-core -p famp-envelope` | ✅ | ⬜ pending |
| 02-02-01 | 02-02 | 2 | FSM-02, FSM-05 | unit | `cargo nextest run -p famp-fsm` | ✅ W0 | ⬜ pending |
| 02-02-02 | 02-02 | 2 | FSM-04 | unit + fixture | `cargo nextest run -p famp-fsm` | ✅ W0 | ⬜ pending |
| 02-03-01 | 02-03 | 3 | FSM-03 | exhaustiveness | `cargo nextest run -p famp-fsm --test consumer_stub` | ✅ W0 | ⬜ pending |
| 02-03-02 | 02-03 | 3 | FSM-08 | proptest | `cargo nextest run -p famp-fsm --test proptest_matrix` | ✅ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Tests are authored in the same task as the code they validate (TDD-adjacent pattern). The plans produce the following test artifacts directly — no separate Wave 0 stub task is required:

- [x] `crates/famp-fsm/tests/deterministic.rs` — 6 legal-arrow fixtures + 3×5×4=60 terminal-immutability matrix (produced by 02-02 Task 2)
- [x] `crates/famp-fsm/tests/consumer_stub.rs` — downstream exhaustiveness gate under `#![deny(unreachable_patterns)]` (produced by 02-03 Task 1, satisfies INV-5 / FSM-03)
- [x] `crates/famp-fsm/tests/proptest_matrix.rs` — Cartesian product over `TaskState × MessageClass × Option<TerminalStatus>` = 5×5×4 = 100 tuples, 2048 cases, oracle-checked (produced by 02-03 Task 2, satisfies FSM-08)
- [x] `proptest` dev-dep added to `crates/famp-fsm/Cargo.toml` (produced by 02-03 Task 2)

*`Option<TaskRelation>` axis is absent because the `relation` field was dropped per D-B3 escape hatch, resolved in 02-RESEARCH.md.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| *(none)* | | | |

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covered by in-task test authoring (documented above)
- [x] No watch-mode flags
- [x] Feedback latency < 60s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-04-13

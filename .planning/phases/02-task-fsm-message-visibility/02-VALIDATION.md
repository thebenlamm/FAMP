---
phase: 2
slug: task-fsm-message-visibility
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-05-10
audited: 2026-05-10
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo-nextest (Rust) |
| **Config file** | `.config/nextest.toml` (workspace-level) |
| **Quick run command** | `cargo nextest run -p famp-inspect-proto -p famp-inspect-server` |
| **Full suite command** | `just test` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo nextest run -p famp-inspect-proto -p famp-inspect-server`
- **After every plan wave:** Run `just test`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 45 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Secure Behavior | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------------|-----------|-------------------|--------|
| 02-01-T1 | 01 | 0 | INSP-TASK-01/02 | Kind-tagged reply enums + serde codec roundtrip + JCS canonicalize_roundtrip | unit | `cargo nextest run -p famp-inspect-proto` | ✅ green |
| 02-01-T2 | 01 | 0 | INSP-RPC-03, INSP-MSG-01..03 | TaskSnapshot/MessageSnapshot handlers; FSM derivation for all 5 states; body hash prefix 12-char | unit | `cargo nextest run -p famp-inspect-server` | ✅ green |
| 02-02-T1 | 02 | 1 | INSP-RPC-04 | spawn_blocking pool sized 1024; BudgetExceeded payload is kind-tagged JSON | unit | `cargo nextest run -p famp --lib -E 'test(/broker_inspect_tests/)'` | ✅ green |
| 02-02-T2 | 02 | 1 | INSP-TASK-01..04, INSP-MSG-01..03 | Lazy taskdir walk and mailbox pre-read; non-I/O inspect kinds skip reads | unit | `cargo nextest run -p famp --lib -E 'test(/broker_inspect_tests/)'` | ✅ green |
| 02-03-T1 | 03 | 2 | INSP-TASK-01..04, INSP-MSG-01..03, INSP-RPC-03 | CLI e2e: task grouping, full JCS through jq, message metadata-only, broker-down, budget-exceeded; orphan column value renders true; orphans-only filter excludes non-orphan rows | integration + unit | `cargo nextest run -p famp --test inspect_tasks --test inspect_messages` | ✅ green |
| 02-03-T2 | 03 | 2 | INSP-RPC-04 | 1000 concurrent cancel — no leaked FDs (lsof count stable) | integration | `cargo nextest run -p famp --test inspect_cancel_1000` | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `crates/famp-inspect-proto/src/lib.rs` — kind-tagged reply enums with serde codec + JCS roundtrip
- [x] `crates/famp-inspect-server/src/lib.rs` — TaskSnapshot/MessageSnapshot on BrokerCtx, FSM derivation, body hash
- [x] `crates/famp/src/cli/broker/mod.rs` — spawn_blocking + 500ms timeout + BudgetExceeded reply

*All three wave-0 items compile and have green unit coverage.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `--orphans` bucket renders above per-task groups | INSP-TASK-02 | CLI-level visual ordering of orphan vs non-orphan groups; unit tests verify filter and column value | Run: `famp inspect tasks`; confirm orphan rows (ORPHAN=true) are visually grouped above non-orphan task groups in table output |

---

## Validation Audit 2026-05-10

| Metric | Count |
|--------|-------|
| Gaps found | 2 |
| Resolved | 2 |
| Escalated | 0 |

**Gap 1 resolved:** `tail_3_returns_only_three_rows` LEAK — fixed by adding explicit broker start/stop matching the `tail_default_is_50` pattern; auto-broker from `famp register` was surviving past test teardown.

**Gap 2 resolved:** Added `orphan_column_value_renders_true_for_orphan_row` and `orphans_only_filter_excludes_non_orphan_rows` unit tests to `crates/famp/src/cli/inspect/tasks.rs`.

**Manual-Only reduction:** `--full` JCS roundtrip promoted from manual-only to COVERED (automated by `id_full_jcs_pipes_through_jq` integration test added in Plan 03).

---

## Validation Sign-Off

- [x] All tasks have automated verify
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 45s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** 2026-05-10

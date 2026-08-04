---
phase: 19
slug: auto-wake-gate
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-03
---

# Phase 19 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness via `cargo test`; Tokio only in `famp` integration tests |
| **Config file** | Workspace `Cargo.toml` and crate manifests |
| **Quick run command** | `cargo test -p famp-bus --lib auto_wake && cargo test -p famp --test auto_wake_gate` |
| **Full suite command** | `cargo test --workspace && just lint && cargo fmt --all -- --check && just check-no-tokio-in-bus` |
| **Estimated runtime** | Measure during Wave 0 and record before sign-off |

---

## Sampling Rate

- **After every task commit:** Run the task's focused test target plus `just check-no-tokio-in-bus`
- **After every plan wave:** Run `cargo test -p famp-bus && cargo test -p famp --test auto_wake_gate --test quarantine_surfaces --test pair_cli && just lint && cargo fmt --all -- --check`
- **Before `$gsd-verify-work`:** Run `cargo test --workspace && just lint && cargo fmt --all -- --check && just check-no-tokio-in-bus`; the full suite must be green
- **Max feedback latency:** Record measured focused-test runtime during Wave 0; keep each task's default feedback command focused rather than workspace-wide

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 19-01-01 | 01 | 1 | QUAR-12, QUAR-13, QUAR-14 | T-19-01, T-19-02, T-19-03 | Gateway and Unknown cannot select or drain into Await; Local wakes; remote-then-local does not starve | broker actor unit | `cargo test -p famp-bus --lib auto_wake` | ❌ W0 cases in existing `handle/tests.rs` | ⬜ pending |
| 19-01-02 | 01 | 1 | QUAR-12, QUAR-13 | T-19-02, T-19-03 | Repeated fixtures preserve production Local stamps without erasing legacy controls | property regression | `cargo test -p famp-bus --test prop01_dm_fanin_order --test prop02_channel_fanout --test prop04_drain_completeness` | ✅ existing files; fixture repair pending | ⬜ pending |
| 19-02-01 | 02 | 2 | QUAR-12, QUAR-13, QUAR-14 | T-19-05, T-19-06 | One real socket proves remote held, Local wakes, and explicit Inbox retains remote | integration | `cargo test -p famp --test auto_wake_gate` | ❌ W0 new file | ⬜ pending |
| 19-02-02 | 02 | 2 | QUAR-12, QUAR-14 | T-19-05 | Conflicting Phase 14 Gateway-Await-wakes expectation is removed while rendering controls stay green | integration regression | `cargo test -p famp --test quarantine_surfaces` | ✅ existing file; expectation replacement pending | ⬜ pending |
| 19-03-01 | 03 | 2 | QUAR-12, QUAR-13, QUAR-14, QUAR-15 | T-19-08, T-19-10 | Documentation states the narrow broker boundary and every residual limitation | docs plus integration | `cargo test -p famp --test auto_wake_gate && cargo test -p famp --test quarantine_surfaces && cargo test -p famp --test pair_cli consent_warning_matches_quarantine_doc` | ✅ existing docs; truth update pending | ⬜ pending |
| 19-03-02 | 03 | 2 | QUAR-15 | T-19-09 | Consent warning precedes pairing code and matches quarantine documentation; phase gate measured | integration regression + phase gate | `cargo test -p famp --test pair_cli consent_warning_matches_quarantine_doc && cargo test -p famp --test pair_cli artifact_code_offset_greater_than_consent_and_install_lines` | ✅ existing tests | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Repair broker test fixtures so local clients register `Origin::Local`, origin-specific fixtures are explicit, and mailbox helpers stamp `Out::AppendMailbox` exactly as production does; retain intentional absent-origin compatibility cases.
- [ ] Add focused actor cases in `crates/famp-bus/src/broker/handle/tests.rs` for Gateway, Unknown, Local, preexisting remote drain, remote-then-local ordering, and the channel path.
- [ ] Add `crates/famp/tests/auto_wake_gate.rs` using the established broker/register/`BusClient` harness.
- [ ] Replace `quarantine_surfaces.rs::await_marks_gateway_origin`, whose expected behavior conflicts with Phase 19.
- [ ] Confirm the existing QUAR-15 `pair_cli` regressions remain green.
- [ ] Measure focused and full validation runtimes and replace the provisional latency entries above.

---

## Manual-Only Verifications

All phase behaviors have automated verification. Documentation wording is asserted where a stable exact string is required and reviewed as part of the plan acceptance criteria.

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency is measured and bounded
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

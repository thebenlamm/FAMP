---
phase: 19
slug: auto-wake-gate
status: passed
nyquist_compliant: true
wave_0_complete: true
created: 2026-08-03
last_run: 2026-08-04
---

# Phase 19 — Validation Strategy

> Final execution ledger for Phase 19. Focused Phase 19 evidence and the phase-wide workspace gate are green.

## Test Infrastructure

| Property | Value |
|---|---|
| Framework | Rust built-in test harness via `cargo test`; Tokio only in `famp` integration tests |
| Quick run | `cargo test -p famp-bus --lib auto_wake && cargo test -p famp --test auto_wake_gate` |
| Full gate | `cargo test --workspace && just lint && cargo fmt --all -- --check && just check-no-tokio-in-bus` |
| Full-gate status | PASS — `cargo test --workspace --no-fail-fast` exits zero |
| Full workspace runtime | Final authoritative rerun completed successfully on 2026-08-05 |

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Automated command | Measured runtime | Status |
|---|---:|---:|---|---|---|---:|---|
| 19-01-01 | 01 | 1 | QUAR-12, QUAR-13, QUAR-14 | T-19-01, T-19-02, T-19-03 | `cargo test -p famp-bus --lib auto_wake` | 7.13s | ✅ green (5 passed) |
| 19-01-02 | 01 | 1 | QUAR-12, QUAR-13 | T-19-02, T-19-03 | `cargo test -p famp-bus --test prop01_dm_fanin_order --test prop02_channel_fanout --test prop04_drain_completeness` | 27.03s | ✅ green (5 passed) |
| 19-02-01 | 02 | 2 | QUAR-12, QUAR-13, QUAR-14 | T-19-05, T-19-06 | `cargo test -p famp --test auto_wake_gate` | 2.23s final warm run | ✅ green (1 passed) |
| 19-02-02 | 02 | 2 | QUAR-12, QUAR-14 | T-19-05 | `cargo test -p famp --test quarantine_surfaces` | 2.32s | ✅ green (12 passed) |
| 19-03-01 | 03 | 3 | QUAR-12, QUAR-13, QUAR-14, QUAR-15 | T-19-08, T-19-10 | Exact Task 19-03-01 `<automated>` fixed-string assertions, focused Rust targets, no-Tokio gate, and `just install` (executed verbatim from `19-03-PLAN.md`) | 186.17s across timed components | ✅ green |
| 19-03-02 | 03 | 3 | QUAR-15 | T-19-09 | `cargo test -p famp --test pair_cli consent_warning_matches_quarantine_doc && cargo test -p famp --test pair_cli artifact_code_offset_greater_than_consent_and_install_lines && cargo test -p famp-bus --lib auto_wake && cargo test -p famp --test auto_wake_gate && cargo test -p famp --test quarantine_surfaces && cargo test --workspace --no-fail-fast && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check` plus the `check-no-tokio-in-bus` recipe body | Final authoritative rerun completed successfully on 2026-08-05 | ✅ green |

## Measured Evidence

- Task 19-03-01 deterministic positive/negative documentation assertions: PASS (0.40s).
- MCP twelve-tool descriptor regression: PASS (61.55s cold build).
- Task 19-03-01 real-socket gate: PASS (25.15s cold run).
- Task 19-03-01 quarantine surfaces: PASS, 12/12 (20.38s cold run).
- Task 19-03-01 consent warning/doc regression: PASS (18.39s cold run).
- `just check-no-tokio-in-bus`: PASS (0.96s during install gate; 0.05s final run).
- `just install`: PASS (59.74s); `~/.cargo/bin/famp` and Claude Code integration were refreshed.
- QUAR-15 exact doc bytes: PASS (0.67s).
- QUAR-15 warning-before-five-word-code order: PASS (0.24s).
- Narrow blocker regressions: `adversarial` 6/6 PASS (0.39s final warm run), `e2e_two_daemons_adversarial` PASS (67.79s cold rebuild), and `quarantine_gate` 2/2 PASS (27.40s).
- `just lint`: PASS (161.33s).
- `cargo fmt --all -- --check`: PASS (1.01s).
- Final `just check-no-tokio-in-bus`: PASS (0.05s).

## Final Full-Gate Result

The authoritative 2026-08-05 rerun of `cargo test --workspace --no-fail-fast` passed in full. It included the formerly failing `e2e_shipping_surface` target, all Phase 19 focused tests, gateway/relay E2Es, and the repaired `famp-transport-http` 19/19 unit target. `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, and the `check-no-tokio-in-bus` recipe body also passed.

One bounded validation-only repair was required: `reqwest_failed_display_contains_source_text` constructed a rustls-backed reqwest client without installing a crypto provider. Commit `76673aa` installs the provider in that test before client construction; no production behavior changed.

## Wave 0 Requirements

- [x] Production-faithful Local/origin-specific broker fixtures exist and pass.
- [x] Actor cases cover Gateway, Unknown, Local, initial remote drain, remote-then-local ordering, and channel delivery.
- [x] `crates/famp/tests/auto_wake_gate.rs` proves the one-broker remote-held/Local-wake/Inbox-visible sequence.
- [x] The obsolete `await_marks_gateway_origin` expectation is removed; truthful rendering controls remain green.
- [x] Both existing QUAR-15 pairing regressions remain green without pairing-file changes.
- [x] Focused and full-gate runtimes were measured and recorded.

`wave_0_complete` is true; Task 19-03-02 and the phase-wide full gate are complete.

## Manual-Only Verifications

None. All Phase 19 behavior and stable wording checks are automated.

## Validation Sign-Off

- [x] Every task has an automated command.
- [x] Sampling continuity has no three-task gap.
- [x] All Wave 0 files and focused tests exist.
- [x] No watch-mode flags are used.
- [x] Focused feedback latency is measured.
- [x] `cargo test --workspace --no-fail-fast` exits zero.
- [x] `nyquist_compliant: true` is set in frontmatter.

**Approval:** passed — focused, workspace, lint, formatting, and architecture gates are green.

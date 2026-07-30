---
phase: 9
slug: end-to-end-cross-host-delivery
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-23
---

# Phase 9 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test harness (`cargo test`) + `assert_cmd` subprocess tests |
| **Config file** | `Cargo.toml` (workspace); `.config/nextest.toml` for subprocess serialization |
| **Quick run command** | `cargo test -p famp-gateway --lib` |
| **Full suite command** | `just ci` (fmt + clippy `-D warnings` + full test) |
| **Estimated runtime** | ~60–180 seconds |

> **Gotcha (memory):** `cargo nextest -p famp` hangs in the test-binary `--list` phase — use plain `cargo test --lib` / `cargo test --test <name>` for the gateway/famp crates. Rust changes MUST run `just lint` (not plain clippy) before push.

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p famp-gateway --lib`
- **After every plan wave:** Run the wave's integration test(s) + `cargo build`
- **Before `/gsd-verify-work`:** `just ci` must be green
- **Max feedback latency:** ~180 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| _(populated by planner/executor)_ | | | GW-01/02/03 | — | verified-only ingress delivery | unit + subprocess E2E | `cargo test -p famp-gateway` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Gateway send/drain unit test scaffold (`ProxiedPrincipal` send path — GW-01/02)
- [ ] `verify_inbound_any` class-dispatch unit test (request/commit/deliver/ack)
- [ ] Two-process loopback E2E harness (`crates/famp-gateway/tests/`) reusing `ChildGuard` + fixture certs (GW-03)

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Live two-physical-machine round trip | GW-01/02/03 | Requires two real hosts + hand-copied keys | Deferred to Phase 10 (DOC-04 setup guide + human UAT); Phase 9's automated gate is two-process loopback |

*The two-process loopback E2E provides automated verification for the phase gate; the true two-machine run is Phase 10.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 180s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

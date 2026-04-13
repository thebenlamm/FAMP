---
phase: 3
slug: memorytransport-tofu-keyring-same-process-example
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-13
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` / `cargo nextest` + `proptest` |
| **Config file** | `Cargo.toml` workspace (nextest defaults) |
| **Quick run command** | `cargo nextest run -p famp-transport -p famp-keyring -p famp-runtime` |
| **Full suite command** | `cargo nextest run --workspace && cargo run --example personal_two_agents` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run quick command (scoped to touched crate)
- **After every plan wave:** Run full suite command
- **Before `/gsd-verify-work`:** Full suite must be green, example binary exits 0
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

Populated by gsd-planner during planning. Requirements to cover:
TRANS-01, TRANS-02, KEY-01, KEY-02, KEY-03, EX-01, CONF-03, CONF-05, CONF-06, CONF-07.

---

## Wave 0 Requirements

- [ ] `crates/famp-transport/Cargo.toml` — new crate with tokio dep
- [ ] `crates/famp-keyring/Cargo.toml` — new crate, depends on famp-crypto
- [ ] `crates/famp-runtime/Cargo.toml` — new crate (or module) for runtime glue + RuntimeError
- [ ] `examples/personal_two_agents.rs` — example binary wired into root workspace

---

## Manual-Only Verifications

*All phase behaviors have automated verification — example binary self-checks ordering and exit code.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

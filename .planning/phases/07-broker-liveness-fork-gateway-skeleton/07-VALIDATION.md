---
phase: 7
slug: broker-liveness-fork-gateway-skeleton
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-23
---

# Phase 7 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` (integration tests under `crates/*/tests/`); property tests via `proptest` where applicable |
| **Config file** | Cargo workspace `Cargo.toml` (no separate test config) |
| **Quick run command** | `cargo test -p famp-gateway` (or the touched crate) |
| **Full suite command** | `cargo test` (workspace) + `just lint` |
| **Estimated runtime** | ~60–120 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p <touched-crate>`
- **After every plan wave:** Run `cargo test` (workspace) + `just lint`
- **Before `/gsd-verify-work`:** Full suite must be green, `just lint` clean
- **Max feedback latency:** ~120 seconds

---

## Per-Task Verification Map

> Populated by the planner/executor. Each success criterion maps to a deterministic test.
> Reaping is timing-dependent (one liveness-sweep interval ≈ 1s) — tests must poll `famp inspect`
> with a bounded deadline, not sleep-then-assert. Tests spawning `famp register`/broker children
> MUST use the ChildGuard RAII kill-on-drop convention.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 07-02-01 | 02 | 1 | LIVE-01 | — | N broker clients sharing one live PID all survive the sweep; all reap together when that PID dies (proves `register()` imposes no pid-uniqueness — the load-bearing Design A fact) | unit | `cargo test -p famp-bus --lib live01_shared_pid_clients_survive_sweep_and_reap_together` | ❌ W0 | ⬜ pending |
| 07-01-01 | 01 | 1 | LIVE-01, LIVE-02, GW-04 | — | `ProxiedPrincipal::register` sends `Register { pid: std::process::id(), listen: false }`; `GatewayRegistry` demuxes strictly by name; killable `famp-gateway` bin builds | unit | `cargo test -p famp-gateway --lib && cargo build -p famp-gateway --bin famp-gateway` | ❌ W0 | ⬜ pending |
| 07-03-01 | 03 | 2 | LIVE-02 | — | Real gateway process backing ≥2 principals keeps them live while running; SIGKILL reaps all within one sweep interval (~1s); no orphan holders | integration | `cargo test -p famp-gateway --test liveness live02_gateway_exit_reaps_all_principals` | ❌ W0 | ⬜ pending |
| 07-03-02 | 03 | 2 | GW-04 | — | One gateway backing `alice` + `bob`: a message to `alice` never appears in `bob`'s mailbox | integration | `cargo test -p famp-gateway --test no_cross_talk gw04_no_cross_talk_between_proxied_principals` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*
*Every automated command is suffixed with `&& just lint` in the plans; reaping tests poll `famp inspect` with a bounded deadline (never sleep-then-assert).*

---

## Wave 0 Requirements

- [ ] `crates/famp-bus/src/broker/handle/tests.rs` — new pure-broker LIVE-01 test via existing `TestEnv` + `FakeLiveness` (plan 02)
- [ ] `crates/famp-gateway/tests/common/child_guard.rs` — copied ChildGuard RAII kill-on-drop helper (plan 03)
- [ ] `crates/famp-gateway/tests/liveness.rs` — LIVE-02 SIGKILL-and-poll integration harness (plan 03)
- [ ] `crates/famp-gateway/tests/no_cross_talk.rs` — GW-04 isolation harness (plan 03)
- [ ] Reuse `crates/famp/tests/broker_proxy_semantics.rs` as the SIGKILL-and-assert-reaping template

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `famp inspect identities` / `famp inspect broker` show the proxied principal as live, then gone after gateway exit | LIVE-01, LIVE-02 | Human-observed CLI output confirms the automated poll matches operator-visible state | Start gateway backing a principal → `famp inspect identities` (live) → kill gateway → `famp inspect identities` (gone within ~1s) |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

---
phase: 07-broker-liveness-fork-gateway-skeleton
verified: 2026-07-23T20:00:00Z
status: passed
score: 3/3 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 7: Broker-Liveness Fork + Gateway Skeleton Verification Report

**Phase Goal:** The same-host `kill(pid,0)` liveness fork is resolved — a gateway-proxied remote principal stays live for as long as the gateway process is alive and reaps cleanly when it exits — and the `famp-gateway` crate skeleton exists to back concurrent remote principals on the local UDS bus (Design A: local-proxy, zero famp-bus change).

**Verified:** 2026-07-23
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | LIVE-01: A gateway-proxied principal registers carrying the gateway's own `std::process::id()`, and the broker's `kill(pid,0)` sweep imposes no pid-uniqueness constraint (N clients sharing one PID all survive/reap together) | ✓ VERIFIED | `crates/famp-gateway/src/principal.rs:41` sends `pid: std::process::id()`. Pure-broker test `live01_shared_pid_clients_survive_sweep_and_reap_together` in `crates/famp-bus/src/broker/handle/tests.rs:2651` — ran `cargo test -p famp-bus --lib live01_shared_pid`: **1 passed**. |
| 2 | LIVE-02: When the gateway process exits, all its proxied principals are reaped cleanly, no orphan holders | ✓ VERIFIED | Ran `cargo test -p famp-gateway --test liveness live02_gateway_exit_reaps_all_principals -- --nocapture`: **1 passed** (real broker + real `famp-gateway` subprocess, SIGKILL, bounded-deadline poll confirms both `alice`+`bob` gone). |
| 3 | GW-04: A single gateway process backs multiple remote principals concurrently with no cross-talk between them | ✓ VERIFIED | Ran `cargo test -p famp-gateway --test no_cross_talk gw04_no_cross_talk_between_proxied_principals -- --nocapture`: **1 passed** (tagged message to `alice` confirmed absent from `bob`'s mailbox). `GatewayRegistry` (`crates/famp-gateway/src/registry.rs`) demuxes strictly by name, `HashMap<String, ProxiedPrincipal>`, rejects duplicate `back()`. |

**Score:** 3/3 truths verified (0 present, behavior-unverified)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/famp-gateway/Cargo.toml` | workspace member, lib + `[[bin]] famp-gateway` | ✓ VERIFIED | Present; `cargo build -p famp-gateway --bins` exits 0. |
| `crates/famp-gateway/src/lib.rs` | crate root, re-exports | ✓ VERIFIED | `pub mod error/principal/registry`, re-exports present. |
| `crates/famp-gateway/src/error.rs` | `GatewayError` (thiserror) | ✓ VERIFIED | All variants present incl. `BrokerUnreachable`. |
| `crates/famp-gateway/src/principal.rs` | `ProxiedPrincipal::register` — Design A mechanism | ✓ VERIFIED | Sends `pid: std::process::id()` (3 occurrences), `listen: false` (2 occurrences), via `connect_no_spawn` (3 occurrences). |
| `crates/famp-gateway/src/registry.rs` | `GatewayRegistry` demux table | ✓ VERIFIED | `HashMap<String, ProxiedPrincipal>`, rejects duplicate names. |
| `crates/famp-gateway/src/main.rs` | `[[bin]] famp-gateway`, killable process | ✓ VERIFIED | Parses `--socket` + principal names, backs each, parks on `ctrl_c`; confirmed genuinely killable in LIVE-02 subprocess test (SIGKILL). |
| Root `Cargo.toml` workspace member | `crates/famp-gateway` registered | ✓ VERIFIED | `grep -c 'crates/famp-gateway' Cargo.toml` = 1. |
| `crates/famp/src/bus_client/mod.rs::connect_no_spawn` | additive no-spawn constructor | ✓ VERIFIED | Single `UnixStream::connect` attempt, no `spawn_broker_if_absent` call, no retry loop — fails loud on `Io` error. Existing `connect()` unchanged (still calls `spawn_broker_if_absent`, 1 occurrence, retry loop intact). |
| `crates/famp-bus/src/broker/handle/tests.rs` | LIVE-01 pure-broker test | ✓ VERIFIED | Test present at line 2651 in module `d10_tests`, passes. |
| `crates/famp-gateway/tests/liveness.rs` | LIVE-02 subprocess test | ✓ VERIFIED | Present, passes, poll-with-deadline (no fixed sleep-then-assert), 8 `ChildGuard` occurrences. |
| `crates/famp-gateway/tests/no_cross_talk.rs` | GW-04 subprocess test | ✓ VERIFIED | Present, passes, 7 `ChildGuard` occurrences. |
| `crates/famp-gateway/tests/common/child_guard.rs` | ChildGuard RAII helper | ✓ VERIFIED | Present, copied from `crates/famp/tests/common/child_guard.rs` convention. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `ProxiedPrincipal::register` | broker `Register` frame | `pid: std::process::id()`, `listen: false` | ✓ WIRED | Confirmed by source read + passing LIVE-01/LIVE-02 tests. |
| `connect_no_spawn` | absent daemon | fail loud, no auto-spawn | ✓ WIRED | Confirmed: single connect attempt, `Io` error mapped to `GatewayError::BrokerUnreachable` in `principal.rs::map_bus_client_err`. |
| `GatewayRegistry` | per-principal isolation | `HashMap` keyed by name, one `ProxiedPrincipal`/connection per key | ✓ WIRED | Confirmed by source + passing GW-04 subprocess test. |
| famp-bus source | Design A zero-change | broker `register()`/`tick()` unmodified | ✓ WIRED | `git diff --name-only 8cd5ec9..HEAD -- crates/famp-bus/src/` returns only `broker/handle/tests.rs` — zero production-code change. |

### Behavioral Spot-Checks / Test Execution

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| LIVE-01 pure-broker proof | `cargo test -p famp-bus --lib live01_shared_pid` | 1 passed | ✓ PASS |
| LIVE-02 subprocess proof | `cargo test -p famp-gateway --test liveness live02_gateway_exit_reaps_all_principals -- --nocapture` | 1 passed | ✓ PASS |
| GW-04 subprocess proof | `cargo test -p famp-gateway --test no_cross_talk gw04_no_cross_talk_between_proxied_principals -- --nocapture` | 1 passed | ✓ PASS |
| No regression in bus_client | `cargo test -p famp --lib bus_client` | 13 passed | ✓ PASS |
| Gateway crate builds as lib+bin | `cargo build -p famp-gateway --bins` | exit 0 | ✓ PASS |
| Workspace lint | `just lint` (`cargo clippy --workspace --all-targets -- -D warnings`) | exit 0 | ✓ PASS |
| Design A zero famp-bus change | `git diff --name-only 8cd5ec9..HEAD -- crates/famp-bus/src/` | only `handle/tests.rs` | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|--------------|------------|-------------|--------|----------|
| LIVE-01 | 07-01, 07-02 | Proxied principal stays live via gateway PID, no pid-uniqueness constraint | ✓ SATISFIED | Source (`std::process::id()` in Register) + passing pure-broker test. |
| LIVE-02 | 07-01, 07-03 | Gateway exit reaps all proxied principals cleanly, no orphans | ✓ SATISFIED | Passing real-subprocess SIGKILL/reap test. |
| GW-04 | 07-01, 07-03 | Single gateway backs multiple principals, no cross-talk | ✓ SATISFIED | Passing real-subprocess isolation test + `GatewayRegistry` per-name demux. |

REQUIREMENTS.md marks all three (`[x] GW-04`, `[x] LIVE-01`, `[x] LIVE-02`) as Complete for Phase 7 — consistent with codebase evidence. No orphaned requirements found for this phase.

### Anti-Patterns Found

None. Scanned all phase-modified/created files (`crates/famp-gateway/src/*.rs`, `crates/famp-gateway/tests/*.rs`, `crates/famp/src/bus_client/mod.rs`, `crates/famp-bus/src/broker/handle/tests.rs`) for `TBD`/`FIXME`/`XXX`/`TODO`/`HACK`/`PLACEHOLDER`/"not yet implemented" — zero matches.

### Human Verification Required

None. All must-haves are mechanically verified via source inspection and passing automated tests (pure-unit + real-subprocess).

### Gaps Summary

No gaps. All three requirement IDs (LIVE-01, LIVE-02, GW-04) have both source-level evidence and passing tests that were independently re-run during this verification (not merely trusted from SUMMARY.md). Design A's zero-famp-bus-change constraint is confirmed by diffing against the pre-phase commit (`8cd5ec9`) — the only file touched under `crates/famp-bus/src/` is the added test file `broker/handle/tests.rs`.

**Unrelated observation (not a phase-7 gap):** the pre-existing `cli::install::codex`/`cli::uninstall::codex` test-staleness issue noted in 07-01-SUMMARY.md as caused by concurrent unrelated session work was independently re-run during this verification (`cargo test -p famp --lib cli::install::codex`) and now passes (6/6) — the `target/debug/famp` staleness has since resolved itself via intervening rebuilds. Confirmed out of scope for phase 7 either way.

---

_Verified: 2026-07-23_
_Verifier: Claude (gsd-verifier)_

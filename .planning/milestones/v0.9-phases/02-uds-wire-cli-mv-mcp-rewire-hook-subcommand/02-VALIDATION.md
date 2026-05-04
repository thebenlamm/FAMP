---
phase: 02
slug: uds-wire-cli-mv-mcp-rewire-hook-subcommand
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-28
---

# Phase 02 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Sourced from `02-RESEARCH.md` §9 "Validation Architecture".

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo-nextest 0.9.132` + `tokio::test` (in-process for time-forward) + `assert_cmd 2.0` (shelled CLI) |
| **Config file** | `Justfile` recipes (`just test`, `just ci`); `.config/nextest.toml` if present |
| **Quick run command** | `cargo nextest run -p famp -p famp-bus` |
| **Full suite command** | `just ci` |
| **Estimated runtime** | quick: ~30s · full: ~3–5 min (includes fmt, clippy, build, nextest, audit) |

---

## Sampling Rate

- **After every task commit:** Run `cargo nextest run -p famp -p famp-bus` (the affected crates only)
- **After every plan wave:** Run `just ci` (full green required)
- **Before `/gsd-verify-work`:** `just ci` must be green at HEAD
- **Max feedback latency:** ~30s on the per-task path; ~5min full

---

## Per-Task Verification Map

> Plan IDs are TBD until `/gsd-plan-phase` produces PLAN.md files. The planner is
> required to populate concrete `Task ID` and `Plan` columns per task; the
> Requirement / Test Type / Automated Command columns below are pre-locked from
> RESEARCH §9 and MUST be the source of truth for each task's `<automated>` block.

| Requirement | Behavior | Test Type | Automated Command | Test File (Wave 0) | Status |
|-------------|----------|-----------|-------------------|--------------------|--------|
| BROKER-01 | Broker accepts UDS connections | Integration | `cargo nextest run -p famp test_broker_accepts_connection` | `crates/famp/tests/broker_lifecycle.rs` ❌ W0 | ⬜ pending |
| BROKER-02 | Broker survives Ctrl-C on terminal | **Manual** (macOS Terminal.app) | — | — | ⬜ pending |
| BROKER-03 | Single-broker exclusion across spawn race | Integration `assert_cmd` | `cargo nextest run -p famp test_broker_spawn_race` | `crates/famp/tests/broker_spawn_race.rs` ❌ W0 | ⬜ pending |
| BROKER-04 | Idle exit at 5-min (fast-forward) | Integration time-forward | `cargo nextest run -p famp test_broker_idle_exit` | `crates/famp/tests/broker_lifecycle.rs` ❌ W0 | ⬜ pending |
| BROKER-05 | NFS warning fires once at startup | Unit (mock path) | `cargo nextest run -p famp test_nfs_warning` | `crates/famp/src/cli/broker/nfs_check.rs` (`#[cfg(test)]`) ❌ W0 | ⬜ pending |
| CLI-01 | `famp register` blocks until Ctrl-C | Integration `assert_cmd` + kill | `cargo nextest run -p famp test_register_blocks` | `crates/famp/tests/cli_dm_roundtrip.rs` ❌ W0 | ⬜ pending |
| CLI-02 | `famp send` DM delivery | Integration `assert_cmd` | `cargo nextest run -p famp test_dm_roundtrip` | `crates/famp/tests/cli_dm_roundtrip.rs` ❌ W0 | ⬜ pending |
| CLI-03 | `famp inbox list` shows DM | Integration `assert_cmd` | `cargo nextest run -p famp test_inbox_list` | `crates/famp/tests/cli_dm_roundtrip.rs` ❌ W0 | ⬜ pending |
| CLI-04 | `famp inbox ack` advances cursor | Integration `assert_cmd` | `cargo nextest run -p famp test_inbox_ack_cursor` | `crates/famp/tests/cli_inbox.rs` ❌ W0 | ⬜ pending |
| CLI-05 | `famp await` unblocks on Send | Integration `assert_cmd` | `cargo nextest run -p famp test_await_unblocks` | `crates/famp/tests/cli_dm_roundtrip.rs` ❌ W0 | ⬜ pending |
| CLI-06 | `famp join` / `famp leave` channel membership | Integration `assert_cmd` | `cargo nextest run -p famp test_channel_fanout` | `crates/famp/tests/cli_channel_fanout.rs` ❌ W0 | ⬜ pending |
| CLI-07 | `famp sessions` lists active | Integration `assert_cmd` | `cargo nextest run -p famp test_sessions_list` | `crates/famp/tests/cli_sessions.rs` ❌ W0 | ⬜ pending |
| CLI-08 | `famp whoami` returns identity | Integration `assert_cmd` | `cargo nextest run -p famp test_whoami` | `crates/famp/tests/cli_dm_roundtrip.rs` ❌ W0 | ⬜ pending |
| CLI-09 | Mailbox file created on disk | Integration | covered by `test_dm_roundtrip` (file-presence assert) | reuse | ⬜ pending |
| CLI-10 | Cursor advanced atomically (temp + rename) | Integration + proptest carry-forward | reuse Phase 1 TDD-02 + new wire-layer assertion in `test_inbox_ack_cursor` | reuse | ⬜ pending |
| CLI-11 | `sessions.jsonl` is diagnostic-only (broker reads ignore it) | Integration | `cargo nextest run -p famp test_sessions_jsonl_diagnostic_only` | `crates/famp/tests/broker_lifecycle.rs` ❌ W0 | ⬜ pending |
| MCP-01 | MCP/bus/broker source paths do not import reqwest/rustls (D-11 source-import grep — Phase 2 audit; cargo-tree-strict reading deferred to Phase 4 when federation CLI is deleted) | Static (source grep) | `bash scripts/check-mcp-deps.sh` | `scripts/check-mcp-deps.sh` (new) ❌ W0 | ⬜ pending |
| MCP-02..09 | Each of 8 MCP tools round-trips through bus | E2E harness | `cargo nextest run -p famp test_mcp_bus_e2e` | `crates/famp/tests/mcp_bus_e2e.rs` ❌ W0 | ⬜ pending |
| MCP-10 | Exhaustive `match BusErrorKind` is compile-checked | Compile-time | `cargo build -p famp` (fails on missing arm because `#![deny(unreachable_patterns)]` consumer stub) | `crates/famp/src/cli/mcp/error_kind.rs` (existing pattern, retargeted) | ⬜ pending |
| HOOK-01 | `famp-local hook add --on Edit:<glob> --to <peer-or-#channel>` writes row to `~/.famp-local/hooks.tsv` | Shell integration | `cargo nextest run -p famp test_hook_add` (shells `scripts/famp-local`) | `crates/famp/tests/hook_subcommand.rs` ❌ W0 | ⬜ pending |
| HOOK-02 | `~/.famp-local/hooks.tsv` row format `<id>\t<event>:<glob>\t<to>\t<added_at>` | Shell integration | covered by `test_hook_add` (TSV-format assert) | reuse | ⬜ pending |
| HOOK-03 | `famp-local hook list` reads back rows | Shell integration | `cargo nextest run -p famp test_hook_list` | `crates/famp/tests/hook_subcommand.rs` ❌ W0 | ⬜ pending |
| HOOK-04a | Hook **registration** persists round-trip via add→list→remove (D-12 split — execution runner HOOK-04b deferred to Phase 3 per ROADMAP SC-5) | Shell integration | `cargo nextest run -p famp test_hook_remove` | reuse | ⬜ pending |
| TEST-01 | 2-client DM round-trip | Integration `assert_cmd` | `cargo nextest run -p famp test_dm_roundtrip` | `crates/famp/tests/cli_dm_roundtrip.rs` ❌ W0 | ⬜ pending |
| TEST-02 | 3-client channel fan-out | Integration `assert_cmd` | `cargo nextest run -p famp test_channel_fanout` | `crates/famp/tests/cli_channel_fanout.rs` ❌ W0 | ⬜ pending |
| TEST-03 | `kill -9` mid-Send → reconnect recovers mailbox | Integration `assert_cmd` | `cargo nextest run -p famp test_kill9_recovery` | `crates/famp/tests/broker_crash_recovery.rs` ❌ W0 | ⬜ pending |
| TEST-04 | Two near-simultaneous `famp register` → exactly one broker | Integration `assert_cmd` | `cargo nextest run -p famp test_broker_spawn_race` | `crates/famp/tests/broker_spawn_race.rs` ❌ W0 | ⬜ pending |
| TEST-05 | Two stdio MCP processes round-trip with `$FAMP_BUS_SOCKET` isolation | E2E harness | `cargo nextest run -p famp test_mcp_bus_e2e` | `crates/famp/tests/mcp_bus_e2e.rs` ❌ W0 | ⬜ pending |
| CARRY-02 | `REQUIREMENTS.md` INBOX-01 wording matches the as-shipped wire shape (typed `Vec<serde_json::Value>` envelopes on the wire, raw application bytes per line on disk, `AnyBusEnvelope::decode` validation between disk and wire — Phase-1 D-09 evolved shape) | Doc/CI grep | `grep -F "Vec<serde_json::Value>" .planning/REQUIREMENTS.md` returns matching line; `grep -F "InboxLine" .planning/REQUIREMENTS.md` empty | — (REQUIREMENTS.md edit) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Test infrastructure that MUST exist (created by an early task in the plan)
before any implementation task can verify against it.

- [ ] `crates/famp/Cargo.toml` [dev-dependencies] adds `assert_cmd = "2.0"`
- [ ] `crates/famp/Cargo.toml` [dev-dependencies] tokio gains `test-util` feature
- [ ] `crates/famp-bus/Cargo.toml` [dev-dependencies] tokio gains `test-util` feature
- [ ] `crates/famp/Cargo.toml` [dependencies] adds `nix = { version = "0.31", features = ["process", "fs"] }`
- [ ] `crates/famp/tests/broker_lifecycle.rs` — stubs for BROKER-01, BROKER-04, CLI-11
- [ ] `crates/famp/tests/broker_spawn_race.rs` — stub for BROKER-03 / TEST-04
- [ ] `crates/famp/tests/broker_crash_recovery.rs` — stub for TEST-03
- [ ] `crates/famp/tests/cli_dm_roundtrip.rs` — stubs for CLI-01/02/03/05/08, TEST-01
- [ ] `crates/famp/tests/cli_channel_fanout.rs` — stubs for CLI-06, TEST-02
- [ ] `crates/famp/tests/cli_inbox.rs` — stubs for CLI-04, CLI-10
- [ ] `crates/famp/tests/cli_sessions.rs` — stubs for CLI-07
- [ ] `crates/famp/tests/mcp_bus_e2e.rs` — stubs for MCP-02..09, TEST-05 (model after `mcp_stdio_tool_calls.rs`)
- [ ] `crates/famp/tests/hook_subcommand.rs` — stubs for HOOK-01..03 + HOOK-04a (shells `scripts/famp-local hook`)
- [ ] `scripts/check-mcp-deps.sh` — D-11 source-import grep audit asserting `crates/famp/src/cli/mcp/`, `crates/famp/src/bus_client/`, `crates/famp/src/broker/` contain no `use reqwest` or `use rustls` imports (MCP-01)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Broker survives terminal Ctrl-C (SIGINT received by foreground process group, not the daemon) | BROKER-02 | Validates `setsid` daemonization. `assert_cmd` cannot send a real terminal SIGINT to a `pre_exec(setsid)` child without a pty harness. | (1) `famp register alice` in iTerm/Terminal.app, (2) `Ctrl-C`, (3) `pgrep -f 'famp broker'` still returns the broker pid for ≤5min, (4) re-running `famp register alice` reconnects without spawning a second broker. |
| `~/.famp/` on NFS at startup → one-line warning to stderr | BROKER-05 (manual augment) | The unit test mocks the magic number. A real NFS mount audit can only happen in a deployed env. | (1) `mkdir /tmp/nfs-mount && mount -t nfs ...`, (2) `FAMP_BUS_SOCKET=/tmp/nfs-mount/bus.sock famp register alice`, (3) stderr line `warning: ~/.famp/ is on NFS — file locking semantics may differ` appears exactly once. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all `❌ W0` references in the verification map
- [ ] No `--watch` / `--listen` mode flags in any test command
- [ ] Feedback latency < 30s on quick-run path
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

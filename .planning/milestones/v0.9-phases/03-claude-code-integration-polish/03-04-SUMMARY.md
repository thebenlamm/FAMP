---
phase: 03-claude-code-integration-polish
plan: 04
subsystem: claude-code-uninstall-roundtrip
tags: [claude-code, uninstall, snapshots, insta, roundtrip]

requires: [03-01, 03-02, 03-03]
provides:
  - uninstall-claude-code CLI subcommand
  - D-04 symmetric inverse of install-claude-code
  - install-to-uninstall roundtrip snapshot gate
affects: [phase-03, CC-01, D-04, plan-03-05]

tech-stack:
  added:
    - insta 1.47.2 json feature as famp dev-dependency
  patterns:
    - uninstall orchestrator reuses install-owned remove helpers
    - insta JSON snapshots for canonical pre-state review

key-files:
  created:
    - .planning/phases/03-claude-code-integration-polish/03-04-SUMMARY.md
    - crates/famp/src/cli/uninstall/mod.rs
    - crates/famp/src/cli/uninstall/claude_code.rs
    - crates/famp/src/cli/uninstall/codex.rs
    - crates/famp/tests/install_uninstall_roundtrip.rs
    - crates/famp/tests/snapshots/install_uninstall_roundtrip__claude_json_pre_state.snap
    - crates/famp/tests/snapshots/install_uninstall_roundtrip__settings_json_pre_state.snap
  modified:
    - Cargo.lock
    - crates/famp/Cargo.toml
    - crates/famp/src/cli/mod.rs
    - crates/famp/src/lib.rs
    - crates/famp/src/bin/famp.rs
    - crates/famp/examples/_gen_fixture_certs.rs
    - crates/famp/examples/cross_machine_two_agents.rs
    - crates/famp/examples/personal_two_agents.rs

key-decisions:
  - "Used exact shim-path equality for uninstall Stop-hook removal, matching the plan's sentinel rule."
  - "Kept uninstall-codex as an intentional stub for plan 03-05 and did not wire Codex CLI variants."
  - "Did not update STATE.md or ROADMAP.md because the orchestrator owns shared tracking for the wave."

requirements-completed: [CC-01]

duration: ~15min
completed: 2026-05-03T18:38:53Z
---

# Phase 03 Plan 04: Claude Code Uninstall Roundtrip Summary

**`famp uninstall-claude-code` now reverses the Claude Code install surface, and the install-to-uninstall path is guarded by checked-in JSON snapshots.**

## Performance

- **Started:** 2026-05-03T18:24:15Z
- **Completed:** 2026-05-03T18:38:53Z
- **Tasks:** 2
- **Files changed:** 15 including this summary

## Accomplishments

- Added `cli::uninstall` with a real `claude_code` handler and an intentional `codex` stub for plan 03-05.
- Wired exactly one `Commands::UninstallClaudeCode` variant and one dispatch arm in `crates/famp/src/cli/mod.rs`.
- Implemented `uninstall::claude_code::run_at(home, out, err)` as the inverse of install:
  - removes `mcpServers.famp` from `~/.claude.json`
  - removes the seven `famp-*.md` slash commands
  - removes `~/.famp/hook-runner.sh`
  - filters only the matching famp Stop hook from `~/.claude/settings.json`
- Added 5 unit tests for no-op behavior, preservation of unrelated JSON keys, surgical Stop-hook removal, and empty Stop cleanup.
- Added 3 integration tests for install-to-uninstall roundtrip, empty-home cleanup, and double-uninstall idempotency.
- Added checked-in `insta::assert_json_snapshot!` baselines for realistic `.claude.json` and `.claude/settings.json` pre-state.

## Task Commits

1. **Task 1: Create uninstall module + Claude Code orchestrator + Codex stub** - `48bb39d` (`feat`)
2. **Task 2: Ship install-to-uninstall roundtrip integration test** - `d406048` (`test`)

## Verification

- `cargo build --workspace --all-targets` - passed. Existing `temp_env` unused-crate warnings in examples remain.
- `cargo nextest run -p famp uninstall::claude_code --no-fail-fast` - passed: 5 tests.
- `cargo nextest run -p famp --test install_uninstall_roundtrip` - passed: 3 tests with `INSTA_UPDATE` unset.
- `cargo run -p famp -- uninstall-claude-code --help` - printed the expected "Uninstall Claude Code integration" description.
- CLI wiring audit:
  - `pub mod uninstall;` present.
  - `UninstallClaudeCode` appears twice in `cli/mod.rs`: one enum variant and one dispatch arm.
  - `InstallCodex|UninstallCodex` appears zero times in `cli/mod.rs`.

`cargo clippy --workspace --all-targets -- -D warnings` remains blocked by the pre-existing `famp-bus` findings documented in plans 03-02 and 03-03:

- `crates/famp-bus/src/broker/handle.rs:384` - `clippy::items_after_statements`
- `crates/famp-bus/src/broker/mod.rs:44` - `clippy::doc_markdown`

## Snapshot Gate

The roundtrip test asserts canonical JSON `Value` snapshots, not raw string formatting. This preserves the D-04 state-restoration invariant while avoiding false failures from harmless pretty-printer whitespace changes. Silent schema or key/value drift fails the default no-update `insta` gate.

Snapshot files committed:

- `crates/famp/tests/snapshots/install_uninstall_roundtrip__claude_json_pre_state.snap`
- `crates/famp/tests/snapshots/install_uninstall_roundtrip__settings_json_pre_state.snap`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Silenced new `insta` unused-crate warnings introduced by Task 2**
- **Found during:** Plan-level `cargo build --workspace --all-targets`
- **Issue:** Adding `insta` as a dev-dependency caused new unused-crate-dependency warnings in the famp lib test unit, bin test unit, and examples.
- **Fix:** Added `use insta as _;` to the existing dependency-silencer blocks.
- **Files modified:** `crates/famp/src/lib.rs`, `crates/famp/src/bin/famp.rs`, `crates/famp/examples/_gen_fixture_certs.rs`, `crates/famp/examples/cross_machine_two_agents.rs`, `crates/famp/examples/personal_two_agents.rs`
- **Commit:** `d406048`

## Known Stubs

- `crates/famp/src/cli/uninstall/codex.rs` - intentional plan stub; real `uninstall-codex` lands in plan 03-05.

## Threat Flags

None beyond the plan-registered uninstall trust boundary. This plan removes only planned user-scope files/config entries and adds no unplanned network endpoint, auth path, or schema boundary.

## State Tracking

Per user instruction, this executor did not update `.planning/STATE.md` or `.planning/ROADMAP.md`; the phase orchestrator owns shared tracking after the wave completes.

## Self-Check: PASSED

- Found `.planning/phases/03-claude-code-integration-polish/03-04-SUMMARY.md`.
- Found `crates/famp/src/cli/uninstall/mod.rs`.
- Found `crates/famp/src/cli/uninstall/claude_code.rs`.
- Found `crates/famp/src/cli/uninstall/codex.rs`.
- Found `crates/famp/tests/install_uninstall_roundtrip.rs`.
- Found `crates/famp/tests/snapshots/install_uninstall_roundtrip__claude_json_pre_state.snap`.
- Found `crates/famp/tests/snapshots/install_uninstall_roundtrip__settings_json_pre_state.snap`.
- Found task commit `48bb39d`.
- Found task commit `d406048`.

---
*Phase: 03-claude-code-integration-polish*
*Completed: 2026-05-03*

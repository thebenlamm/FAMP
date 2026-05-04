---
phase: 03-claude-code-integration-polish
plan: 05
subsystem: codex-integration
tags: [codex, install, uninstall, toml, mcp]

requires: [03-01, 03-02, 03-04]
provides:
  - install-codex CLI subcommand
  - uninstall-codex CLI subcommand
  - atomic TOML structural merge helper for `~/.codex/config.toml`
  - Codex install/uninstall tempdir-home integration coverage
affects: [phase-03, CC-01, D-12]

tech-stack:
  added: []
  patterns:
    - `toml::Table` structural merge with same-directory `NamedTempFile` persist
    - Codex integration is MCP-only with no slash-command or hook artifacts
    - `run_at(home, out, err)` handlers for tempdir-safe integration tests

key-files:
  created:
    - .planning/phases/03-claude-code-integration-polish/03-05-SUMMARY.md
    - crates/famp/tests/install_codex.rs
    - crates/famp/tests/codex_install_uninstall_roundtrip.rs
  modified:
    - crates/famp/src/cli/install/toml_merge.rs
    - crates/famp/src/cli/install/codex.rs
    - crates/famp/src/cli/uninstall/codex.rs
    - crates/famp/src/cli/mod.rs
    - crates/famp/src/cli/error.rs
    - crates/famp/src/cli/mcp/error_kind.rs

key-decisions:
  - "Added `CliError::TomlTableExpected` instead of fabricating a `toml::de::Error` for non-table parent keys."
  - "Kept Codex integration strictly MCP-only per D-12: no `.claude/`, no hook runner, no slash commands."
  - "Did not update STATE.md or ROADMAP.md because the orchestrator owns shared tracking after the wave."

requirements-completed: [CC-01]

duration: ~45min plus checkpoint wait
completed: 2026-05-03T18:51:43Z
---

# Phase 03 Plan 05: Codex Install/Uninstall Summary

**Codex parity is implemented as an MCP-only install/uninstall pair that edits `~/.codex/config.toml` structurally and preserves unrelated TOML state.**

## Performance

- **Completed:** 2026-05-03T18:51:43Z
- **Tasks:** 3
- **Files changed:** 9 including this summary

## Accomplishments

- Replaced the plan-02 `install/codex.rs` stub with `install::codex::run_at(home, out, err)`.
- Replaced the plan-04 `uninstall/codex.rs` stub with `uninstall::codex::run_at(home, out, err)`.
- Implemented `toml_merge::upsert_codex_table` and `toml_merge::remove_codex_table` for `[mcp_servers.famp]`.
- Wired `InstallCodex` and `UninstallCodex` into `crates/famp/src/cli/mod.rs`.
- Added integration coverage for install shape, MCP-only invariants, and install-to-uninstall semantic TOML equality.
- Completed the plan-defined manual sandbox UAT checkpoint after user approval.

## Task Commits

1. **Task 1: Implement toml_merge helper + install-codex + uninstall-codex orchestrators** - `7074f42` (`feat`)
2. **Task 2: Ship install_codex + codex_install_uninstall_roundtrip integration tests** - `31a7247` (`test`)
3. **Task 3: Manual UAT checkpoint** - approved by user; no code commit required before this summary

## Final Shapes

- `install::codex::run_at` resolves `home/.codex/config.toml`, chooses `which("famp")` with `~/.cargo/bin/famp` fallback, and upserts:

```toml
[mcp_servers.famp]
command = "/path/to/famp"
args = ["mcp"]
startup_timeout_sec = 10
```

- `uninstall::codex::run_at` removes only `[mcp_servers.famp]`, preserving other Codex sections.
- `toml_merge::upsert_codex_table` reads existing TOML into `toml::Table`, creates the parent table if absent, no-ops on `AlreadyMatches`, backs up pre-state to `.bak.<unix-ts>`, and persists with a same-directory tempfile.
- `toml_merge::remove_codex_table` returns `NotPresent` on absent files/keys and removes the empty parent table after deleting the last child.

## Test Inventory

- `toml_merge` unit tests: 5 passed.
- `install::codex` unit tests: 3 passed.
- `uninstall::codex` unit tests: 2 passed.
- `crates/famp/tests/install_codex.rs`: 2 passed.
- `crates/famp/tests/codex_install_uninstall_roundtrip.rs`: 2 passed.

Total new/changed Codex test coverage: 14 tests passed.

## Verification

- `cargo build --workspace --all-targets` - passed. Existing `temp_env` unused-crate warnings in examples remain.
- `cargo nextest run -p famp install::toml_merge install::codex uninstall::codex --no-fail-fast` - passed: 10 tests.
- `cargo nextest run -p famp --test install_codex --test codex_install_uninstall_roundtrip --no-fail-fast` - passed: 4 tests.
- `cargo run -p famp -- install-codex --help` - passed and showed the Codex install description.
- `cargo run -p famp -- uninstall-codex --help` - passed and showed the Codex uninstall description.
- `grep -cE '^    (Install|Uninstall)(ClaudeCode|Codex)\(' crates/famp/src/cli/mod.rs` - `4`.
- `rg -n 'NotImplemented' crates/famp/src/cli/install/codex.rs crates/famp/src/cli/install/toml_merge.rs crates/famp/src/cli/uninstall/codex.rs` - no matches.

`cargo clippy --workspace --all-targets -- -D warnings` remains blocked by the pre-existing `famp-bus` findings documented in 03-02, 03-03, and 03-04:

- `crates/famp-bus/src/broker/handle.rs:384` - `clippy::items_after_statements`
- `crates/famp-bus/src/broker/mod.rs:44` - `clippy::doc_markdown`

## Manual UAT

User approved the sandbox Codex UAT on 2026-05-03:

- `install-codex` created `.codex/config.toml`.
- The file contained `[mcp_servers.famp]`.
- `args = ["mcp"]`.
- `command = "/Users/benlamm/.cargo/bin/famp"`.
- `startup_timeout_sec = 10`.
- No `.claude/` directory was created.
- No `.famp/hook-runner.sh` existed.
- `uninstall-codex` removed `[mcp_servers.famp]`.
- Remaining table count after uninstall was `0`.

Optional real Codex CLI UAT was intentionally skipped for this checkpoint.

## CLI Surface Audit

`crates/famp/src/cli/mod.rs` now has exactly four install/uninstall variants:

- `InstallClaudeCode`
- `UninstallClaudeCode`
- `InstallCodex`
- `UninstallCodex`

It also has four matching dispatch arms.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] Added typed TOML table-shape error**
- **Found during:** Task 1
- **Issue:** The plan suggested using `toml::de::Error::custom` for a parent key that exists but is not a table. That constructor is not a good fit for a semantic table-shape error in the pinned API surface.
- **Fix:** Added `CliError::TomlTableExpected { path }` and an MCP error-kind arm, preserving the existing exhaustive error classification invariant.
- **Files modified:** `crates/famp/src/cli/error.rs`, `crates/famp/src/cli/mcp/error_kind.rs`, `crates/famp/src/cli/install/toml_merge.rs`
- **Commit:** `7074f42`

## Known Stubs

None. Stub scan found one fixture line, `args = []`, in `crates/famp/tests/install_codex.rs`; it is part of a realistic pre-existing Codex MCP config fixture and does not flow to UI/runtime rendering.

## Threat Flags

None beyond the plan-registered trust boundary. This plan edits a planned user-owned TOML config file and introduces no unplanned network endpoint, auth path, or file access surface.

## State Tracking

Per user instruction, this executor did not update `.planning/STATE.md` or `.planning/ROADMAP.md`; the phase orchestrator owns shared tracking after the wave completes.

## Self-Check: PASSED

- Found `.planning/phases/03-claude-code-integration-polish/03-05-SUMMARY.md`.
- Found `crates/famp/src/cli/install/toml_merge.rs`.
- Found `crates/famp/src/cli/install/codex.rs`.
- Found `crates/famp/src/cli/uninstall/codex.rs`.
- Found `crates/famp/tests/install_codex.rs`.
- Found `crates/famp/tests/codex_install_uninstall_roundtrip.rs`.
- Found task commit `7074f42`.
- Found task commit `31a7247`.

---
*Phase: 03-claude-code-integration-polish*
*Completed: 2026-05-03*

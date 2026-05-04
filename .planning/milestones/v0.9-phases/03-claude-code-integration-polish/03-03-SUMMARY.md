---
phase: 03-claude-code-integration-polish
plan: 03
subsystem: claude-code-install-orchestrator
tags: [claude-code, install, hooks, slash-commands, integration-tests]

requires: [03-01, 03-02]
provides:
  - install-claude-code orchestrator writing Claude MCP config, slash commands, Stop hook, and hook runner
  - CC-01 tempdir-home integration coverage for all install file-write surfaces
  - HOOK-04b bash-shim dispatch and failure-mode integration coverage
affects: [phase-03, CC-01, HOOK-04b, install-claude-code, plan-03-04, plan-03-06]

tech-stack:
  added:
    - which 7.0.3
    - clap env feature
  patterns:
    - run/run_at install entry pair for tempdir-safe integration tests
    - Sentinel-based hooks.Stop array replacement before structural JSON upsert
    - Bash-shim integration tests using a fake PATH-resolved famp binary

key-files:
  created:
    - .planning/phases/03-claude-code-integration-polish/03-03-SUMMARY.md
    - crates/famp/tests/install_claude_code.rs
    - crates/famp/tests/hook_runner_dispatch.rs
    - crates/famp/tests/hook_runner_failure_modes.rs
  modified:
    - Cargo.lock
    - crates/famp/Cargo.toml
    - crates/famp/src/cli/install/claude_code.rs
    - crates/famp/src/cli/mod.rs
    - crates/famp/src/bin/famp.rs
    - crates/famp/examples/_gen_fixture_certs.rs
    - crates/famp/examples/cross_machine_two_agents.rs
    - crates/famp/examples/personal_two_agents.rs

key-decisions:
  - "Enabled clap's env feature to support the planned hidden FAMP_INSTALL_TARGET_HOME override."
  - "Added which = \"7\" for install-time famp binary discovery, with a ~/.cargo/bin/famp fallback."
  - "Stopped at the plan-defined manual checkpoint and only committed SUMMARY.md after user approval."

patterns-established:
  - "Install handlers expose pub fn run_at(home: &Path, out: &mut dyn Write, err: &mut dyn Write) for tempdir-safe tests."
  - "Hook-runner tests execute the real asset under bash via env!(\"CARGO_MANIFEST_DIR\"), not a copied script."

requirements-completed: [CC-01, HOOK-04b]

duration: ~4h26m wall-clock including checkpoint wait
completed: 2026-05-03
---

# Phase 03 Plan 03: Claude Code Install Orchestrator Summary

**`famp install-claude-code` now performs the full Claude Code install surface and has automated coverage for tempdir-safe file writes plus hook-runner dispatch/failure behavior.**

## Performance

- **Started:** 2026-05-03T03:53:34Z
- **Completed:** 2026-05-03T18:19:37Z
- **Tasks:** 4
- **Files changed:** 12 including this summary

## Accomplishments

- Replaced the plan-02 `install-claude-code` stub with `pub fn run_at(home: &Path, out: &mut dyn Write, err: &mut dyn Write) -> Result<(), CliError>`.
- Wired exactly one new CLI variant and one dispatch arm: `InstallClaudeCode` and `Commands::InstallClaudeCode(args) => install::claude_code::run(args)`.
- Implemented the four install mutations: `~/.claude.json` MCP upsert, 7 slash-command files, `~/.famp/hook-runner.sh` at 0755, and `~/.claude/settings.json` `hooks.Stop` array merge.
- Added 4 unit tests, 3 install integration tests, and 8 hook-runner integration tests.
- Completed the manual sandbox UAT checkpoint after user approval.

## Task Commits

Each implementation/test task was committed atomically:

1. **Task 1: Implement install-claude-code orchestrator + wire dispatch** - `4946ecf` (`feat`)
2. **Task 2: Ship integration test for install-claude-code** - `00aca4a` (`test`)
3. **Task 3: Ship hook-runner dispatch + failure-mode integration tests** - `d51b8e5` (`test`)

Task 4 was the human verification checkpoint; no code commit was needed for approval.

## Files Created/Modified

- `crates/famp/src/cli/install/claude_code.rs` - Real install orchestrator, hidden `--home` / `FAMP_INSTALL_TARGET_HOME` override, Stop-hook array merge, and 4 unit tests.
- `crates/famp/src/cli/mod.rs` - Added `InstallClaudeCode` variant and dispatch arm only.
- `crates/famp/Cargo.toml` - Added `which = "7"` and enabled `clap`'s `env` feature.
- `Cargo.lock` - Locked `which`, `env_home`, and `winsafe`.
- `crates/famp/src/bin/famp.rs` and 3 examples - Added `which as _` unused-dependency silencers required by the repo lint pattern.
- `crates/famp/tests/install_claude_code.rs` - CC-01 file-write surface integration tests.
- `crates/famp/tests/hook_runner_dispatch.rs` - HOOK-04b dispatch tests with a fake `famp` binary.
- `crates/famp/tests/hook_runner_failure_modes.rs` - HOOK-04b D-08 zero-exit failure-mode tests.

## Verification

- `cargo build --workspace --all-targets` - passed. Existing `temp_env` unused-dependency warnings in examples remain.
- `cargo nextest run -p famp install::claude_code --no-fail-fast` - passed: 4 tests.
- `cargo nextest run -p famp --test install_claude_code --no-fail-fast` - passed: 3 tests.
- `cargo nextest run -p famp --test hook_runner_dispatch --test hook_runner_failure_modes --no-fail-fast` - passed: 8 tests.
- `cargo run -p famp -- install-claude-code --help` - printed the expected "Install Claude Code integration" description.
- CLI wiring checks:
  - `InstallClaudeCode(install::claude_code::InstallClaudeCodeArgs)` count: 1
  - `Commands::InstallClaudeCode(args) => install::claude_code::run(args)` count: 1
  - `UninstallClaudeCode|InstallCodex|UninstallCodex` count in `cli/mod.rs`: 0

## Manual UAT

Sandbox path used by executor:

`/var/folders/p2/0kz3xzgx1xg2mv5z3qh0sw5r0000gn/T/famp-uat-XXXX.WwXjA5dZG0`

Observed listing:

```text
.claude.json                                      mode 0644
.claude/settings.json                            mode 0644
.famp/hook-runner.sh                             mode 0755
.claude/commands/famp-channel.md                 mode 0644
.claude/commands/famp-inbox.md                   mode 0644
.claude/commands/famp-join.md                    mode 0644
.claude/commands/famp-leave.md                   mode 0644
.claude/commands/famp-register.md                mode 0644
.claude/commands/famp-send.md                    mode 0644
.claude/commands/famp-who.md                     mode 0644
```

User approval on 2026-05-03 confirmed:

- `.claude.json` contains `mcpServers.famp` with `type = "stdio"` and `args = ["mcp"]`.
- `.claude/commands` contains exactly 7 `.md` files.
- `settings.json` contains `hooks.Stop` with command ending `/.famp/hook-runner.sh`, `timeout = 30`, and no `matcher`.
- `hook-runner.sh` mode is `755`.
- `shellcheck` passed.
- Second run reported `mcpServers.famp -> AlreadyMatches` and `hooks.Stop -> AlreadyMatches`.
- No `*.bak.*` files were created by the idempotent second run.

## Decisions Made

- Followed the plan's `which::which("famp")` preference and added the dependency because it was not already present.
- Enabled `clap`'s `env` feature because the planned hidden `FAMP_INSTALL_TARGET_HOME` override otherwise cannot compile.
- Left the real Claude Code end-to-end Stop-hook fire UAT to plan 03-06, per this plan's revision note.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Enabled clap env feature for the hidden home override**
- **Found during:** Task 1
- **Issue:** Plan 03-02 documented that the crate did not expose clap's `env` arg attribute; plan 03-03 requires `#[arg(env = "FAMP_INSTALL_TARGET_HOME")]`.
- **Fix:** Enabled `clap = { version = "4.6", features = ["derive", "env"] }`.
- **Files modified:** `crates/famp/Cargo.toml`, `Cargo.lock`
- **Verification:** `cargo build --workspace --all-targets` and `cargo run -p famp -- install-claude-code --help` passed.
- **Committed in:** `4946ecf`

**2. [Rule 3 - Blocking] Added `which` unused-dependency silencers for bin/examples**
- **Found during:** Task 1 build
- **Issue:** Adding `which` to the umbrella crate triggered the existing `unused_crate_dependencies` lint in the bin and example compile units.
- **Fix:** Added `use which as _;` to the same silencer blocks already used for other umbrella dependencies.
- **Files modified:** `crates/famp/src/bin/famp.rs`, `crates/famp/examples/_gen_fixture_certs.rs`, `crates/famp/examples/cross_machine_two_agents.rs`, `crates/famp/examples/personal_two_agents.rs`
- **Verification:** `cargo build --workspace --all-targets` passed.
- **Committed in:** `4946ecf`

**Total deviations:** 2 auto-fixed Rule 3 blocking issues. Both were necessary to compile and verify the planned implementation.

## Issues Encountered

- Initial Cargo commands failed under the sandbox because the new dependency required crates.io index access. Rerunning `cargo build --workspace --all-targets` with approved network access resolved `which` and updated `Cargo.lock`.
- `cargo clippy --workspace --all-targets -- -D warnings` still fails on the two pre-existing `famp-bus` findings already documented in 03-02:
  - `crates/famp-bus/src/broker/handle.rs:384` - `clippy::items_after_statements`
  - `crates/famp-bus/src/broker/mod.rs:44` - `clippy::doc_markdown`
- A combined nextest selector command accidentally selected 0 tests; the unit and integration suites were rerun as separate commands and passed.

## Known Stubs

None. Stub scan found no plan-blocking placeholders in files created or modified by this plan.

## Threat Flags

None beyond the plan-registered trust boundaries. This plan implemented the planned user-scope file writes and bash-shim test surface; it did not introduce an unplanned network endpoint, auth path, or schema boundary.

## User Setup Required

None for this plan. Real Claude Code end-to-end Stop-hook fire and CC-09 wall-clock UAT remain mandatory in plan 03-06.

## Next Phase Readiness

Plan 03-04 can mirror the installed file layout for uninstall. Plan 03-06 can rely on `install-claude-code` being available for real Claude Code UAT.

## Self-Check: PASSED

- Found `.planning/phases/03-claude-code-integration-polish/03-03-SUMMARY.md`.
- Found task commit `4946ecf`.
- Found task commit `00aca4a`.
- Found task commit `d51b8e5`.

---
*Phase: 03-claude-code-integration-polish*
*Completed: 2026-05-03*

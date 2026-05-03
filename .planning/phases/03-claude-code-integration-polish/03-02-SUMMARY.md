---
phase: 03-claude-code-integration-polish
plan: 02
subsystem: claude-code-install-foundation
tags: [claude-code, install, slash-commands, hooks, json-merge]

requires: [03-01]
provides:
  - D-05 /famp-send planning amendment
  - D-09 settings.json hooks amendment
  - D-11 cargo install acceptance-gate amendment
  - install helper module skeleton
  - atomic JSON structural merge helper
  - seven Claude Code slash-command assets
  - HOOK-04b hook-runner bash shim asset
affects: [phase-03, CC-02, CC-03, CC-04, CC-05, CC-06, CC-07, CC-08, HOOK-04b]

tech-stack:
  added: []
  patterns:
    - serde_json::Value structural merge with same-directory NamedTempFile persist
    - include_str-backed static assets for Claude Code command templates and hook shim
    - hardcoded asset filenames to avoid path traversal

key-files:
  created:
    - .planning/phases/03-claude-code-integration-polish/03-02-SUMMARY.md
    - crates/famp/src/cli/install/mod.rs
    - crates/famp/src/cli/install/claude_code.rs
    - crates/famp/src/cli/install/codex.rs
    - crates/famp/src/cli/install/toml_merge.rs
    - crates/famp/src/cli/install/json_merge.rs
    - crates/famp/src/cli/install/slash_commands.rs
    - crates/famp/src/cli/install/hook_runner.rs
    - crates/famp/assets/hook-runner.sh
    - crates/famp/assets/slash_commands/famp-register.md
    - crates/famp/assets/slash_commands/famp-send.md
    - crates/famp/assets/slash_commands/famp-channel.md
    - crates/famp/assets/slash_commands/famp-join.md
    - crates/famp/assets/slash_commands/famp-leave.md
    - crates/famp/assets/slash_commands/famp-who.md
    - crates/famp/assets/slash_commands/famp-inbox.md
  modified:
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md
    - .planning/phases/03-claude-code-integration-polish/03-CONTEXT.md
    - Justfile
    - crates/famp/src/cli/error.rs
    - crates/famp/src/cli/mcp/error_kind.rs
    - crates/famp/src/cli/mod.rs

key-decisions:
  - "ROADMAP.md was amended only for required D-05/D-09/D-11 wording, not for wave progress tracking."
  - "The clap env attribute was omitted from stub args because this crate's clap configuration does not enable the env attribute method."
  - "New CliError variants were added to the exhaustive MCP error-kind classifier to preserve the no-wildcard invariant."

requirements-completed: [CC-02, CC-03, CC-04, CC-05, CC-06, CC-07, CC-08, HOOK-04b]

duration: ~46min
completed: 2026-05-03
---

# Phase 03 Plan 02: Claude Code Install Foundation Summary

**Claude Code install foundation is in place: planning amendments are landed, install helper modules compile, JSON merge is tested, and the slash-command plus hook-runner assets are embedded and shellcheck-clean.**

## Performance

- **Started:** 2026-05-03T03:40:52Z
- **Completed:** 2026-05-03
- **Tasks:** 3
- **Files changed:** 23 including this summary

## Accomplishments

- Landed the D-05 rename from `/famp-msg` to `/famp-send` in `.planning/REQUIREMENTS.md` and `.planning/ROADMAP.md`.
- Landed the D-11 install-gate amendment from `brew install famp` to `cargo install famp` with the second-window timing clarification.
- Added the D-09 amendment block in `03-CONTEXT.md`, preserving the original `hooks.json` wording while correcting the target to `~/.claude/settings.json`.
- Added `cli::install` with stub install handlers for Claude Code and Codex, plus a real `json_merge` helper.
- Added 7 Claude Code slash-command markdown assets and writer/remover helpers.
- Added `crates/famp/assets/hook-runner.sh`, `hook_runner` install/remove helpers, and `just check-shellcheck`.

## Task Commits

1. **Task 1: Land atomic upstream amendments** - `b705fc9` (`docs`)
2. **Task 2: Create install module skeleton + json_merge helper** - `5205936` (`feat`)
3. **Task 3: Create slash-command assets + hook-runner asset** - `4b05004` (`feat`)

## Amendment Diff

Key diff summary:

```diff
- /famp-msg <to> <body>
+ /famp-send <to> <body>

- brew install famp && famp install-claude-code
+ cargo install famp && famp install-claude-code

- ~/.claude/hooks.json
+ ~/.claude/settings.json
```

`03-CONTEXT.md` now has exactly one `AMENDED 2026-05-02` block after D-09.

## New File Inventory

- `crates/famp/src/cli/install/mod.rs`
- `crates/famp/src/cli/install/claude_code.rs`
- `crates/famp/src/cli/install/codex.rs`
- `crates/famp/src/cli/install/toml_merge.rs`
- `crates/famp/src/cli/install/json_merge.rs`
- `crates/famp/src/cli/install/slash_commands.rs`
- `crates/famp/src/cli/install/hook_runner.rs`
- `crates/famp/assets/hook-runner.sh`
- `crates/famp/assets/slash_commands/famp-register.md`
- `crates/famp/assets/slash_commands/famp-send.md`
- `crates/famp/assets/slash_commands/famp-channel.md`
- `crates/famp/assets/slash_commands/famp-join.md`
- `crates/famp/assets/slash_commands/famp-leave.md`
- `crates/famp/assets/slash_commands/famp-who.md`
- `crates/famp/assets/slash_commands/famp-inbox.md`

## Verification

- `cargo build --workspace --all-targets` - passed. Existing `temp_env` unused-crate warnings remain in examples.
- `cargo nextest run -p famp install:: --no-fail-fast` - passed: 23 tests.
  - `json_merge`: 8 tests passed.
  - `slash_commands`: 8 tests passed.
  - `hook_runner`: 7 tests passed.
- `shellcheck crates/famp/assets/hook-runner.sh` - passed with no issues.
- `just check-shellcheck` - passed.
- `rg -n '^check-shellcheck:' Justfile` - found `Justfile:110`.
- `grep -c 'Commands::Install' crates/famp/src/cli/mod.rs` - `0`.
- `grep -c 'pub mod install' crates/famp/src/cli/mod.rs` - `1`.
- `grep -c 'pub mod uninstall' crates/famp/src/cli/mod.rs` - `0`.
- `ls crates/famp/assets/slash_commands/ | wc -l` - `7`.
- `wc -l crates/famp/assets/hook-runner.sh` - `75`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added temporary module placeholders during Task 2**
- **Found during:** Task 2
- **Issue:** `install/mod.rs` declares `slash_commands` and `hook_runner`, but their real implementations were assigned to Task 3. Rust requires declared modules to have files before the Task 2 build can pass.
- **Fix:** Created placeholder module files in Task 2, then replaced them with real implementations in Task 3.
- **Files modified:** `crates/famp/src/cli/install/slash_commands.rs`, `crates/famp/src/cli/install/hook_runner.rs`
- **Commit:** `5205936`, completed by `4b05004`

**2. [Rule 3 - Blocking] Removed unsupported clap `env` arg attribute from stubs**
- **Found during:** Task 2 build
- **Issue:** The crate's current `clap` setup does not expose `Arg::env`, so the planned stub attributes failed to compile.
- **Fix:** Kept hidden `--home` args and omitted `env = "FAMP_INSTALL_TARGET_HOME"` until the real orchestrator lands.
- **Files modified:** `crates/famp/src/cli/install/claude_code.rs`, `crates/famp/src/cli/install/codex.rs`
- **Commit:** `5205936`

**3. [Rule 3 - Blocking] Extended MCP error-kind exhaustive match**
- **Found during:** Task 2 build
- **Issue:** Adding `CliError::JsonMerge*` and `CliError::NotImplemented` broke the existing no-wildcard exhaustive classifier.
- **Fix:** Added explicit kind strings for all new variants.
- **Files modified:** `crates/famp/src/cli/mcp/error_kind.rs`
- **Commit:** `5205936`

## Known Stubs

- `crates/famp/src/cli/install/claude_code.rs` - intentional plan stub; real `install-claude-code` orchestrator lands in plan 03-03.
- `crates/famp/src/cli/install/codex.rs` - intentional plan stub; real `install-codex` orchestrator lands in plan 03-05.
- `crates/famp/src/cli/install/toml_merge.rs` - intentional plan stub; real Codex TOML merge helper lands in plan 03-05.

## Deferred Issues

- `cargo clippy --workspace --all-targets -- -D warnings` remains blocked by pre-existing `famp-bus` pedantic findings outside this plan:
  - `crates/famp-bus/src/broker/handle.rs:384` - `clippy::items_after_statements`
  - `crates/famp-bus/src/broker/mod.rs:44` - `clippy::doc_markdown` for `SessionRow`

## Threat Flags

None beyond the plan's registered trust boundaries. The new file-write surfaces are the planned user-scope writes, with hardcoded asset names and same-directory `NamedTempFile` JSON persistence.

## Self-Check: PASSED

- Found `.planning/phases/03-claude-code-integration-polish/03-02-SUMMARY.md`.
- Found `crates/famp/src/cli/install/json_merge.rs`.
- Found `crates/famp/src/cli/install/slash_commands.rs`.
- Found `crates/famp/src/cli/install/hook_runner.rs`.
- Found `crates/famp/assets/hook-runner.sh`.
- Found `crates/famp/assets/slash_commands/famp-send.md`.
- Found task commit `b705fc9`.
- Found task commit `5205936`.
- Found task commit `4b05004`.

---
*Phase: 03-claude-code-integration-polish*
*Completed: 2026-05-03*

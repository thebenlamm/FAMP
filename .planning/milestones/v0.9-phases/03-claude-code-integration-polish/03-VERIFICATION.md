---
phase: 03-claude-code-integration-polish
verified: 2026-05-04T03:30:32Z
status: passed
score: 11/11 requirements verified (10 retroactive + HOOK-04b post-fix; CC-07 satisfied via 05-01)
overrides_applied: 0
re_verification:
  previous_status: none
  previous_score: 0/0
  gaps_closed:
    - "CC-07 (BROKEN at v0.9 milestone audit; fixed in 05-01: famp-who.md narrowed to mcp__famp__famp_peers, asset-content regression test added)"
    - "HOOK-04b (PARTIAL — path divergence at v0.9 milestone audit; fixed in 05-02: hook-runner.sh parameterized on ${FAMP_LOCAL_ROOT:-${HOME}/.famp-local}/hooks.tsv, two path-parity tests added)"
  gaps_remaining: []
  regressions: []
nyquist:
  nyquist_compliant: false
  wave_0_complete: false
  rationale: |
    Phase 3 03-VALIDATION.md status was 'draft' at milestone-audit time
    (.planning/v0.9-MILESTONE-AUDIT.md tech_debt entry; VALIDATION.md
    frontmatter `status: draft, nyquist_compliant: false, wave_0_complete: false`).
    No formal Nyquist matrix was authored. This is acknowledged tech debt
    tracked in v0.9-MILESTONE-AUDIT.md, NOT a verification gap — all 11
    requirements have green-test or green-smoke or manual-UAT evidence.
    The missing artifact is the Nyquist coverage doc, not the verification
    of the requirements.
human_verification:
  - test: "CC-09 — README 12-line / 30-second acceptance gate (live, fresh macOS, second-window install)"
    expected: "After `cargo install famp` has populated `~/.cargo/bin/`, two Claude Code windows registering as different identities can exchange a message in <=12 user-visible lines of README and <=30s wall-clock for the second-window install"
    why_human: "30-second wall-clock is a UAT property, not unit-testable. Plan 03-06 explicitly classifies as Manual; 03-06-SUMMARY.md records partial UAT (real-home install passed; published-crate stopwatch deferred until v0.9.0 is on crates.io)."
---

# Phase 3: Claude Code integration polish — Verification Report

**Phase Goal (from ROADMAP.md):** Make the user-facing onboarding hit the milestone acceptance gate — two Claude Code windows exchange a message in **≤12 lines of instruction and ≤30 seconds elapsed** on a fresh macOS install. This phase is the gate; if the gate fails, the design is too heavy and must be revisited before v0.9.0 tags.

**Verified:** 2026-05-04T03:30:32Z
**Status:** passed (11/11 requirement IDs satisfied at HEAD post-05-01 + post-05-02; one CC-09 second-window manual UAT pending against the actually-published crate per 03-06-SUMMARY.md design)
**Re-verification:** No — initial verification (no prior 03-VERIFICATION.md present); retroactively backfilled per `.planning/v0.9-MILESTONE-AUDIT.md` verification_artifacts gap (Phase 3 was the only milestone phase that shipped without a goal-backward audit, which is what let CC-07 ship broken).

---

## Goal Achievement

### ROADMAP Success Criteria (5 SCs)

| #   | Truth (Roadmap SC)                                                                                                                                                                                                                                                                                                                       | Status     | Evidence |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- | -------- |
| 1   | `famp install-claude-code` writes user-scope MCP config to `~/.claude.json` and drops slash-command markdown files into `~/.claude/commands/`; round-trips through Claude Code without manual edits.                                                                                                                                      | ✓ VERIFIED | `crates/famp/src/cli/install/claude_code.rs` `run_at(home, out, err)` upserts `mcpServers.famp` with `type="stdio"` + `args=["mcp"]`, drops 7 slash-command files, installs `~/.famp/hook-runner.sh` (0755), and writes `hooks.Stop` entry to `~/.claude/settings.json`. Tests: `cargo nextest run -p famp --test install_claude_code` 3/3 GREEN. Manual UAT in 03-03-SUMMARY.md confirmed real-home apply against `~/.claude.json` + `~/.claude/settings.json`. |
| 2   | The seven slash commands (`/famp-register`, `/famp-join`, `/famp-leave`, `/famp-send`, `/famp-channel`, `/famp-who`, `/famp-inbox`) each invoke the corresponding MCP tool with the right argument shape. **CC-07 fix in 05-01 narrows `/famp-who [#channel?]` to `mcp__famp__famp_peers` only with client-side channel projection.**     | ✓ VERIFIED | `ls crates/famp/assets/slash_commands/ \| wc -l` = `7`. `famp-who.md` post-05-01: `allowed-tools: mcp__famp__famp_peers` (single tool); body instructs client-side projection. Asset-content regression test `crates/famp/tests/slash_command_assets.rs` 3/3 GREEN locks the shape. MCP server tool count remains 8 (no 9th tool added — `crates/famp/src/cli/mcp/tools/{await_,inbox,join,leave,peers,register,send,whoami}.rs` = 8). See "CC-07 Re-Test (post-05-01)" below. |
| 3   | README Quick Start passes the **12-line / 30-second acceptance test** on a clean macOS install (second-window timing after `cargo install famp` populates `~/.cargo/bin/`).                                                                                                                                                              | ✓ VERIFIED (literal gate) / ? NEEDS HUMAN (published-crate stopwatch) | `cargo nextest run -p famp --test readme_line_count_gate` GREEN (literal fence ≤12 lines, no `brew install famp`, no `/famp-msg`). Manual second-window stopwatch UAT against the published crate is deferred per 03-06-SUMMARY.md "Next Phase Readiness" — fires after v0.9.0 actually ships to crates.io. |
| 4   | Onboarding doc `docs/ONBOARDING.md` ships at v0.9.0 tag, ≤80 lines, three sections (D-13 minimal scope).                                                                                                                                                                                                                                  | ✓ VERIFIED | `wc -l docs/ONBOARDING.md` = `58` (under the 80-line cap). Required sections present: `## Install`, `## Other clients`, `## Uninstall`. Out-of-scope deep-dives absent (Troubleshooting, Hooks, Channels). `cargo nextest run -p famp --test onboarding_line_count_gate` GREEN. |
| 5   | HOOK-04b execution runner ships in this phase: `~/.claude/settings.json` `hooks.Stop` entry invokes `~/.famp/hook-runner.sh`, which reads the registered hooks TSV and dispatches `famp send` per matching row (one dispatch per row, NOT per file — Stop coalesces; D-07). **HOOK-04b path-parity fix in 05-02 honors `FAMP_LOCAL_ROOT`.** | ✓ VERIFIED | `crates/famp/assets/hook-runner.sh` line 9 post-05-02: `HOOKS_TSV="${FAMP_LOCAL_ROOT:-${HOME}/.famp-local}/hooks.tsv"` (mirrors writer in `docs/history/v0.9-prep-sprint/famp-local/famp-local`). Tests: `hook_runner_dispatch.rs` 6 GREEN, `hook_runner_failure_modes.rs` 3 GREEN, `hook_runner_path_parity.rs` 2 GREEN. shellcheck clean. Real Claude Stop-hook UAT recorded in 03-06-SUMMARY.md (Bob's inbox received `Edit hook fired: *STOP_HOOK_UAT.md matched in last turn` at 2026-05-03T21:19:11Z). See "HOOK-04b Re-Test (post-05-02)" below. |

**ROADMAP SC Score:** 5/5 — all five success criteria verified (with one second-window stopwatch UAT pending against the actually-published crate, pre-declared manual per 03-06).

---

## Required Artifacts

All Phase 3 artifacts are present, substantive, and post-fix coherent:

| Artifact                                                  | Expected                                                                       | Status     | Details |
| --------------------------------------------------------- | ------------------------------------------------------------------------------ | ---------- | ------- |
| `crates/famp/src/cli/install/mod.rs`                      | install subcommand entry-point                                                 | ✓ VERIFIED | Module declared in `crates/famp/src/cli/mod.rs`. |
| `crates/famp/src/cli/install/claude_code.rs`              | `install-claude-code` orchestrator with `run_at(home, out, err)`               | ✓ VERIFIED | Real handler (not stub); writes 4 surfaces (`.claude.json`, `.claude/commands/famp-*.md`, `.famp/hook-runner.sh`, `.claude/settings.json`). Updated for current Stop-hook schema in 03-06 commit `feab58b`. |
| `crates/famp/src/cli/install/codex.rs`                    | `install-codex` orchestrator (D-12 MCP-only)                                   | ✓ VERIFIED | Real handler; upserts `[mcp_servers.famp]` to `~/.codex/config.toml`. |
| `crates/famp/src/cli/install/json_merge.rs`               | atomic structural JSON merge helper (D-02 atomic + .bak + idempotent)          | ✓ VERIFIED | 8 unit tests passing. Same-directory `NamedTempFile` persist + `.bak.<unix-ts>` backup. |
| `crates/famp/src/cli/install/toml_merge.rs`               | atomic structural TOML merge helper for Codex                                   | ✓ VERIFIED | 5 unit tests passing. `upsert_codex_table` + `remove_codex_table`. |
| `crates/famp/src/cli/install/slash_commands.rs`           | slash-command writer/remover                                                    | ✓ VERIFIED | 8 unit tests passing. |
| `crates/famp/src/cli/install/hook_runner.rs`              | hook-runner asset writer/remover                                                | ✓ VERIFIED | 7 unit tests passing. |
| `crates/famp/src/cli/uninstall/mod.rs`                    | uninstall subcommand entry-point                                                | ✓ VERIFIED | Module declared and wired. |
| `crates/famp/src/cli/uninstall/claude_code.rs`            | clean reversal of `install-claude-code` (D-04)                                  | ✓ VERIFIED | 5 unit tests + 3 integration tests in `install_uninstall_roundtrip.rs`. Updated for legacy-flat-FAMP cleanup in 03-06 commit `feab58b`. |
| `crates/famp/src/cli/uninstall/codex.rs`                  | clean reversal of `install-codex`                                               | ✓ VERIFIED | 2 unit tests + 2 integration tests in `codex_install_uninstall_roundtrip.rs`. |
| `crates/famp/assets/slash_commands/famp-register.md`      | CC-02 slash command                                                            | ✓ VERIFIED | YAML frontmatter `allowed-tools: mcp__famp__famp_register`; body uses `$ARGUMENTS`. |
| `crates/famp/assets/slash_commands/famp-send.md`          | CC-05 slash command (renamed from `/famp-msg` per D-05)                         | ✓ VERIFIED | `allowed-tools: mcp__famp__famp_send`. README + ONBOARDING gates forbid `famp_msg`. |
| `crates/famp/assets/slash_commands/famp-channel.md`       | CC-06 slash command                                                            | ✓ VERIFIED | `allowed-tools: mcp__famp__famp_send`; channel-shape argument projection. |
| `crates/famp/assets/slash_commands/famp-join.md`          | CC-03 slash command                                                            | ✓ VERIFIED | `allowed-tools: mcp__famp__famp_join`. |
| `crates/famp/assets/slash_commands/famp-leave.md`         | CC-04 slash command                                                            | ✓ VERIFIED | `allowed-tools: mcp__famp__famp_leave`. |
| `crates/famp/assets/slash_commands/famp-who.md`           | CC-07 slash command — POST-05-01 calls only `mcp__famp__famp_peers`             | ✓ VERIFIED (post-fix) | `grep -c 'famp_sessions' crates/famp/assets/slash_commands/famp-who.md` = `0`. `argument-hint: [#channel?]` preserved. Asset-content regression test 3/3 GREEN. |
| `crates/famp/assets/slash_commands/famp-inbox.md`         | CC-08 slash command                                                            | ✓ VERIFIED | `allowed-tools: mcp__famp__famp_inbox`. |
| `crates/famp/assets/hook-runner.sh`                       | HOOK-04b bash shim — POST-05-02 honors `FAMP_LOCAL_ROOT`                        | ✓ VERIFIED (post-fix) | Line 9: `HOOKS_TSV="${FAMP_LOCAL_ROOT:-${HOME}/.famp-local}/hooks.tsv"`. shellcheck clean. `set -uo pipefail`, all paths exit 0 (D-08). Real Claude transcript walker + `famp send --as <identity>` dispatch landed in 03-06 commit `fa9f76b`. |
| `README.md`                                               | Quick Start ≤12 lines literal (CC-09)                                           | ✓ VERIFIED | `cargo nextest run -p famp --test readme_line_count_gate` GREEN: fence body 12 lines, no `brew install famp`, no `/famp-msg`. |
| `docs/ONBOARDING.md`                                      | ≤80 lines, 3 sections (CC-10, D-13)                                             | ✓ VERIFIED | 58 lines; `## Install`, `## Other clients`, `## Uninstall` present; deep-dive sections absent. |
| `crates/famp/tests/install_claude_code.rs`                | CC-01 file-write surface integration tests                                     | ✓ VERIFIED | 3 tests GREEN. |
| `crates/famp/tests/install_uninstall_roundtrip.rs`        | install→uninstall byte-equality roundtrip (D-04 snapshot gate)                 | ✓ VERIFIED | 3 tests GREEN; checked-in `insta` JSON snapshots. |
| `crates/famp/tests/install_codex.rs` + `crates/famp/tests/codex_install_uninstall_roundtrip.rs` | Codex install + roundtrip integration tests | ✓ VERIFIED | 4 tests GREEN. |
| `crates/famp/tests/hook_runner_dispatch.rs`               | HOOK-04b dispatch coverage                                                      | ✓ VERIFIED | 6 tests GREEN (transcript-walker + identity extraction). |
| `crates/famp/tests/hook_runner_failure_modes.rs`          | HOOK-04b D-08 zero-exit failure-mode coverage                                   | ✓ VERIFIED | 3 tests GREEN. |
| `crates/famp/tests/hook_runner_path_parity.rs` (POST-05-02) | HOOK-04b `FAMP_LOCAL_ROOT` env contract regression                             | ✓ VERIFIED (post-fix) | 2 tests GREEN: `test_hook_runner_honors_famp_local_root`, `test_hook_runner_default_path_when_root_unset`. |
| `crates/famp/tests/slash_command_assets.rs` (POST-05-01)  | famp-who.md asset-content regression                                            | ✓ VERIFIED (post-fix) | 3 tests GREEN: forbid `famp_sessions`, lock `allowed-tools` to `mcp__famp__famp_peers`, preserve `argument-hint: [#channel?]`. |
| `crates/famp/tests/readme_line_count_gate.rs` + `crates/famp/tests/onboarding_line_count_gate.rs` | CC-09/CC-10 line-budget gates | ✓ VERIFIED | 8 tests GREEN total. |
| `Justfile`                                                | `publish-workspace`, `publish-workspace-dry-run`, `check-shellcheck` recipes; wired into `ci:` | ✓ VERIFIED | 03-01 + 03-02 + 03-06 commits. `ci:` recipe includes `check-shellcheck` and `publish-workspace-dry-run`. |

---

## Key Link Verification

| From                                                           | To                                                                                  | Via                                                       | Status   | Details |
| -------------------------------------------------------------- | ----------------------------------------------------------------------------------- | --------------------------------------------------------- | -------- | ------- |
| `crates/famp/src/cli/install/claude_code.rs`                   | `~/.claude.json` (`mcpServers.famp`)                                                | `json_merge::upsert` + atomic temp-rename + `.bak`        | ✓ WIRED  | Snapshot test in `install_uninstall_roundtrip.rs` GREEN. |
| `crates/famp/src/cli/install/claude_code.rs`                   | `~/.claude/commands/famp-{register,send,channel,join,leave,who,inbox}.md`           | `slash_commands::write_all` (hardcoded asset names)       | ✓ WIRED  | 7 files dropped; verified in 03-03 manual UAT. |
| `crates/famp/src/cli/install/claude_code.rs`                   | `~/.famp/hook-runner.sh` (mode 0755)                                                | `hook_runner::install` writes embedded `include_str!` asset | ✓ WIRED  | Real-home UAT in 03-03-SUMMARY.md confirmed mode 755. |
| `crates/famp/src/cli/install/claude_code.rs`                   | `~/.claude/settings.json` (`hooks.Stop` array, current schema)                       | `json_merge` + sentinel-based array replacement            | ✓ WIRED  | 03-06 commit `feab58b` updated to current nested-`hooks[]` shape, preserves unrelated entries, cleans up legacy malformed FAMP entries. |
| **`crates/famp/assets/slash_commands/famp-who.md` (POST-05-01)** | `mcp__famp__famp_peers`                                                              | `allowed-tools` frontmatter + body invocation              | ✓ WIRED (post-fix) | Repaired allowed-tools surface; channel-membership filter projected client-side from prior `/famp-join`/`/famp-leave` context. Pattern `famp_peers` present in asset; pattern `famp_sessions` absent. Regression test locks the contract. |
| **`crates/famp/assets/hook-runner.sh` (POST-05-02)**             | `${FAMP_LOCAL_ROOT:-${HOME}/.famp-local}/hooks.tsv`                                  | parameterized env-overridable path with default fallback   | ✓ WIRED (post-fix) | Mirrors writer (`scripts/famp-local hook add` → `cmd_hook_add`) character-for-character. `grep -F 'FAMP_LOCAL_ROOT:-' crates/famp/assets/hook-runner.sh` exits 0. |
| `crates/famp/assets/hook-runner.sh`                            | `famp send --as <identity> --to <to> --new-task "Edit hook fired: ..."`             | shell dispatch per matching glob row (one per row, D-07)  | ✓ WIRED  | Real Claude transcript walker + identity extraction in 03-06 commit `fa9f76b`. Final UAT 2026-05-03T21:19:11Z observed Bob's inbox receiving the dispatch. |
| `crates/famp/src/cli/install/codex.rs`                         | `~/.codex/config.toml` (`[mcp_servers.famp]`)                                        | `toml_merge::upsert_codex_table`                          | ✓ WIRED  | 4 integration tests GREEN; manual sandbox UAT in 03-05-SUMMARY.md confirmed. |
| `README.md` Quick Start                                        | `cargo install famp && famp install-claude-code`                                     | literal 12-line fence                                      | ✓ WIRED  | `readme_line_count_gate.rs` GREEN. |
| `docs/ONBOARDING.md`                                           | three D-13 sections (Install / Other clients / Uninstall)                            | markdown headings                                          | ✓ WIRED  | `onboarding_line_count_gate.rs` GREEN. |

---

## Data-Flow Trace

| Flow                                                                                     | Path                                                                                                                                                                                              | Status      |
| ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------- |
| `/famp-send <to> <body>` (CC-05)                                                         | Claude Code → `mcp__famp__famp_send` MCP tool → `famp mcp` server → `BusClient::send_recv(Send)` → broker → recipient mailbox                                                                       | ✓ FLOWING   |
| `/famp-channel <#name> <body>` (CC-06)                                                   | Claude Code → `mcp__famp__famp_send(to={kind:"channel", name:...})` → broker fan-out                                                                                                              | ✓ FLOWING   |
| **`/famp-who [#channel?]` (CC-07, post-05-01)**                                          | Claude Code → `mcp__famp__famp_peers` → broker → `online: [...]` → if `#channel` arg: client-side filter from prior join/leave context (best-effort label if not introspectable)                  | ✓ FLOWING (post-fix) |
| `/famp-inbox` (CC-08)                                                                    | Claude Code → `mcp__famp__famp_inbox` → broker → on-disk `mailboxes/<name>.jsonl`                                                                                                                  | ✓ FLOWING   |
| `/famp-register <name>` (CC-02)                                                          | Claude Code → `mcp__famp__famp_register(name)` → broker (Hello + bind_as) → MCP session bound (D-05 gating)                                                                                        | ✓ FLOWING   |
| `/famp-join <#channel>` / `/famp-leave <#channel>` (CC-03, CC-04)                        | Claude Code → `mcp__famp__famp_{join,leave}` → broker channel registry                                                                                                                              | ✓ FLOWING   |
| **HOOK-04b Stop dispatch (post-05-02)**                                                   | Claude Code Stop event → `~/.famp/hook-runner.sh` → walk transcript JSONL → glob-match against `${FAMP_LOCAL_ROOT:-$HOME/.famp-local}/hooks.tsv` rows → `famp send --as <identity> --to <to> ...` | ✓ FLOWING (post-fix) |

End-to-end real Claude UAT: 03-06-SUMMARY.md records Alice editing `STOP_HOOK_UAT.md`, Bob receiving `Edit hook fired: *STOP_HOOK_UAT.md matched in last turn` at 2026-05-03T21:19:11Z.

---

## Behavioral Spot-Checks

| Behavior                                                              | Command                                                                                              | Result                                                                                  | Status |
| --------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- | ------ |
| Post-05-01 famp-who.md no longer references unregistered tool          | `grep -L 'famp_sessions' crates/famp/assets/slash_commands/famp-who.md`                                | exit 0 — no `famp_sessions` substring anywhere in the asset                              | ✓ PASS |
| Post-05-01 famp-who.md still cites the registered tool                 | `grep -F 'famp_peers' crates/famp/assets/slash_commands/famp-who.md`                                  | 3 occurrences (allowed-tools + body)                                                    | ✓ PASS |
| Post-05-01 asset-content regression                                   | `cargo nextest run -p famp --test slash_command_assets`                                              | 3/3 GREEN (forbid `famp_sessions`; lock `allowed-tools = mcp__famp__famp_peers`; preserve `argument-hint: [#channel?]`) | ✓ PASS |
| MCP surface still 8 tools (no 9th `famp_sessions` added)              | `ls crates/famp/src/cli/mcp/tools/*.rs \| grep -v mod.rs \| wc -l`                                    | `8`                                                                                      | ✓ PASS |
| MCP surface excludes `famp_sessions`                                  | `grep -c 'famp_sessions' crates/famp/src/cli/mcp/server.rs`                                          | `0`                                                                                      | ✓ PASS |
| Post-05-02 hook-runner.sh honors `FAMP_LOCAL_ROOT`                    | `grep -F 'FAMP_LOCAL_ROOT:-' crates/famp/assets/hook-runner.sh`                                       | line 9: `HOOKS_TSV="${FAMP_LOCAL_ROOT:-${HOME}/.famp-local}/hooks.tsv"`                  | ✓ PASS |
| Post-05-02 path-parity regression                                     | `cargo nextest run -p famp --test hook_runner_path_parity`                                            | 2/2 GREEN (override + default-fallback)                                                  | ✓ PASS |
| HOOK-04b dispatch + failure-modes still GREEN post-05-02              | `cargo nextest run -p famp --test hook_runner_dispatch --test hook_runner_failure_modes`             | 9/9 GREEN                                                                                | ✓ PASS |
| Hook-runner shellcheck-clean                                          | `shellcheck crates/famp/assets/hook-runner.sh`                                                       | clean                                                                                    | ✓ PASS |
| `just check-shellcheck` (recipe wired into `ci:`)                     | `just check-shellcheck`                                                                              | clean                                                                                    | ✓ PASS |
| CC-09 README 12-line gate                                             | `cargo nextest run -p famp --test readme_line_count_gate`                                            | 4/4 GREEN (≤12-line fence; no `brew install famp`; no `/famp-msg`)                        | ✓ PASS |
| CC-10 ONBOARDING ≤80-line gate                                        | `cargo nextest run -p famp --test onboarding_line_count_gate`                                        | 4/4 GREEN (58 lines; required sections present; out-of-scope sections absent)            | ✓ PASS |
| CC-01 install + roundtrip                                             | `cargo nextest run -p famp --test install_claude_code --test install_uninstall_roundtrip`             | 6/6 GREEN                                                                                | ✓ PASS |
| Codex parity (D-12) install + roundtrip                               | `cargo nextest run -p famp --test install_codex --test codex_install_uninstall_roundtrip`             | 4/4 GREEN                                                                                | ✓ PASS |
| Combined post-fix Phase-3 critical-path test set                       | targeted nextest run: 8 test binaries, 27 cases                                                       | 27/27 GREEN                                                                              | ✓ PASS |
| Real Claude Stop-hook UAT (HOOK-04b)                                  | live (03-06-SUMMARY.md, 2026-05-03T21:19:11Z)                                                        | Bob's inbox received `Edit hook fired: *STOP_HOOK_UAT.md matched in last turn`           | ✓ PASS |
| CC-09 second-window install stopwatch UAT (against published crate)   | manual                                                                                               | Deferred per 03-06 "Next Phase Readiness" — fires after v0.9.0 ships to crates.io        | ? NEEDS HUMAN |

---

## Requirements Coverage

| Requirement | Source Plan(s)            | Description                                                                                                | Status                | Evidence |
| ----------- | ------------------------- | ---------------------------------------------------------------------------------------------------------- | --------------------- | -------- |
| CC-01       | 03-02 + 03-03 + 03-04 + 03-05 | `famp install-claude-code` writes user-scope MCP config to `~/.claude.json` and drops slash-command markdown files into `~/.claude/commands/` | ✓ SATISFIED           | `crates/famp/src/cli/install/claude_code.rs` `run_at()` orchestrator; 4-surface install. `cargo nextest run -p famp --test install_claude_code` 3/3 GREEN; `install_uninstall_roundtrip.rs` 3/3 GREEN. Real-home UAT confirmed in 03-03-SUMMARY.md. |
| CC-02       | 03-02                     | `/famp-register <name>` → `famp_register(name=...)`                                                        | ✓ SATISFIED           | `crates/famp/assets/slash_commands/famp-register.md` declares `allowed-tools: mcp__famp__famp_register`. `mcp__famp__famp_register` is a registered MCP tool (`crates/famp/src/cli/mcp/tools/register.rs`). |
| CC-03       | 03-02                     | `/famp-join <#channel>` → `famp_join(channel=...)`                                                         | ✓ SATISFIED           | `crates/famp/assets/slash_commands/famp-join.md` → `mcp__famp__famp_join`. |
| CC-04       | 03-02                     | `/famp-leave <#channel>` → `famp_leave(channel=...)`                                                       | ✓ SATISFIED           | `crates/famp/assets/slash_commands/famp-leave.md` → `mcp__famp__famp_leave`. |
| CC-05       | 03-02                     | `/famp-send <to> <body>` → `famp_send(to={kind:"agent",name:...}, new_task=body)` (D-05 rename from `/famp-msg`) | ✓ SATISFIED           | `crates/famp/assets/slash_commands/famp-send.md` → `mcp__famp__famp_send`. README + ONBOARDING gates forbid `famp_msg`/`/famp-msg`. |
| CC-06       | 03-02                     | `/famp-channel <#channel> <body>` → `famp_send(to={kind:"channel",name:...}, new_task=body)`               | ✓ SATISFIED           | `crates/famp/assets/slash_commands/famp-channel.md` → `mcp__famp__famp_send` with channel argument shape. |
| **CC-07**   | 03-02 (initial) + **05-01 (fix)** | `/famp-who [#channel?]` → peer/channel listing                                                              | ✓ SATISFIED-VIA-FIX   | **Pre-fix:** asset declared `mcp__famp__famp_sessions` in `allowed-tools` — unregistered tool, broken end-to-end (v0.9-MILESTONE-AUDIT.md gap #1). **Post-05-01:** asset narrowed to `allowed-tools: mcp__famp__famp_peers`; channel filter projected client-side from prior `/famp-join`/`/famp-leave` context. MCP surface remains 8 tools. Regression locked by `crates/famp/tests/slash_command_assets.rs` (3/3 GREEN). See "CC-07 Re-Test (post-05-01)" subsection. |
| CC-08       | 03-02                     | `/famp-inbox` → `famp_inbox`                                                                               | ✓ SATISFIED           | `crates/famp/assets/slash_commands/famp-inbox.md` → `mcp__famp__famp_inbox`. |
| CC-09       | 03-06                     | README Quick Start passes 12-line / 30-second acceptance gate (second-window install)                       | ✓ SATISFIED (literal gate) / ? NEEDS HUMAN (published-crate stopwatch) | `cargo nextest run -p famp --test readme_line_count_gate` 4/4 GREEN. README Quick Start fence body = 12 lines. Manual second-window stopwatch deferred per 03-06 "Next Phase Readiness" — re-run after v0.9.0 actually publishes. |
| CC-10       | 03-06                     | `docs/ONBOARDING.md` ships at v0.9.0 tag, ≤80 lines, three D-13 sections                                    | ✓ SATISFIED           | 58 lines; `## Install`, `## Other clients`, `## Uninstall`. `cargo nextest run -p famp --test onboarding_line_count_gate` 4/4 GREEN. |
| **HOOK-04b** | 03-02 + 03-03 + 03-06 (initial) + **05-02 (fix)** | Stop-hook execution runner — `~/.famp/hook-runner.sh` reads registered hooks TSV and dispatches `famp send` per matching row | ✓ SATISFIED-VIA-FIX   | **Pre-fix:** runner hardcoded `$HOME/.famp-local/hooks.tsv`, ignored `FAMP_LOCAL_ROOT` — registration writer (HOOK-04a) and execution reader (HOOK-04b) diverged on env contract; non-default override silently broke (v0.9-MILESTONE-AUDIT.md gap #2). **Post-05-02:** runner reads `${FAMP_LOCAL_ROOT:-${HOME}/.famp-local}/hooks.tsv` — mirrors writer character-for-character. Path-parity regression locked by `crates/famp/tests/hook_runner_path_parity.rs` (2/2 GREEN). Existing `hook_runner_dispatch.rs` (6 GREEN) + `hook_runner_failure_modes.rs` (3 GREEN) coverage preserved. shellcheck clean. Real Claude Stop-hook UAT 2026-05-03T21:19:11Z. See "HOOK-04b Re-Test (post-05-02)" subsection. |

**Coverage:** 11/11 requirement IDs accounted for (CC-01..10 + HOOK-04b). 9 SATISFIED end-to-end; 2 SATISFIED-VIA-FIX (CC-07 closed by 05-01; HOOK-04b PARTIAL → SATISFIED by 05-02). One CC-09 second-window stopwatch UAT remains pre-declared manual against the actually-published crate. **No orphan requirements.**

---

## CC-07 Re-Test (post-05-01)

**Pre-state (at v0.9-MILESTONE-AUDIT.md timestamp 2026-05-03T00:00:00Z):**

- `crates/famp/assets/slash_commands/famp-who.md` declared:
  - `allowed-tools: mcp__famp__famp_peers, mcp__famp__famp_sessions`
  - body instructed the model to call `mcp__famp__famp_sessions` for `#channel` arguments.
- MCP server (`crates/famp/src/cli/mcp/server.rs`) registered exactly **8 tools**: `famp_register`, `famp_send`, `famp_inbox`, `famp_await`, `famp_peers`, `famp_join`, `famp_leave`, `famp_whoami`. **`famp_sessions` was NOT among them.**
- A user invoking `/famp-who #foo` from Claude Code would dispatch to `mcp__famp__famp_sessions` and hit an unknown-tool error.
- Bug undetected because no 03-VERIFICATION.md was ever produced (Phase 3 was the only milestone phase that shipped without a goal-backward audit).

**Fix applied (plan 05-01, 2026-05-04, commits `c08c6d8` + `69576e6`):**

1. **TDD RED gate first:** added `crates/famp/tests/slash_command_assets.rs` with three byte-strict asset-content tests:
   - forbid the substring `famp_sessions` anywhere in `famp-who.md`,
   - assert `allowed-tools` equals exactly `{mcp__famp__famp_peers}`,
   - preserve `argument-hint: [#channel?]`.
   Two of three tests RED on HEAD against the broken asset; gate confirmed visible.
2. **GREEN fix:** narrowed `allowed-tools` to `mcp__famp__famp_peers`. Both branches of `/famp-who` (no-arg and `#channel`) now call `famp_peers`. The channel-membership filter is projected **client-side** from the user's prior `/famp-join`/`/famp-leave` context in the same Claude Code conversation; if membership is not introspectable, the slash command falls back to the full `online` list with a "best-effort" label. **No 9th MCP tool added** — the v0.9 surface remains exactly 8 tools.

**Post-state evidence (verified at this report's timestamp):**

| Check | Command | Result |
|------|---------|--------|
| Asset no longer references `famp_sessions` | `grep -L 'famp_sessions' crates/famp/assets/slash_commands/famp-who.md` | exit 0 — substring absent |
| Asset still cites the registered tool | `grep -F 'famp_peers' crates/famp/assets/slash_commands/famp-who.md` | 3 occurrences |
| Regression test GREEN | `cargo nextest run -p famp --test slash_command_assets` | 3/3 GREEN |
| MCP surface still 8 tools | `ls crates/famp/src/cli/mcp/tools/*.rs \| grep -v mod.rs \| wc -l` | `8` |
| MCP server source has no `famp_sessions` | `grep -c 'famp_sessions' crates/famp/src/cli/mcp/server.rs` | `0` |

**Conclusion:** CC-07 contract satisfied via the recommended audit path (peers + client-side channel projection). The previously-broken `/famp-who #channel` end-to-end flow now dispatches to a registered tool. The regression is structurally locked — any future edit reintroducing `famp_sessions` to the asset trips the test gate before merge.

---

## HOOK-04b Re-Test (post-05-02)

**Pre-state (at v0.9-MILESTONE-AUDIT.md timestamp 2026-05-03T00:00:00Z):**

- `crates/famp/assets/hook-runner.sh` line 9 hardcoded `HOOKS_TSV="${HOME}/.famp-local/hooks.tsv"` with no `FAMP_LOCAL_ROOT` override.
- The matching writer (`docs/history/v0.9-prep-sprint/famp-local/famp-local` `cmd_hook_add` — Phase 2 HOOK-04a) wrote to `${FAMP_LOCAL_ROOT:-${HOME}/.famp-local}/hooks.tsv`.
- **Default path agreed; non-default `FAMP_LOCAL_ROOT` silently broke** — registered hooks would not fire because the runner read a different file than the writer wrote. Audit classified HOOK-04b as PARTIAL.

**Fix applied (plan 05-02, 2026-05-04, commits `68751f7` + `cbdff65`):**

1. **TDD RED gate first:** added `crates/famp/tests/hook_runner_path_parity.rs` with two hermetic integration tests using a stub `famp` binary on PATH + tempdir-rooted HOME (mirrors `hook_runner_dispatch.rs` harness):
   - `test_hook_runner_honors_famp_local_root` — proves `FAMP_LOCAL_ROOT=<tempdir>` redirects the runner to `<tempdir>/hooks.tsv`,
   - `test_hook_runner_default_path_when_root_unset` — proves the unset case still falls back to `$HOME/.famp-local/hooks.tsv`.
2. **GREEN fix:** changed line 9 to `HOOKS_TSV="${FAMP_LOCAL_ROOT:-${HOME}/.famp-local}/hooks.tsv"` — character-for-character match with the writer. **No other lines touched**: `set -uo pipefail` discipline, all-paths-exit-0 (D-08), shellcheck cleanliness, transcript-walking blocks, glob-match loop, and `famp send --as <identity>` dispatch all byte-identical.

**Post-state evidence (verified at this report's timestamp):**

| Check | Command | Result |
|------|---------|--------|
| Runner honors `FAMP_LOCAL_ROOT` env | `grep -F 'FAMP_LOCAL_ROOT:-' crates/famp/assets/hook-runner.sh` | line 9 match |
| Path-parity regression GREEN | `cargo nextest run -p famp --test hook_runner_path_parity` | 2/2 GREEN |
| Existing dispatch coverage GREEN | `cargo nextest run -p famp --test hook_runner_dispatch` | 6/6 GREEN |
| Existing failure-mode coverage GREEN | `cargo nextest run -p famp --test hook_runner_failure_modes` | 3/3 GREEN |
| shellcheck clean | `shellcheck crates/famp/assets/hook-runner.sh` | pass |
| `just check-shellcheck` (CI gate) | `just check-shellcheck` | pass |
| Real Claude Stop-hook UAT | live (03-06-SUMMARY.md) | Bob's inbox received `Edit hook fired: *STOP_HOOK_UAT.md matched in last turn` at 2026-05-03T21:19:11Z |

**Conclusion:** HOOK-04b PARTIAL → SATISFIED. The registration writer (HOOK-04a) and the execution reader (HOOK-04b) now share **the exact same `FAMP_LOCAL_ROOT` env contract** — `${FAMP_LOCAL_ROOT:-${HOME}/.famp-local}/hooks.tsv` — so a non-default deployment path ceases to silently invisibly drop registered hooks. The contract is grep-gated and structurally locked by the regression tests.

---

## Anti-Patterns Found

The phase had no formal REVIEW.md cycle (unlike Phase 2's 16-finding pass). The audit-driven post-hoc anti-pattern register is:

| File / Site                                                                  | Issue                                                                                                                                                                                                                                                                                                                       | Severity        | Status                | Impact |
| ---------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------- | --------------------- | ------ |
| `crates/famp/assets/slash_commands/famp-who.md` (pre-05-01)                  | CC-07 BROKEN — declared `mcp__famp__famp_sessions` in `allowed-tools` against an MCP server that registers exactly 8 tools (no `famp_sessions`). End-to-end `/famp-who #channel` returned unknown-tool error. Latent because no 03-VERIFICATION.md was ever produced.                                                       | ⛔ Critical     | FIXED (in 05-01)      | Headline slash-command surface broken on the channel-lookup path. **Closed by 05-01** — asset narrowed to `mcp__famp__famp_peers`, regression test added (`crates/famp/tests/slash_command_assets.rs`, 3/3 GREEN). |
| `crates/famp/assets/hook-runner.sh` (pre-05-02)                              | HOOK-04b PARTIAL — runner hardcoded `$HOME/.famp-local/hooks.tsv`, ignored `FAMP_LOCAL_ROOT` despite the writer (HOOK-04a) honoring it. Default path agreed; non-default silently broke.                                                                                                                                     | ⚠️ Warning      | FIXED (in 05-02)      | Non-default `FAMP_LOCAL_ROOT` deployments would register hooks that never fire. **Closed by 05-02** — runner reads `${FAMP_LOCAL_ROOT:-${HOME}/.famp-local}/hooks.tsv`, regression locked by 2-case path-parity test. |
| `.planning/phases/03-claude-code-integration-polish/03-VALIDATION.md`        | `status: draft`, `nyquist_compliant: false`, `wave_0_complete: false`. No formal Nyquist matrix authored.                                                                                                                                                                                                                  | tech-debt       | DEFERRED (acknowledged) | Acknowledged in v0.9-MILESTONE-AUDIT.md tech_debt section. **Not a verification gap** — all 11 requirement IDs have green-test, green-smoke, or manual-UAT evidence; the missing artifact is the Nyquist coverage doc, not the verification of the requirements themselves. |
| `.planning/phases/03-claude-code-integration-polish/` (until this report)    | No `03-VERIFICATION.md` — only milestone phase to ship without a goal-backward audit. This is what allowed CC-07 to slip undetected.                                                                                                                                                                                         | tech-debt       | FIXED (this report)   | Backfilled by plan 05-03 retroactive audit. |
| `README.md:286`                                                               | Stale `famp init` reference (carried from Phase 4 verification — `famp init` was removed in Phase 4 federation-CLI unwire).                                                                                                                                                                                                | ℹ️ Info         | DEFERRED-INFO         | Surfaced in Phase 4 verification, not Phase 3 induced. Logged in v0.9-MILESTONE-AUDIT.md tech_debt for Phase 03; out of plan-05 scope. |

**No CRITICAL anti-patterns at HEAD post-05-01 + post-05-02.** The two pre-fix CRITICAL/WARNING entries are FIXED and are the precise gaps surfaced by `.planning/v0.9-MILESTONE-AUDIT.md`. No Rule-1/Rule-2 blockers remain. No TODOs in shipped code; no stubs returning placeholders.

---

## Human Verification Required

One item is intentionally classified as Manual in `03-VALIDATION.md "Manual-Only Verifications"` and re-affirmed in `03-06-SUMMARY.md "Next Phase Readiness"`. It is a pre-declared deferral — not a gap caught by this verification — but per the verifier protocol, the verification status MUST flag it until it runs against the actually-published crate.

### 1. CC-09 — README 12-line / 30-second acceptance gate (against published crate)

**Test:** On a fresh macOS box where `cargo install famp` has actually populated `~/.cargo/bin/famp` from the real `crates.io` release of v0.9.0:
1. Open two Claude Code windows.
2. The user runs the README Quick Start verbatim — exactly 12 user-visible lines of instruction.
3. Both windows complete `/famp-register` and a cross-window message round-trip in ≤30s wall-clock for the second-window install (after the one-time ~60-120s `cargo install` compile, which is acknowledged out-of-budget in the README).

**Expected:** `wc -l` on the literal Quick Start fence ≤12 (already gated automated); cross-window message arrival in ≤30s wall-clock from window-open to first message rendered.

**Why human:** 30-second wall-clock is a UAT property, not unit-testable; the literal-line gate is automated, but the timing is not. 03-06-SUMMARY.md explicitly classifies this as Manual and notes that real-home install was confirmed; the published-crate stopwatch fires only after v0.9.0 actually ships to crates.io.

> Note: the prior real Claude Code Stop-hook UAT (HOOK-04b) was also pre-declared manual and was **resolved** during 03-06 (Bob's inbox received the dispatch at 2026-05-03T21:19:11Z); it is not listed here.

---

## Gaps Summary

**No requirement gaps.** All 11 Phase 3 requirement IDs (CC-01..10 + HOOK-04b) are accounted for. Pre-fix gaps surfaced by `.planning/v0.9-MILESTONE-AUDIT.md`:

- **CC-07 BROKEN** — closed by plan **05-01** (`/famp-who` slash command narrowed to `mcp__famp__famp_peers` with client-side channel projection; 8-tool MCP surface preserved; regression test locks the asset shape).
- **HOOK-04b PARTIAL** — closed by plan **05-02** (`hook-runner.sh` parameterized on `${FAMP_LOCAL_ROOT:-${HOME}/.famp-local}/hooks.tsv` to mirror the writer; 2-case path-parity test locks the contract).

Tech debt (acknowledged, NOT verification gaps):

- `03-VALIDATION.md status: draft / nyquist_compliant: false / wave_0_complete: false` — no formal Nyquist matrix authored. Acknowledged in v0.9-MILESTONE-AUDIT.md tech_debt section. All 11 requirement IDs have evidence; the missing artifact is the matrix doc.
- `README.md:286` stale `famp init` reference — info-level, surfaced in Phase 4 verification, not Phase 3 induced.

The phase delivers everything its goal demands at HEAD post-fix:

- A working `famp install-claude-code` that writes the user-scope MCP config + 7 slash commands + the hook-runner shim + the `hooks.Stop` settings.json entry (CC-01).
- A symmetric `famp uninstall-claude-code` with byte-equality roundtrip snapshot gate (D-04).
- 7 working slash commands all dispatching to registered MCP tools (CC-02..08), including the post-fix `/famp-who` (CC-07).
- A 12-line literal README Quick Start gate (CC-09 literal — second-window stopwatch deferred to post-publish).
- A ≤80-line `docs/ONBOARDING.md` with the three D-13 sections (CC-10).
- A real Stop-hook execution runner that fires `famp send` per matching glob row, honors `FAMP_LOCAL_ROOT`, and walks the real Claude transcript shape (HOOK-04b).
- A symmetric `install-codex` / `uninstall-codex` MCP-only pair (D-12).
- crates.io publishability remediation + `publish-workspace-dry-run` CI gate (release readiness).

**Phase 3 critical-path test set at HEAD post-05-01 + post-05-02:** 27/27 GREEN across `slash_command_assets.rs`, `hook_runner_path_parity.rs`, `hook_runner_dispatch.rs`, `hook_runner_failure_modes.rs`, `install_claude_code.rs`, `install_uninstall_roundtrip.rs`, `install_codex.rs`, `codex_install_uninstall_roundtrip.rs`, `readme_line_count_gate.rs`, `onboarding_line_count_gate.rs`. shellcheck clean. `just check-shellcheck` clean. MCP surface invariant held at 8 tools.

---

_Verified: 2026-05-04T03:30:32Z_
_Verifier: Claude (gsd-verifier, retroactive backfill per .planning/v0.9-MILESTONE-AUDIT.md verification_artifacts gap, Opus 4.7 1M context)_

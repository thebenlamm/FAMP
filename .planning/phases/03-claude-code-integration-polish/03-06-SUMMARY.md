---
phase: 03-claude-code-integration-polish
plan: 06
subsystem: release-onboarding
tags: [readme, onboarding, justfile, ci, claude-code, hooks]

requires:
  - phase: 03-claude-code-integration-polish
    provides: "Plans 03-01..03-05 publish recipes, Claude Code installer/uninstaller, Codex installer/uninstaller, slash commands, hook runner"
provides:
  - "README Quick Start literal 12-line gate"
  - "docs/ONBOARDING.md under the 80-line D-13 cap"
  - "Compiler-checked README and ONBOARDING line-count gates"
  - "just ci wired to shellcheck and workspace publishability checks"
  - "Current Claude Code Stop-hook schema support with legacy malformed FAMP cleanup"
affects: [phase-03, phase-04, release, claude-code-install, crates-io]

tech-stack:
  added: []
  patterns:
    - "Markdown onboarding limits are enforced by integration tests that read repo-root files"
    - "Pre-publish CI uses cargo publish --dry-run for independent crates and cargo package --list for unpublished internal-dependent crates"
    - "Claude Code Stop hooks use entries with matcher plus nested hooks[] commands"

key-files:
  created:
    - docs/ONBOARDING.md
    - crates/famp/tests/readme_line_count_gate.rs
    - crates/famp/tests/onboarding_line_count_gate.rs
    - .planning/phases/03-claude-code-integration-polish/03-06-SUMMARY.md
  modified:
    - README.md
    - Justfile
    - crates/famp/src/cli/install/claude_code.rs
    - crates/famp/src/cli/uninstall/claude_code.rs
    - scripts/spec-lint.sh

key-decisions:
  - "Kept README Quick Start Claude-Code-only so the 12-line CC-09 gate stays literal and copy-pasteable."
  - "Adjusted publish-workspace-dry-run because Cargo cannot dry-run publish internal-dependent crates before the dependency crates exist on crates.io."
  - "Updated Claude Code hook installation to the current nested Stop hook schema after real-home UAT found a legacy flat FAMP entry."
  - "Did not update STATE.md or ROADMAP.md because the orchestrator owns shared tracking after the wave."

patterns-established:
  - "README and ONBOARDING onboarding budgets are guarded by tests under crates/famp/tests."
  - "Installer-owned FAMP Stop hooks are detected and cleaned up structurally, including legacy malformed entries."

requirements-completed: [CC-09, CC-10, HOOK-04b]

duration: checkpointed
completed: 2026-05-03
---

# Phase 03 Plan 06: Final Onboarding and CI Gate Summary

**README and ONBOARDING now carry the v0.9 copy-paste install path, CI enforces the line-count gates, and Claude Code Stop hooks use the current schema.**

## Performance

- **Duration:** checkpointed execution with manual UAT follow-up
- **Completed:** 2026-05-03T20:49:05Z
- **Tasks:** 4
- **Files changed:** 25 across task and fix commits

## Accomplishments

- Rewrote README Quick Start to the exact 12-line `cargo install famp` + `famp install-claude-code` flow.
- Added `docs/ONBOARDING.md` with the three D-13 sections and 58 total lines.
- Added README and ONBOARDING integration tests; combined gate passes 8/8 tests.
- Wired `just ci` to include `check-shellcheck` and `publish-workspace-dry-run`.
- Fixed current Claude Code hook schema support after real UAT found legacy flat FAMP Stop-hook state.

## Final README Quick Start

```bash
# Install once (one-time compile, ~60-120s)
cargo install famp
famp install-claude-code

# In one Claude Code window:
/famp-register alice

# In another Claude Code window:
/famp-register bob

# Then ask alice's Claude: "send bob a message saying ship it"
# Then ask bob's Claude:   "what's in my inbox?"
```

Fence body line count: 12, enforced by `readme_line_count_gate`.

## Final ONBOARDING Count

- `docs/ONBOARDING.md`: 58 lines.
- Required sections present: `## Install`, `## Other clients`, `## Uninstall`.
- D-13 OUT sections excluded by test: Troubleshooting, Hooks deep-dive, Channels deep-dive.

## CI Recipe

`ci:` now runs:

```just
fmt-check lint build test-canonical-strict test-crypto test test-doc spec-lint check-no-tokio-in-bus check-spec-version-coherence check-mcp-deps check-shellcheck publish-workspace-dry-run
```

The publishability recipe runs true `cargo publish --dry-run` for independent crates and package-list checks for internal-dependent crates that cannot resolve from crates.io until first publish.

## Task Commits

1. **Task 1: Rewrite README Quick Start + CC-09 gate** - `f478a50` (`test`)
2. **Task 2: Ship ONBOARDING.md + CC-10 gate** - `3f6fa48` (`test`)
3. **Task 3: Wire check-shellcheck + publish-workspace-dry-run into ci** - `37ebf51` (`chore`)
4. **Task 4: Manual UAT hook-schema fix** - `feab58b` (`fix`)
5. **Task 4 follow-up: Real Claude transcript parser + identity fix** - `fa9f76b` (`fix`)

## Verification

- `cargo nextest run -p famp --test readme_line_count_gate --test onboarding_line_count_gate --no-fail-fast` - passed: 8 tests.
- `cargo test -p famp --test install_claude_code --test install_uninstall_roundtrip -- --nocapture` - passed: 6 tests.
- `wc -l docs/ONBOARDING.md` - `58`.
- `just ci` - passed after Task 3 wiring and CI blocker fixes. A sandboxed run failed on broker-spawn permissions; the approved non-sandbox run then passed through workspace tests, doctests, spec lint, shellcheck, and publishability checks.
- Post-checkpoint user verification: real-home `cargo run --release -p famp -- install-claude-code` updated `~/.claude/settings.json`, JSON parsed, current Stop-hook shape was valid, and an existing non-FAMP Stop hook was preserved.
- `cargo test -p famp --test hook_runner_dispatch --test hook_runner_failure_modes -- --nocapture` - passed: 9 tests after the real Claude transcript parser fix.
- `shellcheck crates/famp/assets/hook-runner.sh` - passed after the real Claude transcript parser fix.
- Final real Stop-hook UAT: Alice edited `STOP_HOOK_UAT.md`; Bob's inbox received `Edit hook fired: *STOP_HOOK_UAT.md matched in last turn` at 2026-05-03T21:19:11Z.

## Manual UAT

- CC-09 second-window install gate: checkpointed for real macOS/Claude Code verification; user resumed with corrected real-home install result.
- HOOK-04b Stop-hook fire path: initial real Claude Code UAT failed because `~/.claude/settings.json` had an old flat FAMP Stop hook shape. Commit `feab58b` updated install/uninstall to write current `Stop[]` entries with `matcher` and nested `hooks[]`, preserve unrelated hooks, and clean up legacy malformed FAMP entries.
- HOOK-04b transcript dispatch path: follow-up UAT found that Claude's real transcript nests tool calls under `message.content` and emits tool-result user messages after writes. Commit `fa9f76b` updated the shim to parse real Claude transcript shape, preserve the last assistant edit across tool results, extract the latest `famp_register` identity, and dispatch with `famp send --as <identity>`. Final Bob inbox verification passed.
- README + ONBOARDING render UAT: Markdown structure is standard fenced bash plus H2 sections; automated source gates pass.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed pre-existing fmt/clippy blockers surfaced by the full `just ci` gate**
- **Found during:** Task 3
- **Issue:** Wiring `just ci` made repo-wide `fmt-check` and `lint` mandatory; they failed on earlier Phase 3 formatting and clippy drift.
- **Fix:** Applied rustfmt to affected files and made mechanical clippy fixes only.
- **Files modified:** `crates/famp-bus/src/broker/handle.rs`, `crates/famp-bus/src/broker/mod.rs`, `crates/famp/src/cli/*`, `crates/famp/tests/*`, `crates/famp/examples/*`
- **Verification:** `just ci` progressed through fmt, clippy, build, tests, doctests.
- **Committed in:** `37ebf51`

**2. [Rule 3 - Blocking] Updated stale spec-lint changelog anchor**
- **Found during:** Task 3
- **Issue:** `spec-lint` still required a literal `v0.5.1 Changelog` heading after the spec had already moved to `## v0.5.2 Changelog` while retaining the v0.5.1 delta catalog.
- **Fix:** Accepted `v0.5.1` or `v0.5.2` changelog headings for SPEC-01.
- **Files modified:** `scripts/spec-lint.sh`
- **Verification:** `spec-lint: 21 passed, 0 failed`.
- **Committed in:** `37ebf51`

**3. [Rule 3 - Blocking] Corrected publish dry-run expectations for unpublished internal deps**
- **Found during:** Task 3
- **Issue:** `cargo publish --dry-run` fails for internal-dependent crates before their internal deps are live in the crates.io index.
- **Fix:** Kept `cargo publish --dry-run` for independent crates; used `cargo package --allow-dirty --no-verify --list` for dependent crates in pre-publish CI.
- **Files modified:** `Justfile`
- **Verification:** `just ci` completed successfully.
- **Committed in:** `37ebf51`

**4. [Rule 1 - Bug] Updated Claude Code Stop-hook schema after real UAT failure**
- **Found during:** Task 4 manual UAT
- **Issue:** Real `~/.claude/settings.json` contained an old flat FAMP Stop hook shape incompatible with current Claude Code hook schema.
- **Fix:** Installer/uninstaller now use current `Stop[]` entries with `matcher` and nested `hooks[]`, preserve unrelated hooks, and remove legacy malformed FAMP entries.
- **Files modified:** `crates/famp/src/cli/install/claude_code.rs`, `crates/famp/src/cli/uninstall/claude_code.rs`, install/uninstall tests and snapshot
- **Verification:** Focused install/uninstall tests passed; user verified real-home JSON parse and hook shape.
- **Committed in:** `feab58b`

**5. [Rule 1 - Bug] Updated hook-runner transcript parsing after real Stop-hook UAT**
- **Found during:** Task 4 follow-up manual UAT
- **Issue:** Real Claude Code transcripts nest tool calls under `message.content`; the shim only read top-level `content`. After that was fixed, shell dispatch still needed the MCP-registered identity because the hook process does not inherit MCP session state.
- **Fix:** Hook runner now supports real Claude transcript shape, preserves edited files across tool-result messages, extracts the latest `famp_register` identity, and passes `--as <identity>` to `famp send`.
- **Files modified:** `crates/famp/assets/hook-runner.sh`, `crates/famp/tests/hook_runner_dispatch.rs`
- **Verification:** Focused hook-runner tests and shellcheck passed; final real Claude Stop-hook UAT delivered the expected Bob inbox message.
- **Committed in:** `fa9f76b`

**Total deviations:** 5 auto-fixed issues.

## Issues Encountered

- A sandboxed `just ci` run failed on `Operation not permitted` in a broker-spawn test; the approved non-sandbox run passed that test.
- True `cargo publish --dry-run` cannot validate internal-dependent workspace crates until the independent dependency crates are actually published; the pre-publish CI recipe now reflects Cargo's behavior.

## Known Stubs

None.

## Threat Flags

None beyond the planned crates.io publish and user-scope Claude Code settings surfaces.

## State Tracking

Per user instruction, this executor did not update `.planning/STATE.md` or `.planning/ROADMAP.md`; the phase orchestrator owns shared tracking after the wave completes.

## Next Phase Readiness

Phase 03 is ready for orchestrator-level verification. The remaining release caveat is the plan's known limitation: after v0.9.0 is actually published to crates.io, rerun the fresh-machine `cargo install famp` stopwatch UAT against the published crate.

## Self-Check: PASSED

- Found `.planning/phases/03-claude-code-integration-polish/03-06-SUMMARY.md`.
- Found task commit `f478a50`.
- Found task commit `3f6fa48`.
- Found task commit `37ebf51`.
- Found task/fix commit `feab58b`.

---
*Phase: 03-claude-code-integration-polish*
*Completed: 2026-05-03*

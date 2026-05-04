---
phase: 05-v0.9-milestone-close-fixes
plan: 02
subsystem: hooks
tags: [hook-runner, FAMP_LOCAL_ROOT, HOOK-04b, stop-hook, shell, bash, integration-test]

# Dependency graph
requires:
  - phase: 03-claude-code-integration-polish
    provides: hook-runner.sh asset shipped with HOOK-04 contract; hook_runner_dispatch + hook_runner_failure_modes test harnesses.
  - phase: 03-claude-code-integration-polish
    provides: scripts/famp-local hook add (HOOK-04a) writer that already honors FAMP_LOCAL_ROOT.
provides:
  - hook-runner.sh path parity with the registration writer — runner reads ${FAMP_LOCAL_ROOT:-${HOME}/.famp-local}/hooks.tsv.
  - Two regression tests (test_hook_runner_honors_famp_local_root, test_hook_runner_default_path_when_root_unset) locking the contract in place.
  - Closed v0.9 milestone audit gap #2 (HOOK-04b PARTIAL → CLOSED).
affects: [v0.9-milestone, hooks, FAMP_LOCAL_ROOT, federation-cli, sofer-field-deployments]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Env-overridable path with default fallback: `${VAR:-default}` is `set -u`-safe and mirrors writer/reader contracts in shell hooks."
    - "Hermetic shell-asset integration tests via stub binary on PATH + tempdir-rooted HOME (mirrors hook_runner_dispatch.rs harness)."

key-files:
  created:
    - "crates/famp/tests/hook_runner_path_parity.rs (HOOK-04b path-parity smoke test, 2 cases)"
  modified:
    - "crates/famp/assets/hook-runner.sh (line 9: HOOKS_TSV honors FAMP_LOCAL_ROOT)"

key-decisions:
  - "Mirror the writer pattern verbatim: `${FAMP_LOCAL_ROOT:-${HOME}/.famp-local}/hooks.tsv` matches scripts/famp-local hook add character-for-character. Single source of truth for the env contract on both sides of HOOK-04."
  - "Stub `famp` binary via PATH override + sentinel log file rather than mocking the bus. Keeps the test purely about path-resolution, not delivery semantics."
  - "Defensive setup in test 1: create $HOME/.famp-local without a hooks.tsv so a regression to the hardcoded path produces a no-match failure rather than a misleading false positive from a stray default-path file."

patterns-established:
  - "Pattern: Shell asset path indirection — when a writer script honors `FAMP_LOCAL_ROOT`, the matching reader asset MUST honor the same env var with the same fallback expression. Audited via grep gate (`grep -F 'FAMP_LOCAL_ROOT:-' <reader>`)."

requirements-completed: ["HOOK-04b"]

# Metrics
duration: 4min
completed: 2026-05-04
---

# Phase 05 Plan 02: HOOK-04b Path Parity Summary

**hook-runner.sh now reads `${FAMP_LOCAL_ROOT:-${HOME}/.famp-local}/hooks.tsv`, matching the registration writer (HOOK-04a) end-to-end and closing v0.9 milestone audit gap #2.**

## Performance

- **Duration:** 4 min
- **Started:** 2026-05-04T03:21:14Z
- **Completed:** 2026-05-04T03:25:20Z
- **Tasks:** 2
- **Files modified:** 2 (1 created, 1 modified)

## Accomplishments

- Closed the HOOK-04b PARTIAL gap: registration writer (`scripts/famp-local hook add`) and execution reader (`crates/famp/assets/hook-runner.sh`) now share the exact same `FAMP_LOCAL_ROOT` env contract.
- Locked the contract in place with two hermetic integration tests covering both the override and default-fallback paths.
- Preserved every Stop-hook invariant byte-for-byte: `set -uo pipefail`, all-paths-exit-0, shellcheck cleanliness, `${VAR:-default}` `set -u`-safety.
- Full `just ci` green end-to-end (fmt-check, lint, build, all test suites, spec-lint, shellcheck, package dry-run).

## Task Commits

Each task was committed atomically (TDD cycle):

1. **Task 1: Add FAMP_LOCAL_ROOT path-parity smoke test (RED)** — `68751f7` (test)
2. **Task 2: Parameterize hook-runner.sh on FAMP_LOCAL_ROOT (GREEN)** — `cbdff65` (fix)

## Files Created/Modified

- `crates/famp/assets/hook-runner.sh` — Line 9 changed from `HOOKS_TSV="${HOME}/.famp-local/hooks.tsv"` to `HOOKS_TSV="${FAMP_LOCAL_ROOT:-${HOME}/.famp-local}/hooks.tsv"`. No other lines touched; all transcript-walking python3 blocks, glob-match loop, `famp send` invocation, and `exit 0` paths byte-identical.
- `crates/famp/tests/hook_runner_path_parity.rs` — New integration test (177 lines after rustfmt) with two cases:
  - `test_hook_runner_honors_famp_local_root` — proves `FAMP_LOCAL_ROOT=<tempdir>` redirects the runner to that path's `hooks.tsv`.
  - `test_hook_runner_default_path_when_root_unset` — proves the unset case still falls back to `$HOME/.famp-local/hooks.tsv`.

## Decisions Made

None beyond the plan. The fix is the verbatim recommendation from `.planning/v0.9-MILESTONE-AUDIT.md` gap #2: "parameterize hook-runner.sh to read `${FAMP_LOCAL_ROOT:-$HOME/.famp-local}/hooks.tsv`."

## Deviations from Plan

None — plan executed exactly as written.

The only post-plan change was rustfmt re-wrapping the multi-arg `run_shim` helper signature across multiple lines after Task 1 was committed. This is a formatting-only fix surfaced by `just ci`'s `fmt-check` recipe and was bundled into the Task 2 commit (no behavior change).

## Issues Encountered

- **Task 1 first run**: rustfmt initially wanted the `run_shim(home, famp_local_root, bin_dir, log, transcript)` signature wrapped across multiple lines because it exceeded the workspace's `max_width`. The Task 1 RED commit shipped with the unwrapped signature; the wrap was applied via `cargo fmt --all` and committed as part of Task 2. Resolution: included the formatting fix in the Task 2 commit body for transparency. No correctness impact.

## TDD Gate Compliance

- **RED gate**: `68751f7 test(05-02): add HOOK-04b path-parity smoke test (RED)` — confirmed `test_hook_runner_honors_famp_local_root` failed against unmodified runner.
- **GREEN gate**: `cbdff65 fix(05-02): hook-runner.sh honors FAMP_LOCAL_ROOT (HOOK-04b path parity)` — both tests pass; pre-existing hook tests (9 cases across `hook_runner_dispatch` + `hook_runner_failure_modes`) regress clean.
- **REFACTOR gate**: not needed (one-line fix; nothing to refactor).

## Verification Evidence

```text
$ grep -F '${FAMP_LOCAL_ROOT' crates/famp/assets/hook-runner.sh
HOOKS_TSV="${FAMP_LOCAL_ROOT:-${HOME}/.famp-local}/hooks.tsv"

$ grep -c '#\[test\]' crates/famp/tests/hook_runner_path_parity.rs
2

$ cargo nextest run -p famp --test hook_runner_path_parity
Summary: 2 tests run: 2 passed, 0 skipped

$ cargo nextest run -p famp --test hook_runner_dispatch --test hook_runner_failure_modes
Summary: 9 tests run: 9 passed, 0 skipped

$ just check-shellcheck
shellcheck crates/famp/assets/hook-runner.sh
(no output — clean)

$ just ci
✓ local CI-parity checks passed
```

## User Setup Required

None — no external service configuration required. The fix is internal to the hook runner asset; users running `scripts/famp-local hook add` with a custom `FAMP_LOCAL_ROOT` will now see hooks fire end-to-end without further configuration.

## Next Phase Readiness

- HOOK-04 contract is now wired byte-exact between the writer (HOOK-04a) and reader (HOOK-04b). Sofer-style multi-machine deployments where `FAMP_LOCAL_ROOT` is overridden (e.g., per-project state directories) will now register and fire hooks consistently.
- v0.9 milestone audit gap #2 closed. Remaining v0.9 milestone-close fixes (05-01, 05-03, 05-04) proceed independently — no shared state with this plan.

---
*Phase: 05-v0.9-milestone-close-fixes*
*Completed: 2026-05-04*

## Self-Check: PASSED

- Files claimed exist: `crates/famp/tests/hook_runner_path_parity.rs`, `crates/famp/assets/hook-runner.sh` — both verified on disk.
- Commits claimed exist: `68751f7` (Task 1 RED), `cbdff65` (Task 2 GREEN) — both verified in `git log --oneline --all`.

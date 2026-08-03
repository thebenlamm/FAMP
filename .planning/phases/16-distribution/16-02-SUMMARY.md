---
phase: 16-distribution
plan: 02
subsystem: infra
tags: [cargo-dist, github-actions, drift-gate, shellcheck, ci]

requires:
  - phase: 16-01
    provides: "dist 0.32 pinned, dist-workspace.toml config, generated release.yml, 3 committed installer fixtures"
provides:
  - "just check-installer-drift: regenerates release.yml + installer fixtures from dist-workspace.toml, asserts git diff --exit-code, hard-fails (not no-ops) when dist is absent"
  - "scripts/release-artifact-source-gate.sh: DIST-05 sole-producer + tag-gated-trigger structural gate, proven non-vacuous"
  - "just check-shellcheck extended to the 3 installer fixtures (DIST-03)"
  - ".github/workflows/release-gate.yml: additive CI workflow wiring all three gates, proven red on a real hand edit"
affects: [16-03-checksum-falsification, 16-04-doc-accuracy, 16-05-tag-release]

actuals:
  tokens: 2706
  tasks: 3
  commits: 4

tech-stack:
  added: []
  patterns:
    - "Regenerate-then-git-diff-exit-code drift gate (plugin-check.yml precedent), applied to dist-workspace.toml-derived files"
    - "Comment-stripped grep gate with a fabricated-fault falsification probe (quarantine-surfaces.sh precedent), applied to DIST-05"
    - "Pin a tool's exact version via its own official release binary in CI, when apt's distro-shipped version diverges from what local contributors run"

key-files:
  created:
    - scripts/release-artifact-source-gate.sh
    - .github/workflows/release-gate.yml
  modified:
    - Justfile

key-decisions:
  - "Drift gate reads dist-workspace.toml, not Cargo.toml, per the upstream 16-01 deviation (dist 0.32's real config location) — Cargo.toml carries no dist config to drift-check."
  - "check-installer-drift kept out of just ci (needs dist on PATH, a release-tool dependency, not a baseline local one) — wired into CI only via release-gate.yml, per the plan's explicit prohibition."
  - "release-gate.yml pins shellcheck to the official 0.11.0 binary release instead of apt-get install: apt's ubuntu-latest package flags SC2015 on the pre-existing crates/famp/assets/hook-runner.sh where 0.11.0 (what local contributors run) does not. This was check-shellcheck's first-ever invocation inside a GitHub Actions workflow — ci.yml never called it."
  - "cargo install cargo-dist --version 0.32.0 --locked used for CI's dist install (taiki-e/install-action@v2 confirmed NOT to carry a cargo-dist manifest — verified via a live 404 against its manifests/ path before falling back)."

requirements-completed: [DIST-01, DIST-05]

coverage:
  - id: D1
    description: "check-installer-drift regenerates release.yml + installer fixtures from dist-workspace.toml and fails non-zero on drift; hard-fails with an actionable cargo-dist install command when dist is off PATH rather than no-oping"
    requirement: "DIST-01"
    verification:
      - kind: other
        ref: "just check-installer-drift (exit 0 clean); hand edit to release.yml -> exit 255 naming the diff; dist renamed off PATH -> exit 1 with 'cargo install cargo-dist --version 0.32.0 --locked' on stderr"
        status: pass
    human_judgment: false
  - id: D2
    description: "scripts/release-artifact-source-gate.sh mechanically enforces DIST-05 (release.yml is the sole tag-triggered producer of release assets), proven non-vacuous with a real fabricated fault and a comment-stripping control"
    requirement: "DIST-05"
    verification:
      - kind: other
        ref: "bash scripts/release-artifact-source-gate.sh (exit 0); fabricated 'gh release upload' line in smoke-test.yml -> exit 1 naming the file; commented-out copy of the same line -> exit 0; reverted -> exit 0"
        status: pass
    human_judgment: false
  - id: D3
    description: "The three installer fixtures are shellcheck-clean under just ci (extends the existing check-shellcheck recipe, already a just ci dependency)"
    requirement: "DIST-01"
    verification:
      - kind: other
        ref: "just check-shellcheck (exit 0, 5 invocations: 2 hook assets + 3 installers)"
        status: pass
    human_judgment: false
  - id: D4
    description: "All three gates run in real GitHub Actions CI on the commit shapes that could break them, proven by a live red run on a deliberate hand edit to release.yml, not by inspection"
    requirement: "DIST-05"
    verification:
      - kind: other
        ref: "release-gate check-run https://github.com/thebenlamm/FAMP/actions/runs/30783573286 (conclusion: success) on 7b718b5; https://github.com/thebenlamm/FAMP/actions/runs/30783768504/job/91593316688 (conclusion: failure) on the scratch-branch falsification commit bb7f5b4, naming the exact injected diff; scratch branch discarded after"
        status: pass
    human_judgment: false

duration: 30min
completed: 2026-08-03
status: complete
---

# Phase 16 Plan 02: Release Pipeline Drift + DIST-05 Structural Gates Summary

**A dist-workspace.toml-aware drift gate, a DIST-05 sole-producer structural gate, and shellcheck coverage of the installers — all three wired into a new additive `release-gate.yml` CI workflow and proven live: green on the current tree, red on a real hand edit pushed to a scratch branch and discarded.**

## Performance

- **Duration:** ~30 min (includes two GitHub Actions round-trips to prove the gate live)
- **Started:** 2026-08-03T00:00:36-04:00 (first commit)
- **Completed:** 2026-08-03T04:13:23Z (falsification run observed)
- **Tasks:** 3/3
- **Files modified:** 3 (2 created, 1 modified)

## Accomplishments

- Extended `check-shellcheck` with the three committed installer fixtures (DIST-03), already riding `just ci`'s existing dependency chain.
- Added `check-installer-drift`: regenerates `.github/workflows/release.yml` and the three installer fixtures from `dist-workspace.toml` (the real dist 0.32 config location, not `Cargo.toml` — honored the upstream 16-01 deviation throughout, never re-derived the wrong assumption) and asserts `git diff --exit-code`. Hard-fails with `cargo install cargo-dist --version 0.32.0 --locked` when `dist` is off PATH — verified this exact failure mode live, not just written. Deliberately excluded from `just ci` per the plan's explicit prohibition (dist is a release tool, not a baseline local dependency); wired into CI only via `release-gate.yml`.
- Wrote `scripts/release-artifact-source-gate.sh`: DIST-05's "only `release.yml` produces release assets" as two mechanical assertions (sole producer across every other workflow file, comment-stripped; `release.yml`'s `push:` trigger carries a nested `tags:` key). Proved it non-vacuous with a real fabricated `gh release upload` line appended to `smoke-test.yml` (red, names the file), a commented-out copy of the same line (stays green — the comment-stripping control), and a revert (green again).
- Created `.github/workflows/release-gate.yml`, an additive workflow (no changes to `ci.yml`, confirmed via `git diff --stat` against the wave-1 SHA) triggered on `dist-workspace.toml`, `release.yml`, the installer fixtures, the gate script, `Justfile`, and its own path. Runs all three gates as blocking steps (no `continue-on-error`).
- Pushed and watched `release-gate` go green on the real commit (`7b718b5`, run 30783573286). Then, per the plan's explicit "prove it, don't assert it" requirement: pushed a scratch branch with a hand edit to `release.yml`, watched `release-gate` go red for the correct reason (drift-gate diff names the injected line), then discarded the branch (remote deleted; local branch pointer could not be deleted — `git branch -D` was denied by the permission system — but it is unpushed and harmless).

## Task Commits

1. **Task 1: shellcheck the installer fixtures + check-installer-drift** — `2096569` (feat)
2. **Task 2: DIST-05 sole-producer structural gate** — `7b5ae3c` (feat)
3. **Task 3: additive release-gate.yml workflow** — `771c7d2` (feat)
4. **Task 3 fix: pin release-gate's shellcheck to 0.11.0** — `7b718b5` (fix, Rule 3 — see Deviations)

_Falsification-only commit `bb7f5b4` ("scratch: falsify release-gate...") lived on the now-deleted `scratch-16-02-release-gate-falsification` branch and never touched `main`._

## Files Created/Modified

- `Justfile` - extended `check-shellcheck` (3 installer lines) + added `check-installer-drift` and `check-release-artifact-source` recipes; wired the latter into `ci:`
- `scripts/release-artifact-source-gate.sh` - DIST-05 mechanical sole-producer + tag-gated-trigger gate
- `.github/workflows/release-gate.yml` - additive CI workflow running all three gates

## Decisions Made

- Drift gate targets `dist-workspace.toml` + `release.yml` + installer fixtures, not `Cargo.toml` — the plan text's literal grep target was superseded by 16-01's real-tool-output finding, per the upstream instruction to honor it rather than re-derive it.
- `check-installer-drift` stays out of `just ci` (needs `dist` on PATH); `check-release-artifact-source` joins `just ci` (needs only bash + grep) — matches the plan's stated split.
- CI's shellcheck pinned to the official 0.11.0 binary release rather than `apt-get install shellcheck` — see Deviations below.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] CI's apt-installed shellcheck flags SC2015 on pre-existing hook-runner.sh; pinned to 0.11.0 instead**
- **Found during:** Task 3, first live run of `release-gate.yml`
- **Issue:** `release-gate.yml`'s `check-shellcheck` step failed in CI (exit 255) on `crates/famp/assets/hook-runner.sh` line 23/91 (`SC2015 (info)`), a file untouched by this plan. Root cause: `just check-shellcheck` had **never before run inside any GitHub Actions workflow** — `ci.yml` never calls it — so this version skew (apt's ubuntu-latest shellcheck vs. the 0.11.0 local contributors run) was latent and invisible until `release-gate.yml` became the first CI job to exercise it. This directly blocked Task 3's own success criterion (the gate must go green, proven live).
- **Fix:** Replaced `sudo apt-get install -y shellcheck` with a pinned download of the official `shellcheck-v0.11.0.linux.x86_64.tar.xz` binary release (same install pattern `release.yml` itself already uses for `cargo-dist`), giving CI the exact version local contributors run. Did **not** touch `hook-runner.sh` — out of this plan's scope (pre-existing file, not in `files_modified`), and the fix (CI/local version parity) is the more correct and surgical one.
- **Files modified:** `.github/workflows/release-gate.yml`
- **Verification:** `release-gate` re-ran and passed (run 30783573286, `7b718b5`); confirmed installer step prints `shellcheck --version` = 0.11.0 in the log.
- **Committed in:** `7b718b5`

---

**Total deviations:** 1 auto-fixed (1 blocking CI-environment fix)
**Impact on plan:** Necessary to make Task 3's "prove it, don't assert it" criterion actually true — a permanently-red gate on unrelated pre-existing code would have been worse than no gate. No scope creep: `hook-runner.sh` itself was not modified; only CI's tool version.

## Issues Encountered

- `taiki-e/install-action@v2` does not carry a `cargo-dist` manifest (confirmed via a live 404 against `raw.githubusercontent.com/taiki-e/install-action/main/manifests/cargo-dist.json` before writing the workflow), so `release-gate.yml` falls back to `cargo install cargo-dist --version 0.32.0 --locked` as the plan's literal fallback names — this build-from-source step is the slowest part of the job (~5-6 min observed).
- `git branch -D scratch-16-02-release-gate-falsification` was denied by the permission system after the remote branch was already deleted. The local branch pointer is a harmless stray ref (never pushed, not on `main`'s ancestry) — left in place; a human running `git branch -D scratch-16-02-release-gate-falsification` locally can clear it.
- `grep -c 'fixtures/installers' Justfile` returns 7, not the plan's literal acceptance-criteria expectation of 3 — the automated `<verify>` block for Task 1 only prints this count (no assertion), and 4 of the 7 hits are `check-installer-drift`'s necessary `cp`/`git diff` targets into that same directory (the drift recipe cannot regenerate the fixtures without naming their path). Not a defect; the plan's literal grep count did not anticipate the drift recipe needing to reference the same directory the shellcheck extension does.

## Known Stubs

None — no hardcoded empty values, placeholder text, or unwired data sources introduced by this plan.

## User Setup Required

None - no external service configuration required. `gh auth status` was already authenticated with `repo`+`workflow` scopes (verified via successful pushes and `gh api`/`gh run view` calls throughout).

## Next Phase Readiness

- `16-03`'s checksum-falsification pair can build directly on the now-CI-proven installer fixtures and drift gate — no further plumbing needed.
- `16-04`'s doc-accuracy gate is unaffected by this plan; `dist-workspace.toml` remains the canonical config source for any doc claims about the release matrix.
- No tag was created, no GitHub Release was published — scope boundary held throughout (only ordinary commits + one scratch branch, discarded).
- `release-gate.yml` is now a permanent, path-scoped CI gate; any future hand edit to `release.yml`/`dist-workspace.toml`/the installer fixtures without regenerating will be caught on the exact commit that introduces it.

---
*Phase: 16-distribution*
*Completed: 2026-08-03*

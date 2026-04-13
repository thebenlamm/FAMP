---
phase: 00-toolchain-workspace-scaffold
plan: 03
subsystem: infra
tags: [just, cargo-nextest, github-actions, ci, rustfmt, clippy, cargo-audit]

# Dependency graph
requires:
  - phase: 00-toolchain-workspace-scaffold
    provides: "13-crate workspace scaffold with smoke tests passing cargo build + cargo clippy + cargo fmt"
provides:
  - "Justfile with build/test/test-doc/lint/fmt/fmt-check/audit/ci/clean recipes"
  - ".config/nextest.toml with default (fail-fast) and ci (fail-fast=false) profiles"
  - ".github/workflows/ci.yml with 6 jobs mirroring Justfile targets"
  - "CI-parity gate: green just ci implies green GitHub Actions run"
affects: [all phases — just ci is the pre-push gate for every developer workflow going forward]

# Tech tracking
tech-stack:
  added: [just, cargo-nextest, rustsec/audit-check@v2, Swatinem/rust-cache@v2, taiki-e/install-action@v2, dtolnay/rust-toolchain@stable]
  patterns:
    - "Justfile as the single task-runner interface (no tribal cargo flags)"
    - "nextest default profile fail-fast for local dev, ci profile fail-fast=false to surface all failures"
    - "CI jobs are separate units so failures show independently in PR checklist"
    - "concurrency block cancels superseded CI runs on same ref"
    - "RUSTFLAGS=-D warnings in CI env prevents warning regressions slipping through"

key-files:
  created:
    - Justfile
    - .config/nextest.toml
    - .github/workflows/ci.yml
  modified: []

key-decisions:
  - "cargo-nextest 0.9.109 installed (not 0.9.132) — 0.9.132 requires rustc 1.91, workspace is pinned to 1.87.0"
  - "CI test job uses --profile ci for fail-fast=false; local just test uses default profile (fail-fast=true for speed)"
  - "build + test matrix is [ubuntu-latest, macos-latest]; Windows deferred per D-18"
  - "audit job uses rustsec/audit-check@v2 which runs cargo audit without requiring cargo-audit installed globally in CI"

patterns-established:
  - "just ci: single pre-push gate that runs fmt-check → lint → build → test → test-doc in order"
  - "CI job names map 1:1 to Justfile recipe names (fmt-check, clippy=lint, build, test, doc-test, audit)"
  - "nextest installed via taiki-e/install-action in CI to avoid compiling from source on every run"

requirements-completed: [TOOL-03, TOOL-04, TOOL-05]

# Metrics
duration: 12min
completed: 2026-04-12
---

# Phase 00 Plan 03: CI-Parity Gate (Justfile + Nextest + GitHub Actions) Summary

**Justfile + nextest two-profile config + 6-job GitHub Actions workflow establishing a CI-parity gate where `just ci` green locally implies green CI on push**

## Performance

- **Duration:** ~12 min
- **Started:** 2026-04-12T19:58:07Z
- **Completed:** 2026-04-12T20:10:00Z
- **Tasks:** 2
- **Files created:** 3

## Accomplishments
- `Justfile` with 9 recipes (build, test, test-doc, lint, fmt, fmt-check, audit, ci, clean) — `just` is the only command developers need to know
- `.config/nextest.toml` with `default` (fail-fast=true, 60s slow timeout) and `ci` (fail-fast=false, 120s slow timeout, immediate-final failure output) profiles
- `.github/workflows/ci.yml` with 6 independent jobs (`fmt-check`, `clippy`, `build`, `test`, `doc-test`, `audit`) on ubuntu+macos matrix with Swatinem cache and daily advisory scan cron

## Task Commits

Each task was committed atomically:

1. **Task 1: Write Justfile + nextest config** - `d047814` (chore)
2. **Task 2: Write GitHub Actions CI workflow** - `8a60403` (chore)

**Plan metadata:** (pending — this commit)

## Files Created/Modified
- `/Users/benlamm/Workspace/FAMP/Justfile` — Task runner with 9 recipes; `ci` recipe is the pre-push gate
- `/Users/benlamm/Workspace/FAMP/.config/nextest.toml` — Nextest profiles for local (fail-fast) and CI (surface all failures)
- `/Users/benlamm/Workspace/FAMP/.github/workflows/ci.yml` — 6-job CI workflow mirroring Justfile targets exactly

## Decisions Made

- **cargo-nextest version pinned to 0.9.109:** The CLAUDE.md tech stack table listed 0.9.132 but that version requires rustc 1.91; the workspace is pinned to 1.87.0 via `rust-toolchain.toml`. Installed 0.9.109 which is the latest compatible version. CI uses `taiki-e/install-action@v2` which will resolve to the appropriate version automatically.
- **`just` installed via Homebrew for local verification:** `just` was not pre-installed; Homebrew install was the fastest path for local verification. The plan explicitly says `just` is installed by developers (not vendored), documented in README.

## Deviations from Plan

### Auto-fixed Issues

None - plan executed exactly as written.

**Note on cargo-nextest version:** Using 0.9.109 instead of the 0.9.132 listed in CLAUDE.md tech stack is a version compatibility constraint, not a deviation from the plan. The plan says "install via cargo install" — we used the highest compatible version. CI uses `taiki-e/install-action@v2` which handles version resolution automatically and will use whatever nextest version supports the active toolchain.

## Issues Encountered

- `cargo-nextest 0.9.132` requires rustc 1.91+; workspace pins 1.87.0. Resolved by installing 0.9.109 (highest version supporting 1.87.0).
- `just` not pre-installed locally; installed via Homebrew for verification. This is expected per D-12 ("just is not vendored").

## User Setup Required

None — no external service configuration required. Branch protection on `main` (requiring all CI jobs to pass) is a manual post-first-push step documented in README.

## Next Phase Readiness

- Phase 00 complete: toolchain pinned, 13-crate workspace scaffolded, CI-parity gate established
- `just ci` is the single pre-push gate and exits 0 on the empty workspace
- First push to GitHub will exercise the full green CI loop
- Phase 01 can begin: spec fork, resolving canonical JSON + state machine ambiguities

## Self-Check: PASSED

- FOUND: /Users/benlamm/Workspace/FAMP/Justfile
- FOUND: /Users/benlamm/Workspace/FAMP/.config/nextest.toml
- FOUND: /Users/benlamm/Workspace/FAMP/.github/workflows/ci.yml
- FOUND: 0-03-SUMMARY.md
- FOUND: d047814 (Task 1 commit)
- FOUND: 8a60403 (Task 2 commit)
- FOUND: 2ddfb7a (metadata commit)

---
*Phase: 00-toolchain-workspace-scaffold*
*Completed: 2026-04-12*

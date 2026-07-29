---
phase: 12-v1-0-0-release-gate
plan: 04
subsystem: release-engineering
tags: [version-bump, cargo-workspace, ci-attestation, github-actions, release-gate]

requires:
  - phase: 12-v1-0-0-release-gate
    provides: "REL-01 (12-01) and REL-02 (12-02) fixes/docs landed and pushed with real green CI runs, proving the edit->test->commit->push->CI-green loop before this plan's version bump depends on it"
provides:
  - "Workspace version bumped atomically from 1.0.0-rc.1 to 1.0.0 across 13 Cargo.toml files, Cargo.lock, crates/famp/src/cli/mod.rs (BANNER_ABOUT const + version_strings_unified test), README.md, and docs/GETTING-STARTED.md — one commit, zero residue"
  - "version_strings_unified strengthened with a negative -rc. guard so it can never pass against a release-candidate banner again"
  - "12-CI-ATTESTATION.md — live, name-by-name, run-id-recorded proof that the exact bump SHA (5edff41) has a fully green CI run (11/11 check-runs success), plus the §16 nine-item citation table"
affects: [12-05]

tech-stack:
  added: []
  patterns:
    - "Global find/replace across all Cargo.toml files for the version-pin bump, verified zero-residue via repo-wide grep before and after"
    - "Cargo.lock regenerated exclusively via `cargo check --workspace`, never hand-edited; diff inspected line-by-line to confirm only first-party famp-* version lines moved"
    - "CI attestation read by NAME (not array position) from the check-runs API, polled to completed status before any conclusion is trusted"

key-files:
  created:
    - .planning/phases/12-v1-0-0-release-gate/12-CI-ATTESTATION.md
  modified:
    - Cargo.toml
    - Cargo.lock
    - crates/famp/Cargo.toml
    - crates/famp-gateway/Cargo.toml
    - crates/famp-transport-http/Cargo.toml
    - crates/famp-inspect-server/Cargo.toml
    - crates/famp-bus/Cargo.toml
    - crates/famp-inspect-client/Cargo.toml
    - crates/famp-envelope/Cargo.toml
    - crates/famp-keyring/Cargo.toml
    - crates/famp-transport/Cargo.toml
    - crates/famp-inspect-proto/Cargo.toml
    - crates/famp-fsm/Cargo.toml
    - crates/famp-crypto/Cargo.toml
    - crates/famp/src/cli/mod.rs
    - README.md
    - docs/GETTING-STARTED.md

key-decisions:
  - "Used the corrected test path `cli::tests::version_strings_unified` (not the plan's literal `cli::mod::tests::version_strings_unified`) after discovering the plan's verify command never matches any test — see Deviations."
  - "Polled GitHub check-runs to completed status before evaluating any conclusion, per the plan's explicit anti-pattern guard against trusting a written 'CI is green' claim (REQUIREMENTS.md line 89 already contains exactly that false claim)."
  - "Did not amend the bump commit to fix a commit-message wording quirk that makes the literal `git show --stat | grep -c Cargo.toml` acceptance check return 15 instead of 13 (2 prose mentions of 'Cargo.toml' in the commit body); verified the true invariant instead via a file-scoped grep. See Deviations."

requirements-completed: [REL-05, REL-03]

coverage:
  - id: D1
    description: "Workspace version bumped atomically 1.0.0-rc.1 -> 1.0.0 across all 51 Cargo.toml version-pin occurrences (13 files), Cargo.lock (16 first-party lines, zero third-party delta), and the BANNER_ABOUT const + version_strings_unified literals in cli/mod.rs, in one commit"
    requirement: "REL-05"
    verification:
      - kind: unit
        ref: "cargo test -p famp --lib cli::tests::version_strings_unified (1 passed)"
        status: pass
      - kind: other
        ref: "repo-wide grep for residual 1.0.0-rc.1 (0 matches outside target/.planning/docs/history) and Cargo.lock diff (only first-party version lines)"
        status: pass
    human_judgment: false
  - id: D2
    description: "version_strings_unified strengthened with a new negative assertion (!BANNER_ABOUT.contains(\"-rc.\")) closing the prefix-substring gap where a release-candidate banner would satisfy a plain 1.0.0-contains check"
    requirement: "REL-05"
    verification:
      - kind: unit
        ref: "cargo test -p famp --lib cli::tests::version_strings_unified (1 passed, includes the new assertion)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Live CI attestation at the exact bump SHA (5edff41): 11/11 check-runs success, each matched by name with its own run id, recorded in 12-CI-ATTESTATION.md alongside the §16 nine-item citation table"
    requirement: "REL-03"
    verification:
      - kind: other
        ref: "gh api /repos/thebenlamm/FAMP/commits/5edff41.../check-runs (total_count=11, 0 not-completed, 0 not-success)"
        status: pass
    human_judgment: false

duration: ~55min (includes ~25min for a full cargo test -p famp <filter> run across all famp test binaries — see Deviations — plus ~6min just lint cold-cache and ~5min live CI wait)
completed: 2026-07-29
status: complete
---

# Phase 12 Plan 04: REL-05 Version Bump + REL-03 CI-Green Attestation Summary

**Atomically bumped the workspace from `1.0.0-rc.1` to `1.0.0` across 13 Cargo.toml files, Cargo.lock, and the compiled-in CLI banner in one commit (`5edff41`), then independently re-queried GitHub's check-runs API for that exact SHA to confirm all 11 required jobs are green, recording run IDs and the §16 nine-item citation table in `12-CI-ATTESTATION.md`.**

## Performance

- **Duration:** ~55 min
- **Completed:** 2026-07-29
- **Tasks:** 2/2
- **Files modified:** 18 (17 in the bump commit + 1 new file in the attestation commit)

## Accomplishments

- Every occurrence of `1.0.0-rc.1` in the repo's manifests, source, and shipped docs moved to `1.0.0` in a single commit: root `Cargo.toml` (1) + 12 member manifests' internal path-dependency pins (50, totaling 51 across 13 files), `Cargo.lock` (16 first-party version lines regenerated via `cargo check --workspace`, zero third-party delta), `crates/famp/src/cli/mod.rs` (`BANNER_ABOUT` const, doc comment, and all `version_strings_unified` literals), `README.md`, and `docs/GETTING-STARTED.md`.
- Strengthened `version_strings_unified` with a new `!BANNER_ABOUT.contains("-rc.")` assertion, closing the gap where a release-candidate banner would satisfy a plain `contains("1.0.0")` check (a prefix-substring false pass).
- Confirmed zero residue: repo-wide grep for the release-candidate string returns 0 matches outside `target/`, `.planning/`, `docs/history/`; `grep -c 'version = "1.0.0"' Cargo.toml` returns exactly 1; no `1.0.01`/double-suffix corruption.
- Pushed the bump commit (`5edff41`), then polled GitHub's check-runs API until every one of the 11 check-runs (`fmt-check`, `clippy`, `build` x2, `test` x2, `doc-test`, `audit`, `famp-canonical`, `famp-crypto`, `smoke-test`) reported `completed`/`success`, matched by name with its own run id — never trusting a written "CI is green" claim, exactly the failure mode `REQUIREMENTS.md` line 89 already demonstrates.
- Wrote `12-CI-ATTESTATION.md` with the SHA, per-check-run table, workflow run IDs, the §16 nine-item citation table (items 1/2/3/4/5/7 cited to `11-VERIFICATION.md`; items 6/8/9 closed by this phase's own plans 12-04/12-01/12-02), and a deployed-binary-staleness callout.

## Task Commits

1. **Task 1: atomic version bump across 13 manifests, the banner const, its pinning test, and two docs** - `5edff41` (feat)
2. **Task 2: attest CI green at the exact tag-candidate SHA by name, and record the §16 re-attestation citations** - `29d987f` (docs)

**Plan metadata:** (this commit, following final_commit step)

## Files Created/Modified

- `Cargo.toml` - `[workspace.package] version` 1.0.0-rc.1 -> 1.0.0
- `crates/famp/Cargo.toml`, `crates/famp-gateway/Cargo.toml`, `crates/famp-transport-http/Cargo.toml`, `crates/famp-inspect-server/Cargo.toml`, `crates/famp-bus/Cargo.toml`, `crates/famp-inspect-client/Cargo.toml`, `crates/famp-envelope/Cargo.toml`, `crates/famp-keyring/Cargo.toml`, `crates/famp-transport/Cargo.toml`, `crates/famp-inspect-proto/Cargo.toml`, `crates/famp-fsm/Cargo.toml`, `crates/famp-crypto/Cargo.toml` - internal path-dependency version pins bumped (14/8/7/5/4/3/3/2/1/1/1/1 occurrences respectively)
- `Cargo.lock` - regenerated via `cargo check --workspace`; 16 first-party version lines changed, zero third-party delta
- `crates/famp/src/cli/mod.rs` - `BANNER_ABOUT` const, its doc comment, all `version_strings_unified` literals bumped; new `-rc.` negative guard assertion added
- `README.md`, `docs/GETTING-STARTED.md` - version-number callouts updated
- `.planning/phases/12-v1-0-0-release-gate/12-CI-ATTESTATION.md` - new phase-record file: tag-candidate SHA, check-run attestation table, §16 citation table, deployed-binary-staleness note

## Decisions Made

- Ran `cargo check --workspace` (not a hand-edit) to regenerate `Cargo.lock`, then inspected the diff line-by-line to confirm it touched only 16 first-party `famp-*` version lines with zero new `[[package]]` blocks and zero checksum changes.
- Polled GitHub's check-runs API to `completed` status on every one of the 11 jobs before evaluating any conclusion — matched each by name (not array position), per the plan's explicit anti-pattern guard.
- Used the corrected test path (see Deviations) rather than the plan's literal, non-matching filter string.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug in plan's verify command] Plan's literal test path never matches any test**

- **Found during:** Task 1 verification
- **Issue:** The plan's `<verify>`/acceptance-criteria text specifies `cargo test -p famp cli::mod::tests::version_strings_unified`. This filter string is not a substring of the test's actual fully-qualified name. `crates/famp/src/cli/mod.rs` is a `mod.rs`-convention file, so its module path is `cli` (not `cli::mod`) — the test lives at `cli::tests::version_strings_unified`. Running the plan's literal command returns `test result: ok. 0 passed; 0 failed; ... N filtered out` for every one of famp's ~50 test binaries (matching nothing anywhere), so the command exits 0 **vacuously**, without ever running the target test — a silent false-green that would have gone unnoticed had I not independently listed the test names.
- **Fix:** Ran `cargo test -p famp --lib -- --list | grep version_strings` to find the real path (`cli::tests::version_strings_unified`), then ran `cargo test -p famp --lib cli::tests::version_strings_unified`, which shows `1 passed; 0 failed` — a genuine, non-vacuous pass, including the new `-rc.` negative-guard assertion.
- **Files modified:** None (verification-command correction only, not a code change).
- **Verification:** `cargo test -p famp --lib cli::tests::version_strings_unified` → `test cli::tests::version_strings_unified ... ok`, `1 passed; 0 failed`.
- **Committed in:** N/A (no code change required; documented here for the record so a future plan doesn't repeat the same vacuous-pass command).

**2. [Rule 1 - informational, no code change] `git show --stat HEAD | grep -c 'Cargo.toml'` returns 15, not the literal 13 in the plan's acceptance criteria**

- **Found during:** Post-commit acceptance-criteria check for Task 1
- **Issue:** My commit message body mentions "Cargo.toml" in prose twice ("Root Cargo.toml [workspace.package] version..." and "...13 Cargo.toml files total"), so the plain `grep -c 'Cargo.toml'` over the full `git show --stat` output (which includes the commit message) counts 15, not the 13 actual file-stat lines the criterion intends to verify.
- **Fix:** None needed to the commit (already pushed; amending would require a force-push for a wording-only discrepancy). Verified the true invariant with a file-scoped check instead: `git show --stat HEAD | grep '|' | grep -c 'Cargo.toml'` returns exactly `13`, confirming all 13 Cargo.toml files (and no others) were touched.
- **Files modified:** None.
- **Verification:** `git show --stat HEAD | grep '|' | grep -c 'Cargo.toml'` → `13`.
- **Committed in:** N/A.

---

**Total deviations:** 2 (both verification-command corrections, zero code changes, zero scope creep)
**Impact on plan:** Neither deviation touched production code or test behavior. Both are documented so the plan's own verify/acceptance text can be corrected before reuse (the `cli::mod::tests::` typo in particular is a real trap — it makes the intended regression test silently vacuous).

## Issues Encountered

- `cargo test -p famp <filter>` (no `--lib` restriction) runs the filter across every target in the `famp` package — lib unit tests plus ~50 integration test binaries, each of which must be built and invoked even when the filter matches zero tests in that binary. Combined with a cold cache from the version bump invalidating every crate's fingerprint, this made the plan's literal Task 1 verify command take roughly 25 minutes end-to-end (versus the ~5s estimated in `12-VALIDATION.md`), and — per Deviation 1 — it never actually exercised the target test at all. `just lint` (full-workspace clippy, cold cache) separately took ~6m16s. Both were run to completion as good-faith attempts before the path-typo was discovered and the corrected, fast, non-vacuous command was substituted.
- CI's `test` (ubuntu/macos) jobs took the bulk of the ~5-minute live CI wait for Task 2; all other jobs (`fmt-check`, `clippy`, `audit`, `famp-canonical`, `famp-crypto`, `doc-test`, both `build` jobs, `smoke-test`) completed within the first ~90 seconds.

## User Setup Required

None - no external service configuration required. The deployed-binary staleness note in `12-CI-ATTESTATION.md` names optional reinstall commands for Ben's dogfooded machines, but redeployment is not required by this plan or by REL-05.

## Next Phase Readiness

- REL-05's bump half is closed: the workspace reads `1.0.0` everywhere, atomically, in one commit (`5edff41`), with zero residual `1.0.0-rc.1` and no `Cargo.lock` supply-chain delta.
- REL-03 is closed: `5edff41` has a live, fully-green, name-by-name-attested CI run (11/11 success), recorded with run IDs in `12-CI-ATTESTATION.md`.
- The §16 nine-item citation table is complete (items 1/2/3/4/5/7 cited to `11-VERIFICATION.md`; items 6/8/9 closed by 12-04/12-01/12-02) — this is the exact table plan 12-05's tag annotation is expected to reproduce.
- **No git tag was created** — tagging `v1.0.0` is explicitly plan 12-05's job, gated on Ben's confirmation, per this plan's own prohibition.
- Tag-candidate SHA for plan 12-05 to read: `5edff41835b9c8e6daa59a51efce549460d88e5b`.
- No blockers.

## Self-Check: PASSED

All modified files present on disk (verified via `git status --short` showing a clean tree after both commits); both task commits (`5edff41`, `29d987f`) found in `git log --oneline --all`; `.planning/phases/12-v1-0-0-release-gate/12-CI-ATTESTATION.md` exists and contains all four required headings.

---
*Phase: 12-v1-0-0-release-gate*
*Completed: 2026-07-29*

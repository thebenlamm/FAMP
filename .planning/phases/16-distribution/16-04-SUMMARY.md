---
phase: 16-distribution
plan: 04
subsystem: docs
tags: [distribution, cargo-dist, installer, docs, ci-gate, checksums]

requires:
  - phase: 16-distribution
    provides: "16-01 generated the three dist installers (famp-installer.sh, famp-gateway-installer.sh, famp-relay-installer.sh) at releases/latest/download/<name> and the dist-workspace.toml release matrix these docs describe"
provides:
  - "All four onboarding docs (README.md, docs/GETTING-STARTED.md, docs/GATEWAY-SETUP.md, docs/ONBOARDING.md) lead with the curl prebuilt-binary installer; every surviving from-source fallback is a working cargo install --path/--git form"
  - "docs/DISTRIBUTION.md — maintainer release doc (dist tooling, release procedure, platform matrix, DOC-07 boundary, named follow-ups)"
  - "crates/famp/tests/install_docs_accuracy.rs — the compiled DIST-04 accuracy gate (3 tests)"
  - ".github/workflows/install-docs-gate.yml — additive CI workflow proven to fire red on a docs-only regression"
affects: [16-05, phase-20-doc-07, distribution]

actuals:
  tokens: 7800
  tasks: 3
  commits: 3

tech-stack:
  added: []
  patterns:
    - "Table-driven doc-accuracy test across N docs (extends gateway_setup_doc_accuracy.rs's single-file pattern to a Doc{label,rel_path} slice)"
    - "Clause-count vs full-sentence-count comparison to mechanically detect a locked-wording paraphrase, not just its absence"

key-files:
  created:
    - docs/DISTRIBUTION.md
    - crates/famp/tests/install_docs_accuracy.rs
    - .github/workflows/install-docs-gate.yml
  modified:
    - README.md
    - docs/GETTING-STARTED.md
    - docs/GATEWAY-SETUP.md
    - docs/ONBOARDING.md
    - crates/famp/tests/readme_line_count_gate.rs

key-decisions:
  - "readme_line_count_gate.rs's D-11 gate hard-asserted the README Quick Start fence contains literal `cargo install famp` — a direct, pre-existing conflict with this plan's own goal. Updated its assertion to check for the installer URL instead (Rule 3, blocking)."
  - "docs/DISTRIBUTION.md's checksum paragraph was first written as a plural paraphrase of D-06 ('they do not, by themselves, prove...'); tightened to D-06's exact singular locked sentence so it passes install_docs_accuracy.rs's own clause-count-vs-full-sentence-count check, which treats any paraphrase as a locked-wording violation."
  - "GATEWAY-SETUP.md's install bullet documents both famp and famp-gateway installers (curl + --path fallback), not just famp — that doc's reader needs famp-gateway on PATH (D-02)."

requirements-completed: [DIST-02, DIST-04]

coverage:
  - id: D1
    description: "Every onboarding doc's first install instruction is the prebuilt-binary curl command; from-source appears only below it as fallback"
    requirement: "DIST-04"
    verification:
      - kind: integration
        ref: "crates/famp/tests/install_docs_accuracy.rs#binary_install_path_leads_every_onboarding_doc"
        status: pass
      - kind: other
        ref: "install-docs-gate check-run on head SHA 3441117 (green, total_count=13); deliberately-red proof run https://github.com/thebenlamm/FAMP/actions/runs/30786477525/job/91600850030 (naming docs/ONBOARDING.md)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Every from-source command printed in any doc actually runs today against this repo; no doc instructs installing an unpublished crate by bare name"
    requirement: "DIST-04"
    verification:
      - kind: integration
        ref: "crates/famp/tests/install_docs_accuracy.rs#from_source_fallback_command_is_a_working_form; manually ran `cargo install --path crates/famp-gateway --root /tmp/... --locked` end to end before documenting it"
        status: pass
    human_judgment: false
  - id: D3
    description: "The from-source fallback string documented in README is the same command smoke-test.yml runs, so the fallback can't rot undetected"
    requirement: "DIST-04"
    verification:
      - kind: integration
        ref: "crates/famp/tests/install_docs_accuracy.rs#from_source_fallback_command_is_a_working_form (cross-checks README against .github/workflows/smoke-test.yml); falsified by removing the command from smoke-test.yml and confirming red, then restored"
        status: pass
    human_judgment: false
  - id: D4
    description: "The checksum security claim is exactly D-06's locked wording, no doc claims more (authenticity/provenance/publisher identity)"
    requirement: "DIST-04"
    verification:
      - kind: integration
        ref: "crates/famp/tests/install_docs_accuracy.rs#checksum_security_claim_matches_the_locked_wording; falsified by injecting an 'verifies the publisher' overclaim into README and confirming red, then restored"
        status: pass
    human_judgment: false
  - id: D5
    description: "A docs-only commit that breaks any of the above turns a CI check red — install-docs-gate.yml fires on the paths-ignore'd commit shape ci.yml never sees"
    requirement: "DIST-02"
    verification:
      - kind: other
        ref: "scratch-branch push (docs/ONBOARDING.md ordering reverted) produced a RED install-docs-gate check-run naming the doc: https://github.com/thebenlamm/FAMP/actions/runs/30786477525/job/91600850030; head SHA 3441117 check-runs total_count=13 (not 0) with install-docs-gate=success"
        status: pass
    human_judgment: false
  - id: D6
    description: "docs/DISTRIBUTION.md gives a maintainer the release procedure, platform matrix, and the DOC-07 non-satisfaction boundary"
    requirement: "DIST-02"
    verification:
      - kind: other
        ref: "docs/DISTRIBUTION.md — contains DOC-07, Phase 20, aarch64, signing, check-installer-drift, check-release-artifact-source, ubuntu-22.04, and a 16-D08-EVIDENCE.md pointer, all asserted present by the test's checksum_security_claim_matches_the_locked_wording precondition check"
        status: pass
    human_judgment: false

duration: 35min
completed: 2026-08-03
status: complete
---

# Phase 16 Plan 04: Doc-Accuracy Gate Summary

**All four onboarding docs now lead with `curl | sh` (dist's prebuilt-binary installer) instead of the always-broken `cargo install famp`; every surviving from-source fallback is `cargo install --path crates/famp[-gateway]`, cross-checked byte-for-byte against `smoke-test.yml`'s own install step, and locked behind a compiled test that a real, deliberately-triggered CI run proved fires red on the exact docs-only commit shape `ci.yml` ignores.**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-08-03T05:05:34Z (Task 1 commit)
- **Completed:** 2026-08-03T05:16:54Z (Task 3 commit + CI verification)
- **Tasks:** 3/3
- **Files modified:** 8 (3 created, 5 modified)

## Accomplishments

- Replaced all seven `cargo install famp` sites across README.md, docs/GETTING-STARTED.md, docs/GATEWAY-SETUP.md, and docs/ONBOARDING.md with the dist-generated curl installer, positioned strictly before any surviving from-source mention in every doc (verified by byte-offset comparison, not just presence).
- GATEWAY-SETUP.md's install step now covers both `famp` and `famp-gateway` (curl + `--path` fallback for each) — that guide's reader needs the gateway binary on PATH, which the original single-binary instruction never provided.
- Added README's "Install (prebuilt binary — recommended)" section: `~/.cargo/bin` target, required-reading PATH warning, `download/<tag>` pinning form, fetch-then-read-before-running option, D-06's exact checksum claim, a "Supported platforms" note (aarch64-linux named as a follow-up, not shipped), and an Upgrading-section line for binary-path upgraders.
- Created `docs/DISTRIBUTION.md` — the maintainer release doc: dist tooling and the `just check-installer-drift`/`check-release-artifact-source` gates, the release procedure, the 3×3 platform matrix with D-08a's runner-pinning rationale, what the 16-05 CI container job proves vs. does not (explicitly not DOC-07), and the three named follow-ups (signing, Linux aarch64, crates.io) recorded as decisions.
- Built `crates/famp/tests/install_docs_accuracy.rs` (3 tests, table-driven across the four docs) and the additive `.github/workflows/install-docs-gate.yml`. Pushed a scratch branch with a deliberately reverted doc ordering and confirmed a real RED check-run naming the broken doc, then confirmed GREEN on the real head SHA with `total_count=13` (not the false-positive `0` this repo has been burned by before).

## Task Commits

1. **Task 1: Lead all four onboarding docs with the prebuilt-binary curl install** — `26fa2cc` (docs)
2. **Task 2: Maintainer release doc + D-06 checksum boundary** — `f24a226` (docs) — README's checksum/platforms/upgrading additions landed inside Task 1's commit since they're part of the same "Install" section edit; this commit is `docs/DISTRIBUTION.md` only.
3. **Task 3: Compiled doc-accuracy gate + additive CI workflow** — `3441117` (test) — also tightens `docs/DISTRIBUTION.md`'s checksum wording to the exact locked sentence and fixes `readme_line_count_gate.rs`'s conflicting D-11 assertion.

**Plan metadata:** (this commit)

## Files Created/Modified

- `README.md` — new "Install (prebuilt binary)" section; Quick Start step 2 and its compile-time comment fixed; "Build from Source" retitled as fallback; Upgrading section gained a binary-installer upgrade line
- `docs/GETTING-STARTED.md` — Step 2 replaced with the curl installer + a from-source fallback pointer
- `docs/GATEWAY-SETUP.md` — install bullet now covers `famp` + `famp-gateway`, curl-first with `--path` fallbacks
- `docs/ONBOARDING.md` — all four `cargo install famp` sites (install block, prose, Codex line, Grok line) replaced
- `docs/DISTRIBUTION.md` — new maintainer release doc
- `crates/famp/tests/install_docs_accuracy.rs` — new compiled gate, 3 tests
- `crates/famp/tests/readme_line_count_gate.rs` — D-11 assertion updated from literal `cargo install famp` to the installer URL
- `.github/workflows/install-docs-gate.yml` — new additive CI workflow

## Decisions Made

- See `key-decisions` in frontmatter: the `readme_line_count_gate.rs` fix, the DISTRIBUTION.md wording tightening, and the GATEWAY-SETUP.md dual-binary install bullet.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `readme_line_count_gate.rs`'s D-11 gate hard-asserted the literal string this plan removes**
- **Found during:** Task 1, before editing README's Quick Start fence
- **Issue:** A pre-existing test (not in this plan's `files_modified`) asserts the README Quick Start fence body contains `cargo install famp` verbatim — the exact line Task 1's action step requires replacing.
- **Fix:** Updated the assertion to check for `releases/latest/download/famp-installer.sh` instead, with a comment explaining the D-01/16-04 supersession of the earlier D-11 (`brew install famp` → `cargo install famp`) amendment.
- **Files modified:** `crates/famp/tests/readme_line_count_gate.rs`
- **Verification:** `cargo test -p famp --test readme_line_count_gate` — all 3 tests pass; fence body still exactly 12 lines (CC-09 cap unchanged).
- **Committed in:** `26fa2cc` (Task 1 commit)

**2. [Rule 1 - Bug] `docs/DISTRIBUTION.md`'s checksum paragraph was a paraphrase, not D-06's exact wording**
- **Found during:** Task 3, while writing the checksum-claim compiled test
- **Issue:** The Task 2 draft used "Checksums verify ... they do not, by themselves, prove ..." (plural) instead of D-06's exact singular locked sentence. The plan's must-have ("no doc claims more than D-06") reasonably extends to "no doc paraphrases D-06 either" — a paraphrase is how wording drifts into an overclaim over time.
- **Fix:** Rewrote the paragraph to D-06's exact sentence, then built the compiled test's `checksum_security_claim_matches_the_locked_wording` to mechanically enforce this going forward (clause-count vs. full-sentence-count comparison, not just presence).
- **Files modified:** `docs/DISTRIBUTION.md`
- **Verification:** `cargo test -p famp --test install_docs_accuracy checksum_security_claim_matches_the_locked_wording` passes.
- **Committed in:** `3441117` (Task 3 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking pre-existing-test conflict, 1 bug/wording-precision fix)
**Impact on plan:** Both fixes were necessary for the plan's own stated goals (D-11's old gate would have broken the moment Task 1 shipped; the DISTRIBUTION.md paraphrase would have violated the plan's own "no doc claims more than D-06" must-have). No scope creep — no new install target, no new claim, no new binary.

## Issues Encountered

**Falsification methodology bug (self-caught, not a plan deviation).** The first attempt to falsify `checksum_security_claim_matches_the_locked_wording` used `sed` to mutate README.md's checksum sentence — `sed` operates line-by-line and the sentence line-wraps across two lines in the markdown source, so the substitution silently matched nothing and the "falsification" was actually a no-op (exit 0, file byte-identical). Caught by comparing `git diff` before/after rather than trusting the exit code; redone with a `re.DOTALL`-equivalent whitespace-tolerant Python regex, which correctly forced the test red and then green on restore. Recorded per this repo's standing "a clean grep/negative isn't proof" incident pattern — worth flagging since it's the same failure class, not a new one.

**Completeness sweep beyond the plan's five `files_modified`.** Re-grepped the whole repo for `cargo install famp`, `crates.io`, `install famp`, and `releases/latest` after all edits. The only remaining `cargo install famp` occurrences are inside test source (asserting its absence) and one historical doc comment (`readme_line_count_gate.rs`'s explanatory comment). One historical file, `docs/superpowers/specs/2026-04-17-local-first-bus-design.md`, contains two `brew install famp` mentions from a frozen 2026-04-17 design-spec exit criterion — explicitly marked "Historical note" at the top of the file, predates even the D-11 `brew`→`cargo` amendment, and is not live install instruction. Left untouched, consistent with this repo's "historical docs trip verifier false-positives (reject, don't fix)" convention (see `project_docs_update_verifier_notes` memory).

## User Setup Required

None — no external service configuration required.

## Known Stubs

None. The curl installer URLs are real, dist-generated release-asset URLs (`releases/latest/download/<name>`) that will resolve once 16-05 publishes a tag — that non-resolution is explicitly in this plan's scope boundary, not a stub.

## Threat Flags

None — this plan closes threat surface (T-16-11, T-16-02, T-16-03, T-16-12 from the plan's own threat register) rather than introducing new surface.

## Next Phase Readiness

- 16-05 (no-Rust container install gate, version bump, human-gated pre-release tag) can now proceed: the docs it will make true by publishing a release are already written and gated.
- The scratch branch used for the deliberate-red proof (`scratch/install-docs-gate-regression-check`) had its remote ref deleted after the proof run completed. The local branch ref itself could not be deleted — `git branch -D` is blocked by this environment's destructive-git-command guard even for a self-created, unmerged scratch branch — so a stale local-only branch pointer remains in this checkout; it carries no unique commits reachable from `main` and has no effect on the repo, remote, or any other clone.
- No tag was created, no GitHub Release was published — both remain 16-05's explicit, human-gated step.

---
*Phase: 16-distribution*
*Completed: 2026-08-03*

## Self-Check: PASSED

All 8 created/modified files and 3 task commits (`26fa2cc`, `f24a226`, `3441117`) verified present on disk and in git log.

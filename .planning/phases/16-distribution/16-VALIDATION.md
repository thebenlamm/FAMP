---
phase: 16
slug: distribution
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-02
---

# Phase 16 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Seeded by `/gsd-plan-phase` from `16-RESEARCH.md` § Validation Architecture.
> The planner fills in the per-task rows; the framework/sampling rows below are settled.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo nextest` (existing) for Rust-level tests; `shellcheck` (existing, via `just check-shellcheck`) for the generated `install.sh`; GitHub Actions itself as the integration harness for the release pipeline |
| **Config file** | `justfile` (existing recipes) — a new recipe is needed to bring the installer under shellcheck coverage |
| **Quick run command** | `just check-shellcheck` (extended to cover the generated installer) |
| **Full suite command** | Tag-triggered `release.yml` run, plus the container-based DIST-02 proxy job |
| **Estimated runtime** | shellcheck ~seconds; full release pipeline ~minutes (matrix build across 3 targets) |

---

## Sampling Rate

- **After every task commit:** `just check-shellcheck` + `cargo nextest run` for any Rust test touched
- **Per PR touching `.github/workflows/release.yml` or `[workspace.metadata.dist]`:** the `release-gate`
  workflow — `just check-installer-drift` (drift, cheap) + `just check-release-artifact-source` +
  `just check-shellcheck`.
  **Correction made at plan time:** the container-based DIST-02 proxy job **cannot** run per-PR. It curls
  the installer from a real GitHub Release asset, and no release exists until a tag is pushed. It is wired
  as a `dist` post-announce job inside the tag-triggered release run instead (16-05), which is the earliest
  moment the real curl path exists to exercise. Running it per-PR would require either a synthetic
  release or a softened assertion, and both would make it stop testing the thing it exists to test.
- **Per PR touching `README.md` or `docs/**`:** the `install-docs-gate` workflow —
  `cargo test -p famp --test install_docs_accuracy`. This is a separate additive workflow because
  `ci.yml` `paths-ignore`s `docs/**` and `**/*.md`, so a docs-only commit produces **zero** check-runs
  under `ci.yml`; `total_count == 0` is blocking, never a pass.
- **Per tag push:** the full release workflow — this *is* the gate for DIST-01/03/05, which only have
  meaning at tag-push time
- **Before `/gsd-verify-work`:** a real pre-release tag push exercising the full pipeline end-to-end
- **Max feedback latency:** seconds for the shellcheck/drift gates; one pipeline run for the tag-time gates

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 16-01 T1 (tracer) | 16-01 | 1 | DIST-01 | — | D-08 decided from a real arm64-macOS build log, not from reading; one binary flows config → generated workflow → named artifact | integration (CI probe + dry run) | `gh run list --workflow=darwin-cross-probe.yml --json conclusion` terminal; `dist plan --tag=v1.0.0 --output-format=json` names `famp-x86_64-apple-darwin` | ❌ W0 | ⬜ pending |
| 16-01 T2 | 16-01 | 1 | DIST-01, DIST-03 | T-16-01 | All 3 apps × 3 targets enumerated with a checksum each; Linux image pinned to `ubuntu-22.04` | integration (dry run) + shellcheck | `dist plan --tag=v1.0.0 --output-format=json` names all 9 archives + `.sha256` + 3 installers; `shellcheck crates/famp/tests/fixtures/installers/famp-installer.sh` | ❌ W0 | ⬜ pending |
| 16-02 T1 | 16-02 | 2 | DIST-01 | T-16-06 | Generated files cannot silently diverge from `[workspace.metadata.dist]`; installers are shellcheck-clean | drift gate | `just check-shellcheck`; `just check-installer-drift` (0 clean, non-zero on a fabricated hand edit, non-zero with `dist` off PATH) | ❌ W0 | ⬜ pending |
| 16-02 T2 | 16-02 | 2 | DIST-05 | T-16-05 | No workflow other than `release.yml` can create a release asset | structural CI-config gate, non-vacuity-probed | `bash scripts/release-artifact-source-gate.sh` (0 clean; non-zero + names the file on a fabricated `gh release upload`); `just check-release-artifact-source` in `just ci` | ❌ W0 | ⬜ pending |
| 16-02 T3 | 16-02 | 2 | DIST-01, DIST-05 | T-16-06 | Both gates above run in CI on the commit shapes that could break them | integration (CI) | `gh api repos/thebenlamm/FAMP/commits/<sha>/check-runs` shows a green `release-gate`; `total_count == 0` is BLOCKING | ❌ W0 | ⬜ pending |
| 16-03 T1 | 16-03 | 2 | DIST-03 | T-16-10 | The shipped installer runs hermetically against fabricated artifacts; `CARGO_HOME`/`HOME` redirected so the real `~/.cargo/bin` is untouched | harness | `cargo test -p famp --test installer_checksum_gate`; `shasum -a 256 ~/.cargo/bin/famp` unchanged before/after | ❌ W0 | ⬜ pending |
| 16-03 T2 | 16-03 | 2 | DIST-03 | T-16-01 | **Falsification pair.** MUST-FAIL under removal: `installer_rejects_a_corrupted_artifact_without_installing` (non-zero exit **and** empty install root — fails closed). MUST-STILL-PASS under removal: `installer_accepts_a_matching_artifact`. Attribution proven by running a checksum-stripped installer copy against the same corrupt artifact and asserting it exits 0. | falsification pair + control | `cargo nextest run -p famp --profile ci -E 'binary(installer_checksum_gate)'` | ❌ W0 | ⬜ pending |
| 16-04 T1 | 16-04 | 2 | DIST-04 | T-16-11 | Binary install command precedes any from-source command in all four onboarding docs; no doc names this project's crate bare against crates.io | doc edit | byte-offset comparison in `install_docs_accuracy.rs` (gated by 16-04 T3) | ❌ W0 | ⬜ pending |
| 16-04 T2 | 16-04 | 2 | DIST-02, DIST-04 | T-16-02 | Checksum claim is exactly D-06's locked sentence and no stronger; the three non-goals are recorded as decisions | doc edit | whitespace-normalized substring assertion in `install_docs_accuracy.rs` | ❌ W0 | ⬜ pending |
| 16-04 T3 | 16-04 | 2 | DIST-04 | T-16-12 | A docs-only regression fails CI instead of shipping — `ci.yml` `paths-ignore`s `docs/**` and `**/*.md`, so this needs its own additive workflow | doc-accuracy (compiled test) + CI | `cargo test -p famp --test install_docs_accuracy`; `gh api .../check-runs` shows a green `install-docs-gate`; `total_count == 0` is BLOCKING | ❌ W0 | ⬜ pending |
| 16-05 T1 | 16-05 | 3 | DIST-02 | T-16-15 | A container with no Rust toolchain installs and runs both binaries; the job asserts `cargo`/`rustc` are absent rather than assuming it | integration (container) | `install-gate` job inside the tag-triggered `release.yml` run | ❌ W0 | ⬜ pending |
| 16-05 T2 | 16-05 | 3 | DIST-01 | T-16-13 | Version bump carries every literal pinned to the old version; the guarding test survives intact | unit + full local gate | `cargo nextest run -p famp --profile ci -E 'test(version_strings_unified)'` (must report **1** test run, not 0); `just ci` | ✅ exists | ⬜ pending |
| 16-05 T3 | 16-05 | 3 | — | — | The one-way public act is gated behind an explicit human decision | `checkpoint:decision` (blocking) | n/a — human gate | n/a | ⬜ pending |
| 16-05 T4 | 16-05 | 3 | DIST-01, DIST-02, DIST-05 | T-16-14 | The published Release actually carries 9 archives + 9 checksums + 3 installers + `dist-manifest.json`; verified by asset list and `install-gate` log, never by a job name | integration (real tag) | `gh release view v1.1.0-rc.1 --json assets`; `gh run list --workflow=release.yml --json conclusion` | ❌ W0 | ⬜ pending |
| 16-05 T5 | 16-05 | 3 | DIST-02 (macOS leg) | T-16-04 | No Gatekeeper prompt on a curl-installed unsigned binary; binaries land in `~/.cargo/bin` | `checkpoint:human-verify` (blocking) | n/a — see Manual-Only Verifications below | n/a | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

**DIST-03 must be a falsification pair, not a single test.** Per the standing project rule
([[feedback_falsification_needs_a_control]] / Phase 14's precedent): name one test that MUST FAIL when
checksum verification is removed, and one that MUST STILL PASS under the same removal. Green under both
states is zero information.

---

## Wave 0 Requirements

- [ ] `[workspace.metadata.dist]` block in root `Cargo.toml` — does not exist yet
- [ ] `.github/workflows/release.yml` — does not exist yet (generated by `dist`, not hand-written)
- [ ] Container-based DIST-02 proxy job (new; additive workflow file preferred over editing `ci.yml`)
- [ ] Checksum-corruption falsification pair for DIST-03
- [ ] Doc-accuracy compiled test for DIST-04
- [ ] Fix so the *literal* documented fallback command is the one CI actually tests — today
      `smoke-test.yml` exercises `cargo install --path`, while the docs tell users `cargo install famp`,
      which is why the broken command survived undetected (research Pitfall 1)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| macOS Gatekeeper does not block a curl-installed unsigned binary | DIST-02 (macOS leg) | A Linux container cannot exercise Gatekeeper at all. GitHub's `macos-latest` runners are fresh per job and are the closest automated proxy, but a real fresh shell profile and OS-level prompts are not reproducible in CI. | Install via the curl command on a macOS machine with no prior `~/.cargo`/`~/.famp` state; confirm `famp --version` runs without a Gatekeeper prompt |
| Fresh-machine, no-prior-FAMP-state guide validation | DOC-07 (**Phase 20, NOT this phase**) | Requires a genuinely fresh machine and an unassisted human follower | Deferred to Phase 20. **The container proxy job in this phase does not satisfy DOC-07** — do not mark it so. |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies (the two `checkpoint:*` tasks in 16-05 are human gates by design)
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references — every ❌ W0 row above is created by the plan named in its Task ID column
- [x] No watch-mode flags
- [x] DIST-03's falsification pair has both a must-fail and a must-pass named — `installer_rejects_a_corrupted_artifact_without_installing` (MUST FAIL under removal) and `installer_accepts_a_matching_artifact` (MUST STILL PASS under removal), in 16-03 T2
- [ ] `nyquist_compliant: true` set in frontmatter — set by `/gsd-validate-phase` after execution

**Approval:** plan-time rows filled 2026-08-02; status column tracked during execution.

## Additional Wave 0 gap found at plan time (not in the RESEARCH list)

- [ ] **A workspace version bump is a hard prerequisite to any tag-triggered release.** `dist` derives the
      tag from `[workspace.package] version`, which is `1.0.0`, and tag `v1.0.0` already exists. Bumping it
      trips `crates/famp/src/cli/mod.rs`'s `version_strings_unified` test, which pins
      `env!("CARGO_PKG_VERSION")` to the literal `"1.0.0"` and asserts `BANNER_ABOUT` contains it. Handled
      as 16-05 Task 2; the test is updated, never weakened or deleted.

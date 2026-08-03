---
phase: 16-distribution
verified: 2026-08-03T18:00:00Z
status: passed
score: 4/4 roadmap truths verified
behavior_unverified: 0
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 3/4 roadmap truths cleanly verified; 1 truth verified-with-flagged-defect
  gaps_closed:
    - "Onboarding docs lead with the binary install path (DIST-04, ROADMAP SC4) — the /latest/download/ URL every doc leads with now returns HTTP 200 (release un-prereleased), and a new fail-closed gate (scripts/check-doc-release-urls.sh) is wired into CI to prevent regression."
  gaps_remaining: []
  regressions: []
---

# Phase 16: Distribution — Verification Report (Re-verification)

**Phase Goal:** A second person with no Rust toolchain installs a working `famp` from a
published release artifact on a clean machine — closing the gap where the only install
path required a full Rust toolchain and compiling 15 crates.
**Verified:** 2026-08-03T18:00:00Z
**Status:** passed
**Re-verification:** Yes — after gap closure (single gap, DIST-04 broken install URL)

## Scope of This Pass

Per the orchestrator's instruction, this pass re-verifies **only** the single gap
recorded in the prior VERIFICATION.md (DIST-04 / ROADMAP SC4: onboarding docs' primary
install command 404s live). DIST-01, DIST-02, DIST-03, and DIST-05 were **not**
re-verified from scratch — the prior pass verified those independently and nothing in
this change touches them. All commands below were re-run live against the current
codebase and live GitHub state, not taken from SUMMARY.md claims.

## Gap Re-verification

### Claim 1 — GitHub Release `v1.1.0-rc.1` edited to non-prerelease and set as `latest`

**Verified independently:**

```
$ gh api repos/thebenlamm/FAMP/releases/tags/v1.1.0-rc.1 --jq '{tag_name, prerelease, draft}'
{"draft":false,"prerelease":false,"tag_name":"v1.1.0-rc.1"}
$ gh api repos/thebenlamm/FAMP/releases/latest --jq '{tag_name, prerelease}'
{"prerelease":false,"tag_name":"v1.1.0-rc.1"}
```

`prerelease=false` and it is the repo's `/releases/latest`. Confirmed the URL now
resolves:

```
$ curl -sIL -o /dev/null -w '%{http_code}\n' \
    https://github.com/thebenlamm/FAMP/releases/latest/download/famp-installer.sh
200
```

**VERIFIED.**

### Claim 2 — `scripts/check-doc-release-urls.sh` added, discriminating, fails closed

Read the script in full (87 lines). Ran it live against the real repo — 10 URLs
checked, 1 template correctly skipped, 0 failures, exit 0.

**Adversarial test — proved it can actually go red, not just narrated:** copied
README.md + docs/*.md into a scratch dir, corrupted one filename to a nonexistent
asset, re-ran the script unmodified:

```
FAIL 404  README.md:113  .../famp-installer-DOES-NOT-EXIST.sh
FAIL 404  README.md:255  .../famp-installer-DOES-NOT-EXIST.sh
-- checked 10 URL(s), skipped 1 template(s), 2 failure(s) --
ERROR: at least one documented release URL does not resolve.
EXIT: 1
```

The gate is genuinely discriminating — not vacuously green.

**Fail-closed on zero-URL extraction, also independently proven:** with all URLs
removed from a scratch README/docs set, the script exits 1 with "the extraction regex
matched nothing, which almost certainly means this gate is silently vacuous" rather
than a silent pass.

**Template-skip is not a hole for real broken URLs:** the skip predicate is
`grep -q '<[^>]*>'` — it only matches literal `<tag>`-style placeholder syntax. The
adversarial test above used a concrete (but broken) filename, which was correctly
NOT skipped and correctly failed. A real, live broken URL cannot be mistaken for a
template.

**VERIFIED.**

### Claim 3 — Wired into CI and `just`, correctly excluded from `just ci`

- `.github/workflows/install-docs-gate.yml` triggers on `push`/`pull_request` with
  `paths: README.md, docs/**, ...` — this fires on a **docs-only** commit (the exact
  dark commit-shape `ci.yml`'s `paths-ignore` misses). Confirmed by reading the
  workflow file directly.
- `just check-doc-release-urls` exists (justfile:291-292) and calls the script.
- `just ci` (justfile:301) does **not** include it — confirmed by grep; matches the
  stated rationale (network calls, CI must stay offline-runnable).
- **Confirmed a real CI run actually executed the new step, not just in theory.**
  Queried `gh run list --workflow=install-docs-gate.yml`: a real push-triggered run
  (`30837906967`, 2026-08-03T17:41:48Z, `main`, conclusion `success`) exists, and its
  log contains the "Documented release URLs resolve" step output — the same 10-OK/
  1-skip/0-fail result reproduced above, run live in GitHub Actions, not just locally.

**VERIFIED — wired, and proven to fire in a real CI run.**

### Claim 4 — `check-shellcheck` widened to `scripts/*.sh`, glob fail-closed, SC2016 annotations genuine

- Read justfile:119-140: explicit file list replaced with `scripts/*.sh` glob via
  `shopt -s nullglob`; fails closed (`exit 1`) if the glob matches zero files, with an
  explicit "this check has gone vacuous" message.
- Ran `just check-shellcheck` live: all 6 files in `scripts/` (including the new
  `check-doc-release-urls.sh`) pass clean, 0 findings.
- Found and read both `# shellcheck disable=SC2016` annotations:
  - `scripts/gen-plugin.sh:158` — a `sed` that must emit a **literal**
    `${CLAUDE_PLUGIN_ROOT}` into a generated hook file so it expands at hook runtime,
    not at generation time. Single quotes are correct; SC2016 is a genuine false
    positive.
  - `scripts/spec-lint.sh:45` — a ripgrep regex (not a shell string) containing
    literal backticks around `` `to` ``; single quotes required to pass the pattern to
    `rg` unexpanded. Genuine false positive.
  - Confirmed no `.shellcheckrc` or other global relaxation exists — `grep -rn
    "shellcheck disable"` across `scripts/*.sh` and `crates/famp/assets/*.sh` shows
    only these two new per-line disables plus two pre-existing, unrelated ones
    (`SC2254`, `SC2064`). Nothing was globally relaxed.

**VERIFIED.**

## New Finding (informational, not a blocker) — git tag annotation is now stale

The pushed **annotated git tag** `v1.1.0-rc.1` (immutable, separate object from the
GitHub Release's `prerelease` API flag) reads, verbatim:

```
$ git for-each-ref refs/tags/v1.1.0-rc.1 --format='%(contents)'
v1.1.0-rc.1 — first dist-produced release

Pre-release. v1.1 (Open-Internet Federation) is NOT shipped: phases 18
(pairing), 19 (auto-wake gate), and 20 (human acceptance) remain open. ...
```

This literal "Pre-release." now **contradicts** the GitHub Release object, which was
edited to `prerelease: false` to close this gap and is now `/releases/latest`. The
substance of the sentence is still true (v1.1 the milestone genuinely hasn't shipped —
phases 18/19/20 remain open), but the standalone label is now inconsistent between the
two systems of record. Git tag annotations are immutable once pushed and fetched by
others — fixing this would mean deleting and recreating the tag, a separate decision
with its own blast radius, not something this verifier should do unilaterally.

Checked README.md, docs/*.md, and the dist-generated Release body for the same
contradiction — none found; only the git tag message carries it.

**Flagging plainly per instructions. Not a blocker on this gap's closure** (it does not
affect DIST-04's user-facing truth — the install command works — and it predates
nothing this fix broke; the fix is what created the label mismatch). Recommend either
leaving it (the git tag's prose is still substantively accurate) or a deliberate,
separate decision to retag if the "Pre-release." label matters going forward.

## Updated Observable Truths (ROADMAP Success Criteria)

| # | Truth (ROADMAP SC) | Status | Evidence |
|---|---|---|---|
| 1 | DIST-01/05 — tagged release publishes prebuilt binaries, sole tag-triggered producer | VERIFIED (unchanged, not re-verified this pass) | Carried forward from prior pass; nothing in this change touches it. |
| 2 | DIST-02 — single documented command installs with no Rust toolchain | VERIFIED (unchanged, not re-verified this pass) | Carried forward from prior pass. |
| 3 | DIST-03 — checksums verified, fail closed | VERIFIED (unchanged, not re-verified this pass) | Carried forward from prior pass. |
| 4 | DIST-04 — onboarding docs lead with a *working* binary install path | **VERIFIED (gap closed)** | All 10 documented `/latest/download/` URLs return live HTTP 200 (curl re-run above); the one tag-pinned occurrence still 302→200; a new fail-closed, CI-wired gate (`check-doc-release-urls.sh`) proven discriminating via live injected-failure test, preventing regression. `install_docs_accuracy.rs` re-run: 3/3 pass, no regression. |

**Score:** 4/4 roadmap truths verified.

### Gaps Summary

**No gaps remaining.** The single recorded gap — the primary documented install command
404ing live — is closed: the GitHub Release is no longer a pre-release, every
documented URL resolves (verified live, not from cache), and a new gate now makes this
regression class structurally impossible to reintroduce silently (proven to actually
fail on a broken URL, proven to fail closed on zero URLs, proven wired into a real CI
run on the exact docs-only commit shape that `ci.yml`'s `paths-ignore` would otherwise
miss).

One informational, non-blocking finding is disclosed above (stale "Pre-release." text
in the immutable git tag annotation, now inconsistent with the Release's `prerelease`
flag) — a developer judgment call, not a re-opened gap.

---

_Verified: 2026-08-03T18:00:00Z_
_Verifier: Claude (gsd-verifier)_

---
status: cataloged
phase: 16-distribution
reviewers: [zed-velocity-engineer, fable-5]
method: two independent cold reviews, diff-only briefs, no shared context
diff_scope: 15b2ea2..69dd238, 34 files, 1834 insertions (dist-generated output excluded)
created: 2026-08-03
---

# Phase 16 Post-Session Adversarial Review — Finding Catalog

Two reviewers, identical cold briefs, neither given the author's narrative. Every
finding below was re-verified by the orchestrator against real files, the published
release, or CI run logs before being cataloged. **Nothing has been fixed yet** —
per the standing audit rule, catalog first, execute second.

Ordering is by blast radius: security → false claims users act on → gates that
prove less than advertised → gates that never fire → inaccurate records → nits.

## Agreement map

| Finding | Zed | Fable-5 | Orchestrator re-verified |
|---|---|---|---|
| Shell injection in install-gate.yml | #1 HIGH | #1 HIGH | ✓ 18 quotes in live plan JSON |
| README checksum mechanism is false | #8 | #2 HIGH | ✓ 0 `.sha256` in published installer |
| Drift gate compares an unshipped artifact | #4 | — | ✓ byte-diff: glibc 2.31 vs 2.35, 0 vs 3 embedded checksums |
| Trigger paths leave gates unfired | #2 #3 #5 | #3 #5 | ✓ release-gate paths, ci.yml runs neither |
| DIST-05 gate is a denylist, overclaimed | #14 | #7 | not yet re-verified |
| URL-gate remedy contradicts docs test | — | #6 | not yet re-verified |
| Checksum-test control semantics overstated | #9 (sound) | #8 (inaccurate) | **reviewers disagree — see below** |
| `/tmp` fixed path, cli/mod.rs stale comment | #15 | #10 | not yet re-verified |

**The one genuine disagreement:** Zed calls `installer_checksum_gate.rs`'s falsification
pair "sound as written"; Fable-5 says the *advertised* control semantics don't literally
hold, because `strip_checksum_verification` panics if the call site is absent, so a
template-level removal fails BOTH tests rather than exhibiting control/falsification
asymmetry. These are compatible: both agree the test is fail-closed and cannot pass
vacuously. The defect is in the **claim**, not the code — and that claim is in a SUMMARY
the orchestrator wrote. Treat as a records-accuracy fix, not a test fix.

---

## P0 — Security

### F-01 · Shell injection in a job with `secrets: inherit`
`.github/workflows/install-gate.yml:71` — `PLAN='${{ inputs.plan }}'`

GitHub Actions substitutes the expression textually *before* the shell parses. The live
plan JSON contains **18 literal single quotes** (dist's `announcement_github_body`
embeds `curl --proto '=https' ...`). Even count today, so it survives by accident.
An apostrophe in a changelog entry ("doesn't") makes it odd and shatters the assignment;
a `$(...)` or `';` in an unquoted region executes with inherited secrets.

Both reviewers rated HIGH. Fable-5 adds: the `PLAN=` branch has **never run under the
current workflow** — the successful dispatch run 30823153868 exercised only the
`inputs.tag` branch. So this path is both dangerous and untested.

`inputs.tag` at lines 62–63 has the same shape (lower reach: requires dispatch access).

**Fix:** pass both via `env:` and read `"$PLAN"` / `"$TAG"` inside the script.

## P1 — False claims users act on

### F-02 · README states an installer mechanism that does not exist
`README.md:149-150`, softer at `docs/GETTING-STARTED.md`

> "The installer downloads a `.sha256` checksum alongside the release archive and
> verifies the downloaded bytes match"

The published `famp-installer.sh` contains **zero** `.sha256` references. Checksums are
baked into the script at generation time and compared inline. The follow-on threat
sentence therefore describes a file that is never fetched.

Fable-5's sharpest point: with baked checksums **the installer script itself is the sole
trust root**, which makes D-06's caveat *more* true than the README implies, not less.
`install_docs_accuracy.rs` locks only the D-06 conclusion sentence, so this false
mechanism sentence is permanently ungated.

Aggravating: the same session wrote the *correct* mechanism analysis in
`installer_checksum_gate.rs`'s module docs and the *wrong* sentence in README.

### F-03 · The drift gate validates an artifact nobody downloads
`Justfile` `check-installer-drift` uses `dist build --artifacts=global`, which builds no
targets. Byte-diff, committed fixture vs published installer:

| | fixture | published |
|---|---|---|
| glibc floor | `2.31` | `2.35` |
| embedded per-target checksum constants | **0** | **3** |

> Correction (2026-08-03): an earlier revision of this table said "1 vs 4". That was a
> grep artifact — `grep -c '_checksum_value='` also counts `local _checksum_value="$3"`,
> the function parameter inside `verify_checksum`. The real figures are 0 baked constants
> in the fixture vs 3 in the published installer (lines 229/243/257), which makes the gap
> *wider* than first reported, not narrower.

The gate passes by comparing global-generation to global-generation. Its success message
("no drift: release.yml and installer fixtures match") is true only of an artifact that
is never shipped.

**Consequence for the phase record:** `16-03`'s DIST-03 proof exercises the *fixture*,
not the shipped bytes. The claim "the shipped installer fails closed" is stronger than
what was proven. D-04's glibc-2.35 floor is asserted nowhere — README says 2.35, the
fixture says 2.31, no gate compares them.

## P2 — Gates that never fire

### F-04 · `release-gate.yml` trigger paths
Omit `.github/workflows/**` → a new workflow that uploads a release asset never trips the
DIST-05 sole-producer gate. Omit `Cargo.toml`/`Cargo.lock` → a version-bump-only commit
leaves all three fixtures stale with zero signal (this class already bit once this
session, commit `7f0af24`). Cover only one `scripts/` file → the widened
`check-shellcheck` never runs in CI for any other script. `ci.yml` runs neither
`check-release-artifact-source` nor `check-shellcheck`.

Note the irony: `check-shellcheck`'s own comment says "an explicit list silently stops
covering anything added later" — and the trigger one layer up is an explicit list.

### F-05 · URL gate cannot catch its own originating incident
`check-doc-release-urls.sh` runs only on docs-path commits. The 404 it memorializes was
caused by a **release-side** event (a prerelease flag), not a docs edit. Publishing,
deleting, or re-flagging a release can kill every documented install URL with no CI
signal until someone happens to touch a doc. Also `install-docs-gate.yml`'s paths omit
`scripts/check-doc-release-urls.sh` itself, so a breaking edit to the script doesn't run
the workflow that runs it. Needs a `schedule:` trigger and/or a post-announce hook.

### F-06 · `check-installer-drift` hardcodes three installers
The `cp` list names `famp`/`famp-gateway`/`famp-relay` explicitly. A fourth app in
`dist-workspace.toml` would emit a fourth installer that is never copied, never
fixtured, never shellchecked, never checksum-tested — and `git diff` would show nothing,
so the gate reports "no drift." Same explicit-list failure as F-04, fixed with a glob in
the sibling recipe and left as a list here.

### F-07 · Cross-gate contradiction
`check-doc-release-urls.sh`'s failure message advises "point the docs at a tag-pinned URL
until a non-prerelease exists." But `install_docs_accuracy.rs` hard-requires the literal
`releases/latest/download/famp-installer.sh` in every onboarding doc. **Following the
remedy turns the other gate red.** The remedy actually chosen (publish the RC with
`prerelease=false`) re-arms the trap for every future rc.

### F-08 · DIST-05 gate is a 5-pattern denylist sold as a guarantee
`scripts/release-artifact-source-gate.sh` scans only `.github/workflows/*.y{a,}ml` for
five known upload mechanisms. `gh api` uploads, third-party upload actions, or a `run:`
line calling a script under `scripts/` all pass. Zed adds: assertion 2 checks nesting by
proximity, not indentation, so `push: {branches: [main], tags: [...]}` passes — i.e.
publishing on every main push. The script's own comment is honest; `DISTRIBUTION.md`'s
"made mechanical" framing overclaims.

## P3 — Records that overstate what was proven

- `16-05-SUMMARY.md` claims the checksum test's control "MUST STILL PASS if checksum
  verification were removed." Per Fable-5, stripping the fixture's call site panics the
  helper and fails both tests. Fail-closed and loud, but not the advertised asymmetry.
- `16-VERIFICATION.md` marks DIST-03 verified against "the shipped installer" — F-03
  shows it was verified against the fixture.
- `docs/DISTRIBUTION.md` still says `--tag=v1.0.0` (recipe now derives it) and "No tag
  has been pushed and no GitHub Release has been published as part of Phase 16" — both
  false as of this session.
- `install-gate.yml`'s glibc framing: `debian:stable-slim` ships glibc ≥2.36, above the
  2.35 floor, so the job proves "runs on debian stable," not that the floor holds.

## P4 — Nits

- `Justfile` writes a fixed `/tmp/famp-dist-build-drift.json` (collision / symlink risk).
- `crates/famp/src/cli/mod.rs` comment cites a `contains(CARGO_PKG_VERSION)` assertion
  that does not exist; the invariant holds via two hardcoded literals instead.
- `release-gate.yml` fetches the shellcheck binary via unverified `curl | tar` — in a
  phase about checksum verification.
- `check-installer-drift` uses `git diff --exit-code` (unstaged only); staged drift passes.
- `check-doc-release-urls.sh` strips at most one trailing punctuation char.
- `ONBOARDING.md` `curl … | sh && famp install-codex` breaks on a fresh box where
  `~/.cargo/bin` isn't yet on PATH — the exact DOC-07 scenario.
- `famp-relay-installer.sh` ships as a release asset but is documented nowhere and is not
  exercised by `install-gate`.

## Cleared — do not re-litigate

Both reviewers independently checked and cleared: the `install_docs_accuracy.rs` ordering
logic; the tag-resolution grep against release.yml's compact JSON; `famp-gateway`'s bare
invocation emitting its usage banner; version-bump propagation across crate manifests;
`FixtureServer`'s port-0 binding; the `set -e`/`continue` interaction in the DIST-05
script; darwin-cross-probe's scoping.

One genuine miss to note: Fable-5 flagged `install_docs_accuracy.rs`'s regex as correct;
Zed found it uses `[^\n]*?` and therefore misses line-wrapped `cargo install` mentions.

**RE-VERIFIED — Zed is right, Fable-5's clearance was wrong.** Measured against the real
files:

| doc | `cargo install` mentions | regex matches |
|---|---|---|
| README.md | 7 | 4 |
| docs/GATEWAY-SETUP.md | 2 | 1 |
| **docs/ONBOARDING.md** | **1** | **0** |

The gate inspects **5 of 10** mentions. `ONBOARDING.md` — an onboarding doc, precisely
what the gate exists to police — is checked zero times, because its command wraps as
`cargo install --path\ncrates/famp`. Promote to **P2**: this is a gate that reports green
while covering half its surface. Fix by normalizing whitespace before matching (the
sibling checksum test already calls `normalize()`; this one does not).

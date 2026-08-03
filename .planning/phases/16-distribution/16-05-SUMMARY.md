---
phase: 16-distribution
plan: 05
subsystem: infra
tags: [dist, cargo-dist, release, github-releases, installer, gatekeeper, macos, container]

requires:
  - phase: 16-distribution
    provides: "16-01's dist 0.32 pipeline and dist-workspace.toml — the config this plan extends with post-announce-jobs and bumps the version in"
  - phase: 16-distribution
    provides: "16-02's check-installer-drift and release-gate — which this plan's version bump broke and then fixed"
  - phase: 16-distribution
    provides: "16-03's DIST-03 checksum proof and 16-04's install docs — the claims this release makes real"
provides:
  - "A published GitHub Release, v1.1.0-rc.1, carrying 9 archives + 9 checksums + 3 installers + dist-manifest.json"
  - ".github/workflows/install-gate.yml — a no-Rust container job proving DIST-02 against real published assets"
  - "Workspace bumped to 1.1.0-rc.1 across every pinned literal"
affects: [18-pairing, 20-human-acceptance, distribution, release-pipeline]

actuals:
  tokens: 9200
  tasks: 5
  commits: 4

tech-stack:
  added: []
  patterns:
    - "dist post-announce-jobs for gates that need assets only a real release produces"
    - "Version-derived (never hardcoded) tags in release-adjacent tooling"

key-files:
  created:
    - .github/workflows/install-gate.yml
  modified:
    - dist-workspace.toml
    - Cargo.toml
    - Cargo.lock
    - crates/famp/src/cli/mod.rs
    - justfile
    - crates/famp/tests/fixtures/installers/famp-installer.sh
    - crates/famp/tests/fixtures/installers/famp-gateway-installer.sh
    - crates/famp/tests/fixtures/installers/famp-relay-installer.sh

key-decisions:
  - "Tagged 1.1.0-rc.1, not 1.1.0. A pre-release exercises the whole pipeline for real without claiming v1.1 is shipped (phases 18/19/20/21 remain open) and leaves the v1.1.0 name free."
  - "The tag was cut by the orchestrator that received the human approval, never delegated to a subagent. A subagent cannot distinguish a faithful relay of consent from an inference, so the executor for Tasks 1-2 was explicitly forbidden from tagging."
  - "install-gate asserts famp-gateway EXECUTES and emits its usage banner rather than `--version`. famp-gateway is a daemon requiring --listen and has no --version flag; the banner assertion additionally proves glibc linkage, which is the real risk a no-Rust container exists to catch."
  - "check-installer-drift derives its tag from [workspace.package].version and fails closed if unparseable, rather than hardcoding one."

patterns-established:
  - "Release-adjacent tooling must never hardcode a version: it silently stops matching the workspace at the next bump and then fails for a reason unrelated to what it checks."
  - "For a daemon binary with no zero-exit flag, the liveness assertion is 'executes and emits its usage banner' — a stronger DIST-02 signal than --version."

requirements-completed: [DIST-01, DIST-02, DIST-05]

coverage:
  - id: D1
    description: "A real tag push produced 9 checksummed archives, 3 installers, and a dist-manifest on a real public GitHub Release — the pipeline proven by artifacts, not by dist plan"
    requirement: "DIST-01"
    verification:
      - kind: e2e
        ref: "gh release view v1.1.0-rc.1 --json assets — 9 binary archives, 9/9 paired .sha256, 3 *-installer.sh, dist-manifest.json, isPrerelease=true"
        status: pass
  - id: D2
    description: "A machine with no Rust toolchain runs one documented command and ends up with a working famp on PATH"
    requirement: "DIST-02"
    verification:
      - kind: e2e
        ref: ".github/workflows/install-gate.yml — debian:stable-slim container, no cargo/rustc (asserted), run 30823153868"
        status: pass
  - id: D3
    description: "The same install path works on real macOS without triggering a Gatekeeper unidentified-developer prompt"
    requirement: "DIST-02"
    verification:
      - kind: manual_procedural
        ref: "Task 5 human-verify, 2026-08-03 — which famp -> ~/.cargo/bin/famp; famp --version -> 1.1.0-rc.1; no com.apple.quarantine attribute; installed SHA differs from the pre-install local build, confirming the released artifact actually replaced it"
        status: pass
  - id: D4
    description: "Only the tag-triggered workflow produces release artifacts — no hand-built or hand-uploaded asset"
    requirement: "DIST-05"
    verification:
      - kind: integration
        ref: "scripts/release-artifact-source-gate.sh via .github/workflows/release-gate.yml (16-02); no gh release upload was run by hand at any point in this plan"
        status: pass
---

# 16-05: Release Proof — v1.1.0-rc.1

## What shipped

The first GitHub Release this project has ever published. Before this plan, `v1.0.0`
and earlier existed as **git tags only** — there was no release pipeline, so no Release
object and no downloadable artifact ever existed.

Release: https://github.com/thebenlamm/FAMP/releases/tag/v1.1.0-rc.1

| Asset class | Count | Verified |
|---|---|---|
| Binary archives (famp, famp-gateway, famp-relay × 3 targets) | 9 | ✓ |
| SHA-256 checksums, every archive paired | 9/9 | ✓ |
| Shell installers | 3 | ✓ |
| `dist-manifest.json` | 1 | ✓ |
| Marked pre-release | — | ✓ |

## Tasks

1. **`install-gate.yml`** — no-Rust `debian:stable-slim` container, wired as a
   `post-announce-jobs` entry in `dist-workspace.toml`. Runs post-announce because the
   real curl path needs assets that exist only after publication. Asserts the no-Rust
   premise (no `cargo`, no `rustc`) rather than assuming it.
2. **Version bump** 1.0.0 → 1.1.0-rc.1 across the workspace version, 13 intra-workspace
   path-dep pins, `BANNER_ABOUT`, the `version_strings_unified` literal, `Cargo.lock`,
   and the 3 regenerated installer fixtures.
3. **Human decision gate** — approved 2026-08-03 to push the public tag.
4. **Tag + Release verification** by asset, not assumption (table above).
5. **macOS human-verify** — passed (coverage D3).

## Two gates were failing for the wrong reason — both fixed, neither weakened

**`check-installer-drift` hardcoded `--tag=v1.0.0`.** Once the workspace version moved,
`dist build` hard-errored with "this workspace doesn't have anything for dist to
Release", turning 16-02's release-gate red for a reason unrelated to drift. Now derives
the tag from `[workspace.package].version` and fails closed if it cannot be parsed — an
empty tag must never degrade into a silent pass.

**`install-gate` asserted `famp-gateway --version`.** That flag does not exist;
`famp-gateway` is a daemon requiring `--listen` and exits non-zero without args. This
turned the real v1.1.0-rc.1 release run red while every artifact was good. Replaced with
two assertions that are strictly stronger:

- `famp --version` must exit 0 **and match the tag under test** — previously a stale
  `famp` already on PATH would have passed while proving nothing about this release.
- `famp-gateway` must execute and emit its usage banner — which proves it downloaded,
  is on PATH, is executable, and links against the container's glibc (D-04 pins the
  floor at 2.35 precisely for this). A wrong-arch or missing-symbol binary fails here
  rather than printing usage.

## Claim boundary — the container job does NOT satisfy DOC-07

Stated in the workflow header, in `docs/DISTRIBUTION.md`, and here. `install-gate` proves
the installer runs without Rust. It is not fresh-machine validation: a Linux container is
not a clean shell profile and cannot exercise macOS Gatekeeper at all. Task 5 covered the
macOS leg on **one already-used machine**; DOC-07 (previously-untouched machine) remains
open and owned by Phase 20.

## Known gap, filed at close — README's headline curl command 404s

`README.md:113` documents
`curl -fsSL https://github.com/thebenlamm/FAMP/releases/latest/download/famp-installer.sh | sh`.
That URL returns **404 today**: GitHub's `/releases/latest/` deliberately excludes
pre-releases, and `v1.1.0-rc.1` is the only Release that exists. Verified —
`gh release list` returns exactly one row, marked Pre-release.

This is self-correcting the moment a non-prerelease `v1.1.0` ships, but it is wrong now.
16-04's doc-accuracy gate checked install-path ordering and the from-source fallback, but
nothing asserts the documented curl URLs actually resolve — so the gate passes while the
headline command 404s. **Follow-up: extend the doc-accuracy gate to assert a 200 on every
documented release URL.** Task 5 used the tag-pinned form (`README.md:136`), which is
correct and unaffected.

## Self-Check: PASSED

- [x] Release published with 9 archives, 9 paired checksums, 3 installers, dist-manifest
- [x] No-Rust container installed from the real release and both binaries execute
- [x] macOS install verified: `~/.cargo/bin`, correct version, no quarantine attribute, no prompt
- [x] Installed SHA differs from the pre-install local build (a false pass was ruled out)
- [x] Public tag gated behind explicit human approval, cut by the approval recipient
- [x] No artifact built or uploaded by hand (DIST-05)
- [x] `latest`-404 gap recorded rather than quietly left

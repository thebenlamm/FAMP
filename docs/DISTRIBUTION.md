# Distribution & Release Process (maintainer-facing)

This document covers how a FAMP release is actually cut: the tooling, what it generates and must
never hand-edit, the gates that keep the generated files honest, the shipped platform matrix, and
the named follow-ups that are deliberate non-goals rather than oversights. For the user-facing
install instructions, see [README.md's "Install (prebuilt binary)" section](../README.md#install-prebuilt-binary--recommended).

---

## Tooling: `dist` (cargo-dist) 0.32, pinned

`dist` 0.32.0 (pinned, `--locked`) generates three things as one unit from `dist-workspace.toml`:

- `.github/workflows/release.yml` — the tag-triggered release workflow (build matrix, checksum
  generation, GitHub Release upload).
- The three shell installers (`famp-installer.sh`, `famp-gateway-installer.sh`,
  `famp-relay-installer.sh`), committed as CI fixtures under
  `crates/famp/tests/fixtures/installers/`.
- The checksum (`.sha256`) generation step embedded in `release.yml`.

**None of these three are ever hand-edited.** They are generated output. If you need to change
the release matrix, the installer's PATH-handling behavior, or the checksum step, edit
`dist-workspace.toml` (and `[profile.dist]` in the root `Cargo.toml`, dist's required build
profile companion) and regenerate:

```bash
just check-installer-drift
```

This is the drift gate (16-02): it runs `dist generate --check`, then `dist generate` and
`dist build --artifacts=global` with a tag derived from `[workspace.package].version` in
`Cargo.toml` to regenerate `release.yml` and the three installer fixtures, copies the regenerated
fixtures over the committed ones, and asserts `git diff --exit-code` against all three — i.e. it
fails if the committed files have drifted from what the pinned `dist` version would generate
today. It requires `dist` on `PATH` and is not part of `just ci` (a release tool, not a baseline
local dependency); it runs in CI via `.github/workflows/release-gate.yml`.

The companion structural gate, `just check-release-artifact-source`
(`scripts/release-artifact-source-gate.sh`), checks `.github/workflows/*.yml` for five known
upload mechanisms to deter accidental manual uploads or separate upload scripts — catching common
patterns but not a complete guarantee (alternative mechanisms like `gh api` commands or
third-party upload actions are not caught). This one *is* part of `just ci` (bash + grep only, no
external tool dependency).

## Release procedure

1. Bump `version` under `[workspace.package]` in the root `Cargo.toml`. Every crate in the
   workspace inherits this via `version.workspace = true`, so this is the one place the version
   changes.
2. Run `just check-installer-drift` locally to confirm `release.yml` and the installer fixtures
   are still in sync with `dist-workspace.toml` at the new version. Commit any regenerated output.
3. Push a `v<version>` tag matching the bumped version exactly (e.g. `v1.0.1`) — this is what
   triggers `.github/workflows/release.yml`.
4. Confirm the triggered run on GitHub Actions completes successfully and the GitHub Release it
   creates carries all expected archives, `.sha256` files, and the three installer scripts.

A `v<version>` tag push and corresponding GitHub Release publication are the final steps of
phase-level release work — they are explicit, human-gated decision points, never automatic.

## Shipped platform matrix

Three binaries (`famp`, `famp-gateway`, `famp-relay`) across three targets:

| Target | Runner | Notes |
|---|---|---|
| `aarch64-apple-darwin` | `macos-14` | |
| `x86_64-apple-darwin` | `macos-14` (cross-compiled) | Pinned to the *same* runner as the aarch64 target via `[dist.github-custom-runners]`, rather than dist's own default (`macos-15-intel`, a runner class GitHub has published a sunset date for). See D-08a below. |
| `x86_64-unknown-linux-gnu` | `ubuntu-22.04` (pinned, not `ubuntu-latest`) | Pins the glibc floor (2.35) as a deliberate, stable choice rather than one that silently rises with each Ubuntu LTS bump (D-04). |

**D-08 -> D-08a:** the open question was whether `x86_64-apple-darwin` needs its own native Intel
runner, or whether it cross-compiles cleanly from an arm64 `macos-latest` runner (the `aws-lc-sys`
cmake cross-toolchain step was the specific risk). Resolved from a real build log, not from
reading: a single arm64 runner cross-builds all three binaries cleanly, no `aws-lc-sys`/cmake
failure observed. Full evidence (run URL, `uname -m`, `cmake --version`, conclusion):
[`16-D08-EVIDENCE.md`](../.planning/phases/16-distribution/16-D08-EVIDENCE.md).

## What the CI container install check proves — and what it does not

16-05 adds a CI job that installs `famp` inside a container with no Rust toolchain present, to
mechanically prove DIST-02's claim that the curl installer works without `cargo`/`rustc`. That is
**all** it proves. It does **not** satisfy **DOC-07**, Phase 20's fresh-machine, no-prior-FAMP-state
human acceptance validation:

- A Linux container cannot exercise macOS Gatekeeper at all — the curl-first install path was
  chosen specifically to avoid that surface, but a Linux-only CI job can't confirm it.
- A container image is not a fresh shell profile; it doesn't validate the PATH-warning UX a real
  first-time user hits.

DOC-07 belongs to **Phase 20** and remains open until a human runs the install on an actual fresh
machine.

## Named follow-ups (deliberate non-goals, not oversights)

- **Artifact signing** (minisign / cosign / Sigstore). A checksum verifies the download completed
  correctly and matches what the release workflow produced — it does not, by itself, prove the
  release workflow was not compromised. An attacker who could substitute the archive could
  substitute the checksum beside it. Signing is the real answer to that threat and is recorded as
  a follow-up, not shipped in this phase (D-06).
- **Linux aarch64** (`aarch64-unknown-linux-gnu`). Plausible for an ARM VPS or a Pi-hosted relay,
  but required by neither DIST-01's text nor Phase 20's acceptance event. Named as a follow-up
  rather than silently widened (D-05).
- **crates.io publication.** Explicitly rejected, not merely deferred (D-01). The rejected
  alternative — running `just publish-workspace` to make `cargo install famp` literally true —
  costs a manual 12-crate dependency-ordered publish and, more importantly, permanently claims the
  `famp` crate name on a public registry and commits the project to version-bump publishing
  discipline on every future release, or the docs go stale again by a different route. The docs
  were rewritten to lead with the prebuilt-binary path and fall back to a working `--path`/`--git`
  form instead.

---

*Phase: 16-distribution*

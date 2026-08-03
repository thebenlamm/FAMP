# Phase 16: Distribution - Research

**Researched:** 2026-08-02
**Domain:** Multi-platform Rust binary release engineering (GitHub Actions release workflow, cross-compilation, checksum-verified shell installer)
**Confidence:** MEDIUM — stack and pitfalls are well-grounded (HIGH), but two real design forks (crates.io publish story, cargo-dist vs hand-rolled) need an explicit planner/user decision; no CONTEXT.md exists to pre-resolve them.

## Summary

Today `famp` has **no working non-source install path**. The README's own
"Quick Start" (the primary, first-shown path) tells a user to run
`cargo install famp` — but `famp` **does not exist on crates.io**
(`crates.io/api/v1/crates/famp` → `"crate `famp` does not exist"`,
[VERIFIED: crates.io registry, curl'd this session]). The only path that
actually works today is `cargo install --path crates/famp` from a full
clone with a Rust toolchain, which is exactly the toolchain dependency this
phase exists to remove. `justfile`'s own `smoke-test` recipe (and
`.github/workflows/smoke-test.yml`) quietly test `--path`, not the literal
command README prints — so the broken command has never been caught by CI.
This is a pre-existing bug this phase's docs work must fix as a side
effect, independent of the new binary-install path.

The workspace produces **three** shipping binaries — `famp`, `famp-gateway`,
`famp-relay` — via three separate `[[bin]]` targets
[VERIFIED: crates/{famp,famp-gateway,famp-relay}/Cargo.toml, read this
session]. DIST-01's requirement text names only `famp`. But
`docs/GATEWAY-SETUP.md` (the guide Phase 20's second person follows for the
actual acceptance event) requires the peer to separately install and run
`famp-gateway` directly — `justfile`'s `install` recipe installs only
`famp`, and `install-gateway` is a distinct, non-bundled recipe
[VERIFIED: justfile, read this session]. **Shipping only `famp` binaries
under DIST-01 does not produce a working Phase 20 acceptance event** — this
is a real scope hole, not a hypothetical one, and is flagged loudly below.
`famp-relay` is Ben-operated infrastructure (the already-provisioned
Lightsail box) and is not needed by the second person's machine, so it is
reasonably out of scope for this phase.

The build itself has one load-bearing, verified landmine: despite the
workspace's own `rustls` dependency declaring `features = ["ring", ...]`,
the **actually resolved** crypto backend in `Cargo.lock` is `aws-lc-rs`
(`rustls`'s locked `dependencies = ["aws-lc-rs", "log", ...]`
[VERIFIED: Cargo.lock:2141-2147, read this session]) — because
`famp-transport-http`'s `rustls = "0.23"` declaration doesn't route through
the workspace-pinned feature set, and Cargo unifies features across the
whole build graph. `aws-lc-rs` requires **cmake and a C compiler** to build
(confirmed via `aws-lc-sys`'s locked build-dependencies on `cc` and `cmake`
[VERIFIED: Cargo.lock `cargo tree -p rustls -e features`, this session]).
This is a real, but survivable, cross-compilation cost — `aws-lc-rs`
officially supports cross-compiling to `x86_64-unknown-linux-musl` and
`aarch64-unknown-linux-musl` with pre-generated bindings
[CITED: aws.github.io/aws-lc-rs/requirements/linux], but it is exactly the
kind of landmine that silently breaks a naive `cargo build --target
x86_64-unknown-linux-musl` on a stock `ubuntu-latest` runner without a
matching C cross-toolchain.

**Primary recommendation:** Adopt `cargo-dist` (0.32.0, actively
maintained — updated 2026-05-22, `OK` verdict from the package-legitimacy
check [VERIFIED: crates.io registry + `gsd-tools query package-legitimacy`,
this session]) to generate the tag-triggered release workflow, the
checksummed shell installer, and the checksum files in one auditable,
version-controlled step — following the exact "generate + CI drift-check"
pattern this repo already uses for `plugins/` (`scripts/gen-plugin.sh` +
`plugin-check.yml`). Target `famp` **and** `famp-gateway` (not just `famp`)
for all three platforms; build Linux on `ubuntu-22.04` (not `ubuntu-latest`)
against glibc rather than musl, to sidestep both the `aws-lc-sys` musl
cross-toolchain risk and musl's `getaddrinfo` quirks — `famp-gateway` makes
outbound HTTPS calls, so DNS-resolution correctness matters here more than
maximum distro portability at this stage.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Binary build + cross-compilation | CI / Build tier (GitHub Actions) | — | Artifacts must be reproducible and tag-triggered only (DIST-05); belongs entirely in CI, never a developer's local machine |
| Artifact publishing (GH Release) | CI / Build tier | — | DIST-05 requires the tag-triggered workflow be the *only* producer — no human upload step |
| Checksum generation | CI / Build tier | — | Must be produced in the same trusted step that builds the artifact, not a separate manual step |
| Checksum verification | Client / Installer (shell script, runs on user's machine) | — | DIST-03 requires verification to happen *before* install, on the installing machine |
| Install-path resolution (PATH, existing binary detection) | Client / Installer | — | Local filesystem state only the target machine has |
| Docs ordering (binary-first) | Documentation / Onboarding tier | — | Pure content change, no runtime component |
| `famp-gateway` outbound HTTPS (post-install runtime concern, not build-time) | API / Backend tier (the gateway process) | Database/Storage n/a | Named here only because it drives the musl-vs-glibc DNS-resolution tradeoff in Fork B |

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DIST-01 | Tagged release publishes prebuilt `famp` binaries for macOS arm64/x86_64 + Linux x86_64 | Standard Stack (cargo-dist config), Architecture Patterns, **Scope hole flagged: `famp-gateway` must also ship** |
| DIST-02 | Single documented command installs on a machine with no Rust toolchain, proven clean-env | Code Examples (installer invocation), Common Pitfalls (PATH, arch detection), Validation Architecture (what a container job can/cannot prove) |
| DIST-03 | Artifacts carry checksums; installer verifies before install, fails closed | Fork D (what property this actually buys), Security Domain |
| DIST-04 | Docs lead with binary install; `cargo install famp` remains documented fallback | Summary (the fallback is currently broken — needs its own fix), Fork F |
| DIST-05 | Artifacts produced only by the tag-triggered workflow — no hand-built/manual uploads | Fork A (cargo-dist vs hand-rolled), Package Legitimacy Audit |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `cargo-dist` | 0.32.0 [VERIFIED: crates.io registry, `max_version`/`newest_version` both 0.32.0, updated 2026-05-22] | Generates the release GH Actions workflow, shell installer, checksums, and GH Release upload in one config-driven step | Purpose-built for exactly DIST-01/02/03/05; avoids hand-rolling a checksummed multi-platform installer (a "don't hand-roll" case per this project's own conventions) |
| `dtolnay/rust-toolchain@stable` | (Action, not a crate) | Rust toolchain setup in CI | Already the toolchain-install convention used by every job in `ci.yml`/`smoke-test.yml`/`nightly-full-corpus.yml` [VERIFIED: `.github/workflows/*.yml`, read this session] — a new release workflow should match it |
| `Swatinem/rust-cache@v2` | (Action) | Cargo build caching in CI | Used by every existing build/test job [VERIFIED: `.github/workflows/ci.yml`] |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `taiki-e/install-action@v2` | (Action, already in use) | Install prebuilt CLI tools in CI without a from-source build | Already used for `cargo-nextest`, `just`, `cargo-audit` [VERIFIED: `.github/workflows/ci.yml`]; can install `cargo-dist` itself the same way if not using cargo-dist's own bootstrap step |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| cargo-dist | Hand-rolled matrix job + `softprops/action-gh-release` + hand-written `install.sh` | More auditable line-by-line, no generator "magic" — but you must get OS/arch detection, PATH handling, and checksum verification right yourself (exactly Fork E's failure surface), and DIST-05's "only the tag workflow produces artifacts" invariant is self-enforced rather than a tool default |
| glibc (`x86_64-unknown-linux-gnu`) | `x86_64-unknown-linux-musl` static | musl avoids glibc version skew entirely, but combines two risks at once here: `aws-lc-sys` needs a matching musl C cross-toolchain (supported but historically fragile — recent cargo-zigbuild fixes needed [CITED: WebSearch, aws-lc-rs GitHub issues]), and musl's `getaddrinfo` behaves differently from glibc's NSS-based resolver, which matters because `famp-gateway` does outbound HTTPS |

**Installation (planner-facing, not yet executed):**
```bash
cargo install cargo-dist --locked   # or via dist-installer.sh per cargo-dist docs
dist init                            # interactive; writes [workspace.metadata.dist] + .github/workflows/release.yml
```

**Version verification:** `cargo-dist` verified via `curl -A "..." https://crates.io/api/v1/crates/cargo-dist` this session → `max_version: 0.32.0`, `updated_at: 2026-05-22`. Confirmed actively maintained (not archived) via web search of the `axodotdev/cargo-dist` GitHub repo — recent releases and open issues as of mid-2026 [CITED: WebSearch, github.com/axodotdev/cargo-dist].

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| cargo-dist | crates.io | ~4 yrs (created 2022-09-16) | 1,679/wk [VERIFIED: `gsd-tools query package-legitimacy`, this session] | github.com/axodotdev/cargo-dist | OK | Approved |

**Packages removed due to [SLOP] verdict:** none
**Packages flagged as suspicious [SUS]:** none

No new runtime (Rust) dependencies are being added to the workspace by this
phase — `cargo-dist` is a dev/CI-time tool, not a `Cargo.toml` dependency of
any shipping crate. If the planner instead chooses the hand-rolled path
(Fork A rejected option), `softprops/action-gh-release` (a GitHub Action,
not a Cargo package) would need its own pin-and-verify step at plan time —
GitHub Actions are outside this gate's ecosystem scope; verify its tag via
`gh` at plan/execute time instead.

## Architecture Patterns

### System Architecture Diagram

```
Developer                                    GitHub                              User's machine
   |                                            |                                       |
   | git tag v1.1.0 && git push --tags          |                                       |
   |------------------------------------------->|                                       |
   |                                    tag-triggered workflow fires                    |
   |                                    (release.yml, on: push tags: v*)                |
   |                                            |                                       |
   |                                   +--------v---------+                             |
   |                                   | build matrix job  |                             |
   |                                   | macos-latest(arm64)|                            |
   |                                   |  -> aarch64-apple-darwin (native)                |
   |                                   |  -> x86_64-apple-darwin  (cross, rustup target)  |
   |                                   | ubuntu-22.04       |                             |
   |                                   |  -> x86_64-unknown-linux-gnu (native)            |
   |                                   +--------+---------+                             |
   |                                            |                                       |
   |                                   +--------v---------+                             |
   |                                   | checksum + package |                            |
   |                                   | (sha256sum per     |                            |
   |                                   |  artifact + a       |                           |
   |                                   |  combined manifest) |                           |
   |                                   +--------+---------+                             |
   |                                            |                                       |
   |                                   +--------v---------+                             |
   |                                   | GH Release upload   |                           |
   |                                   | (artifacts + .sha256|                           |
   |                                   |  + install.sh)       |                          |
   |                                   +--------+---------+                             |
   |                                            |                                       |
   |                                            |         curl -fsSL <install.sh URL> | sh
   |                                            |<--------------------------------------|
   |                                            |    installer detects OS/arch,        |
   |                                            |    downloads matching artifact +      |
   |                                            |    checksum over HTTPS, verifies,     |
   |                                            |    installs to a PATH-visible dir     |
   |                                            |-------------------------------------->|
   |                                            |          famp --version works        |
```

### Recommended Project Structure
```
.github/workflows/
├── release.yml          # NEW — tag-triggered; generated + checked-in by `dist generate`
├── ci.yml                # unchanged
├── asset-gate.yml        # unchanged
├── plugin-check.yml       # unchanged — precedent for the generate+diff-gate pattern release.yml follows
├── smoke-test.yml        # should gain a "does README's documented command actually work" assertion
└── nightly-full-corpus.yml # unchanged
Cargo.toml                 # gains [workspace.metadata.dist] block
docs/
├── GETTING-STARTED.md    # DIST-04: binary install path leads
├── ONBOARDING.md          # DIST-04: binary install path leads
├── GATEWAY-SETUP.md       # must be updated to install famp-gateway via the new binary path too
└── DISTRIBUTION.md        # NEW (recommended) — documents the release process for maintainers
```

### Pattern 1: Generate-and-drift-check for CI workflow files
**What:** A script regenerates a derived artifact (here: `release.yml` +
`install.sh`) from a checked-in source of truth (`[workspace.metadata.dist]`
in `Cargo.toml`), and a CI job fails if the checked-in output has drifted
from a fresh regeneration.
**When to use:** Any time a workflow file itself is machine-generated,
to prevent silent hand-edits from diverging from the generator's config.
**Example (existing precedent in this repo):**
```yaml
# Source: .github/workflows/plugin-check.yml, read this session
- name: Regenerate and assert no drift (claude-code, codex, grok)
  run: |
    set -euo pipefail
    for host in claude-code codex grok; do
      bash scripts/gen-plugin.sh "$host"
      git diff --exit-code -- "plugins/$host/commands" "plugins/$host/hooks"
    done
```
`dist generate --check` (or the CI-native `dist plan` step) is cargo-dist's
direct equivalent — wire it the same way, as an additive workflow, not a
rewrite of `ci.yml`.

### Pattern 2: macOS multi-arch build from a single arm64 runner
**What:** `macos-latest` GitHub-hosted runners are Apple Silicon (arm64)
[CITED: WebSearch, github.blog/changelog macOS runner posts, 2025-2026].
Cross-compiling `x86_64-apple-darwin` from this runner works via Xcode's
universal `clang` toolchain plus `rustup target add x86_64-apple-darwin`;
a separate `macos-13` (Intel) runner is not required for a standard Rust
crate.
**When to use:** Building both macOS artifacts in one job avoids doubling
CI matrix size and avoids relying on the x86_64 macOS runner class, which
GitHub has begun sunsetting (x86_64 macOS runner support ends after the
macOS 15 image retires, "Fall 2027" per GitHub's own changelog
[CITED: WebSearch, github.blog macOS 13 runner closing-down post]).
**Caveat (flag for plan-time verification, not yet executed):** `aws-lc-sys`
invokes `cmake` in its build script. `cmake` cross-compiling on macOS for a
different `-arch` generally works via Xcode's toolchain, but this specific
combination (aws-lc-sys + cross `x86_64-apple-darwin` build from an arm64
host) should be smoke-tested early in Task 1 of the plan, not assumed —
mark this **[ASSUMED — verify at plan/execute time with a real cross build]**.

### Anti-Patterns to Avoid
- **Hand-writing OS/arch detection in the installer without testing all four
  combinations (macOS arm64/x86_64, Linux x86_64):** the classic install.sh
  bug is `uname -m` returning `arm64` on macOS but `aarch64` on Linux for
  the same logical architecture — a naive `case` statement silently installs
  the wrong artifact or fails with a cryptic download 404.
- **Rewriting `ci.yml`'s triggers to add release logic:** this repo's own
  precedent and its MEMORY entry explicitly prefer an additive new workflow
  file over touching `ci.yml`'s trigger surface.
- **Trusting `ubuntu-latest`'s glibc as a floor:** `ubuntu-latest` tracks
  the newest Ubuntu LTS image and its glibc version rises over time,
  silently raising the minimum glibc a built binary requires on user
  machines. Pin `ubuntu-22.04` explicitly for the release build job (not
  `ubuntu-latest`) so the glibc floor is a deliberate, stable choice.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Multi-platform checksummed shell installer | A custom `install.sh` with hand-rolled OS/arch detection, download, and checksum-verify logic | `cargo-dist`'s generated installer | Installer scripts are a well-known source of subtle bugs (PATH handling, partial-download races, arch-string mismatches) — a generator that thousands of Rust CLI projects already exercise catches classes of bugs a first-written script won't |
| Tag-triggered multi-OS build matrix + GH Release upload | Hand-rolled `actions/upload-release-asset` + manual checksum step | `cargo-dist`'s generated `release.yml` | DIST-05's "only the tag workflow produces artifacts" invariant is a first-class cargo-dist guarantee, not something to re-derive |

**Key insight:** This phase's entire surface area (build matrix, checksums,
installer, GH Release publish) is exactly what `cargo-dist` exists to own
as one auditable unit. Hand-rolling it here would re-derive a solved
problem under a hard requirement (DIST-05) that a generator satisfies by
construction.

## Runtime State Inventory

Not applicable — this is a net-new distribution mechanism, not a
rename/refactor/migration phase. No prior release artifacts exist to
migrate (`git tag -l` shows `v0.8.1`, `v0.8.1-federation-preserved`, `v0.9`,
`v1.0.0`, `v1.0.0-rc.1` — none of these tags have any GitHub Release binary
artifacts attached; no `.github/workflows/release*.yml` exists today
[VERIFIED: `ls .github/workflows/`, `grep -rln tags:` this session]).

## Common Pitfalls

### Pitfall 1: The "fallback" install command is already broken
**What goes wrong:** DIST-04 says `cargo install famp` "remains documented
only as the from-source fallback" — implying it currently works. It does
not: `famp` is not published to crates.io
[VERIFIED: crates.io API this session]. If Phase 16's docs work simply
reorders sections without fixing this, the from-source fallback becomes a
dead end for anyone who does have a Rust toolchain but hits a binary-install
failure (unsupported OS, corporate proxy blocking the curl, etc.).
**Why it happens:** The command was likely written aspirationally, before
`publish-workspace` was ever run, and no CI check exercises the literal
string in README's Quick Start (the `smoke-test.yml` job tests `cargo
install --path crates/famp`, a different command, not the one users see).
**How to avoid:** Either (a) actually run `just publish-workspace` (a
12-crate, manual, ordered `cargo publish` sequence per crate — not CI-run,
per its own comment) to make `cargo install famp` real, or (b) change the
documented fallback to `cargo install --path crates/famp` /
`cargo install --git https://github.com/thebenlamm/FAMP famp` and stop
claiming crates.io publication. This is Fork F below — it needs an explicit
decision, it cannot be silently "left as is."
**Warning signs:** Docs describe a command no test in CI actually runs.

### Pitfall 2: Shipping only `famp` leaves the acceptance event's binary unreachable
**What goes wrong:** DIST-01's literal text names only `famp`. Phase 20's
UAT-02 requires cross-host federation via `famp-gateway`, which
`docs/GATEWAY-SETUP.md` requires the operator to run as a **separate**
binary the operator must independently locate on `PATH`
(`$(which famp-gateway)`) [VERIFIED: `docs/GATEWAY-SETUP.md`, read this
session]. If the release workflow only publishes `famp`, the second person
still has no way to get `famp-gateway` without a Rust toolchain — the exact
gap this phase exists to close, just one binary short.
**Why it happens:** DIST-01 was likely drafted before the multi-binary
shape (`famp-gateway`, `famp-relay`) was fully accounted for, or under the
assumption that "famp" colloquially means "the whole product."
**How to avoid:** Ship `famp-gateway` as a release artifact alongside
`famp` in this phase, even though DIST-01's literal text doesn't name it —
flagged to the user/planner as a requirement-text gap to close, not a
silent scope addition.
**Warning signs:** A DOC-07 fresh-machine dry run of the binary install path
that only exercises `famp --version`, never the gateway flow.

### Pitfall 3: `aws-lc-sys`'s cmake dependency breaks a naive cross build
**What goes wrong:** A `cargo build --target x86_64-unknown-linux-musl` (or
any target lacking a matching C/cmake cross-toolchain) run on a stock
`ubuntu-latest` runner fails deep inside `aws-lc-sys`'s build script with an
opaque cmake/compiler error, not an obvious "wrong target" message.
**Why it happens:** The workspace's own `rustls` feature declaration
(`ring`) is overridden by feature unification once any other crate in the
graph (here: `axum-server`, `hyper-rustls`, `rustls-platform-verifier`, all
pulled in via `famp-transport-http`/`famp-gateway`/`famp`) depends on
`rustls` with default features — the actually-resolved backend is
`aws-lc-rs`, not `ring`, and this is invisible unless you read `Cargo.lock`.
**How to avoid:** For the recommended glibc/native-runner target choice
(Fork B), this landmine mostly disappears (native `cc`+`cmake` are already
present on both `ubuntu-22.04` and `macos-latest` runners). If musl is ever
revisited, budget explicit time for a matching musl C cross-toolchain (e.g.
via `cross`/Docker) rather than a bare `rustup target add`.
**Warning signs:** A build failure mentioning `cmake`, `aws-lc-sys`, or
`Could NOT find` deep in a target-specific build log.

## Code Examples

### Existing `just install`/`just install-gateway` recipes (source of truth for what "installed" means locally)
```makefile
# Source: justfile, read this session
install:
    cargo install --path crates/famp --locked --force
    famp install-claude-code

install-gateway:
    cargo install --path crates/famp-gateway --locked --force

install-all: install install-gateway
```
The release installer's *effective* end-state should match `install-all`'s
scope (both binaries on `PATH`), not `install`'s alone.

### Confirming the crate does not exist on crates.io (evidence for Pitfall 1)
```bash
# Source: this session, ran verbatim
$ curl -s -A "famp-research (benlamm25@gmail.com)" "https://crates.io/api/v1/crates/famp"
{"errors":[{"detail":"crate `famp` does not exist"}]}
```

### Confirming the resolved TLS crypto backend
```bash
# Source: this session, ran verbatim (excerpt)
$ cargo tree -p rustls -e features
rustls v0.23.38
├── aws-lc-rs v1.16.2
│   ├── aws-lc-sys v0.39.1
│   │   [build-dependencies]
│   │   ├── cc feature "default" ...
│   │   ├── cmake feature "default" ...
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|---------------|--------|
| `cargo install <crate>` as the only distribution path | Prebuilt binaries + checksummed shell installer, `cargo install` as fallback | Industry-wide norm for Rust CLIs since ~2021-2023 (ripgrep, uv, rustup follow this shape) | Removes the Rust-toolchain requirement from the critical path, which is this phase's entire goal |
| `ring` as rustls's default crypto provider | `aws-lc-rs` as rustls's default crypto provider (since rustls 0.23) | rustls 0.23 line | Adds a cmake/C-compiler build dependency that this workspace inherited transitively, not by direct choice |

**Deprecated/outdated:**
- Manually uploading release binaries via the GitHub web UI: explicitly
  forbidden by DIST-05 in this project regardless of general industry
  practice.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `aws-lc-sys` cross-compiles cleanly for `x86_64-apple-darwin` from an arm64 `macos-latest` runner without extra toolchain setup | Architecture Patterns, Pattern 2 | If wrong, Fork C's single-runner recommendation fails and a second (or containerized cross-compile) job is needed — costs CI time and complexity, not correctness |
| A2 | `cargo-dist 0.32.0`'s generated installer's PATH-handling and upgrade-detection behavior meets DIST-02/DIST-03 without heavy customization | Standard Stack, Fork A | If wrong, the "generate once, done" savings shrink and more custom installer logic is needed than budgeted |
| A3 | No corporate/CI secret or code-signing requirement blocks an unsigned binary from working via the curl path (no macOS notarization needed given the Gatekeeper/quarantine finding below) | Summary, hard constraint verification | If Apple changes curl's quarantine-exemption behavior in a future macOS release, the whole doc-ordering strategy (curl-first) breaks — verified true as of this session, but is Apple platform behavior that could change |
| A4 | `famp-relay` does not need to ship as a release artifact in this phase | Summary, Don't Hand-Roll | If a future acceptance-event variant needs a self-hosted relay operator who also lacks a Rust toolchain, this would need revisiting — currently Ben's own Lightsail box, built with his own toolchain |

**If this table is empty:** N/A — table is populated; see risks above before locking design forks A-F.

## Open Questions / Decision Forks

No CONTEXT.md exists for this phase — these forks are the primary input the
planner needs to resolve, either by user discussion or by the planner's own
locked judgment call, before writing PLAN.md.

### Fork A — Release tooling: `cargo-dist` vs hand-rolled matrix
**Recommendation: `cargo-dist` 0.32.0.**
- Actively maintained (updated 2026-05-22), `OK` package-legitimacy verdict.
- Satisfies DIST-05 by construction (tag-triggered, single producer of
  artifacts — no separate manual-upload code path exists).
- Generates checksums as part of the same build step (DIST-03).
- Matches this repo's existing "generate + CI drift-check" convention
  (`plugin-check.yml` precedent) — `dist generate --check` slots into the
  same pattern.
- **Rejected alternative cost (hand-rolled):** more auditable line-by-line,
  but the installer's OS/arch detection, PATH handling, and checksum
  verification all become code this project authors and must independently
  get right — exactly the failure class Fork E worries about. Higher
  ongoing maintenance surface for marginal auditability gain.

### Fork B — Linux target: gnu vs musl
**Recommendation: `x86_64-unknown-linux-gnu`, built on `ubuntu-22.04`
(pinned, not `ubuntu-latest`).**
- Avoids the `aws-lc-sys` + musl cross-toolchain risk entirely (native
  build on a glibc runner — `cc`/`cmake` already present).
- Avoids musl's `getaddrinfo` divergence from glibc's NSS-based resolver,
  which matters because `famp-gateway` makes outbound HTTPS calls (the
  relay-fetch loop, Phase 17).
- Pinning `ubuntu-22.04` (glibc 2.35) instead of `ubuntu-latest` keeps the
  glibc floor a deliberate, stable choice rather than silently rising with
  each Ubuntu LTS bump.
- **Rejected alternative cost (musl):** genuinely more portable across old
  distros with zero glibc-skew risk, but combines two real, verified risks
  (cross-toolchain fragility + DNS resolver divergence) for a benefit
  (running on very old/minimal distros) that isn't in this phase's stated
  audience (a "second person" installing FAMP, not an embedded/container
  deployment).
- **aarch64-linux flag:** DIST-01 names only x86_64 Linux. An ARM VPS or
  Raspberry Pi relay/second-machine is plausible per the phase's own
  framing, but is **not required** by DIST-01's literal text or by Phase
  20's acceptance event. Recommend leaving `aarch64-unknown-linux-gnu` as a
  documented follow-up, not silently adding it to this phase's scope.

### Fork C — macOS: one runner or two
**Recommendation: one `macos-latest` (arm64) runner, cross-compiling both
`aarch64-apple-darwin` (native) and `x86_64-apple-darwin` (cross via
`rustup target add`).**
- `macos-latest` is arm64 [CITED: WebSearch, GitHub changelog].
- Avoids relying on the x86_64 macOS runner class GitHub is sunsetting.
- **Caveat, not yet verified:** whether `aws-lc-sys`'s `cmake` invocation
  cross-compiles cleanly for `x86_64-apple-darwin` from this host should be
  smoke-tested as an early plan task (see Assumption A1), not assumed
  clean from research alone.
- **Rejected alternative cost (two runners, incl. `macos-13`):** doubles
  macOS CI minutes and matrix complexity for no benefit once cross-compile
  is confirmed working; `macos-13` (Intel) images are also on GitHub's own
  deprecation path.

### Fork D — Checksum verification's honest security claim
**What DIST-03 actually buys:** a checksum fetched from the same origin
(the GitHub Release, same trust domain as the binary itself) protects
against **corruption and truncated/partial downloads**. It does **not**
protect against a **compromised release host or a compromised GitHub
account pushing a malicious tag** — an attacker who can substitute the
binary can trivially substitute the checksum file alongside it. The honest
documentation claim is: *"verifies the download completed correctly and
matches what the release workflow produced — it does not, by itself, prove
the release workflow itself was not compromised."*
**Recommendation:** State this precisely in the installer docs; do not
imply checksum verification defends against a compromised release
pipeline. Signing (`minisign`/`cosign`/Sigstore, e.g. `cargo-dist`'s
built-in Sigstore/`cargo-dist`-signing support) is the real answer to that
threat and is a reasonable **named follow-up**, not required to satisfy
DIST-03's literal text ("fails closed on a corrupted or substituted
artifact" — "substituted" is satisfiable at the corruption/truncation
level; a fully compromised-pipeline threat model is a separate, larger
scope this project has not currently asked for).

### Fork E — Installer script mechanics
**Recommendation (cargo-dist defaults, verify at plan time):**
- **Where it lives:** a release asset (`install.sh`) fetched via a stable
  URL cargo-dist emits, not a raw `githubusercontent` path to a mutable
  branch — this keeps the installer version-pinned to the tag being
  installed.
- **Install target:** cargo-dist's shell installer defaults to
  `~/.cargo/bin` when a Cargo-managed environment is detected, otherwise a
  user-local bin dir (commonly `~/.local/bin` via a shimmed
  `CARGO_HOME`-style convention) — **this should be explicitly configured
  to prefer `~/.cargo/bin`** to match `just install`'s existing target
  [VERIFIED: `justfile install` recipe uses `cargo install --path
  crates/famp --locked --force`, which always installs to `~/.cargo/bin`
  unless `CARGO_INSTALL_ROOT`/`--root` overrides it]. A mismatch between
  the source-build target and the binary-install target would make "which
  famp am I running" genuinely ambiguous for contributors who do both.
- **PATH handling:** cargo-dist's generated installer prints an explicit
  warning + shell-profile snippet when the install dir isn't already on
  `PATH` — this is the single most common "installed but command not
  found" failure and must not be silently skipped.
- **OS/arch detection:** cargo-dist's installer handles `uname -s`/`uname
  -m` normalization (including the `arm64` vs `aarch64` naming mismatch
  between macOS and Linux) as part of its generated script — do not
  hand-write this detection separately.
- **Upgrade-over-existing / running daemon:** DIST-02's proof scope is
  explicitly a **clean environment with no prior FAMP state** — there is no
  running daemon to restart in that scenario. For a *future* upgrade
  scenario (out of this phase's proof requirement but worth a one-line doc
  note), the existing `famp daemon restart` command
  [VERIFIED: `crates/famp/src/cli/daemon/restart.rs`, read this session]
  is the correct post-upgrade step — the installer itself does not need to
  invoke it; docs should mention it for upgraders, matching the existing
  "Upgrading" section pattern already in README.md.

### Fork F — Fix the currently-broken `cargo install famp` fallback (new fork, not in the original prompt list, discovered this session)
**Recommendation:** Change every doc site currently reading
`cargo install famp` (`README.md:192`, `docs/GATEWAY-SETUP.md:24`,
`docs/GETTING-STARTED.md:43`, `docs/ONBOARDING.md:12,32,37`
[VERIFIED: grep across all four files, read this session]) to
`cargo install --path crates/famp` (from a clone) as the from-source
fallback, **unless** the user explicitly wants to also run
`just publish-workspace` (a manual, 12-crate, dependency-ordered
`cargo publish` sequence, ~9 minutes of sleeps per its own comments) to
make crates.io publication real. This is a genuine scope decision — actual
crates.io publication is a bigger, separate commitment (crate-name
ownership, ongoing version-bump publishing discipline) than fixing four
doc lines. Flag to the user; do not default to publishing without
confirmation.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain (build machine, i.e. CI) | Building all release artifacts | Yes (project standard) | 1.89.0 pinned via `rust-toolchain.toml` [VERIFIED, read this session] | — |
| `cmake` + C compiler (CI runners) | `aws-lc-sys` build step | Yes on `ubuntu-22.04`/`macos-latest` GitHub-hosted runners (preinstalled) | — | Native runners avoid needing a fallback (Fork B) |
| `cargo-dist` | Generating release.yml + installer | Not yet installed in this repo | 0.32.0 (verified on crates.io) | Hand-rolled matrix (Fork A rejected option) |
| `gh` CLI / GitHub Releases API | Publishing artifacts | Available in GitHub Actions by default (`GITHUB_TOKEN`) | — | — |
| A machine with **no** Rust toolchain, for DIST-02's proof | Manual/CI-simulated clean-environment validation | Not applicable to this research session — flagged for Validation Architecture below | — | Containerized job (see below); genuinely fresh physical/VM machine for the real DOC-07 gate |

**Missing dependencies with no fallback:** none blocking research; `cargo-dist` install is a plan-time task, not a blocker.

**Missing dependencies with fallback:** none.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo nextest` (existing) for any Rust-level unit tests; `shellcheck` (existing, via `just check-shellcheck`) for the new `install.sh`; GitHub Actions itself as the integration harness for the release pipeline |
| Config file | `justfile` (existing recipes); new recipe(s) needed for installer shellcheck coverage |
| Quick run command | `shellcheck <path-to-generated-install.sh>` (add to `just check-shellcheck`) |
| Full suite command | New `.github/workflows/release-dry-run.yml` (or a `workflow_dispatch`-triggered dry run of `release.yml` against a non-`v*` test tag) |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DIST-01 | Tag push produces macOS arm64/x86_64 + Linux x86_64 artifacts | integration (CI) | `dist plan` (cargo-dist's dry-run planner) or a `workflow_dispatch` test run of `release.yml` | ❌ Wave 0 — release.yml doesn't exist yet |
| DIST-02 | A no-Rust-toolchain machine installs a working `famp` | **partially automatable** — see note below | A Docker container job (`FROM debian:stable-slim`, no Rust preinstalled) running the installer + `famp --version` | ❌ Wave 0 |
| DIST-03 | Installer verifies checksum, fails closed on corruption | unit/integration | A test that deliberately corrupts a downloaded artifact byte and asserts the installer exits non-zero *before* installing | ❌ Wave 0 — needs a falsification-style test (corrupt-input must fail, valid-input must pass) |
| DIST-04 | Docs lead with binary install; `cargo install famp` documented as fallback only | doc-accuracy (existing pattern: `gateway_setup_doc_accuracy.rs`-style compiled test) | A test asserting the binary-install command appears before any `cargo install` string in the target docs | ❌ Wave 0 |
| DIST-05 | Artifacts produced only by tag-triggered workflow | structural/CI-config | A grep-based CI gate asserting `release.yml`'s only trigger is `push: tags: v*` (no `workflow_dispatch` artifact-upload path, or if present, gated to not actually publish) | ❌ Wave 0 |

**On DIST-02's automatability — the honest answer:** A container job
(`debian:stable-slim` or `alpine` with no Rust/cargo preinstalled) is a
**reasonable, cheap proxy** for "no Rust toolchain" and should absolutely
be wired into CI — it will catch the large majority of real regressions
(broken curl command, wrong artifact URL, checksum mismatch, PATH
misconfiguration inside a minimal shell). It is **not** a full substitute
for DOC-07's actual requirement ("a fresh machine with no prior FAMP
state"), because a container doesn't exercise real hardware quirks: macOS
Gatekeeper/quarantine behavior (Linux containers can't test this at all —
the macOS leg of DIST-02 needs a real or CI-hosted macOS runner with no
prior `~/.cargo`/`~/.famp` state, which GitHub's `macos-latest` runners
already provide fresh per-job), a genuinely fresh shell profile (PATH not
yet containing `~/.cargo/bin`), and OS-level firewall/Gatekeeper prompts.
**Recommendation:** wire the container job as the *automated, per-tag CI
gate* for DIST-02's mechanical claim (installer runs successfully with zero
Rust preinstalled), and treat DOC-07's fresh-*macOS*-machine validation
(Phase 20, not this phase) as the place where the harder, non-automatable
claim (Gatekeeper behavior, real fresh shell) gets its human-driven proof.
Do not claim this phase's CI gate alone satisfies DOC-07.

### Sampling Rate
- **Per tag push:** the full release workflow runs — this *is* the
  per-commit gate for DIST-01/03/05, since these requirements only have
  meaning at tag-push time.
- **Per PR touching `.github/workflows/release.yml` or `[workspace.metadata.dist]`:**
  `dist generate --check` (drift gate, cheap) + the container-based
  DIST-02 proxy job.
- **Phase gate:** a real tag push (even a `v1.1.0-rc.1`-style pre-release
  tag) exercising the full pipeline end-to-end before `/gsd-verify-work`.

### Wave 0 Gaps
- [ ] `[workspace.metadata.dist]` block in root `Cargo.toml` — does not exist yet
- [ ] `.github/workflows/release.yml` — does not exist yet (generated, not hand-written)
- [ ] A container-based DIST-02 proxy test (new, likely `.github/workflows/` or a `just` recipe)
- [ ] A checksum-corruption falsification test for DIST-03
- [ ] A doc-accuracy compiled test for DIST-04 (matches the existing `gateway_setup_doc_accuracy.rs` pattern already used for `GATEWAY-SETUP.md`)
- [ ] Fix to `smoke-test.yml`/`README.md` so the *literal* documented fallback command is the one actually tested (closes the Pitfall 1 gap)

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | No | Not applicable — installer has no auth concept |
| V3 Session Management | No | Not applicable |
| V4 Access Control | No | Not applicable |
| V5 Input Validation | Yes | Installer must validate downloaded artifact size/shape before executing (don't `chmod +x` and run an unverified partial download) |
| V6 Cryptography | Yes | SHA-256 checksums (integrity only, per Fork D) — do not hand-roll a checksum algorithm; use the standard `sha256sum`/`shasum -a 256` already available on macOS/Linux, or cargo-dist's built-in checksum generation |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| `curl \| sh` script substitution (MITM or DNS hijack serving a malicious install.sh) | Tampering | HTTPS-only URLs (already required — no `http://` fallback), and documenting that the script itself should be reviewable (`curl -fsSL <url> \| less` before piping to `sh`, mentioned as an option in docs, not enforced) |
| Corrupted/truncated download silently producing a broken binary | Tampering | DIST-03's checksum verification — the actual purpose this phase implements |
| Checksum-substitution by a compromised release host | Tampering, Spoofing | Out of scope for DIST-03's literal text per Fork D; named follow-up is artifact signing (Sigstore/minisign) |
| Installer writing to a directory the current user doesn't control (e.g. `/usr/local/bin` requiring `sudo`) silently prompting for a password in a piped `sh` context | Elevation of Privilege (accidental) | Default to a user-writable dir (`~/.cargo/bin`), never silently `sudo` |

## Sources

### Primary (HIGH confidence)
- `crates.io` registry API (`curl https://crates.io/api/v1/crates/{famp,cargo-dist}`) — ran this session, confirms `famp` unpublished and `cargo-dist` 0.32.0 current
- `gsd-tools query package-legitimacy check --ecosystem crates cargo-dist` — ran this session, `OK` verdict
- Repo files read directly this session: `Cargo.toml`, `Cargo.lock`, `justfile`, `.github/workflows/{ci,asset-gate,plugin-check,smoke-test,nightly-full-corpus}.yml`, `crates/*/Cargo.toml`, `README.md`, `docs/GATEWAY-SETUP.md`, `docs/CONFIGURATION.md`, `crates/famp/src/cli/daemon/{restart,install,status}.rs`, `.planning/REQUIREMENTS.md`, `.planning/ROADMAP.md`, `.planning/config.json`

### Secondary (MEDIUM confidence)
- WebSearch: aws-lc-rs cross-compilation/build requirements (`aws.github.io/aws-lc-rs/requirements/linux`, GitHub issues on `axodotdev/cargo-dist` maintenance activity, GitHub changelog posts on `macos-latest` runner architecture and x86_64 macOS runner sunsetting, curl-vs-quarantine Gatekeeper behavior)

### Tertiary (LOW confidence)
- None used without a corroborating primary/secondary source in this document.

## Metadata

**Confidence breakdown:**
- Standard stack (cargo-dist choice): MEDIUM — tool identity/version/maintenance is HIGH confidence (verified via registry + package-legitimacy gate); the *recommendation to adopt it* over hand-rolling is a judgment call, not a verified fact
- Architecture (build matrix, target choices): MEDIUM — glibc/runner choices are well-reasoned from verified `Cargo.lock` evidence, but the macOS cross-compile-from-arm64 claim for this specific dependency set (A1) is unverified in this session
- Pitfalls: HIGH — all three pitfalls are grounded in files read this session, not inferred
- Scope holes (famp-gateway, broken `cargo install famp`): HIGH — both independently verified via direct file reads and a live registry query, not assumption

**Research date:** 2026-08-02
**Valid until:** 2026-09-01 (30 days — GitHub Actions runner images and cargo-dist both move fast enough to warrant a re-check before execution if the phase is delayed)

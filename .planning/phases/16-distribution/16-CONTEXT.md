# Phase 16 — Distribution: Context

**Source:** Orchestrator-captured user decisions, 2026-08-02. No `/gsd-discuss-phase` pass was run —
the user chose to plan directly from ROADMAP + REQUIREMENTS + research. The decisions below were
then surfaced by `16-RESEARCH.md` and answered explicitly by the user before planning, so they carry
the same authority as discuss-phase output.

**Written against:** `ed04661` (main, clean).

---

## Phase Boundary

**In scope:** a tag-triggered release workflow that publishes prebuilt binaries + checksums + a curl
installer; docs reordered to lead with the binary path; the automated gates that keep all of it honest.

**Out of scope:** crates.io publication (see D-01), macOS notarization/Gatekeeper signing (avoided by
construction — see the curl-first constraint), artifact signing beyond checksums (see D-06), Linux
aarch64 (see D-05), and any actual relay deployment onto the Lightsail box (no phase covers that; this
phase only makes the binary *obtainable*).

---

## Implementation Decisions

### D-01 — The broken `cargo install famp` fallback: rewrite the docs, do NOT publish to crates.io (LOCKED)

`famp` **does not exist on crates.io** — VERIFIED twice this session, independently: the researcher hit
the crates.io API, and the orchestrator re-ran `curl -s https://crates.io/api/v1/crates/famp` and got
`{"errors":[{"detail":"crate \`famp\` does not exist"}]}`. Six documented sites currently instruct users
to run it:

| File | Line |
|---|---|
| `README.md` | 192 |
| `docs/GETTING-STARTED.md` | 43 |
| `docs/GATEWAY-SETUP.md` | 24 |
| `docs/ONBOARDING.md` | 12, 26, 32, 37 |

**Decision:** change every one of those to a from-source command that actually works
(`cargo install --path crates/famp` from a clone, or the `--git` form) and stop claiming crates.io
publication anywhere.

**Rejected:** running `just publish-workspace` to make the command true. Cost of the rejected option:
a manual 12-crate dependency-ordered publish (~9 min of sleeps per the recipe's own comments), and —
the actual reason it was rejected — it permanently claims the `famp` crate name and commits the project
to version-bump publishing discipline on every future release, or the docs go stale again by a
different route.

**Note for the planner:** DIST-04's requirement text says "`cargo install famp` remains documented only
as the from-source fallback." That text presumes the command works. It has been corrected in
REQUIREMENTS.md as part of this phase — the fallback is now the `--path`/`--git` form.

### D-02 — Ship three binaries, not one: `famp`, `famp-gateway`, `famp-relay` (LOCKED)

DIST-01's literal text names only `famp`. That is a requirement-text gap, not a scope reduction:

- `famp-gateway` is a **separate `[[bin]]` target** (`crates/famp-gateway/Cargo.toml:59`) and
  `docs/GATEWAY-SETUP.md` requires the operator to locate it on `PATH` (`$(which famp-gateway)`,
  lines 57-58). Shipping only `famp` leaves Phase 20's second person one binary short of the exact
  thing this phase exists to unblock.
- `famp-relay` is likewise a separate bin target (`crates/famp-relay/Cargo.toml:51`). It is what would
  actually be installed on the provisioned-but-bare Lightsail box (`54.158.102.139`), and it is nearly
  free once the build matrix exists.

REQUIREMENTS.md DIST-01 and ROADMAP.md's Phase 16 success criterion 1 have both been widened to name
all three. This was an explicit, user-approved scope decision — not a silent addition.

### D-03 — Release tooling: `cargo-dist` / `dist` 0.32 (LOCKED)

Generates the tag-triggered workflow, the curl installer (OS/arch detection, PATH warning, checksum
verification), and the checksums as one unit — most of DIST-01/02/03/05 by construction. Satisfies
DIST-05 structurally: there is no separate manual-upload code path to police. Matches this repo's
existing **generate + CI drift-check** convention (`plugin-check.yml` precedent) — `dist generate --check`
slots into the same shape.

**Cost accepted:** `dist` owns `.github/workflows/release.yml` (generated, not hand-edited) and adds a
`[workspace.metadata.dist]` block to the root `Cargo.toml`.

**Rejected:** a hand-rolled GH Actions matrix + hand-written `install.sh`. Cost of the rejected option:
this project would author OS/arch detection, PATH handling, and checksum verification itself — precisely
the parts that are easy to get subtly wrong, for a marginal line-by-line auditability gain.

### D-04 — Linux target: `x86_64-unknown-linux-gnu` on a **pinned** `ubuntu-22.04` (LOCKED, from research)

Not `ubuntu-latest` — pinning keeps the glibc floor (2.35) a deliberate, stable choice rather than one
that silently rises with each LTS bump.

**Rejected:** `x86_64-unknown-linux-musl`. Cost of the rejected option: combines two verified risks —
the `aws-lc-sys` cmake cross-toolchain problem (see D-08) and musl's `getaddrinfo` divergence from
glibc's NSS resolver, which matters here because `famp-gateway` makes outbound HTTPS calls in the
Phase 17 relay-fetch loop — for a portability benefit (very old / minimal distros) that is not in this
phase's stated audience.

### D-05 — Linux aarch64 is a documented follow-up, not this phase (LOCKED)

An ARM VPS or Pi is plausible for a relay, but `aarch64-unknown-linux-gnu` is required by neither
DIST-01's text nor Phase 20's acceptance event. Record it as a named follow-up; do not silently widen.

### D-06 — Checksums: state the honest security claim, do not overclaim (LOCKED)

A checksum fetched from the **same origin** as the binary (the GitHub Release) protects against
**corruption and truncated/partial downloads**. It does **not** protect against a compromised release
host or a compromised account pushing a malicious tag — an attacker who can substitute the binary can
substitute the checksum beside it.

**The claim the docs must make, and must not exceed:** *"verifies the download completed correctly and
matches what the release workflow produced — it does not, by itself, prove the release workflow was not
compromised."*

Signing (minisign / cosign / Sigstore) is the real answer to the compromised-pipeline threat and is a
**named follow-up**, not required by DIST-03. Overclaiming here is worse than underclaiming — this is
the same discipline QUAR-08/11 applied to the quarantine boundary in Phase 14.

### D-07 — Installer mechanics: install to `~/.cargo/bin`, never skip the PATH warning (LOCKED, from research)

- **Install target: `~/.cargo/bin`**, matching `just install`'s existing target
  (`cargo install --path crates/famp --locked --force`). A mismatch between the source-build target and
  the binary-install target makes "which famp am I running" genuinely ambiguous for anyone who does both
  — and this project's contributor *is* the person doing both.
- **PATH handling:** the generated installer must print the explicit warning + shell-profile snippet
  when the install dir isn't already on `PATH`. This is the single most common "installed it, command
  not found" failure. Do not suppress it.
- **Installer provenance:** fetched as a **release asset** pinned to the tag being installed, not from a
  mutable `raw.githubusercontent` branch path.
- **Upgrade / running daemon:** out of DIST-02's proof scope (which is explicitly a clean machine with no
  prior FAMP state), but worth one doc line — `famp daemon restart`
  (`crates/famp/src/cli/daemon/restart.rs`) is the correct post-upgrade step for existing installs. The
  installer does not need to invoke it.

### D-08 — macOS: start with one runner, but **prove the cross-compile early** (LOCKED — resolves a live disagreement)

Two sources disagreed and the planner must not paper over it:

- **`16-RESEARCH.md` Fork C** recommends a single `macos-latest` (arm64) runner cross-compiling
  `x86_64-apple-darwin` via `rustup target add` — because GitHub is sunsetting the Intel `macos-13`
  runner class. It flags as **UNVERIFIED** whether `aws-lc-sys`'s `cmake` invocation cross-compiles
  cleanly for `x86_64-apple-darwin`.
- **famp-lead-730** (prior lead, FAMP task `019fc565`) recommends native matrix builds on per-OS
  runners specifically to avoid the cross-compile toolchain problem — explicitly labelled by them as
  "recommendation, not scouting; a starting hypothesis to verify."

**Resolution — this is an execution detail with a cheap decisive test, not a scope question:** the plan
MUST include an **early task that actually builds `x86_64-apple-darwin` on an arm64 macOS runner** and
observes the result, before the rest of the release pipeline is built on the assumption. If it builds
clean → single runner (D-08a). If `aws-lc-sys`/cmake fails → add a second runner for the Intel target
and record the deprecation risk (D-08b). **Do not decide this from reading; decide it from the build
log.** Whichever way it lands, write the outcome into the plan's summary — the next person should not
have to re-derive it.

**Background (VERIFIED, from `Cargo.lock`):** the workspace declares `rustls` feature `"ring"`, but the
**actually-resolved** crypto backend is `aws-lc-rs`, because other crates in the graph
(`axum-server`, `hyper-rustls`, `rustls-platform-verifier`, via `famp-transport-http`/`famp-gateway`)
pull `rustls` with default features and Cargo unifies features across the build. `aws-lc-sys` needs
cmake + a C compiler. This is invisible unless you read `Cargo.lock` — it is the reason this fork
exists at all.

---

## Claude's Discretion

Everything not locked above: plan/task decomposition and wave assignment, the exact shape of the
doc-accuracy test, `[workspace.metadata.dist]` field values, and how the container proxy job is wired
(a new additive workflow file is preferred over editing `ci.yml`'s triggers — established repo pattern).

---

## Canonical References

### This phase
- `.planning/phases/16-distribution/16-RESEARCH.md` — the research this context resolves. Forks A–F,
  three verified pitfalls, and the `## Validation Architecture` section the VALIDATION.md draft is seeded from.
- `.planning/REQUIREMENTS.md` — DIST-01..05 (DIST-01 and DIST-04 amended by D-02 and D-01 respectively).
- `.planning/ROADMAP.md` § "Phase 16: Distribution" — goal, success criteria, and the curl-first constraint.

### Repo conventions this phase must match
- `.github/workflows/plugin-check.yml` — the generate + drift-check precedent D-03 follows.
- `.github/workflows/ci.yml` — runner images, toolchain pinning, caching, job naming.
- `justfile` — `install`, `install-gateway`, `check-shellcheck`, `publish-workspace` recipes.
- `crates/famp/tests/` — the existing compiled doc-accuracy test pattern (`gateway_setup_doc_accuracy.rs`)
  that DIST-04's gate should mirror.

### Downstream consumer
- Phase 20 (Human Acceptance Gate), DOC-07 — the fresh-machine validation that cannot run until this
  phase ships. Note D-06's boundary: this phase's CI container gate does **not** satisfy DOC-07.

---

## Standing Project Rules That Bind This Phase

- **Every validation check MUST be wired to an automated gate** (CI or pre-commit). Unwired scripts
  decay silently. A shell script that verifies the installer but isn't in CI does not count.
- **Committed ≠ pushed.** Verify the remote (`git fetch` then
  `git rev-list --left-right --count origin/main...HEAD`), not just the local tree, before reporting
  anything as done. This bit the prior lead on 2026-08-02.
- **Local `cargo test` green is NOT evidence about CI.** CI runs `cargo nextest` (process-per-test);
  `cargo test` is threads-in-one-process. A `static Mutex` that serializes locally is inert under
  nextest. Verify by SHA: `gh api repos/thebenlamm/FAMP/commits/<sha>/check-runs`.
- **Never capture a test result through a pipe** — `cargo test ... | tail` returns tail's exit code.
  Redirect to a file and echo `$?`.
- **CI `paths-ignore` means docs-only commits get ZERO check-runs, not a pass.** Treat
  `total_count == 0` as blocking, not green. Relevant here: this phase touches both docs and workflows.

---

## Risk Summary

| Risk | Mitigation |
|---|---|
| `aws-lc-sys`/cmake breaks the `x86_64-apple-darwin` cross build | D-08's early build task decides it from a log, not a guess; documented fallback to a second runner |
| `dist`-generated `release.yml` drifts from committed config | `dist generate --check` drift gate in CI (D-03) |
| Docs claim more security than checksums provide | D-06's exact wording is locked; the doc-accuracy gate should assert it |
| DIST-02's container job is mistaken for DOC-07 satisfaction | Stated explicitly in D-06/research; the plan must not mark DOC-07 satisfiable here |
| A release tag published before the pipeline is proven | Exercise the full pipeline with a pre-release tag before `/gsd-verify-work` |

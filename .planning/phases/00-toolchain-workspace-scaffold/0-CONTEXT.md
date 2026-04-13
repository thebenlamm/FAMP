# Phase 0: Toolchain & Workspace Scaffold - Context

**Gathered:** 2026-04-12
**Status:** Ready for planning
**Mode:** `--auto` (recommended defaults selected; no codebase exists yet)

<domain>
## Phase Boundary

Bootstrap a green `cargo build` + `cargo nextest run` on an empty 12-crate Rust workspace with strict lints and CI enforcing the loop on every push. Delivers the edit-build-test substrate for all downstream phases — zero FAMP protocol code is written in this phase. User is new to Rust; tolerate generous learning time on the cargo cycle before Phase 1.

</domain>

<decisions>
## Implementation Decisions

### Toolchain pinning
- **D-01:** Install Rust via `rustup`; pin exact version in `rust-toolchain.toml` at repo root with `channel = "1.87.0"` (current stable floor from tech-stack research) and `components = ["rustfmt", "clippy"]`.
- **D-02:** CI uses `dtolnay/rust-toolchain@stable` but respects `rust-toolchain.toml` for reproducibility across local + CI.
- **D-03:** No nightly features anywhere in v1. If a crate needs nightly, it's rejected.

### Workspace layout
- **D-04:** Single Cargo workspace rooted at repo root. Member crates live under `crates/<name>/`. Umbrella binary + CLI live at `crates/famp/` (shares workspace Cargo.toml).
- **D-05:** Ship all 12 library crates as empty `lib.rs` stubs in Phase 0 — do NOT stage-merge yet. The Phase 2-3 `famp-foundation` merge is deferred; Phase 0 locks the final crate DAG so downstream phases just fill in bodies.
- **D-06:** Crate list (final): `famp-core`, `famp-canonical`, `famp-crypto`, `famp-envelope`, `famp-identity`, `famp-causality`, `famp-fsm`, `famp-protocol`, `famp-extensions`, `famp-transport`, `famp-transport-http`, `famp-conformance`, plus `famp` (umbrella, bin + lib re-exports).
- **D-07:** Every crate declares `edition = "2021"`, `rust-version = "1.87"`, `license = "Apache-2.0 OR MIT"`, and inherits `version`, `authors`, `repository` from `[workspace.package]`.
- **D-08:** `[workspace.dependencies]` in root `Cargo.toml` pins every external crate version exactly once (per TOOL-06 + tech-stack table). Member crates reference via `dep = { workspace = true }`. No per-crate version drift is possible.

### Dependency pinning (seed set for Phase 0)
- **D-09:** Phase 0 only needs the minimum to compile stubs + run CI. Seed `[workspace.dependencies]` with the full tech-stack table from `CLAUDE.md` (ed25519-dalek 2.2, serde_jcs 0.2, serde 1.0.228, serde_json 1.0.149, uuid 1.23 with v7+serde, base64 0.22.1, sha2 0.11, axum 0.8.8, reqwest 0.13.2 with rustls-tls default-features-off, rustls 0.23.38, tokio 1.51.1, thiserror 2.0.18, anyhow 1.0.102, proptest 1.11, stateright 0.31, insta 1.47.2) — but Phase 0 stubs don't need to actually `use` any of them. Pinning them now prevents version scramble later.
- **D-10:** `reqwest` configured with `default-features = false` + `features = ["rustls-tls-native-roots", "json"]`; `tokio` at workspace level has no features (bins opt in to `["full"]`, libs opt in narrowly).

### Task runner (`just`)
- **D-11:** `Justfile` at repo root with the following targets, all mapped 1:1 to CI jobs:
  - `just build` → `cargo build --workspace --all-targets`
  - `just test` → `cargo nextest run --workspace`
  - `just lint` → `cargo clippy --workspace --all-targets -- -D warnings`
  - `just fmt` → `cargo fmt --all`
  - `just fmt-check` → `cargo fmt --all -- --check`
  - `just ci` → runs fmt-check + lint + build + test in order (local CI-parity gate)
- **D-12:** `just` is not vendored — developers install via `cargo install just` or Homebrew. README documents the install step. CI installs via `taiki-e/install-action@v2`.

### Test runner
- **D-13:** `cargo-nextest` is the default test runner. `.config/nextest.toml` created with one profile (`default`) for local and one (`ci`) that enables `fail-fast = false` and slower-test reporting. Doc tests continue to run via plain `cargo test --doc` in a separate CI step (nextest doesn't run doctests).

### Clippy strictness
- **D-14:** Workspace-level `[workspace.lints.rust]` and `[workspace.lints.clippy]` tables at repo root. Every member crate opts in with `[lints] workspace = true`.
- **D-15:** `unsafe_code = "forbid"` at workspace root (TOOL-07). Any crate needing unsafe must explicitly opt out with reviewer sign-off — not allowed in v1.
- **D-16:** Clippy baseline: deny `clippy::all`, `clippy::pedantic` (with the following allows to cut noise: `module_name_repetitions`, `must_use_candidate`, `missing_errors_doc`, `missing_panics_doc`), deny `clippy::unwrap_used` and `clippy::expect_used` in non-test code, warn `clippy::nursery`. `rust.unused_crate_dependencies = "warn"` catches over-pinning.
- **D-17:** `rustfmt.toml` uses defaults (stable rustfmt only) — no nightly-only knobs. `max_width = 100`, `edition = "2021"`.

### CI (GitHub Actions)
- **D-18:** Single workflow file `.github/workflows/ci.yml` triggered on `push` and `pull_request` against any branch. Matrix: `{ os: [ubuntu-latest, macos-latest], rust: [stable] }`. Windows deferred.
- **D-19:** Jobs (each a separate job so failures show independently in the PR checklist): `fmt-check`, `clippy`, `build`, `test` (nextest), `doc-test`, `audit`. All must be green for merge to `main`.
- **D-20:** Cache with `Swatinem/rust-cache@v2`. Install `cargo-nextest` and `just` via `taiki-e/install-action@v2` (no from-source compilation per CI run). `rustsec/audit-check@v2` runs `cargo audit` on a daily cron + on every PR.
- **D-21:** Branch protection rule on `main`: all CI jobs required to pass; linear history; no force-push. Configured manually by repo owner after first green run — documented in README, not scripted in Phase 0.
- **D-22:** `concurrency: { group: ci-${{ github.ref }}, cancel-in-progress: true }` on the workflow so new pushes cancel superseded runs.

### Minimum viable stub content
- **D-23:** Each library crate ships exactly: `Cargo.toml`, `src/lib.rs` containing `//! Crate-level docstring\n#![forbid(unsafe_code)]\n`, no public items. This compiles and links under workspace strict lints.
- **D-24:** `famp` umbrella ships `src/lib.rs` (re-export stubs, empty for now) + `src/bin/famp.rs` containing a minimal `fn main() { println!("famp v0.5.1 placeholder"); }` to prove the workspace can produce an executable binary.
- **D-25:** One smoke test per crate: `#[test] fn crate_compiles_and_links() {}` — ensures nextest has a non-zero test count so a broken runner fails loudly rather than silently reporting zero tests.

### Repository hygiene
- **D-26:** `.gitignore` covers `target/`, `.DS_Store`, `*.swp`, `.idea/`, `.vscode/` (except `.vscode/extensions.json` if added later).
- **D-27:** `README.md` includes Phase 0 bootstrap commands copy-pasteable from the tech-stack `## Installation commands` block. Documents `just ci` as the single pre-push check.
- **D-28:** `CONTRIBUTING.md` deferred to Phase 1 (docs phase). `LICENSE-APACHE` + `LICENSE-MIT` files committed now since crates reference them in metadata.
- **D-29:** No pre-commit hooks in Phase 0 — `just ci` is the pre-push check. Pre-commit is revisited after the team grows beyond one developer.

### Claude's Discretion
- Exact clippy allow list fine-tuning if a specific pedantic lint turns out to be noise on empty stubs
- Whether `famp-causality`-style crate ordering in `[workspace.members]` is alphabetical or DAG-topological (no functional impact on empty stubs)
- `Justfile` cosmetic conventions (emoji, colors, help text)
- Whether to ship a `deny.toml` for `cargo-deny` now or defer to Phase 1 (inclination: defer, since `rustsec/audit-check` covers advisories and `cargo-deny` adds license-scan noise this early)

</decisions>

<specifics>
## Specific Ideas

- Installation commands and version numbers come verbatim from the `## Technology Stack` block in `CLAUDE.md`. Do not re-research versions in this phase — the research is already locked and downstream phases will break if versions drift.
- "Beginner-friendly" is a stated constraint: every command a developer runs should be in the `Justfile` or the README — no tribal knowledge about cargo flags.
- `just ci` must exactly mirror the GitHub Actions matrix so a green local run implies a green CI run (learned rule: CI parity for pre-commit gates).

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-level (in repo root)
- `CLAUDE.md` §`## Technology Stack` — Full pinned version table and rationale for every crate. This is the authority for `[workspace.dependencies]` content in Phase 0.
- `CLAUDE.md` §`## Installation commands (Phase 0 bootstrap)` — Copy-paste bootstrap block for rustup, cargo-binstall, cargo-nextest, just.
- `CLAUDE.md` §`## Alternatives summary` + §`## What NOT to Use` — Negative constraints that Phase 0 must enforce via lints or CI (e.g., no openssl, no native-tls).

### Planning artifacts
- `.planning/PROJECT.md` — Project constraints; tech stack frame.
- `.planning/REQUIREMENTS.md` — TOOL-01..TOOL-07 are the exact checklist this phase satisfies.
- `.planning/ROADMAP.md` §`Phase 0: Toolchain & Workspace Scaffold` — Authoritative Success Criteria (5 bullets). Every decision above must ladder to one of them.
- `.planning/STATE.md` §`Accumulated Context → Key Decisions Logged` — Prior decisions already locked (Rust, 12-crate workspace, serde_jcs, rustls-only, native async traits). Do NOT re-litigate these.

### External specs (for later phases — linked here so researchers see the full ref graph early)
- `FAMP-v0.5-spec.md` (repo root) — Original v0.5 spec. Phase 0 only needs to know it exists; Phase 1 forks it.

**No v0.5.1 fork yet — that's Phase 1's output.**

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **None.** Repo contains only `CLAUDE.md` and `FAMP-v0.5-spec.md`. Phase 0 writes the first line of Rust code in this project.

### Established Patterns
- **None yet.** Phase 0 *establishes* the patterns (workspace lint inheritance, `workspace.dependencies` pinning, `Justfile`-as-CI-mirror) that every later phase inherits.

### Integration Points
- **Phase 1** consumes the docs/ directory layout Phase 0 creates (the v0.5.1 fork lives at `docs/FAMP-v0.5.1-spec.md` — Phase 0 should create `docs/` as a placeholder).
- **Phase 2** will be the first phase to add real dependencies (`ed25519-dalek`, `serde_jcs`, `sha2`). Phase 0 must pin them in `[workspace.dependencies]` even though no crate `use`s them yet, so Phase 2 only has to flip `workspace = true` flags.

</code_context>

<deferred>
## Deferred Ideas

- **`cargo-deny` license/advisory scanning** — Deferred to Phase 1 or later. `cargo audit` via `rustsec/audit-check` is sufficient for Phase 0's security posture.
- **Pre-commit hook framework** (`pre-commit`, `lefthook`, husky-equivalent) — Deferred. `just ci` covers the pre-push check; hook infra adds setup friction for a single developer.
- **Windows CI** — Deferred. Ubuntu + macOS matrix is sufficient for v1; Windows support is a post-v1 decision.
- **MSRV policy + `cargo-msrv` verification** — Deferred. Pinning to `1.87` in `rust-toolchain.toml` is the v1 stance; formal MSRV scanning is a post-v1 concern.
- **Docs.rs / doc generation CI** — Deferred. Crate-level `//!` docstrings exist from Phase 0 but automated doc publishing is a release-engineering concern for v1 wrap-up.
- **Benchmarking infrastructure** (`criterion`, `iai`) — Deferred. Phase 2 or later will add benches for canonicalization and signature hot paths.
- **FFI / Python / TS bindings scaffolding** — Explicitly out of v1 per `CLAUDE.md` constraints. Not in Phase 0.
- **`famp-foundation` stage-merge crate** — The roadmap notes Phase 2-3 *may* temporarily merge foundation crates. Phase 0 ships the final 12-crate DAG; merging is a Phase 2 decision if beginner build velocity demands it.

</deferred>

---

*Phase: 00-toolchain-workspace-scaffold*
*Context gathered: 2026-04-12 (auto mode)*

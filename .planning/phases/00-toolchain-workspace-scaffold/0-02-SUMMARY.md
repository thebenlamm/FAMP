---
phase: 00-toolchain-workspace-scaffold
plan: "02"
subsystem: infra
tags: [rust, cargo, workspace, clippy, rustfmt, ed25519-dalek, serde, tokio, axum]

# Dependency graph
requires:
  - phase: 00-01
    provides: rust-toolchain.toml pinning 1.87.0, LICENSE files, README, .gitignore

provides:
  - Root Cargo.toml with [workspace.dependencies] pinning all 16 external crates once
  - rustfmt.toml with stable-only config (edition=2021, max_width=100)
  - 12 library crate stubs (famp-core through famp-conformance)
  - famp umbrella crate with bin/famp.rs placeholder binary
  - Workspace-level strict lints: unsafe_code=forbid, deny clippy::all+pedantic, deny unwrap_used+expect_used
  - 13 smoke tests (one per crate) passing under cargo test

affects:
  - 00-03 (CI workflow reads this workspace structure)
  - All subsequent phases (body implementations fill stubs created here)
  - Phase 2 (famp-canonical + famp-crypto bodies land here first)

# Tech tracking
tech-stack:
  added:
    - ed25519-dalek 2.2.0 (pinned, not yet used)
    - sha2 0.11.0 (pinned, not yet used)
    - serde 1.0.228 + serde_json 1.0.149 (pinned, not yet used)
    - serde_jcs 0.2.0 (pinned, not yet used)
    - uuid 1.23.0 (pinned, not yet used)
    - base64 0.22.1 (pinned, not yet used)
    - axum 0.8.8 (pinned, not yet used)
    - reqwest 0.13.2 (pinned, not yet used)
    - rustls 0.23.38 (pinned, not yet used)
    - tokio 1.51.1 (pinned, not yet used)
    - thiserror 2.0.18 + anyhow 1.0.102 (pinned, not yet used)
    - proptest 1.11.0 + stateright 0.31.0 + insta 1.47.2 (pinned, not yet used)
  patterns:
    - All workspace deps pinned once in [workspace.dependencies]; member crates use workspace=true, never per-crate version
    - Lints inherited via [lints] workspace=true in every member crate
    - Smoke test pattern: crate_compiles_and_links() in each lib.rs ensures cargo nextest reports non-zero per crate
    - Bins that don't yet use their crate's lib suppress unused_crate_dependencies with #![allow(unused_crate_dependencies)]

key-files:
  created:
    - Cargo.toml (workspace root — workspace deps, lints, package metadata)
    - rustfmt.toml (stable rustfmt config)
    - crates/famp-core/Cargo.toml + src/lib.rs
    - crates/famp-canonical/Cargo.toml + src/lib.rs
    - crates/famp-crypto/Cargo.toml + src/lib.rs
    - crates/famp-envelope/Cargo.toml + src/lib.rs
    - crates/famp-identity/Cargo.toml + src/lib.rs
    - crates/famp-causality/Cargo.toml + src/lib.rs
    - crates/famp-fsm/Cargo.toml + src/lib.rs
    - crates/famp-protocol/Cargo.toml + src/lib.rs
    - crates/famp-extensions/Cargo.toml + src/lib.rs
    - crates/famp-transport/Cargo.toml + src/lib.rs
    - crates/famp-transport-http/Cargo.toml + src/lib.rs
    - crates/famp-conformance/Cargo.toml + src/lib.rs
    - crates/famp/Cargo.toml + src/lib.rs + src/bin/famp.rs
  modified: []

key-decisions:
  - "Cargo bin self-reference: famp/src/bin/famp.rs uses #![allow(unused_crate_dependencies)] to suppress workspace lint false positive until Phase 8 re-exports land"
  - "All 16 workspace deps pinned at Phase 0 even though no crate uses them yet — prevents version drift when Phase 2-7 bodies land"
  - "Toolchain discovered at ~/.rustup/toolchains/1.87.0-aarch64-apple-darwin after puccinialin rustup auto-downloaded it on first invocation"

patterns-established:
  - "Pattern: workspace dep pinning — every external crate pinned once in [workspace.dependencies]; member crates reference via dep = { workspace = true }"
  - "Pattern: lint inheritance — [lints] workspace = true in every member Cargo.toml; no per-crate lint configuration"
  - "Pattern: stub smoke test — each crate has exactly one #[test] fn crate_compiles_and_links() {} so nextest always reports non-zero test count"

requirements-completed: [TOOL-02, TOOL-06, TOOL-07]

# Metrics
duration: 7min
completed: 2026-04-13
---

# Phase 00 Plan 02: Workspace Scaffold Summary

**13-crate Cargo workspace with [workspace.dependencies] pinning all 16 protocol-stack crates, strict clippy deny-all lints, and green cargo build + test on empty stubs**

## Performance

- **Duration:** 7 min
- **Started:** 2026-04-12T23:54:18Z
- **Completed:** 2026-04-13T00:02:05Z
- **Tasks:** 2 of 2
- **Files modified:** 29

## Accomplishments

- Root Cargo.toml pins all 16 external crates (ed25519-dalek through insta) exactly once; zero per-crate version drift possible from Day 1
- Workspace lint baseline active: `unsafe_code = "forbid"`, deny `clippy::all + pedantic`, deny `unwrap_used + expect_used`; enforced on every crate via `[lints] workspace = true`
- 13 crates compile and test clean: `cargo build --workspace --all-targets` and `cargo clippy --workspace --all-targets -- -D warnings` both exit 0 with zero warnings; 13 smoke tests pass; `famp` binary prints placeholder

## Task Commits

Each task was committed atomically:

1. **Task 1: Write workspace root Cargo.toml + rustfmt.toml** - `3fadeed` (chore)
2. **Task 2: Scaffold all 13 crate stubs with inherited lints + smoke tests** - `2b194b4` (feat)

**Plan metadata:** (committed with this summary)

## Files Created/Modified

- `/Users/benlamm/Workspace/FAMP/Cargo.toml` — Workspace root: members, [workspace.package], [workspace.dependencies] (16 crates), [workspace.lints], [profile.release]
- `/Users/benlamm/Workspace/FAMP/rustfmt.toml` — Stable rustfmt config: edition=2021, max_width=100
- `crates/famp-{core,canonical,crypto,envelope,identity,causality,fsm,protocol,extensions,transport,transport-http,conformance}/Cargo.toml` — 12 library crate manifests (all fields workspace-inherited)
- `crates/famp-{...}/src/lib.rs` — 12 library stubs: forbid(unsafe_code) + smoke test
- `crates/famp/Cargo.toml` — Umbrella crate manifest with [[bin]] section
- `crates/famp/src/lib.rs` — Umbrella lib stub
- `crates/famp/src/bin/famp.rs` — Placeholder binary that prints "famp v0.5.1 placeholder"

## Decisions Made

- Crate `famp` bin uses `#![allow(unused_crate_dependencies)]` — the bin compiles against the famp lib as an implicit extern crate even though the bin doesn't `use` anything from it yet. The workspace `unused_crate_dependencies = "warn"` lint fires as a false positive. The allow suppresses it cleanly; it will be removed in Phase 8 when the bin gains real re-export usage.
- All 16 crates from the CLAUDE.md tech-stack table are pinned in `[workspace.dependencies]` even though Phase 0 stubs use none of them. This is intentional: it freezes the version contract so later phases only need to flip `workspace = true` flags without touching version strings.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed `unused_crate_dependencies` false positive warning blocking `cargo build --all-targets`**
- **Found during:** Task 2 (scaffold + build verification)
- **Issue:** `cargo build --workspace --all-targets` produced a warning: `extern crate famp is unused in crate famp` on the famp bin target. This comes from the bin test target implicitly linking the lib as an extern crate when `--all-targets` is specified, even though the bin doesn't yet `use` anything from the lib. The workspace `unused_crate_dependencies = "warn"` lint fires correctly per the lint's contract, but for a Phase 0 stub it's a false positive.
- **Fix:** Added `#![allow(unused_crate_dependencies)]` with a comment to `crates/famp/src/bin/famp.rs`. CONTEXT D-16 explicitly authorizes fine-tuning the allow list for empty stubs.
- **Files modified:** `crates/famp/src/bin/famp.rs`
- **Verification:** `cargo build --workspace --all-targets` exits 0 with 0 warnings after fix
- **Committed in:** `2b194b4` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - bug / false-positive warning)
**Impact on plan:** Single targeted allow annotation; no scope creep. Plan D-16 explicitly anticipated this type of fine-tuning.

## Issues Encountered

- `cargo` not in shell PATH during execution (no rustup shims in `~/.cargo/bin`). Resolved by using the direct toolchain path `/Users/benlamm/.rustup/toolchains/1.87.0-aarch64-apple-darwin/bin/cargo`. The 1.87.0 toolchain was auto-downloaded by puccinialin's rustup on first invocation. No impact on output; the rust-toolchain.toml file ensures the correct version is used by any cargo invocation.

## Known Stubs

All 13 crates are intentional Phase 0 stubs. No crate exports any public API yet. This is the expected state — bodies land in Phases 2–7. The `crate_compiles_and_links` smoke test in each lib.rs is not a stub in the problematic sense; it is the intended content for Phase 0.

The `famp` binary placeholder string is the intended Phase 0 content per D-24.

## Next Phase Readiness

- Phase 00-03 (CI workflow) can now build this workspace on GitHub Actions — the workspace structure and toolchain pin are locked
- Phase 2 (famp-canonical body) only needs to add `serde_jcs = { workspace = true }` to `crates/famp-canonical/Cargo.toml` and write the implementation — no version decisions remain
- All 16 workspace deps pre-pinned; downstream phases cannot accidentally introduce version drift

---
*Phase: 00-toolchain-workspace-scaffold*
*Completed: 2026-04-13*

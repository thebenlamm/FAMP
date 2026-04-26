---
quick_id: 260425-cic
slug: bump-rustls-webpki-2026-0104
subsystem: infra
tags: [cargo, dependencies, security, rustsec, rustls-webpki, ci]

# Tech tracking
tech-stack:
  added: []
  patterns: []

key-files:
  created: []
  modified:
    - Cargo.lock

key-decisions:
  - "Lockfile-only bump (cargo update -p rustls-webpki) — patched range >=0.103.13 is SemVer-compatible with the existing 0.103 minor; no Cargo.toml change needed."
  - "Did NOT add RUSTSEC-2026-0104 to .cargo/audit.toml ignore list — fix the advisory, never mask it (per CLAUDE.md and plan's Out-of-scope section)."

patterns-established: []

requirements-completed: []

# Metrics
duration: ~5min (excluding 1m24s one-time cargo-audit install)
completed: 2026-04-26
---

# Quick Task 260425-cic: Bump `rustls-webpki` to Clear RUSTSEC-2026-0104 — Summary

**`rustls-webpki` 0.103.12 → 0.103.13 in Cargo.lock; CI `audit` job advisory cleared with no source changes and no Cargo.toml touch.**

## Performance

- **Duration:** ~5 min for the task itself (cargo-audit was not installed locally; one-time `cargo install --locked cargo-audit` added ~84s of compile time, not counted)
- **Started:** 2026-04-26T02:05:00Z
- **Completed:** 2026-04-26T02:12:51Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Bumped `rustls-webpki` from `0.103.12` to `0.103.13` in `Cargo.lock` — clears RUSTSEC-2026-0104 ("Reachable panic in CRL parsing") advisory.
- Verified `cargo audit` reports **zero** unignored vulnerabilities (the two pre-existing ignores — RUSTSEC-2026-0097 rand, RUSTSEC-2025-0134 rustls-pemfile — remain untouched).
- Verified `cargo build --workspace --all-targets` succeeds (no transitive version-skew).
- Verified `cargo test --workspace --no-run` succeeds (test compilation green; full test runtime deferred to CI per plan).
- Confirmed `git diff --stat` shows **only `Cargo.lock`** changed (zero source files touched).

## Task Commits

1. **T1: Bump rustls-webpki in lockfile** — `58eb1b9` (chore: deps)

## Files Modified

- `Cargo.lock` — `rustls-webpki` version field: `0.103.12` → `0.103.13` (2-line change)

## Verification Evidence

```
$ grep -A1 'name = "rustls-webpki"' Cargo.lock
name = "rustls-webpki"
version = "0.103.13"

$ git diff --stat HEAD~1 HEAD
 Cargo.lock | 4 ++--
 1 file changed, 2 insertions(+), 2 deletions(-)

$ cargo audit
    Fetching advisory database from `https://github.com/RustSec/advisory-db.git`
      Loaded 1058 security advisories (from /Users/benlamm/.cargo/advisory-db)
    Updating crates.io index
    Scanning Cargo.lock for vulnerabilities (322 crate dependencies)
EXIT=0

$ cargo build --workspace --all-targets
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 35.37s
EXIT=0

$ cargo test --workspace --no-run
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.13s
EXIT=0
```

The advisory database loaded 1058 advisories; scan completed silently with exit 0 — RUSTSEC-2026-0104 is no longer reported. The two pre-ignored advisories in `.cargo/audit.toml` (rand 0.8 unsoundness, rustls-pemfile unmaintained) are still ignored, untouched.

## Decisions Made

- **Did not bypass via ignore-list.** The plan's "Out of scope" and the project's CLAUDE.md both forbid masking advisories. The fix is a real version bump within the SemVer-compatible range, not an audit.toml entry.
- **Did not bump `rustls` itself or anything else.** `cargo update -p rustls-webpki` is targeted; no broader updates needed because the patched range (`>=0.103.13`) is reachable inside the existing `0.103.x` resolver constraint.
- **Did not touch Cargo.toml.** `rustls-webpki` enters as a transitive dep via `rustls 0.23.39` (and via `rustls-platform-verifier` / TLS plumbing); the workspace-level `rustls = "0.23"` pin remains correct.

## Deviations from Plan

None — plan executed exactly as written. No Rule 1/2/3 auto-fixes; no Rule 4 architectural decisions.

**Note (not a deviation):** `cargo-audit` was not installed in the local toolchain — installed via `cargo install --locked cargo-audit` (cargo-audit `0.22.1`) so the verification step in the plan could run. The install itself is local-developer tooling (matches what `taiki-e/install-action` does in CI) and is not committed to the repo.

## Issues Encountered

None.

## Self-Check: PASSED

- `Cargo.lock` modified: FOUND (`rustls-webpki version = "0.103.13"` confirmed via grep)
- Commit `58eb1b9` exists: FOUND (`git log --oneline -1` → `58eb1b9 chore(deps): bump rustls-webpki to clear RUSTSEC-2026-0104`)
- `cargo audit` exit code: 0 (RUSTSEC-2026-0104 cleared)
- `cargo build --workspace --all-targets` exit code: 0
- `cargo test --workspace --no-run` exit code: 0
- Diff scope: only `Cargo.lock`, 2 insertions / 2 deletions
- No deletions of tracked files (verified via post-commit deletion check — clean)
- No source `*.rs` files modified (must-have constraint)

---

*Quick task: 260425-cic*
*Completed: 2026-04-26*

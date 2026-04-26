---
quick_id: 260425-cic
slug: bump-rustls-webpki-2026-0104
description: Bump rustls-webpki to clear RUSTSEC-2026-0104 (CI audit job failing)
date: 2026-04-25
mode: quick
---

# Quick Task 260425-cic: Bump `rustls-webpki` to Clear RUSTSEC-2026-0104

## Background

CI's `audit` job has been failing since 2026-04-22 against `rustls-webpki 0.103.12` due to **RUSTSEC-2026-0104** — *"Reachable panic in certificate revocation list parsing"* (advisory published 2026-04-22, same day failures started). The patched range is `>=0.103.13, <0.104.0-alpha.1`. FAMP is on `0.103.12` per `Cargo.lock`.

FAMP does not call `BorrowedCertRevocationList::from_der` / `OwnedCertRevocationList::from_der` — the panic is unreachable in our code. But `rustsec/audit-check@v2` correctly fails the workflow on any `cargo audit` finding regardless of reachability, so the fix is a transitive lockfile bump.

`rustls-webpki` enters via two paths in the workspace dep tree:
- `rustls 0.23.39` → `rustls-webpki 0.103.12`
- (one more caller via `security-framework` / TLS path — likely `rustls-platform-verifier` or `reqwest`)

A `cargo update -p rustls-webpki` (within the existing `0.103` minor range) is sufficient — no Cargo.toml change, no API surface change, no version-skew risk.

## Truths

- `Cargo.lock` currently pins `rustls-webpki = "0.103.12"`.
- Patched range per RUSTSEC-2026-0104: `>=0.103.13, <0.104.0-alpha.1` OR `>=0.104.0-alpha.7`.
- Latest 0.103.x at time of writing is at least `0.103.13` (per advisory's "patched" field).
- No Cargo.toml in the workspace pins `rustls-webpki` directly (it's transitive only).
- `Cargo.toml` workspace pins `rustls = "0.23"` — this is unaffected by a `rustls-webpki` patch bump.
- The `rustsec/audit-check@v2` action fails the job whenever `cargo audit` reports >=1 unignored vulnerability.
- The two existing audit ignores (`RUSTSEC-2026-0097`, `RUSTSEC-2025-0134`) are unrelated and stay as-is.

## Tasks

### T1: Bump `rustls-webpki` in lockfile and verify audit clears

**Files:** `Cargo.lock`

**Action:**

1. Run `cargo update -p rustls-webpki`. This updates within the SemVer-compatible `0.103.x` range.
2. Confirm the new pinned version is `>= 0.103.13` (read updated `Cargo.lock`).
3. Run `cargo audit` locally and confirm `RUSTSEC-2026-0104` is no longer reported. (Expected output: `Success No vulnerable packages found` OR remaining count drops from 1 to 0.)
4. Run `cargo build --workspace --all-targets` and confirm it succeeds — guards against transitive version-skew (e.g., a downstream crate that needed the exact 0.103.12 build).
5. Run `cargo test --workspace --no-run` to ensure tests still compile (full test run not required — the change is dep-only and any test runtime breakage would be caught in CI by the `test` job).

**Verify:**

- `grep -A1 'name = "rustls-webpki"' Cargo.lock` shows `version = "0.103.13"` (or higher within 0.103.x).
- `cargo audit` exit code 0 (or only the two pre-ignored advisories listed).
- `cargo build --workspace --all-targets` exits 0.
- Only `Cargo.lock` changed in `git diff --stat` (zero source files touched).

**Done when:** Cargo.lock shows the bumped version, local cargo audit reports zero unignored vulnerabilities, build succeeds, committed atomically with message `chore(deps): bump rustls-webpki to clear RUSTSEC-2026-0104`.

## Out of scope

- Bumping `rustls` itself, or any other dep — the advisory is patched within the existing minor range.
- Touching `Cargo.toml` — `rustls-webpki` is transitive and intentionally unpinned at the workspace level.
- Adding the advisory to the `cargo audit` ignore list — that would mask the issue, not fix it.
- Investigating whether FAMP could be reached by the panic in production — not relevant for clearing the gate.

## must_haves

- truths:
  - `Cargo.lock` after this task shows `rustls-webpki` version `>= 0.103.13` (and `< 0.104.0`).
  - Local `cargo audit` reports zero non-ignored vulnerabilities (the two existing ignores remain).
  - `cargo build --workspace --all-targets` succeeds with no version-skew errors.
  - No source file (`*.rs`) is modified — Cargo.lock-only change.
- artifacts:
  - Modified: `Cargo.lock`.
- key_links:
  - Advisory: <https://rustsec.org/advisories/RUSTSEC-2026-0104>
  - Failing CI run: <https://github.com/thebenlamm/FAMP/actions/runs/24945256370>
  - Workflow file: `.github/workflows/ci.yml:130-141` (audit job)

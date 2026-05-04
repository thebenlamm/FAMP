---
phase: 4
slug: federation-cli-unwire-federation-ci-preservation
status: draft
nyquist_compliant: true
wave_0_complete: true  # Covered by Wave 1 plans 04-01 (e2e_two_daemons + adversarial), 04-02 (_deferred_v1/README.md), 04-03 (famp-local/README.md), 04-04 (MIGRATION-v0.8-to-v0.9.md), 04-08 (cli_help_invariant.rs)
created: 2026-05-03
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: 04-RESEARCH.md §"Audit 12 — Validation Architecture (Nyquist)"

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo nextest 0.9.x` (workspace gate; `.config/nextest.toml`) |
| **Config file** | `.config/nextest.toml` (CARRY-01 pin lives here) |
| **Quick run command** | `cargo nextest run -p famp e2e_two_daemons` |
| **Full suite command** | `just ci` (fmt-check, clippy -D warnings, nextest --workspace, spec-version-coherence, no-tokio-in-bus, shellcheck, publish-workspace-dry-run) |
| **Estimated runtime** | ~120 seconds (full `just ci`); ~15s (quick) |

---

## Sampling Rate

- **After every task commit:** `cargo nextest run -p famp` (umbrella crate; covers `e2e_two_daemons` + adversarial)
- **After every plan wave:** `just ci`
- **Before `/gsd-verify-work`:** `just ci` green AND `cargo tree -i openssl` empty AND `git rev-parse v0.8.1-federation-preserved` succeeds AND `cargo run --bin famp -- --help` shows 15 variants (no `init`/`setup`/`listen`/`peer` rows)
- **Max feedback latency:** 30 seconds (quick); 180 seconds (full)

---

## Per-Requirement Verification Map

| Req ID | Behavior | Test Type | Automated Command | File Exists | Status |
|--------|----------|-----------|-------------------|-------------|--------|
| FED-01 | 6 federation verbs absent from CLI help output | smoke (CLI help) | `cargo run --bin famp -- --help \| grep -E "^  (init\|setup\|listen\|peer)\b"` exits non-zero | ❌ W0 — needs `tests/cli_help_invariant.rs` | ⬜ pending |
| FED-02 | Workspace `Cargo.toml` carries v1.0-internals comment for transport-http + keyring | static (grep) | `grep -c "v1.0 federation internals" Cargo.toml` ≥ 2 | manual one-shot | ⬜ pending |
| FED-03 | `e2e_two_daemons` targets `famp-transport-http` library API directly | integration | `cargo nextest run -p famp -E 'test(=e2e_two_daemons::happy_path)'` | ❌ W0 — refactor IS the test | ⬜ pending |
| FED-04 | Federation e2e green in `just ci` on every commit | integration | `just ci` | ✅ existing (refactor brings it back into active set) | ⬜ pending |
| FED-05 | `v0.8.1-federation-preserved` lightweight tag exists at correct SHA | git | `git rev-parse v0.8.1-federation-preserved` returns valid SHA | manual (post-refactor commit, pre-deletion) | ⬜ pending |
| FED-06 | `cargo tree` shows federation crates consumed only by e2e test, no top-level CLI | static (cargo tree) | `cargo tree -p famp-transport-http -i` lists only `famp-transport-http` itself + the e2e test target | ✅ existing tooling | ⬜ pending |
| MIGRATE-01 | `docs/MIGRATION-v0.8-to-v0.9.md` exists with CLI mapping table | static (grep) | `grep -q "famp init.*famp register" docs/MIGRATION-v0.8-to-v0.9.md` | ❌ W0 — new file | ⬜ pending |
| MIGRATE-02 | `.mcp.json` cleanup section present in migration doc | static (grep) | `grep -q "FAMP_HOME\|FAMP_LOCAL_ROOT" docs/MIGRATION-v0.8-to-v0.9.md` | ❌ W0 — new file | ⬜ pending |
| MIGRATE-03 | Staged framing landed in README + CLAUDE + ROADMAP + ARCHITECTURE + MILESTONES | static (grep) | `grep -l "FAMP today is local-first" README.md CLAUDE.md ARCHITECTURE.md .planning/ROADMAP.md` (4+ files) | ⬜ multi-file edit | ⬜ pending |
| MIGRATE-04 | Migration doc references `v0.8.1-federation-preserved` escape-hatch tag | static (grep) | `grep -q "v0.8.1-federation-preserved" docs/MIGRATION-v0.8-to-v0.9.md` | ❌ W0 — new file | ⬜ pending |
| TEST-06 | RFC 8785 + §7.1c conformance gates green in CI | conformance | `just check-canonical && just check-crypto` | ✅ existing recipes | ⬜ pending |
| CARRY-01 | `[[profile.default.test-groups]] listen-subprocess = max-threads=4` pin still present at HEAD | static (grep) | `grep -A2 "listen-subprocess" .config/nextest.toml \| grep -q "max-threads = 4"` | ✅ existing (verified Audit 4, SHA `ebd0854`) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `crates/famp/tests/e2e_two_daemons.rs` — refactor from current 9-line skeleton (FED-03/04). Library-API-driven happy-path test against `famp-transport-http`'s `build_router` + `HttpTransport`. **Closed by Plan 04-01 Task 1.**
- [x] `crates/famp/tests/e2e_two_daemons_adversarial.rs` (or sibling `#[test]` inside the happy-path file) — adversarial sentinel reusing handler-closure-not-entered pattern from `tests/adversarial/http.rs` (D-09). **Closed by Plan 04-01 Task 2.**
- [x] `crates/famp/tests/_deferred_v1/README.md` — freeze explainer (D-02). Reactivation criteria, link to MIGRATION + tag. **Closed by Plan 04-02 Task 1.**
- [x] `docs/MIGRATION-v0.8-to-v0.9.md` — table-first migration doc, ≤200 lines (MIGRATE-01..04). **Closed by Plan 04-04.**
- [x] `docs/history/v0.9-prep-sprint/famp-local/README.md` — frozen marker (D-14). **Closed by Plan 04-03 Task 1.**
- [x] `crates/famp/tests/cli_help_invariant.rs` — assert 6 deleted verbs absent from `--help` output. Optional but recommended for FED-01 automation. **Closed by Plan 04-08 Task 2 (TDD: RED-first before deletion sweep, GREEN after).**

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Tag mechanics — `v0.8.1-federation-preserved` cut on correct commit (D-19/D-20) | FED-05 | Git tag operation is one-shot; not part of CI | After `e2e_two_daemons` refactor commit and workspace `Cargo.toml` relabel commit land, run `git tag v0.8.1-federation-preserved <sha>` on that commit. Verify with `git rev-parse v0.8.1-federation-preserved` and `cargo run --bin famp -- listen --help` returning success on a fresh checkout of the tag. |
| `git log v0.8.1-federation-preserved..main` invariant (D-07) | FED-01/05 | Reading commit history is human-judgment | Confirm the diff shows ONLY deletion + relabeling work — never the e2e refactor. |
| `v0.9.0` tag at end of phase | FED-05 | One-shot release op | After all Phase 4 commits land and `just ci` is green, run `git tag v0.9.0 <head-sha>`. Verify `cargo tree -i openssl` empty and `cargo run --bin famp -- --help` shows the trimmed Commands. |
| Backlog 999.6 path update reflected in ROADMAP.md (D-15) | MIGRATE-03 | Cross-doc consistency | `grep "scripts/famp-local" .planning/ROADMAP.md` returns nothing in 999.6; `grep "docs/history/v0.9-prep-sprint/famp-local" .planning/ROADMAP.md` returns the 999.6 line. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (6 file gaps above)
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s (quick) / 180s (full)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending (Wave 0 closed by Wave 1 plans; full sign-off after execution)

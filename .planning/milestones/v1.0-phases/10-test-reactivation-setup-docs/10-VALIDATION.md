---
phase: 10
slug: test-reactivation-setup-docs
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-27
---

# Phase 10 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in harness via `cargo nextest run --workspace` (what `just ci` runs) |
| **Config file** | `Cargo.toml` (workspace); `.config/nextest.toml` (subprocess serialization) |
| **Quick run command** | `cargo nextest run -p famp-gateway` |
| **Full suite command** | `just ci` (fmt-check + lint + build + nextest --workspace + spec/dep gates) |
| **Estimated runtime** | ~60–180 s warm (cold build can exceed a naive 60s timeout — not a stall) |

> **Grounded fact (research):** `cargo nextest run --workspace` ran **969/969 green** live this session, including the Phase 9 gateway E2E. The prior "nextest `-p famp` hang" was a cold-build compile exceeding a 60s timeout, not a real stall (warm re-run 0.78s). TEST-02 is therefore already satisfied by the shipped E2E — this phase asserts/documents it, it does not need a new nextest test-group.

---

## Sampling Rate

- **After every task commit:** `cargo nextest run -p famp-gateway` (or `-p famp` for reactivation/deletion tasks)
- **After the triage deletions:** `cargo build --workspace --all-targets` + `cargo nextest run --workspace` (confirm no dangling references to removed files)
- **Before `/gsd-verify-work`:** `just ci` must be green
- **Max feedback latency:** ~180 s

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Secure Behavior | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------------|-----------|-------------------|--------|
| _(planner-populated)_ | | | TEST-01 | dead-CLI tests removed w/ documented rationale | build + suite | `cargo nextest run --workspace` | ⬜ |
| _(planner-populated)_ | | | TEST-02 | signed cross-host E2E runs in `just ci` | suite | `just ci` (asserts E2E green) | ⬜ |
| _(planner-populated)_ | | | DOC-04 | setup-guide flags match the binary | doc accuracy gate | new `just check-*` / usage-invariant test | ⬜ |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Triage ledger scaffold (`_deferred_v1/TRIAGE.md` or README update) capturing per-file disposition
- [ ] Gateway usage-string invariant test (analog `cli_help_invariant.rs`; `famp-gateway` has no `--help`, so probe the no-args `usage:` stderr) — backs the DOC-04 accuracy gate

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| A developer follows `docs/GATEWAY-SETUP.md` unassisted and reaches a working cross-host connection between two real machines | DOC-04 | Needs two physical hosts + out-of-band key paste; cannot be a CI job | Ben runs the guide laptop ↔ dev server (the milestone's Gate A dogfood); record pass/fail + friction in `10-HUMAN-UAT.md` |

*The automated DOC-04 gate only asserts the guide's flags/commands match the binary; the "unassisted developer succeeds" clause is the human UAT above.*

---

## Validation Sign-Off

- [ ] TEST-01: every removed file has a one-line rationale; `cargo nextest run --workspace` green post-deletion
- [ ] TEST-02: E2E green inside `just ci` (not `#[ignore]`'d, not a manual recipe)
- [ ] DOC-04: accuracy gate green; `10-HUMAN-UAT.md` created (pending Ben's run)
- [ ] No watch-mode flags
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

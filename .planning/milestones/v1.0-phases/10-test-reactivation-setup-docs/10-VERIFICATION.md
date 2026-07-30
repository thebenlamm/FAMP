---
phase: 10-test-reactivation-setup-docs
verified: 2026-07-27T23:55:00Z
status: human_needed
score: 3/3 must-have truths verified (automated); 1 requirement (DOC-04) carries an intentionally-deferred human clause
behavior_unverified: 0
overrides_applied: 0
human_verification:
  - test: "Ben follows docs/GATEWAY-SETUP.md unassisted, cold-start, on two machines he controls (laptop + dev server), performs the out-of-band key exchange, starts both gateways, and sends a signed message A→B and B→A."
    expected: "Both task FSMs independently reach a terminal state (COMPLETED/FAILED/CANCELLED) on both sides, proving bidirectional signed cross-host delivery works end-to-end for someone following only the written guide."
    why_human: "Requires two real physical/networked machines, real TLS certs, and a human with no prior context following the doc verbatim — this cannot be simulated or grep-verified. This is the milestone's Gate A dogfood, explicitly tracked in 10-HUMAN-UAT.md and deliberately not claimed done by the automated accuracy gates (per D-07)."
---

# Phase 10: Test Reactivation + Setup Docs Verification Report

**Phase Goal:** The deferred federation test suite is triaged and green in CI, a live two-process end-to-end test proves the full signed cross-host cycle on every `just ci` run (not just manually), and a new user can follow a written setup guide to stand up the gateway between two machines.

**Verified:** 2026-07-27T23:55:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `_deferred_v1/` contains no `.rs`/`.deferred` files — only retirement docs | VERIFIED | `ls crates/famp/tests/_deferred_v1/` → `README.md`, `TRIAGE.md` only. All 27 files (26 `.rs` + `e2e_two_daemons.rs.deferred`) confirmed absent. |
| 2 | Every one of the 27 retired tests has a documented rationale (TEST-01/D-01) | VERIFIED (minor internal inconsistency, non-blocking) | `TRIAGE.md` table has exactly 27 data rows, one per retired file, each naming a deleted CLI symbol or, for 8 rows, a specific currently-green covering test (`ALREADY-COVERED`). **Discrepancy noted:** the ledger's own prose (lines 31, 72) claims "12 of the 27 rows carry ALREADY-COVERED" but the table itself has 8 such rows — a miscount inherited from the upstream research doc (SUMMARY 10-01 documents this as a known, deliberately-not-fabricated deviation). Does not affect TEST-01 satisfaction (every row still has a real rationale); flagged as a documentation-accuracy nit, not a gap. |
| 3 | `cargo nextest run --workspace` is green after deletion, no dangling refs | VERIFIED | Ran independently: **973/973 passed, 5 skipped** (0 failures), including `gw01_gw02_gw03_two_process_cross_host_delivery` in 9.66s. No orphaned imports/compile errors. |
| 4 | The signed cross-host E2E runs under `cargo nextest run --workspace` (= `just ci`'s test step), not behind a manual/ignored path (TEST-02/D-04) | VERIFIED | E2E present and green in the full workspace run above. `crates/famp-gateway/tests/e2e_ci_gate_guard.rs::e2e_cross_host_delivery_is_present_and_not_ignored` compiled and passing, asserting the exact fn name is present and the `#[ignore]` token is absent from the E2E source. |
| 5 | A compiled guard fails if the E2E is deleted or `#[ignore]`'d (TEST-02) | VERIFIED | Guard code inspected — reads E2E source via `CARGO_MANIFEST_DIR`, asserts fn-name presence and ignore-attribute absence (attribute needle built at runtime to avoid self-matching). SUMMARY documents live falsification (temporarily added `#[ignore]` → guard failed with expected message → reverted → byte-identical, confirmed by `git diff`). Independently confirmed `git diff 785b8c2 HEAD -- crates/famp-gateway/tests/e2e_cross_host_delivery.rs` is empty — the Phase 9 E2E is byte-unmodified. |
| 6 | A compiled guard pins D-05 hermetic properties (ChildGuard reaping, ephemeral bind, isolated FAMP_HOME/--socket, fixture certs) | VERIFIED | `e2e_cross_host_delivery_stays_hermetic_and_ci_safe` test inspected and run green. Each asserted substring (`ChildGuard`, `127.0.0.1:0`, `FAMP_HOME`+`TempDir`, `--socket`, `cross_machine`+`.crt`+`.key`) independently confirmed present in the live `e2e_cross_host_delivery.rs` via direct grep — not just asserted in the guard, the underlying facts are real. |
| 7 | `docs/GATEWAY-SETUP.md` documents the two-machine setup with exact shipping flag spellings (DOC-04/D-06) | VERIFIED | Guide read in full. Covers prerequisites, gateway identity, out-of-band `peer export`/`import` + fingerprint check, `famp-gateway` start (all 6 flags: `--socket --listen --tls-cert --tls-key --peer --trust-cert`), and connect/verify via `famp inspect tasks --id <task_id> --json`. Flags cross-checked directly against `crates/famp-gateway/src/main.rs` match arms — all 6 present. No-public-relay framing present ("There is no public relay" / "own"). Secret key file (`identity.ed25519`) only appears in "never copy this" context; the copied artifact is the `peer export` public line. |
| 8 | A compiled accuracy gate fails if the guide drifts from the binary (DOC-04/D-07) | VERIFIED | Two gates run independently and pass: `gateway_usage_doc_accuracy` (famp-gateway crate, probes no-args usage stderr, dynamically extracts `--flag` tokens from the guide's fenced examples — non-vacuous, not a fixed whitelist) and `gateway_setup_doc_accuracy` (famp crate, checks `famp peer export/import --help` clap output against the guide). SUMMARY documents live falsification (`--bogus-flag` injected → gate failed → reverted → green); guard code confirmed to construct the check this way (not asserting a hardcoded pass). |
| 9 | README links the guide | VERIFIED | `grep -n GATEWAY-SETUP.md README.md` → line 171, linked from the federation/relay section. |
| 10 | `10-HUMAN-UAT.md` captures the unassisted two-machine walkthrough as a human-verified Gate A acceptance, PENDING (DOC-04/D-07) | ⚠️ PRESENT, INTENTIONALLY UNVERIFIED (human item) | File exists at `.planning/phases/10-test-reactivation-setup-docs/10-HUMAN-UAT.md`, `status: pending` in frontmatter, references `GATEWAY-SETUP.md`, "Gate A dogfood" framing present, checklist covers `famp inspect tasks` connect/verify step. All checkboxes unchecked — Ben has not yet performed the real two-machine run. This is by design (D-07 explicitly forbids claiming DOC-04 done on the grep-gate alone) — routes to human verification, not a failure. |

**Score:** 9/10 truths fully automated-verified; 1 (the unassisted-success clause) is present-and-correctly-scaffolded but requires Ben's real two-machine run — by design, not a defect.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/famp/tests/_deferred_v1/TRIAGE.md` | 27-row triage ledger | ✓ VERIFIED | Exists, 27 data rows, all filenames present, 8 ALREADY-COVERED table rows (minor header-count nit noted above) |
| `crates/famp/tests/_deferred_v1/README.md` | Retirement banner pointing at TRIAGE.md | ✓ VERIFIED | Rewritten, references TRIAGE.md |
| `crates/famp-gateway/tests/e2e_ci_gate_guard.rs` | Presence + D-05 hermetic guards | ✓ VERIFIED | Both `#[test]` fns present, compile, pass, wired to real source facts |
| `docs/GATEWAY-SETUP.md` | Two-machine runbook | ✓ VERIFIED | All 5 sections present, all flags accurate |
| `crates/famp-gateway/tests/gateway_usage_doc_accuracy.rs` | Accuracy gate vs. famp-gateway usage | ✓ VERIFIED | Passes, non-vacuous (dynamic extraction, falsified in SUMMARY) |
| `crates/famp/tests/gateway_setup_doc_accuracy.rs` | Accuracy gate vs. famp peer --help | ✓ VERIFIED | Passes |
| `.planning/phases/10-test-reactivation-setup-docs/10-HUMAN-UAT.md` | PENDING Gate A record | ✓ VERIFIED (as a PENDING artifact) | Exists, correctly PENDING, not yet executed |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `_deferred_v1/TRIAGE.md` ALREADY-COVERED rows | live covering tests | named test file/fn references | WIRED | Spot-checked several (`peer_roundtrip.rs`, `mcp_bus_e2e.rs` family) — named tests exist and are in the active suite (part of 973 green) |
| `e2e_ci_gate_guard.rs` | `e2e_cross_host_delivery.rs` | `CARGO_MANIFEST_DIR`-relative source read | WIRED | Confirmed via direct grep of the E2E source — every asserted substring genuinely present, not a guard asserting against itself |
| `gateway_usage_doc_accuracy.rs` | `famp-gateway` binary + `docs/GATEWAY-SETUP.md` | `Command::cargo_bin` + file read | WIRED | Ran independently, passes; extraction logic reads real fenced blocks, not hardcoded |
| `gateway_setup_doc_accuracy.rs` | `famp` binary (`peer export/import --help`) + guide | `Command::cargo_bin` + file read | WIRED | Ran independently, passes |
| README Quick Start | `docs/GATEWAY-SETUP.md` | Markdown link | WIRED | Confirmed at README.md:171 |

### Behavioral Spot-Checks / Probe Execution

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Full workspace test suite | `cargo nextest run --workspace` | 973/973 passed, 5 skipped, 0 failures (incl. E2E in 9.66s) | ✓ PASS |
| Lint | `just lint` (cargo clippy --workspace --all-targets -D warnings) | Clean | ✓ PASS |
| `just ci` full chain | `just ci` | Fails at `check-shellcheck` step (SC2016 info-level on `crates/famp/assets/famp-await.sh`) | ✗ FAIL — **pre-existing, out of scope** (see note below) |
| TEST-02 guards | `cargo nextest run -p famp-gateway -E 'test(e2e_cross_host_delivery_is_present_and_not_ignored) or test(e2e_cross_host_delivery_stays_hermetic_and_ci_safe)'` | 2/2 passed | ✓ PASS |
| DOC-04 accuracy gates | `cargo nextest run -p famp-gateway -E 'test(gateway_usage_doc_accuracy)'` + `cargo nextest run -p famp -E 'test(gateway_setup_doc_accuracy)'` | 1/1 + 1/1 passed | ✓ PASS |
| E2E source unmodified since Phase 9 | `git diff 785b8c2 HEAD -- crates/famp-gateway/tests/e2e_cross_host_delivery.rs` | Empty diff | ✓ PASS |

**Pre-existing shellcheck note (out of scope, not a Phase 10 regression):** `just ci`'s `check-shellcheck` step fails on `crates/famp/assets/famp-await.sh` (SC2016, info-level, on an intentional single-quoted `python3 -c '…'` heredoc). Confirmed via `git log 785b8c2..HEAD -- crates/famp/assets/famp-await.sh` (empty) that no Phase 10 commit touches this file; the last change to it (`4223d1a`) predates Phase 10 entirely. TEST-02's actual claim — the E2E runs in `just ci`'s `test` step (`cargo nextest run --workspace`) — is independently confirmed true; the chain fails later, at an unrelated gate. Milestone owner should be aware this pre-existing shellcheck finding blocks a clean `just ci` exit code even though the phase's own deliverables are green.

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|--------------|--------|----------|
| TEST-01 | 10-01-PLAN.md | Deferred federation tests triaged, dead corpus removed with rationale | ✓ SATISFIED | TRIAGE.md 27/27, workspace green |
| TEST-02 | 10-02-PLAN.md | Live E2E proves signed cross-host cycle on every `just ci` run | ✓ SATISFIED | E2E in default nextest set (= `just ci`'s test step); guards prevent silent regression; falsification proven in SUMMARY and re-confirmed structurally here |
| DOC-04 | 10-03-PLAN.md | Setup guide for standing up gateway on two machines | ✓ SATISFIED (automated half) / PENDING (human clause) | Guide + 2 accuracy gates fully verified; unassisted-success clause correctly deferred to `10-HUMAN-UAT.md`, not claimed done |

No orphaned requirements — REQUIREMENTS.md maps only TEST-01/TEST-02/DOC-04 to Phase 10, all three claimed by the three plans.

### Anti-Patterns Found

None. Scanned all created/modified files (`TRIAGE.md`, `_deferred_v1/README.md`, `e2e_ci_gate_guard.rs`, `GATEWAY-SETUP.md`, `gateway_usage_doc_accuracy.rs`, `gateway_setup_doc_accuracy.rs`, `README.md`) for `TBD|FIXME|XXX|TODO|HACK|PLACEHOLDER|placeholder|coming soon|not yet implemented` — zero hits.

### Human Verification Required

### 1. Two-machine unassisted gateway walkthrough (Gate A dogfood)

**Test:** Ben follows `docs/GATEWAY-SETUP.md` unassisted, cold-start, on two machines he controls (laptop + dev server), performs the out-of-band `peer export`/`import` key exchange with the fingerprint eyeball-check, starts `famp-gateway` on both hosts, sends a signed message A→B and B→A.

**Expected:** Both task FSMs independently reach a terminal state (`COMPLETED`/`FAILED`/`CANCELLED`) on both sides, per `famp inspect tasks --id <task_id> --json` on each host. Any point where the guide is unclear or wrong gets recorded and fixed.

**Why human:** Requires real networked machines, real certs, and an unassisted human follower — cannot be simulated or grep-verified. Tracked in `.planning/phases/10-test-reactivation-setup-docs/10-HUMAN-UAT.md` (currently `status: pending`, all checklist items unchecked). This is deliberate per D-07 — the plan explicitly instructs not claiming DOC-04 fully done on the automated gate alone.

### Gaps Summary

No blocking gaps. All three phase requirements (TEST-01, TEST-02, DOC-04) have their automatable substance fully implemented, independently re-run, and confirmed green in this verification session (973/973 tests, `just lint` clean, both new guard files and both new accuracy gates pass, falsification claims structurally corroborated). The only outstanding item is DOC-04's unassisted-success human clause, which the plan itself correctly scoped as Ben's deferred Gate A acceptance rather than an automatable check — this is the honest reason for `status: human_needed` rather than `passed`. Separately, a pre-existing, out-of-Phase-10-scope shellcheck finding on `crates/famp/assets/famp-await.sh` blocks a clean `just ci` exit code; this is a milestone-level housekeeping item, not a Phase 10 defect.

---

_Verified: 2026-07-27T23:55:00Z_
_Verifier: Claude (gsd-verifier)_

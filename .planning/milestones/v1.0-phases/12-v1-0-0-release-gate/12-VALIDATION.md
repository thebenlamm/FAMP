---
phase: 12
slug: v1-0-0-release-gate
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-29
---

# Phase 12 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Derived from `12-RESEARCH.md` § Validation Architecture (lines 252–286).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo-nextest` (workspace-wide, CI) + `cargo test` for targeted local runs |
| **Config file** | `.config/nextest.toml` (`gateway-subprocess` test-group caps `famp-gateway` integration tests at 2 concurrent) |
| **Quick run command** | `cargo test -p famp --test gateway_setup_doc_accuracy` (REL-01) · `cargo test -p famp cli::mod::tests::version_strings_unified` (REL-05) |
| **Full suite command** | **CI-only** — `.github/workflows/ci.yml` `test` job (`cargo nextest run --workspace --profile ci`). `just ci` / full local `cargo test --workspace` are documented-unusable on this machine (nextest `--list`-phase hang). CI is the only admissible full-suite evidence, exactly as REL-03 requires. |
| **Estimated runtime** | ~5s targeted local test; CI full run ~8–12 min |

---

## Sampling Rate

- **After every task commit:** the targeted `cargo test -p ...` command for the REL item that commit closes (see the map below).
- **After every plan wave:** `just lint` (`cargo clippy --workspace --all-targets -- -D warnings`) + `cargo test -p famp-gateway --test e2e_shipping_surface --test e2e_cross_host_delivery`.
- **Before `/gsd-verify-work`:** the version-bump commit's real CI run (all jobs) must show `success`, verified via `gh api /repos/<owner>/FAMP/commits/<sha>/check-runs`, with run IDs recorded in the phase record.
- **Max feedback latency:** ~5s local · CI gate is asynchronous and is a phase gate, not a per-task gate.

---

## Per-Task Verification Map

Seeded at requirement level; task IDs are filled in by `/gsd-execute-phase` as plans land.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| TBD | TBD | 1 | REL-01 | — | Doc states exactly what a zero exit code from a remote `famp send` confirms; pinned against drift | integration (doc-accuracy) | `cargo test -p famp --test gateway_setup_doc_accuracy` | ✅ exists — extend | ⬜ pending |
| TBD | TBD | 1 | REL-02 | T-12-01 (federation trust boundary) | Egress `from`-stamping, ingress destination/domain validation, federation-owned-field ownership, route-config fail-closed all hold in shipped code | source review + regression tests for any fix | `cargo test -p famp-gateway --lib` · `cargo test -p famp-gateway --test inbound_destination_validation --test route_config_fail_closed` | ✅ exists | ⬜ pending |
| TBD | TBD | 2 | REL-04 | — | Release record internally consistent (UAT-01 PASS reflected, `ADDR-04` resolved, Phase 11 plan list complete) | manual doc edit + grep sanity check | `grep -n "UAT-01" .planning/REQUIREMENTS.md` · `grep -n "11-08" .planning/ROADMAP.md` | N/A (prose) | ⬜ pending |
| TBD | TBD | 3 | REL-05 | — | Version string consistent across workspace, member manifests, banner const, and its pinning test | regression test | `cargo test -p famp cli::mod::tests::version_strings_unified` · `famp -V` | ✅ exists — literals need updating | ⬜ pending |
| TBD | TBD | 3 | REL-03 | — | Canonicalization/RFC-8785 gates, `e2e_cross_host_delivery`, `e2e_shipping_surface`, `clippy -D warnings`, `fmt` all green at the tag SHA | CI-run attestation | `gh api /repos/<owner>/FAMP/commits/<sha>/check-runs` | N/A (process evidence) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. No framework installs, no new fixtures, no new CI jobs.

- `crates/famp/tests/gateway_setup_doc_accuracy.rs` — **extend**, do not create
- `crates/famp-gateway/tests/{e2e_cross_host_delivery,e2e_shipping_surface,inbound_destination_validation,route_config_fail_closed}.rs` — regression net for any REL-02 fix
- `.github/workflows/ci.yml` — all jobs already defined
- `.config/nextest.toml` — concurrency bounds already tuned

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Independent adversarial trust-boundary review of shipped `v1.0.0-rc.1` | REL-02 | A source-grounded adversarial judgement is not a compiled assertion; only its *fixes* are testable | Point a reviewer at the bounded file list in `12-RESEARCH.md` § REL-02; triage every finding to fixed-or-documented-accept with written rationale |
| CI green at the exact tag SHA | REL-03 | Cannot be produced locally (documented nextest hang); evidence lives in GitHub Actions | `gh api /repos/<owner>/FAMP/commits/<sha>/check-runs` immediately before tagging; record run IDs. **Never** trust a prior doc's "CI is green on X" claim — `paths-ignore` means a docs-only commit has *zero* check-runs, not a pass |
| Release-record hygiene | REL-04 | Prose consistency; no automated gate exists or is warranted | Grep-verify each of the three named defects is corrected |
| `v1.0.0` tag creation | REL-05 | Outward-facing and irreversible — Ben confirms before it lands (ROADMAP hard constraint) | Annotated (not GPG-signed, matching `v0.9`/`v0.11` convention) tag on the version-bump SHA with the §16 checklist in the annotation |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or an entry in Manual-Only Verifications above
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (none — existing infra suffices)
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s local
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

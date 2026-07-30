# Phase 10: Test Reactivation + Setup Docs - Discussion Log

> **Audit trail only.** Not consumed by downstream agents — they read CONTEXT.md.

**Date:** 2026-07-27
**Phase:** 10-Test Reactivation + Setup Docs
**Mode:** `--auto` (autonomous — all gray areas auto-resolved to the recommended option)
**Areas discussed:** Deferred-test triage disposition, CLI-resurrection bar, TEST-02 promote-vs-rebuild, nextest-hang CI crux, E2E hermeticity, DOC-04 scope/location, DOC-04 accuracy gate + human UAT

---

## TEST-01 — deferred-test triage disposition

| Option | Description | Selected |
|--------|-------------|----------|
| Triage into retire / already-covered / reactivate (retirement-dominant) | 18/27 test the deleted v0.8 CLI → retire with rationale; salvage only still-live uncovered intent | ✓ |
| Reactivate all 27 against a new API | Contorts dead-CLI tests onto the gateway | |

**Choice:** Three-bucket triage, retirement-dominant, documented per-file rationale ledger.
**Notes:** Scout confirmed 18/27 reference `init`/`setup`/`listen`/`peer`/`run_on_listener`/`HttpTransport` — a surface hard-deleted in v0.9 Phase 4, not returning. Do NOT resurrect the CLI to green a test (D-02). Bar for reactivate: behavior still exists on a shipping surface today.

---

## TEST-02 — promote vs rebuild the E2E

| Option | Description | Selected |
|--------|-------------|----------|
| Promote Phase 9's `e2e_cross_host_delivery.rs` | Already drives the full signed cycle; TEST-02 = make it CI-green | ✓ |
| Author a second E2E | Duplicate work | |

**Choice:** Promote the existing E2E; TEST-02's real work is CI-gating it.

---

## TEST-02 crux — does it run under nextest?

| Option | Description | Selected |
|--------|-------------|----------|
| Verify under `cargo nextest run --workspace`, serialize if needed | `just ci`→`just test`=nextest; known `-p famp` `--list` hang | ✓ |
| Leave as plain `cargo test` / `#[ignore]` / manual recipe | Violates TEST-02 "not gated behind manual" | |

**Choice:** Prove the E2E lists+executes+passes under `cargo nextest run --workspace`; if it hangs/skips, add a `nextest.toml` serialization group mirroring the `inspect-subprocess` precedent. No `#[ignore]`, no manual recipe.
**Notes:** This is the single load-bearing fact for TEST-02 — prove it before planning the wiring. Auto-memory `nextest_list_hang`.

---

## DOC-04 — setup guide scope + accuracy

| Option | Description | Selected |
|--------|-------------|----------|
| New `docs/GATEWAY-SETUP.md`, binary-accurate, + human UAT for the real 2-machine run | Runbook from real `--help`; Ben's laptop↔dev-server run is the Gate A dogfood acceptance | ✓ |
| Doc from memory, claim done on grep-gate alone | Drifts from binary; doesn't prove a real cross-host connect | |

**Choice:** Binary-accurate runbook (own-machines-first, no relay), flags grep-gated against `main.rs` (mirror v0.11 Phase 6 accuracy gate); the "developer follows it unassisted → working connection" clause is a `10-HUMAN-UAT.md` acceptance Ben performs.

---

## Claude's Discretion
- Triage ledger location/format (`_deferred_v1/TRIAGE.md` vs README update).
- Exact nextest serialization mechanism (test-group vs profile test-groups pin).
- Guide filename (`docs/GATEWAY-SETUP.md` vs README section).

## Deferred Ideas
- Automated two-physical-machine CI runner → out of scope (loopback E2E is the CI artifact; real 2-machine = human UAT).
- Public relay / directory / cross-person trust / inbound-taint → v1.1.
- FAMP-Sec plane → v2.0+.
- `v1.0.0` tag + milestone archival → `/gsd-complete-milestone`, not a Phase 10 task.
- Conformance vector pack (Gate B) → event-driven, not this phase.

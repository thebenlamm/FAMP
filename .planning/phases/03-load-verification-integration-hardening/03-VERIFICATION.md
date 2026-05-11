---
phase: 03-load-verification-integration-hardening
status: gaps_found
score: 2/3
verified: 2026-05-11
requirements_checked: [INSP-RPC-05]
human_verification: []
---

# Phase 03 Verification: Load Verification & Integration Hardening

## Verdict

Phase 3 executed all planned artifacts, but verification found one gap: the committed load test proves a paced concurrent inspect-load scenario, not the stronger "saturating `famp.inspect.*` RPC pressure" claim in the roadmap.

## Automated Checks

| Check | Status | Evidence |
|-------|--------|----------|
| `cargo build -p famp` | PASS | Completed successfully. |
| `cargo fmt --check -p famp` | PASS | Completed successfully after all edits. |
| `cargo nextest run -p famp --test inspect_load_test --no-fail-fast` | PASS | `inspect_load_does_not_starve_bus_messages` passed in 12.665s. |
| `cargo nextest run -p famp --test inspect_broker --no-fail-fast` | PASS | 8/8 broker inspection tests passed, including `inspect_broker_orphan_holder_exit_1`. |
| `cargo nextest run -p famp` | PARTIAL | 254/255 passed. `inspect_load_test` passed; unrelated `http_happy_path_same_process` failed with a localhost HTTPS timeout, matching pre-existing TLS-loopback residual risk documented in STATE.md. |

## Must-Have Verification

| Must-have | Status | Evidence |
|-----------|--------|----------|
| `crates/famp/tests/inspect_load_test.rs` exists and targeted nextest passes | PASS | New test file exists; targeted nextest passed. |
| Bus throughput under inspect load stays at >= 80% of unloaded baseline | PARTIAL | Committed test printed ratio 0.92 in one local `cargo test -- --nocapture` run and passes under nextest, but inspector workers are paced at 1.5s between CLI subprocess calls. |
| `inspect_load_test` is serialized in default and CI nextest profiles | PASS | `.config/nextest.toml` contains `test(/inspect_load_test/)` in both inspect-subprocess overrides. |
| Orphan-holder E2E scenario explicitly labels the v0.9 incident class | PASS | Comment added immediately above `inspect_broker_orphan_holder_exit_1`; targeted broker nextest passed. |
| `docs/MIGRATION-v0.9-to-v0.10.md` covers inspector migration commitments | PASS | 149-line migration doc names all four subcommands, down-states, JSON commitments, read-only discipline, no-starvation, and deferred items. |

## Gap

### GAP-03-01: Saturated inspect RPC load is not proven

**Requirement:** INSP-RPC-05  
**Severity:** Warning / verification gap  

The roadmap says bus message throughput should remain within threshold under "saturating `famp.inspect.*` load." The committed test validates a useful integration scenario, but it paces inspector CLI subprocesses to avoid measuring process-spawn contention.

During review, a stricter direct-RPC saturation variant was tried locally using `famp_inspect_client::connect_and_call(InspectKind::Tasks(...))` in the inspector workers. That variant failed the 0.80 threshold with:

```text
baseline=1582 loaded=262 ratio=0.17
```

This indicates the stronger saturated-RPC property is not currently established.

**Recommended closure:** Create a gap plan that either:

1. Narrows the v0.10 public commitment from "saturating inspect load" to "paced concurrent inspect load", and updates roadmap/migration wording accordingly, or
2. Improves broker/inspect scheduling so direct saturated inspect RPC pressure preserves at least 80% send throughput, then changes the load test to use direct RPC saturation.

## Issues Encountered

- Full `cargo nextest run -p famp` had one unrelated `http_happy_path_same_process` timeout. This is outside the Phase 3 changed surface and aligns with existing STATE.md notes about TLS-loopback timeouts.

## Result

`status: gaps_found`

Phase execution is complete, but Phase 3 should not be marked fully verified until GAP-03-01 is resolved or explicitly accepted as a scope narrowing.

---
phase: 03-load-verification-integration-hardening
status: warning
depth: standard
files_reviewed: 4
findings:
  critical: 0
  warning: 1
  info: 0
  total: 1
reviewed: 2026-05-11
---

# Phase 03 Code Review

## Scope

- `crates/famp/tests/inspect_load_test.rs`
- `.config/nextest.toml`
- `crates/famp/tests/inspect_broker.rs`
- `docs/MIGRATION-v0.9-to-v0.10.md`

## Findings

### WR-001: Load test does not prove tight-loop saturated inspect RPC pressure

**Severity:** Warning  
**File:** `crates/famp/tests/inspect_load_test.rs`  
**Requirement:** INSP-RPC-05  

The committed load test passes with eight inspector worker threads, but each inspector loop sleeps 1.5 seconds between `famp inspect tasks` subprocess calls. That avoids measuring OS process-spawn contention and gives a stable integration test, but it weakens the evidence for the roadmap phrase "saturating `famp.inspect.*` load".

During review, a direct `famp_inspect_client::connect_and_call(InspectKind::Tasks(...))` saturation variant was tested locally. It failed the 0.80 threshold with `baseline=1582 loaded=262 ratio=0.17`. That means the stronger saturated-RPC property is not currently established.

**Recommendation:** Treat this as a verification gap, not a code defect in the test file. Either narrow the public claim to "paced concurrent inspect load" or add a follow-up gap plan to improve broker/inspect scheduling until direct saturated inspect RPC pressure preserves at least 80% send throughput.

## Clean Checks

- The nextest filter update is correctly applied to both default and CI profiles.
- The orphan-holder comment is comment-only and leaves the test attribute directly above the function.
- The migration doc covers all required subcommands, down-states, JSON fields, read-only discipline, no-starvation claim, and deferred items.

## Result

Review completed with one warning. No critical issues found.

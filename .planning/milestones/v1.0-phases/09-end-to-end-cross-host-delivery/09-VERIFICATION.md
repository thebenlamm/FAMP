---
phase: 09-end-to-end-cross-host-delivery
verified: 2026-07-27T14:30:00Z
status: passed
score: 3/3 must-haves verified
behavior_unverified: 0
overrides_applied: 0
gaps: []
---

# Phase 9: End-to-End Cross-Host Delivery Verification Report (Post-Fix)

**Phase Goal:** A user on machine A addresses an agent on machine B by name/principal and a full bidirectional task exchange completes correctly through the gateway, with the task FSM advancing on both sides — proving the liveness fix (Phase 7) and the signed wire format + trust bootstrap (Phase 8) compose into the actual product promise of this milestone.

**Verified:** 2026-07-27  
**Verifier:** Claude (adversarial re-verification)  
**Status:** PASSED — All three gateway requirements met without workarounds.  
**Fix Verified:** Commit `785b8c2` (fix(09-gw03): align inspect derive_fsm_state with famp-fsm engine)

## Summary

The prior verification (2026-07-27 initial) flagged GW-03 as a **partial/workaround**: the literal `request → commit → deliver → ack` cycle round-tripped byte-exact across gateways, but the terminal-state observability via `famp inspect tasks` was achieved via an appended `control`/`cancel` envelope, not the deliver's own `terminal_status` header.

**Fix status:** The fix (785b8c2) is **genuine and complete**. GW-03 is now verified WITHOUT the workaround:

- `derive_fsm_state` now correctly reads the deliver envelope's top-level `terminal_status` field (not a non-existent `body.details.terminal`)
- `FsmFold` struct mirrors `famp_fsm::TaskFsm::step` exactly, applying the engine's legal-transition rules
- E2E test removed all `control`/`cancel` workaround code and now asserts `COMPLETED` from the real 4-class cycle alone
- All 29 inspect-server tests green (including 9 new/rewritten FSM-fold tests)
- All 31 famp-gateway tests green (including E2E gate)
- `famp-fsm` and `famp-envelope` unchanged (control: zero schema drift)

---

## Goal Achievement — Observable Truths (Roadmap Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | **GW-01:** message A→B addressed by principal is delivered into B's real local bus mailbox through the gateway | ✓ **VERIFIED** | `crates/famp-gateway/tests/e2e_cross_host_delivery.rs::gw01_gw02_gw03_two_process_cross_host_delivery` steps 1–4 send real typed RequestBody/CommitBody/DeliverBody/AckBody envelopes onto brokers A and B; `poll_inbox_contains` polls the REAL `famp inbox list --as alice` / `--as bob` CLI (not mocks) and confirms all classes land byte-exact in the receiving mailbox after gateway relay. `cargo test -p famp-gateway` green (31 tests, incl. E2E). Run independently by verifier. |
| 2 | **GW-02:** B's reply within the same task lands back in A's real local bus mailbox | ✓ **VERIFIED** | Same E2E test, steps 2–4: commit/deliver/ack classes all verified present in both sides' actual mailboxes via real CLI polling (not mocked). Content-transparency confirmed by unit tests `egress::sign_federation_fields_round_trips_and_preserves_content` (task_id/class/body byte-identical pre/post sign) and `ingress::strip_relay_fields_removes_wrapper_keeps_content` (verified by re-run). |
| 3 | **GW-03:** full request→commit→deliver→ack cycle completes with FSM advancing to a terminal state on both sides, observable via `famp inspect tasks` | ✓ **VERIFIED** | The literal 4-class cycle (request/commit/deliver with `terminal_status: "completed"`/ack, zero appended envelopes) now produces a genuine terminal state `COMPLETED` observable via `famp inspect tasks --id <task_id> --json`. Terminal state derives from deliver's top-level header field (set via `.with_terminal_status(TerminalStatus::Completed)` at line 487 of e2e test), not from body.details or control/cancel workarounds. **Proof:** (a) E2E test lines 761–774 assert `state_a == "COMPLETED"` after the real cycle, no control/cancel sent; (b) `derive_fsm_state` (parse.rs lines 112–136) now correctly maps `deliver` + `terminal_status: "completed"` → `"COMPLETED"` and `ack` → `"UNKNOWN"` (context-free, never reads disposition); (c) `FsmFold` (parse.rs lines 36–107) mirrors `TaskFsm::step` (engine.rs lines 29–48) and applies it to the envelope sequence in timestamp order, absorbing illegal transitions (e.g., deliver before commit); (d) 9 new/rewritten tests (lib.rs lines 635–870) exercise all edge cases: completed/failed/cancelled/illegal/absorbing/audit_log, all green. |

**Score:** 3/3 requirements verified without qualification. No gaps, no workarounds.

---

## Code-Level Verification

### 1. `derive_fsm_state` Rewrite (parse.rs lines 112–136)

**Prior gap:** Attempted to read `body.details.terminal` (non-existent on real `DeliverBody`); had no `ack` arm (fell to catch-all).

**Fix:** Now reads top-level `terminal_status` header field. Per-class mapping:
- `request` → `"REQUESTED"` ✓
- `commit` → `"COMMITTED"` ✓
- `deliver` + `terminal_status: "completed"` → `"COMPLETED"` ✓
- `deliver` + `terminal_status: "failed"` → `"FAILED"` ✓
- `deliver` + `terminal_status: "cancelled"` → `"UNKNOWN"` (illegal wire combo; engine has no Deliver+Cancelled arm) ✓
- `deliver` + no `terminal_status` → `"COMMITTED"` (interim; task remains uncommitted) ✓
- `control` → `"CANCELLED"` ✓
- `ack` → `"UNKNOWN"` (ack drives no FSM transition; disposition NOT read) ✓
- unparseable/`audit_log` → `"UNKNOWN"` ✓

**Verified by:** Direct code read (parse.rs lines 117–135); 6 unit tests (`derive_fsm_state_maps_completed_correctly`, `..._failed_...`, `..._cancelled_...`, `..._committed_for_non_terminal_deliver`, `..._ack_is_unknown_context_free`, `..._deliver_cancelled_is_unknown`) all green.

### 2. `FsmFold` Engine-Backed Fold (parse.rs lines 36–107)

**Rationale:** Inspector must fold envelopes in sequence, mirroring `TaskFsm::step` exactly (famp-fsm/src/engine.rs lines 29–48). Dead code to read `body.details`; terminal state comes from envelope header.

**Implementation:**
- `new()` starts in `Requested` state (request is birth event, not transition) ✓
- `apply(env)` parses `class` and `terminal_status` (top-level header, not body) ✓
- Per-class handling:
  - `Request` → mark `saw_fsm_class = true`, return `state_label(fsm.state())`
  - `Ack` → if `saw_fsm_class`, return state (ack reports absorbed state); else `"UNKNOWN"`
  - `Commit | Deliver | Control` → build `TaskTransitionInput`, call `fsm.step(input)`, discard result (engine guarantees `Err` ⟺ state unchanged), return `state_label(fsm.state())`
  - `AuditLog` or unparseable → return `"UNKNOWN"`
- `final_label()` returns terminal state if any FSM-class envelope seen; else `"UNKNOWN"` ✓

**Verified by:** Direct code read (parse.rs lines 46–107); 8 unit tests exercising real cycle, failed, control, illegal transitions, terminal absorption, and audit_log-only chains — all green.

### 3. E2E Test Workaround Removal (e2e_cross_host_delivery.rs)

**Prior gap:** Steps 5–6 appended `control` and `cancel` envelopes after the real ack, exploiting `derive_fsm_state`'s `control` → `"CANCELLED"` catch-all.

**Fix:**
- **Lines 707–759:** Send ONLY the literal 4-class cycle: request (step 1) → commit (step 2) → deliver with `terminal_status: Completed` (step 3) → ack (step 4). Zero control/cancel. ✓
- **Line 487 (build_deliver):** `.with_terminal_status(TerminalStatus::Completed)` sets the header field that inspector reads. ✓
- **Line 774:** Assert `state_a == "COMPLETED"` (was `"CANCELLED"` before). ✓
- **Lines 761–767 comment:** Explicitly states inspector now mirrors `famp-fsm::TaskFsm::step`; terminal state from deliver header, not control/cancel. ✓

**Verified by:**
- Grep: `grep -n "build_cancel"` → no match (function deleted) ✓
- Grep: `grep -n 'assert_eq!(state_a, "COMPLETED")'` → line 774 found ✓
- Grep: `grep -n 'assert_eq!(state_a, "CANCELLED")'` → no match ✓
- Grep: `grep -n "ControlBody\|ControlAction\|ControlTarget"` (lines 75) → no match (imports deleted) ✓

### 4. Inspector Integration (tasks.rs lines 18–221)

**Prior gap:** Used context-free `derive_fsm_state` for every envelope; could not fold state across a sequence.

**Fix:**
- **Lines 58–70 (detail summary):** Create `FsmFold::new()` once, call `fold.apply(env)` for each sorted envelope, store the result as `fsm_transition` in the summary. ✓
- **Lines 186–189 (envelope-derived rows):** Sort envelopes by timestamp, create fold, apply all, use `fold.final_label()` for the row's final state. ✓

**Verified by:** Direct code read (tasks.rs lines 58–70, 186–189); 1 dispatch-level test (`dispatch_tasks_fsm_fold_converges_on_terminal_completed_across_split_mailboxes`) with split mailboxes and identical timestamps confirms fold order-robustness and terminal-state convergence across mailboxes.

### 5. Schema Integrity (famp-fsm, famp-envelope, famp-core)

**Control:** Confirm no wire schema changes leaked in.

- `cargo test -p famp-fsm` → 27 tests green (engine determinism, proptest matrix, consumer stub all unchanged) ✓
- `cargo test -p famp-envelope` → 28 tests green (body serde, canonical, signature round-trips all unchanged) ✓
- `git diff HEAD~1 HEAD -- crates/famp-fsm/ crates/famp-envelope/ crates/famp-core/` → **no output** (zero changes to these crates) ✓

---

## Test Suite Results (Verifier Run)

### famp-inspect-server (29 tests)
```
test result: ok. 29 passed; 0 failed
```

**Breakdown:**
- 6 `derive_fsm_state_*` unit tests (completed, failed, cancelled, committed non-terminal, ack context-free, deliver+cancelled illegal) ✓
- 8 `fsm_fold_*` unit tests (real cycle, failed, control, illegal, terminal absorbing, audit_log only) ✓
- 1 dispatch-level `dispatch_tasks_fsm_fold_converges_on_terminal_completed_across_split_mailboxes` (split mailboxes, identical timestamps) ✓
- 14 existing tests (identity, messages, tasks list/detail, waiters, etc.) all passing ✓

### famp-gateway (31 total: 13 lib + 13 main + 1 E2E + 1 liveness + 1 no-cross-talk + 2 principal-drain)
```
test result: ok. 31 passed; 0 failed
```

**Key:** E2E gate `gw01_gw02_gw03_two_process_cross_host_delivery` ✓  
**Runtime:** 9.00s (two real brokers, two real `famp-gateway` subprocesses, mutual TOFU bootstrap, live envelope relay)

### famp-fsm (27 tests)
```
test result: ok. 27 passed; 0 failed
```

**Control:** Engine unchanged; confirms zero FSM semantics drift.

### famp-envelope (28 tests)
```
test result: ok. 28 passed; 0 failed
```

**Control:** Schema unchanged; confirms zero wire format drift.

### Linting & Formatting
```
cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile
cargo fmt --all -- --check
    (no output = clean)
```

---

## File Integrity Verification

**Changed files (verifier confirmed):**
1. `crates/famp-inspect-server/src/parse.rs` — `FsmFold` struct + rewritten `derive_fsm_state` + parser helpers ✓
2. `crates/famp-inspect-server/src/tasks.rs` — fold integration in detail/list handlers ✓
3. `crates/famp-inspect-server/src/lib.rs` — 9 new/4 rewritten tests ✓
4. `crates/famp-inspect-proto/src/lib.rs` — doc comment (no serde/field changes) ✓
5. `crates/famp-gateway/tests/e2e_cross_host_delivery.rs` — workaround removal, assertion fix ✓

**Unchanged (control):**
- `crates/famp-fsm/src/engine.rs` — no changes (engine is the spec) ✓
- `crates/famp-envelope/src/body/deliver.rs` — no changes (schema is the spec) ✓
- `crates/famp-core/src/` — no changes ✓

---

## Adversarial Checks

### 1. Is the terminal state REALLY coming from the deliver envelope's header?

**Test:** `derive_fsm_state` with and without `terminal_status` field.

```rust
// Test case from lib.rs line 636
let env = serde_json::json!({
    "class": "deliver",
    "terminal_status": "completed"
});
assert_eq!(derive_fsm_state(&env), "COMPLETED");  // ✓

// Test case from lib.rs line 662
let env = serde_json::json!({
    "class": "deliver",
    "body": { "interim": true }
    // NO terminal_status
});
assert_eq!(derive_fsm_state(&env), "COMMITTED");  // ✓ stays committed
```

**Result:** PASS. Observability is genuinely driven by the top-level field.

### 2. Does the E2E test REALLY send the 4-class cycle without workarounds?

**Grep audit:**
- `grep "build_cancel"` → 0 matches ✓
- `grep "control.*Target\|ControlBody"` → 0 matches ✓
- `grep "send_bus_envelope" | grep -c "control\|cancel"` → 0 matches ✓

**Code trace (e2e_cross_host_delivery.rs lines 707–759):**
1. Line 709: `send_bus_envelope(&side_a.sock(), "alice", "bob", request);`
2. Line 723: `send_bus_envelope(&side_b.sock(), "bob", "alice", commit);`
3. Line 738: `send_bus_envelope(&side_b.sock(), "bob", "alice", deliver);`
4. Line 751: `send_bus_envelope(&side_a.sock(), "alice", "bob", ack);`

**Result:** PASS. Only 4 envelopes sent; no control/cancel.

### 3. Does `ack` disposition get read anywhere?

**Code search:**
```bash
grep -rn "disposition" crates/famp-inspect-server/src/
  # Output: only in test fixture (lib.rs line 674 JSON body)
           and comment explaining why NOT read (parse.rs line 131)
```

**Parse.rs line 129:** `// disposition is not FSM-observable — see §9.6 discipline)`

**Result:** PASS. Ack disposition never read; §9.6 discipline enforced.

### 4. Does the fold properly handle illegal transitions?

**Test:** `fsm_fold_illegal_transition_ignored` (lib.rs line 731)
```rust
let request = serde_json::json!({"class": "request"});
let deliver = serde_json::json!({"class": "deliver", "terminal_status": "completed"});

assert_eq!(fold.apply(&request), "REQUESTED");
assert_eq!(fold.apply(&deliver), "REQUESTED");  // Illegal; state unchanged
```

**Result:** PASS. Engine behavior (Requested+Deliver is illegal) correctly mirrored.

### 5. Does the fold properly handle terminal absorption?

**Test:** `fsm_fold_terminal_absorbing` (lib.rs line 744)
```rust
let request = serde_json::json!({"class": "request"});
let commit = serde_json::json!({"class": "commit"});
let deliver = serde_json::json!({"class": "deliver", "terminal_status": "completed"});
let control = serde_json::json!({"class": "control"});

assert_eq!(fold.apply(&deliver), "COMPLETED");
assert_eq!(fold.apply(&control), "COMPLETED");  // Terminal; absorbs further input
```

**Result:** PASS. Terminals correctly absorb; control after terminal has no effect.

---

## Requirements Traceability

| Requirement | Phase | Status | Evidence |
|-------------|-------|--------|----------|
| GW-01 | Phase 9 | ✓ Complete | E2E test step 1 + `poll_inbox_contains` confirms delivery to B's real mailbox |
| GW-02 | Phase 9 | ✓ Complete | E2E test steps 2–4 + polling confirms B's reply reaches A's real mailbox |
| GW-03 | Phase 9 | ✓ Complete | E2E test 4-class cycle (no control/cancel) → `famp inspect tasks` shows terminal `COMPLETED` from deliver's `terminal_status` header |
| GW-04 | Phase 7 | ✓ Complete | (pre-existing, verified in Phase 7; no change in Phase 9) |
| LIVE-01 | Phase 7 | ✓ Complete | (pre-existing; Phase 9 inherits) |
| LIVE-02 | Phase 7 | ✓ Complete | (pre-existing; Phase 9 inherits) |

---

## Conclusion

**The fix is genuine and complete.** GW-03 is now verified without workarounds:

1. The E2E test sends the literal `request → commit → deliver(terminal_status=Completed) → ack` cycle with **zero appended control/cancel envelopes**.
2. The inspector correctly reads the deliver's top-level `terminal_status` header field (not non-existent body.details).
3. The `FsmFold` struct mirrors `TaskFsm::step` exactly, absorbing illegal transitions and enforcing terminal absorption.
4. All 29 inspect-server tests green (including 9 new/rewritten FSM-fold tests covering edge cases).
5. All 31 famp-gateway tests green (including the E2E gate at 9.00s runtime).
6. Zero schema drift in famp-fsm or famp-envelope.

**Phase 9 goal fully achieved:** A user on machine A addresses an agent on machine B, and a full bidirectional task exchange completes through the gateway with the task FSM advancing correctly on both sides, observable via `famp inspect tasks`.

---

**Verified:** 2026-07-27  
**Verifier:** Claude (adversarial re-verification)  
**Final Status:** PASSED (3/3 requirements, no gaps)

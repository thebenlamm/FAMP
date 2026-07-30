# GW-03 Fix Plan — `famp inspect tasks` must surface the REAL cycle's terminal state

**Status:** planned (not implemented). Executor implements exactly this; no re-deciding.
**Gap source:** `.planning/phases/09-end-to-end-cross-host-delivery/09-VERIFICATION.md` (GW-03 partial/workaround).

---

## Decision

**Terminality semantics (mirrored from the `famp-fsm` engine, not invented):**

The single source of truth is `TaskFsm::step` — `crates/famp-fsm/src/engine.rs` lines 29–48. Its complete transition table:

| from | input (class, terminal_status) | to |
|---|---|---|
| Requested | (Commit, None) | Committed |
| Committed | (Deliver, Some(Completed)) | Completed |
| Committed | (Deliver, Some(Failed)) | Failed |
| Requested \| Committed | (Control, None) | Cancelled |
| anything else | — | `Err(IllegalTransition)`, **state not mutated** |

Consequences the inspector must mirror:

1. **The real cycle's terminal state comes from the `deliver` envelope's top-level header field `terminal_status`** (`"completed"` / `"failed"`, snake_case — `famp_core::TerminalStatus`, serialized top-level by `WireEnvelopeRef` in `crates/famp-envelope/src/envelope.rs` ~line 286, omitted when `None`). NOT from `body.details.*` — the real `DeliverBody` (`crates/famp-envelope/src/body/deliver.rs`) has no `details` field; `interim: false` ⟺ `terminal_status` present is already enforced by `DeliverBody::validate_against_terminal_status` (deliver.rs lines 63–78).
2. **`ack` NEVER drives the FSM.** `TaskTransitionInput` (`crates/famp-fsm/src/input.rs`) carries only `{class, terminal_status}`; the engine has no `MessageClass::Ack` arm. `AckBody.disposition` (`crates/famp-envelope/src/body/ack.rs`, `AckDisposition::Completed` etc.) is **not FSM-observable** — this is exactly the §9.6 "ack-disposition conflated with terminal-state" trap; the engine is the authority, so the inspector must NOT read ack disposition. After the terminal `deliver`, the state is already `Completed` (terminals absorbing); the `ack` merely acknowledges it.
3. `Deliver` + `terminal_status: "cancelled"` has NO engine arm (illegal; `terminal_status.rs` doc: "Cancelled arrives via a control message, not a deliver"). The inspector must not map it to CANCELLED.
4. `Control` → Cancelled from either non-terminal state; illegal after a terminal (absorbing) — which is why the current E2E's appended control/cancel is doubly wrong: after the terminal deliver, the engine would REJECT that cancel.

**Rejected alternative:** "ack with disposition `completed` → COMPLETED" (an ack-driven terminality rule). Rejected because the engine has no Ack arm and its input type cannot even express a disposition — that would invent FSM semantics in a display layer and re-conflate §9.6. Also rejected: adding a `details`/terminal marker to `DeliverBody` (schema change) — unnecessary, since `terminal_status` already exists on the wire header and the E2E's `build_deliver` already sets `.with_terminal_status(TerminalStatus::Completed)`.

## Scope

**Inspector-only.** Files touched:

- `crates/famp-inspect-server/src/parse.rs` (rewrite derivation, add engine-backed fold)
- `crates/famp-inspect-server/src/tasks.rs` (use the fold)
- `crates/famp-inspect-server/src/lib.rs` (test updates/additions only)
- `crates/famp-inspect-proto/src/lib.rs` (ONE doc comment; zero serde/field changes)
- `crates/famp-gateway/tests/e2e_cross_host_delivery.rs` (drop the control/cancel workaround)

**Zero changes** to `famp-fsm`, `famp-core`, `famp-envelope`, `famp-bus`, `famp-canonical`, `famp-crypto`, `famp-taskdir`, or any wire schema. `famp-inspect-server` **already depends on `famp-fsm` 0.11.0** (its `Cargo.toml`), so no dependency edits either.

---

## Exact edits

### 1. `crates/famp-inspect-server/src/parse.rs`

Add imports: `famp_envelope::MessageClass` (re-export of `famp_core::MessageClass` — same type, see `crates/famp-envelope/src/class.rs`), `famp_envelope::body::TerminalStatus` (re-export of `famp_core::TerminalStatus` via `body/deliver.rs`; if `body::TerminalStatus` isn't at that path, use the `deliver` module's re-export — do NOT add a `famp-core` dep), `famp_fsm::{TaskFsm, TaskTransitionInput}`, `famp_fsm::TaskState`.

**(a) Private parsers — serde-backed, no hand-rolled string tables (they'd re-drift):**

- `fn parse_class(env: &serde_json::Value) -> Option<MessageClass>` — `env.get("class")` then `serde_json::from_value::<MessageClass>(v.clone()).ok()` (snake_case rename on the enum is the canonical wire vocabulary).
- `fn parse_terminal_status(env: &serde_json::Value) -> Option<TerminalStatus>` — same pattern on top-level `env.get("terminal_status")`. **Top-level field, not under `body`.**

**(b) `const fn state_label(s: TaskState) -> &'static str`** — `Requested→"REQUESTED"`, `Committed→"COMMITTED"`, `Completed→"COMPLETED"`, `Failed→"FAILED"`, `Cancelled→"CANCELLED"` (matches the uppercase vocabulary already pinned by `famp-taskdir/src/record.rs` and `famp-inspect-proto`).

**(c) `pub struct FsmFold`** — the engine-backed chain fold; single source of truth is `TaskFsm::step`, no duplicated truth table:

```
pub struct FsmFold { fsm: TaskFsm, saw_fsm_class: bool }
```
- `new()` → `TaskFsm::new()` (starts `Requested` — the request envelope is the FSM's birth event, not a transition; engine has no Request arm), `saw_fsm_class: false`. Also derive/impl `Default`.
- `pub fn apply(&mut self, env: &serde_json::Value) -> String` — returns the state label AFTER this envelope:
  - `parse_class` → `None` or `Some(MessageClass::AuditLog)` → return `"UNKNOWN"` (fold untouched; preserves today's behavior for `famp send` chat traffic).
  - `Some(Request)` → `self.saw_fsm_class = true;` return `state_label(self.fsm.state())`.
  - `Some(Ack)` → if `self.saw_fsm_class` return `state_label(self.fsm.state())` else `"UNKNOWN"`. (Ack drives no transition; it reports the absorbed state — this is what makes the E2E's last-envelope poll show `COMPLETED`.)
  - `Some(class @ (Commit | Deliver | Control))` → `self.saw_fsm_class = true;` build `TaskTransitionInput { class, terminal_status: parse_terminal_status(env) }`; discard the step result (`engine.rs` guarantees `Err` = illegal = state NOT mutated — lines 41–47); return `state_label(self.fsm.state())`. If clippy (nursery, `just lint`) objects to `let _ = self.fsm.step(input);`, use an explicit `if self.fsm.step(input).is_err() { /* illegal transition: engine leaves state unchanged */ }` — never `.unwrap()`.
- `pub fn final_label(&self) -> String` — `"UNKNOWN"` if `!self.saw_fsm_class`, else `state_label(self.fsm.state())`.

**(d) Rewrite `derive_fsm_state(env)`** (kept, context-free — still used by `messages.rs::message_row` line 75 for single-envelope rows). Delete ALL `body.details` reads (`details`/`mode`/`terminal`/`action` locals, lines 17–29, and the whole current match). New mapping, mirroring the engine's input surface:

- class unparseable / `audit_log` → `"UNKNOWN"` (unchanged behavior)
- `request` → `"REQUESTED"`
- `commit` → `"COMMITTED"`
- `deliver` × header `terminal_status`: `Some(Completed)` → `"COMPLETED"`; `Some(Failed)` → `"FAILED"`; `Some(Cancelled)` → `"UNKNOWN"` (illegal wire combo per Decision §3 — comment this arm with the engine cite); `None` → `"COMMITTED"` (interim deliver: task remains Committed; preserves current display)
- `control` → `"CANCELLED"`
- `ack` → `"UNKNOWN"`, with comment: "ack drives no FSM transition (famp-fsm has no Ack arm; disposition is not FSM-observable) — a lone ack determines no state; task-level views use FsmFold instead." **Do not read `body.disposition`.**

Implement via `parse_class`/`parse_terminal_status` so `derive_fsm_state` and `FsmFold` share parsers and cannot drift on field names.

### 2. `crates/famp-inspect-server/src/tasks.rs`

**(a) `inspect_tasks_by_id` (lines 18–74):** immediately after building `envelopes_for_task` (line 23–27), stable-sort it by `parse_rfc3339_to_epoch(env.get("ts")...).unwrap_or(0)` ascending (missing/invalid ts → 0; stable sort preserves mailbox order for ties — the E2E uses identical ts strings, so ties fall back to `BTreeMap` recipient order, and the fold is order-robust anyway because illegal steps are ignored and terminals absorb). Sort BEFORE the `if full` split so both detail views share one order.

In the summary branch (lines 50–68): replace `fsm_transition: derive_fsm_state(env)` with a threaded fold — `let mut fold = FsmFold::new();` before the map, and `fsm_transition: fold.apply(env)` per envelope. (Requires the iteration to be in the sorted order — use a plain `for`/`map` over the sorted vec, not a re-collected iterator that reorders.)

**(b) Envelope-derived rows branch (lines 151–188):** replace

```
state: envelopes.last().copied().map_or_else(|| "REQUESTED".to_string(), derive_fsm_state),
```

with: sort `envelopes` by the same ts key, run `FsmFold::new()` over all of them via `apply`, and use `final_label()`. Behavior notes (intended): audit_log-only chains still show `"UNKNOWN"` (via `saw_fsm_class` guard); `peer` extraction (`first`) now reads the earliest-ts envelope — acceptable and more correct; the snapshot-record rows (lines 108–145) keep `record.state` from `famp-taskdir` untouched.

Keep `derive_fsm_state` in the `use crate::parse::{...}` list only if still referenced in this file; otherwise drop the import (unused imports fail `-D warnings`).

### 3. `crates/famp-inspect-proto/src/lib.rs` (~line 189)

Doc comment on `TaskEnvelopeSummary::fsm_transition` only (NO field/serde changes — the struct is `deny_unknown_fields`; adding fields breaks older clients): replace "One of `REQUESTED | COMMITTED | COMPLETED | FAILED | CANCELLED`." with: "Task FSM state AFTER applying this envelope to the task's transition fold (mirrors `famp-fsm::TaskFsm`). One of `REQUESTED | COMMITTED | COMPLETED | FAILED | CANCELLED`, or `UNKNOWN` for envelopes outside the task FSM (e.g. `audit_log`, malformed class, or an `ack` preceding any FSM-class envelope)."

### 4. `crates/famp-gateway/tests/e2e_cross_host_delivery.rs`

- **Delete** `build_cancel` (fn + its doc comment, lines ~511–539) and steps 5/6 (the two `send_bus_envelope(... cancel ...)` calls + the "5/6. Close the task…" comment, lines ~791–797).
- **Delete** now-unused imports `ControlAction, ControlBody, ControlTarget` from the `famp_envelope::body::{...}` use (line 75). Keep `Relation` (still used by Commits/Delivers/Acknowledges).
- **Change** the final assertion `assert_eq!(state_a, "CANCELLED")` → `assert_eq!(state_a, "COMPLETED")` (line ~808). Keep the two-side convergence `assert_eq!(state_a, state_b)`.
- `poll_terminal_state` stays as-is (last envelope's `fsm_transition`; with the fold, the trailing `ack` reports the absorbed `COMPLETED`, and even if a proxy mailbox lacks the ack, the terminal `deliver` itself reports `COMPLETED`).
- **Update stale doc comments**: module doc lines 27–44 (drop the "closing control/cancel … is the terminal signal GW-03 asserts on" rationale; keep the famp-send-emits-audit_log rationale; note the inspector now mirrors `famp-fsm`: header `terminal_status` on the deliver drives the terminal state and the ack folds to it) and the GW-03 comment above the final asserts (lines ~799–801). Grep the file for `cancel` afterwards to catch stragglers.

---

## Test changes

### `crates/famp-inspect-server/src/lib.rs` (existing tests module — it already carries the unwrap/expect allows)

**Rewrite the 4 existing pins (lines 635–669)** to the real wire shape:

- `derive_fsm_state_maps_completed_correctly`: `{"class":"deliver","terminal_status":"completed"}` → `"COMPLETED"`
- `..._failed_...`: `{"class":"deliver","terminal_status":"failed"}` → `"FAILED"`
- `..._cancelled_...`: `{"class":"control"}` → `"CANCELLED"`
- `..._committed_for_non_terminal_deliver`: `{"class":"deliver","body":{"interim":true}}` (no top-level `terminal_status`) → `"COMMITTED"`

**Add** (unit tests over `parse.rs` exports; place next to the existing four):

1. `derive_fsm_state_ack_is_unknown_context_free`: `{"class":"ack","body":{"disposition":"completed"}}` → `"UNKNOWN"` — pins that ack disposition is NOT read (§9.6 discipline).
2. `derive_fsm_state_deliver_cancelled_is_unknown`: `{"class":"deliver","terminal_status":"cancelled"}` → `"UNKNOWN"`.
3. `fsm_fold_real_cycle_reaches_completed`: fold over the 4-envelope chain `request` / `commit` / `deliver`+`terminal_status:"completed"` / `ack` (ascending distinct `ts`) → `apply` labels exactly `["REQUESTED","COMMITTED","COMPLETED","COMPLETED"]`, `final_label() == "COMPLETED"`.
4. `fsm_fold_failed_deliver_reaches_failed`: request/commit/deliver+`"failed"` → final `"FAILED"`.
5. `fsm_fold_control_cancels`: request/control → final `"CANCELLED"`.
6. `fsm_fold_illegal_transition_ignored`: request then deliver+`"completed"` WITHOUT a commit → labels `["REQUESTED","REQUESTED"]`, final `"REQUESTED"` — pins engine-mirroring (Requested+Deliver is illegal, state unchanged).
7. `fsm_fold_terminal_absorbing`: request/commit/deliver+`"completed"`/control → final `"COMPLETED"` — pins "terminals absorbing" (the very property the old E2E workaround violated).
8. `fsm_fold_audit_log_only_is_unknown`: two `audit_log` envelopes → per-envelope `"UNKNOWN"`, final `"UNKNOWN"` — regression guard for `famp send` chat traffic on the list view.
9. One dispatch-level test through `InspectKind::Tasks` with a `MessageSnapshot{by_recipient}` fixture holding the 4-class cycle split across TWO recipient mailboxes (mimicking the E2E's proxy split, e.g. `"alice": [commit, deliver]`, `"bob": [request, ack]`, identical `ts`): assert the by-id detail reply's LAST `fsm_transition == "COMPLETED"` and the list-view row `state == "COMPLETED"` — this is the exact observable GW-03 names, minus the subprocesses.

### `crates/famp-gateway/tests/e2e_cross_host_delivery.rs`

Covered in Exact edits §4 — the tightening IS the test change: terminal state now asserted from the literal request→commit→deliver→ack cycle with zero appended envelopes, and the expected value is `"COMPLETED"` (what the engine says the cycle produces), not `"CANCELLED"`.

---

## Acceptance criteria (all must pass; plain `cargo test`, NOT nextest — `cargo nextest -p famp` hangs)

1. `cargo test -p famp-inspect-server` — green, including all 9 new/4 rewritten tests above.
2. `cargo test -p famp-gateway` — green (31-test suite incl. the E2E gate).
3. `cargo test -p famp-fsm -p famp-envelope` — green with ZERO diffs in those crates (control: proves no engine/schema drift was introduced).
4. `grep -n "build_cancel" crates/famp-gateway/tests/e2e_cross_host_delivery.rs` → no matches; `grep -n 'assert_eq!(state_a, "COMPLETED")'` → exactly one match.
5. `grep -n "details" crates/famp-inspect-server/src/parse.rs` → no matches (all `body.details` reads gone).
6. `grep -n "disposition" crates/famp-inspect-server/src/` recursively → no matches outside test fixtures (inspector never reads ack disposition).
7. `just lint` clean (nursery lints promoted — not plain clippy) and `just fmt-check` clean.
8. `git diff --name-only` shows ONLY: `crates/famp-inspect-server/src/{parse.rs,tasks.rs,lib.rs}`, `crates/famp-inspect-proto/src/lib.rs`, `crates/famp-gateway/tests/e2e_cross_host_delivery.rs` (+ this plan file). Anything else = scope breach, stop and flag.

## Landmines

- **No `serde(flatten)`, no new wire fields, no schema edits anywhere** — byte-exact wire discipline; `TaskEnvelopeSummary` and all envelope bodies are `deny_unknown_fields`. The proto edit is doc-comment-only.
- **Zero changes to `famp-bus` / `famp-canonical` / `famp-crypto` / `famp-fsm` / `famp-envelope` / `famp-core`.** Nothing in this plan requires them. If the executor thinks one is needed, that's a wrong turn — stop and flag loudly.
- **`just lint` ≠ plain clippy** (promotes nursery lints): watch `let_underscore`-family on the discarded `fsm.step` Result (use the `is_err()` form if flagged), `unwrap_used`/`expect_used` outside the allow-carrying test modules, and unused imports after removing `derive_fsm_state` call sites / Control* imports.
- **ChildGuard**: the E2E already wraps every spawned `famp`/`famp-gateway` child; deletions must not touch that. No new child-spawning tests are planned (test 9 is in-process by design).
- **Envelope ordering**: `MessageSnapshot.by_recipient` is a `BTreeMap` — deterministic but NOT chronological across mailboxes; that is exactly why the fold sorts by `ts` (stable, ties keep mailbox order) AND ignores illegal steps. Do not "simplify" by trusting map order, and do not assert cross-mailbox ordering in tests with identical timestamps.
- **`famp send` traffic is `audit_log`-class** and must keep deriving/folding to `"UNKNOWN"` — test 8 guards this. Do not "improve" chat traffic into REQUESTED.
- **Post-merge ops note (not a test gate)**: the inspector runs inside the broker binary; the live system only reflects this after `just install` + broker restart (see broker restart playbook memory).
- Leave `09-VERIFICATION.md` untouched — re-verification is the verifier's artifact, not this fix's.

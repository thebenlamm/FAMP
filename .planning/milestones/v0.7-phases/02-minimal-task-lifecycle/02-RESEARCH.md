# Phase 2: Minimal Task Lifecycle — Research

**Researched:** 2026-04-13
**Domain:** Rust FSM implementation, typestate patterns, proptest exhaustive enumeration
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**D-A1:** Public model is a flat enum — no typestate generics, no phantom types.
```rust
pub enum TaskState {
    Requested, Committed, Completed, Failed, Cancelled,
}
```

**D-A2:** Single step function: `pub fn step(&mut self, input: TaskTransitionInput) -> Result<TaskState, TaskFsmError>`. No per-transition method family.

**D-A3:** "Illegal transitions unreachable" = terminal states have no outgoing arms (returns `Err`, never panics). Full typestate ceremony explicitly rejected for v0.7.

**D-A4:** FSM-03 compile-time gate = exhaustive `match` under `#![deny(unreachable_patterns)]` in a downstream consumer stub test file.

**D-A5:** `TaskState` and `TaskFsm` are fully owned — no lifetimes, no `&str`/`&[u8]`. `TaskState` derives `Copy`. `TaskFsm` is `Send + Sync`.

**D-B1:** `step` takes `TaskTransitionInput { class: MessageClass, terminal_status: Option<TerminalStatus>, relation: Option<TaskRelation> }`. Callers extract from envelope; `famp-fsm` never parses JSON.

**D-B2:** `famp-fsm` does NOT depend on `famp-envelope`. Depends only on `famp-core`.

**D-B3:** `relation` kept as `Option<TaskRelation>` as a forward-compatible seat. v0.7 logic treats it as advisory only.

**D-B4:** `TaskRelation` is a phase-local narrow enum covering only what v0.7 actually distinguishes.

**D-B5:** `TerminalStatus` currently lives in `famp-envelope::body::deliver`. Researcher must confirm lift-to-`famp-core` vs local-to-`famp-fsm` decision (see resolution below).

**D-C1/C2/C3:** `COMMITTED_PENDING_RESOLUTION` is absent — not optional, not feature-gated, absent.

**D-D1:** Dependency graph locked: `famp-core` → `famp-fsm`. No envelope dependency.

**D-E1:** `TaskFsmError` is a narrow phase-local enum matching v0.6 precedent.

**D-F1–F5:** Test strategy locked: proptest full Cartesian product, deterministic fixtures for 6 legal arrows, terminal-immutability fixtures, downstream consumer stub, no `stateright` in Phase 2.

### Claude's Discretion

- Exact module layout inside `famp-fsm/src/` (`lib.rs` vs split modules)
- Whether `TaskState` derives `Default` (probably `Requested`)
- Name of transition input struct
- Transition table as `match` block, `const` lookup, or helper function
- Final decision on keeping vs dropping `relation` from `TaskTransitionInput`
- Whether `TerminalStatus` lifts into `famp-core` or stays local (D-B5)
- Whether `TaskFsm` exposes a `peek(input)` dry-run method

### Deferred Ideas (OUT OF SCOPE)

- `ConversationFsm` (FSM-01)
- Terminal precedence between competing terminals (FSM-06)
- `stateright` model check (FSM-07)
- `REJECTED`, `EXPIRED`, `COMMITTED_PENDING_RESOLUTION` states
- Transfer-commit race resolution
- Full ENV-13 causal relation vocabulary
- Full `ControlBody` action set beyond `cancel`
- Type-state transition encoding (`TaskFsm<Requested>` → `TaskFsm<Committed>`) — explicitly rejected

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| FSM-02 (narrowed) | `TaskFsm` with 5 states: `REQUESTED → COMMITTED → {COMPLETED \| FAILED \| CANCELLED}` | Flat enum + `step()` engine; `TerminalStatus` lift decision resolves the only external dependency question |
| FSM-03 | Compile-time terminal-state enforcement via exhaustive `match` under `#![deny(unreachable_patterns)]` in downstream consumer stub | Direct precedent in `famp-core/tests/exhaustive_consumer_stub.rs`; same pattern, same file structure |
| FSM-04 | Transitions driven by `(class, relation, terminal_status, current_state)` tuple, rejected when illegal | Transition table as single-function `match` over `(state, class, terminal_status)` — relation advisory only |
| FSM-05 | Owned state types only — no lifetimes in FSM state enums | `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` on `TaskState`; all input types owned |
| FSM-08 | `proptest` property tests for transition legality (every illegal tuple rejected, every legal tuple accepted) | `prop_oneof!` strategies for each enum axis; Cartesian product via `(strategy_a, strategy_b, strategy_c).prop_map(...)` |

</phase_requirements>

---

## Summary

Phase 2 implements `famp-fsm` — a 5-state task lifecycle FSM — on top of the Phase 1 signed envelope layer. All architectural decisions were locked during discuss-phase; this research validates those decisions against the actual codebase and resolves the one open question (D-B5: `TerminalStatus` placement).

**The typestate vs enum question is settled**: the CONTEXT.md locked D-A1 (flat enum, no typestate). This is the right call. Typestate encoding of runtime-driven wire protocol FSMs creates serialization dead ends — you cannot store a `TaskFsm<Committed>` in a HashMap or channel without type erasure, defeating the purpose. The FSM-03 consumer stub under `#![deny(unreachable_patterns)]` gives the relevant compile-time guarantee: adding/removing a `TaskState` variant breaks downstream consumers. That is exactly INV-5. The "unreachable at compile time" claim in the spec means illegal transitions are *not representable in valid wire inputs* (via enum narrowing), not that the type system proves all protocol histories.

**The TerminalStatus placement question (D-B5) has a clear answer**: `TerminalStatus` currently lives in `famp-envelope::body::deliver`. `famp-fsm` cannot depend on `famp-envelope` (D-D1). Therefore, `TerminalStatus` must either be re-declared locally in `famp-fsm` (creating duplication and a type-mismatch in Phase 3 glue code) or lifted to `famp-core`. The right answer is **lift to `famp-core`**, mirroring the existing pattern where `MessageClass` was placed in `famp-envelope` but is re-exported at the same level. The Phase 3 adapter `fn fsm_input_from_envelope(&AnySignedEnvelope) -> TaskTransitionInput` will import `TerminalStatus` from `famp-core` and it will type-check without conversion. Alternatively, the planner may choose to declare `TerminalStatus` locally in `famp-fsm` and have Phase 3 glue do a trivial one-line `.into()` conversion — both approaches work, but lifting to core is cleaner.

**The relation question (D-B3) resolves to: include the field, keep it advisory, no variants needed yet**. Examining the 6 legal v0.7 arrows shows none of them need to *distinguish* on relation to determine the transition target — only `(class, current_state, terminal_status)` determines where you land. The `Relation` enum already ships in `famp-envelope::causality` with 5 variants. If `TaskRelation` is kept in `famp-fsm`, it should be a re-export or mirror of the relevant subset, not a new enum. Given famp-fsm cannot depend on famp-envelope, the simplest solution is: **drop the `relation` field from `TaskTransitionInput` for v0.7** — no v0.7 legal arrow requires it for correct transition dispatch.

**Primary recommendation:** Flat enum + single `step()` function + phase-local `TaskFsmError` + proptest Cartesian product. No FSM crate. `TerminalStatus` lifted to `famp-core`. `relation` field dropped from v0.7 `TaskTransitionInput`. Consumer stub mirrors `famp-core/tests/exhaustive_consumer_stub.rs` exactly.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `thiserror` | `2.0.18` (workspace) | `TaskFsmError` derive | Established project pattern; typed errors, not anyhow |
| `proptest` | `1.11.0` (workspace) | FSM-08 Cartesian product property test | Established project pattern; better shrinking than quickcheck |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `famp-core` | workspace | `MessageClass`, `TaskId`, shared vocabulary | Only external dependency for `famp-fsm` |
| `serde` | `1.0.228` (workspace) | `#[derive(Serialize, Deserialize)]` on `TaskState` for storage/logging | Derive only; no custom impls |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Hand-rolled flat enum | `sm` crate | `sm` adds a proc-macro dep, generates typestate structs — wrong for this use case (serialization breaks, dynamic wire input doesn't map to type-level states) |
| Hand-rolled flat enum | `rust-fsm` crate | `rust-fsm` uses `enum`-based representation internally but the macro generates code you can't audit for conformance; hand-roll is correct here |
| Hand-rolled flat enum | `statig` crate | `statig` is for hierarchical/orthogonal state machines with entry/exit actions; complete overkill for a 5-state linear FSM with no callbacks |
| Hand-rolled `match` table | `const` lookup array | `match` is more readable, Rust exhaustiveness-checks it, compiler optimizes equally well for 5×5 matrices; const array requires unsafe indexing |

**Verdict: Hand-roll, no FSM crate.** All candidate FSM crates assume generic inputs (events you define at compile time), but FAMP transitions are driven by decoded wire tuples known only at runtime. A proc-macro FSM crate either (a) doesn't add value for a 5-state machine or (b) fights the architecture. The conformance requirement means we must be able to read and audit every transition in the table without macro expansion. `famp-fsm/src/` will be ~150 lines total.

**Installation (new deps to add to famp-fsm/Cargo.toml):**
```toml
[dependencies]
famp-core = { path = "../famp-core" }
serde = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
proptest = { workspace = true }
```

---

## Architecture Patterns

### Recommended Module Layout

```
crates/famp-fsm/src/
├── lib.rs          # crate-level docs, #![forbid(unsafe_code)], pub re-exports
├── state.rs        # TaskState enum (5 variants, Copy, Serialize/Deserialize)
├── input.rs        # TaskTransitionInput struct + MessageClass re-export note
├── engine.rs       # TaskFsm struct + step() + transition table
└── error.rs        # TaskFsmError enum

crates/famp-fsm/tests/
├── deterministic.rs     # 6 legal arrows + terminal-immutability (D-F2, D-F3)
├── proptest_matrix.rs   # Cartesian product FSM-08 (D-F1)
└── consumer_stub.rs     # exhaustive match under deny(unreachable_patterns) (FSM-03, D-F4)
```

A single `lib.rs` is acceptable if the implementer prefers it — the above split is the recommendation for readability and parallelism with v0.6 crate conventions.

### Pattern 1: Flat Enum FSM with Single Transition Function

**What:** `TaskState` is a plain `Copy` enum. `TaskFsm` holds `state: TaskState`. One `step()` method contains the entire transition table as a nested `match`.

**When to use:** Always for this phase. The entire transition logic fits in ~40 lines and is auditable at a glance.

**Example:**
```rust
// Source: codebase pattern from famp-core/src/scope.rs (AuthorityScope truth table)
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Requested,
    Committed,
    Completed,
    Failed,
    Cancelled,
}

pub struct TaskFsm {
    state: TaskState,
}

impl TaskFsm {
    pub fn new() -> Self {
        Self { state: TaskState::Requested }
    }

    pub fn state(&self) -> TaskState {
        self.state
    }

    pub fn step(&mut self, input: TaskTransitionInput) -> Result<TaskState, TaskFsmError> {
        let next = match (self.state, input.class, input.terminal_status) {
            (TaskState::Requested,  MessageClass::Commit,  None)                                    => TaskState::Committed,
            (TaskState::Committed,  MessageClass::Deliver, Some(TerminalStatus::Completed))         => TaskState::Completed,
            (TaskState::Committed,  MessageClass::Deliver, Some(TerminalStatus::Failed))            => TaskState::Failed,
            (TaskState::Requested,  MessageClass::Control, None)                                    => TaskState::Cancelled,
            (TaskState::Committed,  MessageClass::Control, None)                                    => TaskState::Cancelled,
            _ => return Err(TaskFsmError::IllegalTransition {
                from: self.state,
                class: input.class,
                terminal_status: input.terminal_status,
            }),
        };
        self.state = next;
        Ok(next)
    }
}
```

**Key note on `control` transitions:** The `control/cancel` arms above pass `None` for `terminal_status` because `ControlBody` never carries a `terminal_status` (it's only on the envelope header for `deliver`). The transition table must not require `terminal_status` for the cancel arms.

### Pattern 2: Downstream Consumer Stub (FSM-03)

**What:** A committed test file with `#![deny(unreachable_patterns)]` that exhaustively `match`es `TaskState`. Adding or removing a variant is a hard compile error.

**When to use:** This is the INV-5 gate. It ships as `tests/consumer_stub.rs` in `famp-fsm`, mirroring `famp-core/tests/exhaustive_consumer_stub.rs` exactly.

**Example:**
```rust
// Source: crates/famp-core/tests/exhaustive_consumer_stub.rs (direct precedent)
#![deny(unreachable_patterns)]

use famp_fsm::TaskState;

const fn describe_state(s: TaskState) -> &'static str {
    match s {
        TaskState::Requested  => "requested",
        TaskState::Committed  => "committed",
        TaskState::Completed  => "completed",
        TaskState::Failed     => "failed",
        TaskState::Cancelled  => "cancelled",
    }
}
```

### Pattern 3: Proptest Cartesian Product (FSM-08)

**What:** Enumerate all `(TaskState, MessageClass, Option<TerminalStatus>)` tuples, compute expected legality from a hand-written truth table, assert `step()` matches.

**When to use:** The FSM-08 proptest matrix. Note that `prop_oneof!` + `prop_map` is the idiomatic way to enumerate finite variant spaces.

**Example:**
```rust
// Source: crates/famp-envelope/tests/prop_roundtrip.rs pattern
use proptest::prelude::*;
use famp_fsm::{TaskState, TaskFsmError, TaskFsm, TaskTransitionInput};
use famp_envelope::MessageClass;

fn arb_task_state() -> impl Strategy<Value = TaskState> {
    prop_oneof![
        Just(TaskState::Requested),
        Just(TaskState::Committed),
        Just(TaskState::Completed),
        Just(TaskState::Failed),
        Just(TaskState::Cancelled),
    ]
}

fn arb_message_class() -> impl Strategy<Value = MessageClass> {
    prop_oneof![
        Just(MessageClass::Request),
        Just(MessageClass::Commit),
        Just(MessageClass::Deliver),
        Just(MessageClass::Ack),
        Just(MessageClass::Control),
    ]
}

fn arb_terminal_status() -> impl Strategy<Value = Option<TerminalStatus>> {
    prop_oneof![
        Just(None),
        Just(Some(TerminalStatus::Completed)),
        Just(Some(TerminalStatus::Failed)),
        Just(Some(TerminalStatus::Cancelled)),
    ]
}

fn is_legal(state: TaskState, class: MessageClass, ts: Option<TerminalStatus>) -> bool {
    matches!(
        (state, class, ts),
        (TaskState::Requested, MessageClass::Commit,   None)
        | (TaskState::Committed, MessageClass::Deliver, Some(TerminalStatus::Completed))
        | (TaskState::Committed, MessageClass::Deliver, Some(TerminalStatus::Failed))
        | (TaskState::Requested, MessageClass::Control, None)
        | (TaskState::Committed, MessageClass::Control, None)
    )
}

proptest! {
    #[test]
    fn fsm_transition_legality(
        state  in arb_task_state(),
        class  in arb_message_class(),
        ts     in arb_terminal_status(),
    ) {
        let input = TaskTransitionInput { class, terminal_status: ts };
        let mut fsm = TaskFsm::with_state(state); // test-only constructor
        let result = fsm.step(input);
        if is_legal(state, class, ts) {
            prop_assert!(result.is_ok(), "expected Ok, got {result:?}");
        } else {
            prop_assert!(
                matches!(result, Err(TaskFsmError::IllegalTransition { .. })),
                "expected IllegalTransition, got {result:?}"
            );
        }
    }
}
```

**Note:** `TaskFsm::with_state(state)` is a test-only constructor that bypasses `new()` to seed arbitrary states. Mark it `#[cfg(test)]` or `#[doc(hidden)]`.

### Anti-Patterns to Avoid

- **Catch-all `_` arm in the transition table:** A `_ =>` arm in the `match` block silently swallows future illegal transitions if states are added. Use the specific-tuple form. The compiler exhaustiveness check is your friend.
- **No `_ =>` in the consumer stub:** The stub must have zero catch-all arms. `#![deny(unreachable_patterns)]` catches them after the fact, but not writing them is the first defense.
- **Re-declaring `MessageClass` in `famp-fsm`:** `MessageClass` lives in `famp-envelope`. Since `famp-fsm` doesn't depend on `famp-envelope`, `MessageClass` must either be re-exported from `famp-core` (preferred) or the `TaskTransitionInput` uses a local mirror. The planner must decide: lift `MessageClass` to `famp-core` alongside `TerminalStatus`, or declare a local `FsmClass` mirror in `famp-fsm`. Lifting to core is cleaner; see D-B5 resolution above.
- **Serialization of state via `usize` index:** Do not use numeric `#[serde(rename = "0")]` or tuple-struct serialization for `TaskState`. Use `#[serde(rename_all = "snake_case")]` so the wire form is `"requested"`, `"committed"`, etc. — human-readable, stable, and matches spec vocabulary.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Typed error with formatted message | `impl fmt::Display` by hand | `#[derive(thiserror::Error)]` | Project-wide precedent; zero-boilerplate; matches `TaskFsmError` D-E1 shape exactly |
| Enum variant enumeration for proptest | `vec![variant1, variant2, …].choose()` | `prop_oneof![Just(V1), Just(V2), …]` | Proptest's built-in shrinking works; random index into vec doesn't shrink to minimal counterexample |
| Exhaustiveness gate | Custom proc-macro | `#![deny(unreachable_patterns)]` + `match` | Existing pattern in project; no new tooling needed |

**Key insight:** The FSM itself is the only place where custom logic is required. Everything else (error types, testing infrastructure, serialization) has established project patterns to follow exactly.

---

## TerminalStatus Placement Resolution (D-B5)

**Current state:** `TerminalStatus` is defined in `famp-envelope::body::deliver` and re-exported via `famp-envelope::body`. It has 3 variants: `Completed`, `Failed`, `Cancelled`.

**The problem:** `famp-fsm` must not depend on `famp-envelope` (D-D1). But `TaskTransitionInput` needs `Option<TerminalStatus>`.

**Two valid options:**

**Option A (recommended): Lift `TerminalStatus` to `famp-core`.**
- Move the enum definition from `famp-envelope::body::deliver` to `famp-core`.
- Re-export from `famp-envelope::body` via `pub use famp_core::TerminalStatus` (backward-compatible).
- `famp-fsm` imports from `famp-core` — no dep on `famp-envelope`.
- Phase 3 glue: `famp_core::TerminalStatus` is the same type used by both `famp-envelope` and `famp-fsm`. No conversion needed.
- Pattern precedent: `famp-core` already owns `MessageClass`'s natural home (it was placed in `famp-envelope` because envelope was the first consumer, but conceptually it's a core vocabulary type).

**Option B: Declare `TerminalStatus` locally in `famp-fsm`, convert in Phase 3 glue.**
- `famp-fsm` has its own `TerminalStatus { Completed, Failed, Cancelled }`.
- Phase 3 glue does `famp_fsm::TerminalStatus::from(env_ts)` with a trivial `impl From`.
- Pro: no change to `famp-core` or `famp-envelope` in this phase.
- Con: two types with the same name and semantics in the workspace; adds a trivial `From` impl to Phase 3 glue scope.

**Recommendation to planner:** Include a Wave 0 task to lift `TerminalStatus` to `famp-core`. It is a ~10-line move + re-export change that cleanly resolves the layering for all future phases. Option B is acceptable if minimizing Phase 2 scope is paramount.

---

## MessageClass Placement Resolution

**Current state:** `MessageClass` is defined in `famp-envelope::class` and re-exported from `famp-envelope`. `famp-fsm` cannot depend on `famp-envelope`.

**Resolution:** This has the same structure as TerminalStatus. Either:
1. Move `MessageClass` to `famp-core` (it is conceptually a core vocabulary type, same as `ProtocolErrorKind`).
2. Declare a local `FsmClass` mirror in `famp-fsm` with `impl From<famp_envelope::MessageClass>` in Phase 3 glue.

**Note:** `famp-core` already has no `MessageClass`. Moving it there does not create a cycle. The `famp-envelope::MessageClass` re-export stays backward-compatible. This is the preferred approach and should be a Wave 0 task alongside `TerminalStatus`.

---

## Transition Table (Complete v0.7 Specification)

This is the authoritative transition table for the 5 legal v0.7 arrows. The planner and implementer must implement exactly this — no more, no less.

| From State | MessageClass | terminal_status | To State | Notes |
|------------|-------------|-----------------|----------|-------|
| `Requested` | `Commit` | `None` | `Committed` | Task accepted by executor |
| `Committed` | `Deliver` | `Some(Completed)` | `Completed` | Terminal: successful delivery |
| `Committed` | `Deliver` | `Some(Failed)` | `Failed` | Terminal: delivery failed |
| `Requested` | `Control` | `None` | `Cancelled` | Cancellation before commit |
| `Committed` | `Control` | `None` | `Cancelled` | Cancellation after commit |
| All other `(state, class, ts)` tuples | → | → | `Err(IllegalTransition)` | |

**Terminal state invariant:** Any `step()` call on a `TaskFsm` already in `Completed`, `Failed`, or `Cancelled` returns `Err(IllegalTransition)` regardless of input. This is INV-6 equivalent for v0.7.

**`request` class:** `MessageClass::Request` never drives a transition — `TaskFsm::new()` starts in `Requested` state. A `request` arriving at an existing FSM is always `IllegalTransition`. This is intentional: `request` creates a new task, it does not transition an existing one.

**`ack` class:** `MessageClass::Ack` never drives a transition. Acks are informational receipts, not state-changing events.

**`Cancelled` terminal_status on `TerminalStatus`:** The spec has `TerminalStatus::Cancelled` as a variant (present in `famp-envelope::body::deliver`). In the transition table, `control/cancel` → `Cancelled` does NOT use `terminal_status` — it's driven by `MessageClass::Control`. The `TerminalStatus::Cancelled` variant exists for ACK disposition semantics but is not a driver of the FSM cancel arrow. The `arb_terminal_status()` strategy in proptest must include `Some(TerminalStatus::Cancelled)` to ensure the matrix is complete, and the `is_legal` function must return `false` for any tuple using it as a driver.

---

## Common Pitfalls

### Pitfall 1: `_` Catch-All in Transition Match
**What goes wrong:** Adding a `_ => Err(...)` arm in the `step()` match table means any future state addition compiles silently instead of producing a "non-exhaustive pattern" error.
**Why it happens:** It feels natural to have a default case for "everything else is illegal."
**How to avoid:** Use a separate explicit arm for terminal states: `(TaskState::Completed | TaskState::Failed | TaskState::Cancelled, _, _) => Err(...)`. Then the top-level match is exhaustive without a `_` arm.
**Warning signs:** `cargo clippy` will not catch this; only a future maintainer noticing a silent wrong behavior catches it. Prevention at write time is the only defense.

### Pitfall 2: `TerminalStatus::Cancelled` in Transition Dispatch
**What goes wrong:** Treating `TerminalStatus::Cancelled` as the driver for the `→ Cancelled` transition (instead of `MessageClass::Control`). Leads to control messages being ignored and deliver-with-cancelled-status incorrectly transitioning.
**Why it happens:** The spec uses `cancelled` in two different contexts: the FSM terminal state and the TerminalStatus enum variant on deliver bodies.
**How to avoid:** The transition table above is explicit — `Control` class, `None` terminal_status → `Cancelled`. Never `(_, _, Some(Cancelled))`.
**Warning signs:** A deliver message with `terminal_status: cancelled` would cause an unintended FSM transition if the dispatch table is wrong.

### Pitfall 3: Non-Exhaustive Proptest Coverage via `arb_terminal_status()`
**What goes wrong:** Forgetting `Some(TerminalStatus::Cancelled)` in the `arb_terminal_status()` strategy. The proptest matrix says "every illegal tuple rejected" but `(Committed, Deliver, Some(Cancelled))` is never tested.
**Why it happens:** Easy to write a 2-element strategy (`None | Completed | Failed`) when thinking about legal arrows.
**How to avoid:** Strategy must enumerate ALL `TerminalStatus` variants plus `None`. For v0.7 that's 4 values: `None, Some(Completed), Some(Failed), Some(Cancelled)`.
**Warning signs:** Code review of `arb_terminal_status()` — count the variants.

### Pitfall 4: Consumer Stub in Same Crate as Implementation
**What goes wrong:** The consumer stub is the wrong thing if it's in `famp-fsm` itself under `#[cfg(test)]`. The point of the stub is to simulate a *downstream* consumer. If the stub is inside `famp-fsm`, the compiler doesn't enforce `#![deny(unreachable_patterns)]` as a separate compilation unit.
**Why it happens:** Convenience — putting tests in the same crate feels natural.
**How to avoid:** The stub must be in `crates/famp-fsm/tests/consumer_stub.rs` (a separate integration test binary). This is how `famp-core/tests/exhaustive_consumer_stub.rs` is structured. Integration test files in `tests/` are separate crates — the `#![deny(unreachable_patterns)]` applies to that separate crate only, which is correct.
**Warning signs:** If the stub is a `#[cfg(test)]` module inside `lib.rs`, it's wrong.

### Pitfall 5: `TaskFsm::with_state` in Public API
**What goes wrong:** Exposing a `with_state(state: TaskState)` constructor in the public API lets callers create a `TaskFsm` in any state, bypassing the `new() → Requested` invariant.
**Why it happens:** Needed for proptest (`arb_task_state()` can seed `Completed`, `Failed`, `Cancelled` to test terminal immutability).
**How to avoid:** `with_state` must be `#[cfg(test)]` or in a `#[doc(hidden)]` test-support module. The public API only offers `new()`.
**Warning signs:** If `cargo doc --open` shows `with_state` in the famp-fsm API docs, it leaked.

### Pitfall 6: Missing `#[forbid(unsafe_code)]`
**What goes wrong:** Inconsistency with every other crate in the workspace.
**How to avoid:** `#![forbid(unsafe_code)]` must be the first line after the crate-level doc comment in `lib.rs`. Copy from `famp-fsm/src/lib.rs` (Phase 0 stub already has it).

---

## Code Examples

### Complete Error Type

```rust
// Source: D-E1 from CONTEXT.md + famp-core/src/error.rs pattern
#[derive(Debug, thiserror::Error)]
pub enum TaskFsmError {
    #[error("illegal transition: cannot apply class={class:?} \
             terminal_status={terminal_status:?} from state={from:?}")]
    IllegalTransition {
        from: TaskState,
        class: MessageClass,
        terminal_status: Option<TerminalStatus>,
    },
}
```

### Complete `TaskTransitionInput` (v0.7, relation dropped)

```rust
// Rationale: no v0.7 legal arrow requires relation for dispatch.
// Relation is a forward-compatible seat but adds dead weight in v0.7.
pub struct TaskTransitionInput {
    pub class: MessageClass,
    pub terminal_status: Option<TerminalStatus>,
}
```

If the planner decides to retain the `relation` field per D-B3, the additional field is:
```rust
pub relation: Option<TaskRelation>,
```
where `TaskRelation` is a local zero-variant enum (`pub enum TaskRelation {}`) or a single-variant future seat — but for v0.7 this field adds noise. Drop it.

### Deterministic Test Pattern for 6 Legal Arrows

```rust
// Source: pattern from crates/famp-envelope/tests/roundtrip_signed.rs
#[test]
fn requested_plus_commit_advances_to_committed() {
    let mut fsm = TaskFsm::new();
    let result = fsm.step(TaskTransitionInput {
        class: MessageClass::Commit,
        terminal_status: None,
    });
    assert_eq!(result.unwrap(), TaskState::Committed);
    assert_eq!(fsm.state(), TaskState::Committed);
}
```

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo-nextest 0.9.132 + proptest 1.11.0 |
| Config file | `.config/nextest.toml` (workspace-level) or none — nextest finds tests automatically |
| Quick run command | `cargo nextest run -p famp-fsm` |
| Full suite command | `cargo nextest run --workspace` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FSM-02 | 5-state TaskFsm with correct initial state | unit | `cargo nextest run -p famp-fsm -E 'test(deterministic)'` | ❌ Wave 0 |
| FSM-03 | Consumer stub compile-error on variant change | compile gate | `cargo nextest run -p famp-fsm -E 'test(consumer_stub)'` | ❌ Wave 0 |
| FSM-04 | All legal tuples accepted, illegal rejected | property | `cargo nextest run -p famp-fsm -E 'test(fsm_transition_legality)'` | ❌ Wave 0 |
| FSM-05 | TaskState/TaskFsm have no lifetimes | compile gate | `cargo build -p famp-fsm` | ❌ Wave 0 |
| FSM-08 | Full Cartesian product coverage | property | `cargo nextest run -p famp-fsm -E 'test(proptest_matrix)'` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo nextest run -p famp-fsm`
- **Per wave merge:** `cargo nextest run --workspace`
- **Phase gate:** `just ci` (full workspace including lint + format check)

### Wave 0 Gaps

- [ ] `crates/famp-fsm/tests/deterministic.rs` — covers FSM-02, FSM-03 (legal arrows + terminal immutability)
- [ ] `crates/famp-fsm/tests/proptest_matrix.rs` — covers FSM-04, FSM-08
- [ ] `crates/famp-fsm/tests/consumer_stub.rs` — covers FSM-03 compile gate under `#![deny(unreachable_patterns)]`
- [ ] `crates/famp-fsm/src/` — all implementation modules (Wave 0 creates them from the Phase 0 stub)
- [ ] `crates/famp-core/src/` — potential `TerminalStatus` + `MessageClass` lift (if Option A chosen for D-B5)

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Typestate FSMs (`struct TaskFsm<S>(PhantomData<S>)`) | Flat enum + exhaustive match + `deny(unreachable_patterns)` | 2022–2023 Rust community shift | Typestate is still valid for systems where you control call sites; for protocol FSMs driven by runtime wire inputs it's the wrong tool |
| `quickcheck` for property testing | `proptest` | 2019–2021 | Proptest's value-tree shrinking produces minimal counterexamples; quickcheck's random shrinking doesn't |
| `stateright` for all model checking | `proptest` for small FSMs, `stateright` for distributed system model checking | Ongoing | 5-state linear FSM doesn't need BFS model checking; proptest exhaustive enumeration suffices |

**Deprecated/outdated:**
- `sm` crate (last release 2018, archived on crates.io): typestate macro for simple FSMs — abandoned, do not use.
- `machine` crate: unmaintained. Do not use.
- `quickcheck`: still maintained but proptest is the project standard (CLAUDE.md §12).

---

## Open Questions

1. **MessageClass + TerminalStatus lift to famp-core**
   - What we know: Both types are needed by `famp-fsm` and currently live in `famp-envelope`. `famp-fsm` cannot depend on `famp-envelope` (D-D1).
   - What's unclear: Whether the planner treats this as a Wave 0 pre-requisite task in Phase 2, or defers to "local mirror + From conversion in Phase 3."
   - Recommendation: Include as Wave 0 task in Phase 2. It's a ~20-line mechanical move with zero risk. Deferring it means Phase 3 carries technical debt.

2. **`relation` field: keep or drop**
   - What we know: No v0.7 legal arrow uses relation to determine its target state.
   - What's unclear: Whether the field provides value as a forward-compatible annotation seat even if unused in dispatch.
   - Recommendation: **Drop it.** YAGNI applies. The field can be added in v0.9 when causality vocabulary is relevant. Including a dead field in `TaskTransitionInput` trains callers to ignore it.

3. **`TaskFsm` optional `TaskId` for diagnostics**
   - What we know: `famp-core::TaskId` exists. `TaskFsmError` includes `from`, `class`, `terminal_status` but not a task identifier.
   - What's unclear: Whether callers need the task ID in error output, or whether they'll wrap `TaskFsmError` with context at the Phase 3 boundary.
   - Recommendation: **Omit `TaskId` from `TaskFsm` struct and `TaskFsmError` for v0.7.** Callers provide context. FSM stays minimal.

---

## Sources

### Primary (HIGH confidence)
- `crates/famp-fsm/src/lib.rs` — Phase 0 stub, confirmed structure
- `crates/famp-core/tests/exhaustive_consumer_stub.rs` — direct precedent for FSM-03 consumer stub
- `crates/famp-envelope/src/body/deliver.rs` — `TerminalStatus` current location + 3 variants confirmed
- `crates/famp-envelope/src/class.rs` — `MessageClass` 5 variants confirmed
- `crates/famp-envelope/src/causality.rs` — `Relation` 5 variants confirmed (v0.7 narrowed)
- `.planning/phases/02-minimal-task-lifecycle/02-CONTEXT.md` — all implementation decisions
- `.planning/REQUIREMENTS.md` — FSM-02 through FSM-08 requirement text

### Secondary (MEDIUM confidence)
- CLAUDE.md tech stack section — confirmed crate versions and workspace pins
- `Cargo.toml` workspace — confirmed `proptest 1.11.0`, `thiserror 2.0.18` as workspace deps
- `crates/famp-envelope/tests/prop_roundtrip.rs` — `prop_oneof!` strategy pattern

### Tertiary (LOW confidence)
- FSM crate landscape assessment (sm, rust-fsm, statig) — based on training knowledge + reasoning from CONTEXT.md explicit rejection of typestate. Not re-verified against crates.io today, but CONTEXT.md D-A1 makes the conclusion moot regardless.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — crate versions verified against workspace Cargo.toml; `famp-fsm` has no new external deps beyond what's workspace-pinned
- Architecture: HIGH — all patterns verified against existing codebase; transition table derived directly from CONTEXT.md locked decisions + spec references
- Pitfalls: HIGH — all pitfalls are either verified against actual code (consumer stub placement) or directly derivable from the architecture choices
- TerminalStatus/MessageClass placement: HIGH for diagnosis, MEDIUM for final choice (planner decides)

**Research date:** 2026-04-13
**Valid until:** 2026-05-13 (stack is pinned, no external dependencies to drift)

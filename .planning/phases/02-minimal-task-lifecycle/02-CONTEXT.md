# Phase 2: Minimal Task Lifecycle — Context

**Gathered:** 2026-04-13
**Status:** Ready for research → planning

<domain>
## Phase Boundary

`famp-fsm` exposes a 5-state task lifecycle — `REQUESTED → COMMITTED → {COMPLETED | FAILED | CANCELLED}` — with 1 initial + 1 intermediate + 3 terminals. Transitions are driven by a narrow decoded input derived from the §7.3a FSM-observable whitelist (`class`, `terminal_status`, optionally `relation`). Terminal states have no outgoing arms. `REJECTED`, `EXPIRED`, timeout-driven transitions, competing-instance races, and conversation FSM are all out of scope and must not be representable. Envelope decoding, transport, keyring, and examples are later phases.

</domain>

<decisions>
## Implementation Decisions

### A. State representation — public enum + runtime transition engine

- **D-A1:** Public model is a flat enum:
  ```rust
  pub enum TaskState {
      Requested,
      Committed,
      Completed,
      Failed,
      Cancelled,
  }
  ```
  No type-state wrapper types, no `TaskFsm<Requested>`/`TaskFsm<Committed>` phantom generics. Roadmap SC#1 (downstream compile error on variant change via exhaustive `match`) is satisfied by the enum directly.
- **D-A2:** Transition engine is a single struct carrying the current state:
  ```rust
  pub struct TaskFsm { state: TaskState }

  impl TaskFsm {
      pub fn new() -> Self;                  // always starts in Requested
      pub fn state(&self) -> TaskState;
      pub fn step(&mut self, input: TaskTransitionInput)
          -> Result<TaskState, TaskFsmError>;
  }
  ```
  One transition function is the only place legality is decided. No `transition_to_committed()` / `transition_to_completed()` family — those re-introduce the scattered illegal-call surface an enum+single-step is meant to collapse.
- **D-A3:** **"Illegal transitions unreachable" is interpreted narrowly for v0.7:**
  - Terminal states (`Completed`, `Failed`, `Cancelled`) have no outgoing arms in the internal transition table — any `step` call on a terminal returns `TaskFsmError::IllegalTransition`, never silently no-ops, never panics.
  - Illegal `(state, class, terminal_status)` combinations return `TaskFsmError::IllegalTransition` with the offending tuple attached.
  - The stronger claim — "the Rust type system proves arbitrary protocol histories impossible at compile time" — is **explicitly rejected** for v0.7. Not worth the type-state ceremony given dynamic wire input.
- **D-A4:** Exhaustive `match` under `#![deny(unreachable_patterns)]` in a downstream consumer stub (FSM-03) is the compile-time gate: adding or removing a `TaskState` variant must produce a hard compile error in the stub. Pattern precedent: v0.6 Phase 3 `ProtocolErrorKind` consumer stub.
- **D-A5:** `TaskState` and `TaskFsm` are fully owned — no lifetimes, no `&str`/`&[u8]` in state enum, no borrow from input (FSM-05). `TaskState` derives `Copy` where cheap; `TaskFsm` is `Send + Sync`.

### B. Transition driver input — narrow struct, not envelopes

- **D-B1:** `step` takes a phase-local decoded input struct, not `&AnySignedEnvelope`:
  ```rust
  pub struct TaskTransitionInput {
      pub class: MessageClass,                    // from famp-core
      pub terminal_status: Option<TerminalStatus>,// from famp-core or famp-fsm-local
      pub relation: Option<TaskRelation>,         // optional, see D-B3
  }
  ```
  Callers (Phase 3 transport/runtime glue) extract the §7.3a whitelist fields from a decoded envelope and hand them to the FSM. `famp-fsm` never parses JSON, never verifies signatures, never touches wire bytes.
- **D-B2:** **`famp-fsm` does NOT depend on `famp-envelope`.** Depends only on `famp-core` (and testing crates). This locks the layering: core → envelope, core → fsm, and Phase 3 transport/runtime does the tiny envelope↔fsm adapter. See D-D1.
- **D-B3:** **Relation vocabulary in v0.7 is deliberately near-empty.** The v0.7 Personal Runtime happy path is:
  - `request` (no prior state) → `Requested`
  - `commit` against a `Requested` task → `Committed`
  - `deliver` with `interim=false` + `terminal_status={completed|failed}` against `Committed` → `Completed|Failed`
  - `control/cancel` against `Requested` or `Committed` → `Cancelled`

  None of these four arrows strictly need an 11-relation ENV-13 vocabulary. The `relation` field on `TaskTransitionInput` is kept as `Option<TaskRelation>` as a forward-compatible seat — v0.7 logic treats it as advisory only. If research shows relation is genuinely not needed by any legal arrow, the field may be dropped before research hands off to planning. The **mechanism** (narrow optional field, not enum bloat) is fixed.
- **D-B4:** `TaskRelation` (if kept) is a **phase-local narrow enum** carrying only the relations v0.7 actually distinguishes. The wider ENV-13 11-relation set defers to v0.9 Causality & Replay Defense. Same "narrow by absence" rule as Phase 1's `ControlBody`: federation-grade relations must not be representable in v0.7.
- **D-B5:** `MessageClass` comes from `famp-core` (already shipped in v0.6). `TerminalStatus` — researcher must confirm whether it already lives in `famp-core` or belongs in `famp-fsm` as a narrow enum (`Completed`, `Failed`). If it lives in envelope body types today, lift the enum to `famp-core` rather than creating a circular dep.

### C. `COMMITTED_PENDING_RESOLUTION` — deferred, documented

- **D-C1:** **Not included in v0.7.** The task FSM is strictly 5 states. The internal `COMMITTED_PENDING_RESOLUTION` state from spec §11.5a is not representable, not reachable, not mentioned in `TaskState`.
- **D-C2:** Rationale captured inline in `famp-fsm` crate-level docs: *"v0.7 Personal Runtime is single-instance. Competing-instance commit races (§11.5a / §12.3a transfer-commit race) defer to the Federation Profile milestone (v0.8+), where multi-instance coordination is in scope. Including the resolution state here would reintroduce exactly the coordination complexity the Personal Profile was cut to avoid."*
- **D-C3:** The narrowing is **absence, not optionality.** There is no `Option<PendingResolution>` field, no internal-only sixth variant gated behind a feature flag. If a future v0.8 consumer needs it, that is a breaking change.

### D. Crate layering — `famp-fsm` stays pure

- **D-D1:** Final dependency graph for v0.7:
  ```
  famp-core      (shared types: Principal, TaskId, MessageClass, ProtocolErrorKind, …)
      ↑        ↑
  famp-envelope  famp-fsm
           ↖      ↗
         famp-transport  (Phase 3)
           ↑
    example glue        (personal_two_agents / cross_machine_two_agents)
  ```
  `famp-fsm` **does not** import `famp-envelope`. The envelope→fsm adapter lives in Phase 3 transport/runtime glue.
- **D-D2:** Consequence: `famp-fsm` tests can construct `TaskTransitionInput` directly without building signed envelopes. Proptest strategies stay tight (build tuples, not JSON).
- **D-D3:** Consequence: Phase 3 will own a ~20-line adapter function like `fn fsm_input_from_envelope(&AnySignedEnvelope) -> TaskTransitionInput`. Acceptable glue cost for a cleaner layering.

### E. Error shape — phase-local narrow enum

- **D-E1:** Phase-local narrow error enum:
  ```rust
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
  Matches v0.6 Plans 01-01 and 02-01 precedent (narrow phase-local enum, not a god enum). Additional variants only if research surfaces decode failure modes the single `IllegalTransition` cannot express cleanly.
- **D-E2:** **`famp-fsm` does NOT map to `ProtocolErrorKind` internally.** The mapping happens at the Phase 3 runtime/transport boundary, same pattern as `famp-canonical` and `famp-crypto` errors. Never reach for `ProtocolErrorKind::Other` inside `famp-fsm`.
- **D-E3:** Every illegal tuple in the proptest matrix (FSM-08) asserts that the returned error is `TaskFsmError::IllegalTransition` with the exact offending tuple — not a panic, not a generic error.

### F. Test strategy — FSM-08 proptest matrix + deterministic fixtures

- **D-F1:** **Full tuple enumeration under proptest.** Strategy enumerates the Cartesian product of:
  - `TaskState` (5 variants)
  - `MessageClass` (5 shipped variants from Phase 1: `request`, `commit`, `deliver`, `ack`, `control`)
  - `Option<TerminalStatus>` (`None`, `Some(Completed)`, `Some(Failed)`)
  - `Option<TaskRelation>` (if kept per D-B3)

  For every tuple: compute expected legality from the hand-written transition table, call `step`, assert the result matches (`Ok(expected_next)` for legal, `Err(IllegalTransition{..})` for illegal). Zero panics across the entire matrix.
- **D-F2:** **Deterministic fixture tests** for the four legal v0.7 arrows:
  1. `Requested + request` — construction path
  2. `Requested + commit` → `Committed`
  3. `Committed + deliver(terminal=completed)` → `Completed`
  4. `Committed + deliver(terminal=failed)` → `Failed`
  5. `Requested + control/cancel` → `Cancelled`
  6. `Committed + control/cancel` → `Cancelled`

  Each is a named test, not a generated one — happy-path documentation as code.
- **D-F3:** **Terminal-immutability fixture test** — a single deterministic test per terminal (3 tests) that calls `step` on a terminal FSM with every `MessageClass` and asserts `IllegalTransition` every time. This is the INV-6-shaped assertion for v0.7.
- **D-F4:** **Downstream consumer stub test** — a committed test file (pattern from v0.6 Phase 3 `ProtocolErrorKind` stub) that exhaustively matches `TaskState` under `#![deny(unreachable_patterns)]`. Adding or removing a variant breaks the stub. This is the FSM-03 compile-time gate.
- **D-F5:** **`stateright` is NOT introduced in Phase 2.** FSM-07 (`stateright` model check) is explicitly out of scope for v0.7 per REQUIREMENTS.md line 75. Phase 2 sticks to proptest for FSM-08. `stateright` enters in v0.14 adversarial conformance.

### Claude's Discretion

- Exact module layout inside `famp-fsm/src/` (single `lib.rs` vs `state.rs` + `transition.rs` + `error.rs`).
- Whether `TaskState` derives `Default` (and which variant is the default — probably `Requested`).
- Exact name of the transition input struct (`TaskTransitionInput` vs `TaskEvent` vs `FsmInput`) — pick what reads best in `step(input)`.
- Whether the transition table is expressed as a `match` block, a `const` lookup table, or a small helper function — whichever makes the FSM-03 consumer stub cleanest.
- Final decision on keeping vs dropping `relation` from `TaskTransitionInput` (D-B3) once research confirms whether any legal v0.7 arrow needs it.
- Whether `TerminalStatus` lifts into `famp-core` or stays local to `famp-fsm` (D-B5).
- Whether `TaskFsm` exposes a `peek(input)` (legality check without mutation) in addition to `step` — add only if a real caller needs it.

</decisions>

<specifics>
## Specific Ideas

- **"Narrow by absence, not by option."** Same rule as Phase 1's `ControlBody::cancel` and `CommitBody` sans `capability_snapshot`. For Phase 2: `REJECTED`, `EXPIRED`, `COMMITTED_PENDING_RESOLUTION`, and the wider ENV-13 relation set literally do not exist as variants — not `Option`, not feature-gated, not commented-out.
- **"Domain logic, not wire-shape logic."** `famp-fsm` is a transition engine over decoded inputs. It is tested without constructing a single byte of JSON or a single signature. The envelope/fsm glue is ~20 lines in Phase 3, and that is the correct place for it.
- **"Relations are a forward-compatible seat, not a vocabulary."** Keep `relation: Option<TaskRelation>` as an optional field if any v0.7 arrow needs it, drop it entirely if none does. Either way, no stubbed enum variants for ENV-13 causal relations.
- **"One transition function, not a family."** No `commit()`, `deliver_completed()`, `cancel()` methods. Every transition goes through `step(input)` so the legal-vs-illegal decision lives in exactly one place.

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Spec — task FSM semantics
- `FAMP-v0.5.1-spec.md` §7.3a — **FSM-observable whitelist.** Only `class`, `relation`, `terminal_status` (envelope) and `interim`, `scope_subset`, `target` (body) drive transitions. No other fields participate. This is the authoritative scope of what `TaskTransitionInput` may carry.
- `FAMP-v0.5.1-spec.md` §9.6a — **Terminal precedence.** Defines which messages crystallize a terminal state: terminal `deliver` (interim=false with terminal_status), `control` with `cancels` against `COMMITTED`, transfer-timeout reversion. Phase 2 ignores transfer-timeout (v0.8+) but must match the first two shapes byte-for-byte in test fixtures.
- `FAMP-v0.5.1-spec.md` §11.5a — **`COMMITTED_PENDING_RESOLUTION` internal state** for competing-instance commit races. Phase 2 **explicitly omits this** per D-C1; read this section only to understand what is being deferred and why.
- `FAMP-v0.5.1-spec.md` §8a.3 — `deliver` body: `interim` bool gates `terminal_status`; `error_detail` REQUIRED iff `terminal_status = failed`. Defines the shape of the deliver-driven terminals `(Completed | Failed)`.
- `FAMP-v0.5.1-spec.md` §8a.4 — `control` body: `action` enum (v0.7 narrows to `cancel` only per Phase 1 ENV-12 narrowing). `target` field tells the FSM what the control action operates on.

### Requirements and roadmap
- `.planning/REQUIREMENTS.md` — **FSM-02 (narrowed), FSM-03, FSM-04, FSM-05, FSM-08.** Note line 75: FSM-01, FSM-06, FSM-07 are explicitly out of scope for v0.7.
- `.planning/ROADMAP.md` Phase 2 — 4 success criteria (5 states, narrowed form, proptest tuple matrix, owned state).
- `.planning/STATE.md` — carried decisions: narrow phase-local error enums, exhaustive consumer stub under `#![deny(unreachable_patterns)]`, 15-category `ProtocolErrorKind` as boundary sink only.

### Phase 1 outputs (direct dependencies of the envelope↔fsm adapter that Phase 3 will build)
- `.planning/phases/01-minimal-signed-envelope/01-CONTEXT.md` — Phase 1 decisions. Especially D-B5 (narrowing = absence), D-E1 (decode paths), and the `AnySignedEnvelope` shape.
- `.planning/phases/01-minimal-signed-envelope/01-RESEARCH.md` — body schema research, §7.3a whitelist field extraction.
- `crates/famp-envelope/src/` — shipped envelope types. **Phase 2 does not depend on this crate**, but researcher must read it to confirm which fields are exposed for Phase 3 glue.

### v0.6 implementation precedents to mirror
- `crates/famp-core/src/error.rs` — `ProtocolErrorKind` 15-category flat enum. Phase 2 error enum converts into this at the Phase 3 boundary, not inside `famp-fsm`.
- `crates/famp-core/src/identity.rs` + `ids.rs` — `TaskId`, `MessageId`. `TaskFsm` may be parameterized by `TaskId` for logging/diagnostics; check before duplicating.
- `.planning/milestones/v0.6-phases/03-core-types-invariants/` — `ProtocolErrorKind` exhaustive consumer stub precedent. This is the template for the Phase 2 FSM-03 consumer stub.
- `.planning/milestones/v0.6-phases/01-canonical-json-foundations/01-01-PLAN.md` D-16 — narrow phase-local error enum pattern.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `famp_core::MessageClass` — already shipped in v0.6 Phase 3. Reuse as the `class` field of `TaskTransitionInput`, do not re-declare.
- `famp_core::TaskId`, `MessageId` — typed IDs for diagnostics/logging in `TaskFsmError` if needed.
- `famp_core::ProtocolErrorKind` — target for boundary conversion **in Phase 3**, not inside `famp-fsm`.
- `crates/famp-envelope/src/body/` — body types expose the §7.3a whitelist fields (`interim`, `terminal_status`, `action`, `target`) that Phase 3 glue will extract. `famp-fsm` itself does not import from here.
- `crates/famp-fsm/Cargo.toml` + `src/lib.rs` — Phase 0 stub exists (empty crate with smoke test). Phase 2 fills it in, does not create a new crate.

### Established Patterns
- **Compile-time unrepresentability over runtime rejection** — extended in Phase 2 via variant *absence* for `REJECTED`/`EXPIRED`/`COMMITTED_PENDING_RESOLUTION` and via the FSM-03 exhaustive consumer stub. NOT extended to full type-state transitions per D-A3.
- **Phase-local narrow error enums** — precedent in Plans 01-01 and 02-01 (v0.6), Phase 1 `EnvelopeDecodeError` (v0.7). Phase 2 `TaskFsmError` is the same shape.
- **Owned types at crate boundaries** — no lifetimes, no `&str`/`&[u8]` in public enums. Matches v0.6 `ProtocolErrorKind` and Phase 1 body types.
- **Narrow by absence** — `ControlBody::cancel` is the canonical example. `TaskState` applies the same rule.

### Integration Points
- `famp-fsm` depends on `famp-core` only. It does **not** depend on `famp-envelope` (D-D1).
- Phase 3 (`famp-transport` + runtime glue) will build a small adapter: `&AnySignedEnvelope → TaskTransitionInput`. That adapter lives in Phase 3 code, not in `famp-fsm`.
- Phase 3's `personal_two_agents` example drives a `TaskFsm` through the full `request → commit → deliver → ack` cycle. The adapter + `step` loop is the integration test of Phase 2's public API.
- Phase 4 HTTP transport consumes the same `TaskFsm` via the same adapter — no new FSM surface is added in Phase 4.

</code_context>

<deferred>
## Deferred Ideas

- **`ConversationFsm`** (FSM-01) — conversation state machine. v0.10 Negotiation & Commitment.
- **Terminal precedence between competing terminals** (FSM-06) — Personal Profile has no competing terminals. Defer to Federation Profile.
- **`stateright` model check** (FSM-07) — exhaustive BFS over the FSM state graph. v0.14 Adversarial Conformance.
- **`REJECTED` state** — v0.8+ when negotiation/proposal flow re-enters scope.
- **`EXPIRED` state + timeout-driven transitions** — v0.9 Causality & Replay Defense (freshness windows, idempotency-key scoping, negotiation-round limits).
- **`COMMITTED_PENDING_RESOLUTION` internal state** (§11.5a) — v0.8+ Federation Profile (multi-instance commit races).
- **Transfer-commit race resolution** (§12.3a) — v0.11 Delegation.
- **Full ENV-13 causal relation vocabulary** (11 relations) — v0.9 Causality.
- **Full `ControlBody` action set** (`supersede`, `close`, `cancel_if_not_started`, `revert_transfer`) — v0.8+ (ENV-12 full form). v0.7 FSM handles `cancel` only because that is all the envelope layer ships.
- **Type-state transition encoding** (`TaskFsm<Requested>` → `TaskFsm<Committed>`) — explicitly rejected for v0.7 per D-A1/D-A3. Reconsider only if a future milestone finds the single-enum approach genuinely insufficient, which it will not.
- **`peek`/dry-run transition API** — add only when a concrete caller needs it.

</deferred>

---

*Phase: 02-minimal-task-lifecycle*
*Context gathered: 2026-04-13*

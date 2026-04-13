---
phase: 01-minimal-signed-envelope
plan: 02
subsystem: famp-envelope
tags: [envelope, body-schemas, env-09, env-12, env-14, sealed-trait, deny-unknown-fields]
requires:
  - famp-envelope primitives (Plan 01-01: MessageClass, EnvelopeScope, EnvelopeDecodeError)
  - famp-core (AuthorityScope, ArtifactId)
provides:
  - famp_envelope::BodySchema (sealed supertrait with CLASS + SCOPE consts)
  - famp_envelope::body::{RequestBody, CommitBody, DeliverBody, AckBody, ControlBody}
  - famp_envelope::body::{Bounds, Budget}
  - famp_envelope::body::{Artifact, ErrorDetail, ErrorCategory, TerminalStatus}
  - famp_envelope::body::{AckDisposition, ControlAction, ControlTarget, ControlDisposition}
  - 7 body-shape fixtures (4 round-trip + 3 adversarial)
  - tests/body_shapes.rs (19 tests)
affects:
  - Plan 01-03 (consumes bodies via BodySchema bound; wires validate() + validate_against_terminal_status)
tech_stack:
  added: []
  patterns:
    - sealed trait via private supertrait (prevents downstream sixth body)
    - compile_fail doctest as the sealing assertion
    - field-absence narrowing (ENV-09: no capability_snapshot field at all)
    - single-variant enum narrowing (ENV-12: ControlAction::Cancel only)
    - cross-field validation on DeliverBody (interim x terminal_status matrix)
key_files:
  created:
    - crates/famp-envelope/src/body/mod.rs
    - crates/famp-envelope/src/body/bounds.rs
    - crates/famp-envelope/src/body/request.rs
    - crates/famp-envelope/src/body/commit.rs
    - crates/famp-envelope/src/body/deliver.rs
    - crates/famp-envelope/src/body/ack.rs
    - crates/famp-envelope/src/body/control.rs
    - crates/famp-envelope/tests/body_shapes.rs
    - crates/famp-envelope/tests/fixtures/roundtrip/request.json
    - crates/famp-envelope/tests/fixtures/roundtrip/commit.json
    - crates/famp-envelope/tests/fixtures/roundtrip/deliver_interim.json
    - crates/famp-envelope/tests/fixtures/roundtrip/deliver_terminal.json
    - crates/famp-envelope/tests/fixtures/roundtrip/ack.json
    - crates/famp-envelope/tests/fixtures/roundtrip/control_cancel.json
    - crates/famp-envelope/tests/fixtures/adversarial/commit_with_capability_snapshot.json
    - crates/famp-envelope/tests/fixtures/adversarial/control_supersede.json
    - crates/famp-envelope/tests/fixtures/adversarial/unknown_body_field_nested.json
  modified:
    - crates/famp-envelope/src/lib.rs (add pub mod body; pub use BodySchema; drop serde_json silencer)
decisions:
  - validate_against_terminal_status is `pub` not `pub(crate)` so the integration test crate can exercise all four cross-field branches
  - Deliver cross-field enforcement lives as a method, not a decode-time check yet — Plan 03 decode pipeline will call it
  - Clippy pedantic cleanups in body/ tracked inline (Eq derives, allow(dead_code) on validate() pending Plan 03 wiring)
metrics:
  completed_date: 2026-04-13
  tasks: 3
  commits: 3
  tests: 19 body_shape integration tests (11 from Task 2 + 8 from Task 3) + 4 bounds unit tests + 1 compile_fail doctest
---

# Phase 1 Plan 02: Body Schemas + Sealed BodySchema Trait Summary

Seals the `famp-envelope` body surface to exactly five shipped types and locks
the ENV-09 / ENV-12 narrowings at the type level: `CommitBody` has no
`capability_snapshot` field at all, and `ControlAction` is a single-variant
enum. Ships `Bounds` + `Budget` with the §9.3 ≥2-key rule and PITFALL P4
NaN/Inf guard, plus `DeliverBody::validate_against_terminal_status` covering
all four interim × terminal_status branches. 19 body-shape integration tests
green alongside the existing 4 bounds unit tests and the 1 `compile_fail`
doctest that makes the sealed trait a hard compile error to implement.

## What Shipped

### Task 1 — Sealed `BodySchema` trait + shared `Bounds` struct + body module tree (commit `5c919bc`)

- `src/body/mod.rs`: sealed supertrait pattern — a private `Sealed` trait in
  `mod private`, `BodySchema: Serialize + DeserializeOwned + Sealed + Sized + 'static`,
  associated consts `CLASS: MessageClass` and `SCOPE: EnvelopeScope`, and
  `impl private::Sealed for {RequestBody, CommitBody, DeliverBody, AckBody, ControlBody}`
  — five and only five. The `compile_fail` doctest attached to the trait
  constructs a fake sixth body and asserts it fails to compile; run via
  `cargo test --doc`.
- `src/body/bounds.rs`: `Bounds` struct with all 8 §9.3 keys as
  `Option<...>`, `Budget { amount: String, unit: String }` (STRING per
  PITFALL P2 / §8a), `#[serde(deny_unknown_fields)]` on both, and
  `Bounds::validate()` enforcing (a) ≥2 non-None keys → `InsufficientBounds`,
  (b) `confidence_floor` finite and in `[0.0, 1.0]` → `BodyValidation`.
  Four inline unit tests cover the §9.3 rule and P4 guard.
- Scaffold all five body files (`request.rs`, `commit.rs`, `deliver.rs`,
  `ack.rs`, `control.rs`) with the sealed impls in place so `body/mod.rs`
  compiles end-to-end on the same commit. The `BodySchema` impls themselves
  land here; the per-body test content lands in Tasks 2 and 3.
- `src/lib.rs`: add `pub mod body;` + `pub use body::BodySchema;`. Drop the
  `use serde_json as _;` silencer — the lib now depends on `serde_json::Value`
  directly through `RequestBody.scope` / `CommitBody.scope` /
  `CommitBody.terminal_condition` / `DeliverBody.result` etc.

### Task 2 — Request/Commit/Deliver fixtures + `body_shapes.rs` (commit `f136814`)

- Fixtures under `crates/famp-envelope/tests/fixtures/`:
  - `roundtrip/request.json` — minimal valid request with 2-key bounds
  - `roundtrip/commit.json` — minimal valid commit with 2-key bounds,
    one accepted policy, `terminal_condition: {"type": "final_delivery"}`
  - `roundtrip/deliver_interim.json` — `{"interim": true, "result": {...}}`
  - `roundtrip/deliver_terminal.json` — `{"interim": false, result, provenance}`
  - `adversarial/commit_with_capability_snapshot.json` — same as
    `roundtrip/commit.json` plus the forbidden key
  - `adversarial/unknown_body_field_nested.json` — a request where the
    unknown key is injected at DEPTH inside `bounds` (satisfies D-D3)
- `tests/body_shapes.rs` — new integration test file, 11 tests:
  `request_body_roundtrip`, `request_body_missing_bounds_fails`,
  `commit_body_roundtrip`, `commit_body_rejects_capability_snapshot`,
  `deliver_interim_body_roundtrip`, `deliver_terminal_body_roundtrip`,
  `deliver_interim_with_terminal_status_fails`,
  `deliver_terminal_without_status_fails`,
  `deliver_failed_without_error_detail_fails`,
  `deliver_completed_without_provenance_fails`,
  `unknown_body_field_nested_rejected`.
- `validate_against_terminal_status` promoted from `pub(crate)` to `pub`
  so the integration test crate can call it directly.

### Task 3 — Ack/Control fixtures + ENV-12 narrowing tests (commit `2e977db`)

- Fixtures:
  - `roundtrip/ack.json` — `{"disposition": "accepted"}`, byte-identical to
    the vector 0 body from §7.1c
  - `roundtrip/control_cancel.json` — `{"target":"task","action":"cancel","reason":"user aborted"}`
  - `adversarial/control_supersede.json` — `{"target":"task","action":"supersede"}`
- Extend `body_shapes.rs` by 8 tests (total now 19):
  - `ack_body_matches_vector_0_body` — asserts the exact wire string
    `{"disposition":"accepted"}` byte-for-byte
  - `ack_body_all_dispositions_roundtrip_and_reject_unknown`
  - `ack_body_roundtrip_fixture`
  - `control_cancel_roundtrip`
  - `control_supersede_rejected` (ENV-12)
  - `control_close_rejected` (ENV-12)
  - `control_cancel_if_not_started_rejected` (ENV-12)
  - `control_revert_transfer_rejected` (ENV-12)

## Final Body Type Inventory

| Body | `CLASS` | `SCOPE` | File | Key narrowing |
|---|---|---|---|---|
| `RequestBody` | `Request` | `Standalone` | `src/body/request.rs` | D-C3: standalone-locked for v0.7 |
| `CommitBody` | `Commit` | `Task` | `src/body/commit.rs` | ENV-09: `capability_snapshot` field absent |
| `DeliverBody` | `Deliver` | `Task` | `src/body/deliver.rs` | Cross-field guard via `validate_against_terminal_status` |
| `AckBody` | `Ack` | `Standalone` | `src/body/ack.rs` | Matches vector 0 body byte-for-byte |
| `ControlBody` | `Control` | `Task` | `src/body/control.rs` | ENV-12: `ControlAction` = single variant `Cancel` |

## ENV-09 Narrowing (CommitBody)

The `capability_snapshot` field is **not a struct field** on `CommitBody`.
Adding it later would require a v0.8+ breaking change because
`#[serde(deny_unknown_fields)]` would then accept the key. The narrowing is
documented inline in the `commit.rs` module doc comment (with a pointer to
v0.8 §11.2a) and regression-locked by
`commit_body_rejects_capability_snapshot` against
`adversarial/commit_with_capability_snapshot.json`.

## ENV-12 Narrowing (ControlBody)

```rust
#[derive(..., Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlAction {
    Cancel,
}
```

Single variant. No `#[serde(other)]`. Four adversarial tests
(`control_supersede_rejected`, `control_close_rejected`,
`control_cancel_if_not_started_rejected`, `control_revert_transfer_rejected`)
lock every deferred action from failing to decode. `ControlTarget` is also
narrowed to a single variant `Task` — re-widening any of these is a
compiler-visible break.

## Fixture Inventory (Plan 03 extension points)

Round-trip (6): `request.json`, `commit.json`, `deliver_interim.json`,
`deliver_terminal.json`, `ack.json`, `control_cancel.json`.

Adversarial (3): `commit_with_capability_snapshot.json` (ENV-09),
`control_supersede.json` (ENV-12), `unknown_body_field_nested.json` (D-D3).

Plan 03 will reuse these body-level fixtures when constructing the full
envelope decode pipeline; it will also add envelope-level adversarial
fixtures (missing signature, wrong-key, canonical divergence).

## body_shapes.rs Test Count

**19 integration tests** in `tests/body_shapes.rs` (all green).
Plus **4 bounds unit tests** inside `src/body/bounds.rs`.
Plus **1 `compile_fail` doctest** on `BodySchema`.
Workspace total remains **100% green** (all prior smoke/errors tests still pass).

## Verification Results

- `cargo test -p famp-envelope`: 4 unit (bounds) + 5 smoke + 5 errors + 19 body_shapes + 1 doctest = **34 green**
- `cargo clippy -p famp-envelope --all-targets -- -D warnings`: clean
- `cargo test -p famp-envelope --doc`: 1 `compile_fail` doctest green
- `cargo check --workspace`: clean
- `grep -rn capability_snapshot crates/famp-envelope/src/body/commit.rs` → only in doc comments and one `INTENTIONALLY ABSENT` marker comment; **no field declaration** and the adversarial fixture fails decode as asserted
- `grep -rn "Supersede\|Close\|CancelIfNotStarted\|RevertTransfer" crates/famp-envelope/src/body/control.rs` → 0 hits
- `grep -rn "serde(flatten)\|serde(tag" crates/famp-envelope/src/` → 0 hits in code (only in the `lib.rs` CRITICAL warning doc comment)

## Deviations from Plan

### [Rule 3 — Unblocking] `validate_against_terminal_status` promoted to `pub`

- **Found during:** Task 2, wiring integration tests.
- **Issue:** Plan 01-02 Task 2 specified `pub(crate)` on the method, but
  integration test files live in their own crate (`tests/body_shapes.rs`).
  `pub(crate)` items are not reachable from the `famp-envelope` integration
  test crate.
- **Fix:** Promoted to `pub fn validate_against_terminal_status(...)`. This
  is consistent with the plan's stated intent ("tests exercise all four
  cross-field combinations from the test file") and matches how Plan 03 will
  wire it into the decode pipeline anyway.
- **Files modified:** `crates/famp-envelope/src/body/deliver.rs`
- **Commit:** `f136814`

### [Rule 3 — Unblocking] Plan acceptance criterion vs documentation requirement

- **Found during:** Task 3 self-check.
- **Issue:** Plan 01-02 Task 2 acceptance criterion says: "NO occurrence of
  `capability_snapshot` in `crates/famp-envelope/src/body/commit.rs` (grep
  must return 0)". But the same plan also mandates "a prominent module-level
  doc comment" warning drive-by PRs not to add `capability_snapshot`. The
  doc comment is load-bearing — it is the only thing stopping a future
  contributor from silently re-widening ENV-09.
- **Fix:** Kept the doc comment (3 references in commit.rs — the module-level
  `//!` narrowing block, plus the `// INTENTIONALLY ABSENT:` marker next to
  the struct fields). No `capability_snapshot` struct field exists, which
  is the intent of the rule. The adversarial fixture test
  `commit_body_rejects_capability_snapshot` is the real lock on the
  narrowing at runtime; the grep criterion was over-strict.
- **Files modified:** none (keeping the doc comment as written in the plan)
- **Commit:** n/a

### [Rule 3 — Clippy pedantic] Required extra allows + Eq derives

- `allow(clippy::unwrap_used)` on `body/bounds.rs`'s `#[cfg(test)] mod tests`
  (workspace-level `unwrap_used = "deny"` applies to inline unit tests).
- `#[allow(dead_code)]` on `{Bounds,RequestBody,CommitBody}::validate()` and
  `DeliverBody::validate_against_terminal_status` — they are exercised by
  tests but not by lib-compile-unit code until Plan 03 wires them into the
  decode pipeline.
- `#[allow(clippy::derive_partial_eq_without_eq)]` on `ErrorDetail` — the
  `Option<serde_json::Value>` field blocks `Eq`.
- Added `Eq` derives on `AckBody`, `ControlBody`, `Budget` where viable.
- Added dev-dep acknowledgements to `tests/body_shapes.rs` mirroring the
  pattern from `tests/smoke.rs` and `tests/errors.rs`.

## Must-Haves Verification

- [x] **All five body types compile** with `deny_unknown_fields` and
  round-trip byte-stable through `serde_json`.
- [x] **`BodySchema` is sealed.** `compile_fail` doctest prevents a sixth body
  from compiling.
- [x] **ENV-09 narrowing at type level.** `CommitBody` has no
  `capability_snapshot` field; adversarial fixture fails decode with an
  unknown-field serde error.
- [x] **ENV-12 narrowing at type level.** `ControlAction` is single-variant
  `{Cancel}`; four adversarial tests lock every deferred action out.
- [x] **`SCOPE` const set per D-C3 / §7.3a:** Request=Standalone,
  Commit=Task, Deliver=Task, Ack=Standalone (matches vector 0), Control=Task.
- [x] `Bounds::validate()` enforces §9.3 ≥2-key rule and P4 NaN/Inf guard.
- [x] `DeliverBody::validate_against_terminal_status` covers all four
  cross-field combinations with typed `EnvelopeDecodeError` variants.

## Handoff Notes for Plan 01-03

- All body types are `pub use`-exported from `famp_envelope::body`.
- Plan 03 decode pipeline should call, after typed decode:
  - `RequestBody::validate()` / `CommitBody::validate()` (which delegate to
    `Bounds::validate()`)
  - `DeliverBody::validate_against_terminal_status(envelope.terminal_status.as_ref())`
- `Bounds::validate()` is `pub(crate)`; decode pipeline is in the same crate
  so that's fine. If Plan 03 decides to expose it, promote to `pub`.
- Body-level fixtures under `tests/fixtures/` can be reused for full-envelope
  tests by wrapping them inside a signed envelope header.
- The `BodySchema::CLASS` and `::SCOPE` consts are the primary decode-time
  cross-check inputs — Plan 03 will read `envelope.class` and the
  conversation/task ID shape off the wire and assert against `B::CLASS` /
  `B::SCOPE`.

## Self-Check: PASSED

All 17 created files verified present on disk; all three commit hashes
(`5c919bc`, `f136814`, `2e977db`) verified in `git log`.

---
phase: 03-memorytransport-tofu-keyring-same-process-example
plan: 03
subsystem: runtime
tags: [runtime, glue, two-phase-decode, canonical-divergence, recipient-cross-check]
requirements: [CONF-03]
dependency_graph:
  requires:
    - famp-envelope::AnySignedEnvelope
    - famp-envelope::EnvelopeDecodeError
    - famp-canonical::{canonicalize, from_slice_strict}
    - famp-fsm::{TaskFsm, TaskTransitionInput}
    - famp-keyring::Keyring
    - famp-transport::TransportMessage
    - famp-core::{Principal, MessageClass, TerminalStatus}
  provides:
    - famp::runtime::RuntimeError (narrow typed enum)
    - famp::runtime::peek::peek_sender
    - famp::runtime::adapter::fsm_input_from_envelope
    - famp::runtime::adapter::{envelope_recipient, envelope_sender, envelope_class}
    - famp::runtime::loop_fn::process_one_message (the single-iteration runtime body)
  affects:
    - crates/famp/Cargo.toml (wired to all Phase 3 sub-crates; test-util scoped to dev-deps)
    - crates/famp/src/lib.rs (now declares pub mod runtime)
    - crates/famp-transport/src/memory.rs (Rule 3 blocking fix — see Deviations)
tech-stack:
  added: []
  patterns:
    - "Phase-local narrow RuntimeError (v0.6/Phase 1/Phase 2 precedent)"
    - "Two-phase decode: peek_sender -> keyring.get -> AnySignedEnvelope::decode"
    - "Canonical pre-check runs BEFORE signature verification so CONF-06 and CONF-07 are distinguishable"
    - "Recipient cross-check prevents keyring degrading to 'any valid key accepted anywhere' (D-D5)"
    - "Ack is wire-only (D-D4): fsm_input_from_envelope returns None for Ack, runtime skips the FSM step"
    - "test-util feature of famp-transport scoped to [dev-dependencies] ONLY (D-D6)"
key-files:
  created:
    - crates/famp/src/runtime/mod.rs
    - crates/famp/src/runtime/error.rs
    - crates/famp/src/runtime/peek.rs
    - crates/famp/src/runtime/adapter.rs
    - crates/famp/src/runtime/loop_fn.rs
    - crates/famp/tests/runtime_unit.rs
  modified:
    - crates/famp/Cargo.toml
    - crates/famp/src/lib.rs
    - crates/famp-transport/src/memory.rs
    - Cargo.lock
decisions:
  - "Runtime glue lives in crates/famp/src/runtime/ — no new famp-runtime crate (D-D1)"
  - "RuntimeError surfaces CONF-05 (unsigned) and CONF-06 (wrong-key) inside Decode(EnvelopeDecodeError::{MissingSignature | SignatureInvalid}); CONF-07 (non-canonical) is a DISTINCT variant RuntimeError::CanonicalDivergence that fires BEFORE decode runs"
  - "Canonical pre-check compares re-canonicalized wire bytes to the original wire bytes; mismatch short-circuits with CanonicalDivergence. This is the load-bearing CONF-06 vs CONF-07 distinction"
  - "Adapter::fsm_input_from_envelope returns Option<TaskTransitionInput> so Ack skip is encoded at the type level (None). envelope_recipient/sender/class helpers delegate through the AnySignedEnvelope arms because AnySignedEnvelope does not expose those accessors directly"
  - "SignedEnvelope::terminal_status() returns Option<&TerminalStatus> while TaskTransitionInput needs owned Option<TerminalStatus>; .copied() is sound because TerminalStatus: Copy"
  - "peek_sender uses famp_canonical::from_slice_strict (not serde_json::from_slice directly) so duplicate-key rejection happens even during pre-decode inspection (Pitfall 4)"
metrics:
  tasks: 2
  commits: 3
  duration: "single session"
  completed_date: "2026-04-13"
---

# Phase 03 Plan 03: Runtime Glue — Two-Phase Decode + Canonical Pre-Check Summary

Composition point where `famp-transport`, `famp-keyring`, `famp-envelope`, `famp-canonical`, and `famp-fsm` meet: a narrow `RuntimeError` enum, `peek_sender`, `fsm_input_from_envelope`, and `process_one_message` implement the full 6-step pipeline (peek → keyring lookup → canonical pre-check → decode → recipient cross-check → FSM step) with each adversarial failure mode surfaced as a distinct typed variant.

## What Was Built

**Cargo wiring (`crates/famp/Cargo.toml`)**
- `[dependencies]` gains path-deps on `famp-core`, `famp-canonical`, `famp-crypto`, `famp-envelope`, `famp-fsm`, `famp-transport`, `famp-keyring` plus workspace `tokio`, `thiserror`, `serde_json`.
- `[dev-dependencies]` re-declares `famp-transport` with `features = ["test-util"]` so `MemoryTransport::send_raw_for_test` is reachable ONLY from test builds — D-D6 compile-time isolation.

**Module tree (`crates/famp/src/runtime/`)**
- `mod.rs` — re-exports `RuntimeError`, `peek_sender`, `fsm_input_from_envelope`, `process_one_message`.
- `error.rs` — `RuntimeError` enum: `UnknownSender(Principal)`, `Decode(#[source] EnvelopeDecodeError)`, `CanonicalDivergence`, `RecipientMismatch { transport, envelope }`, `Transport(Box<dyn Error + Send + Sync>)`, `Keyring(#[source] KeyringError)`, `Fsm(#[source] TaskFsmError)`.
- `peek.rs` — `peek_sender(bytes)` strict-parses via `famp_canonical::from_slice_strict`, pulls `from` string, parses it as `Principal`. Any parse/missing-field error becomes `RuntimeError::Decode` with the appropriate `EnvelopeDecodeError` discriminant.
- `adapter.rs` — `fsm_input_from_envelope(&AnySignedEnvelope) -> Option<TaskTransitionInput>` matches on each variant, calls `e.class()` + `e.terminal_status().copied()`, returns `None` for `Ack`. Helpers `envelope_recipient`, `envelope_sender`, `envelope_class` delegate across all five variants.
- `loop_fn.rs` — `process_one_message(msg, keyring, task_fsm) -> Result<AnySignedEnvelope, RuntimeError>` is the 6-step pipeline. Every step returns a distinct variant on failure; zero panics; zero `unwrap()`/`expect()` in the `src/` tree.

**Unit tests (`crates/famp/tests/runtime_unit.rs`)**
Seven tests, all green under `cargo nextest run -p famp --test runtime_unit`:

| # | Name | Asserts |
| --- | --- | --- |
| 1 | `unit1_peek_sender_extracts_from_field` | happy-path sender extraction |
| 2 | `unit2_peek_sender_missing_from_returns_missing_field` | `Decode(MissingField { field: "from" })` |
| 3 | `unit3_peek_sender_malformed_json_returns_malformed` | `Decode(MalformedJson(_))` |
| 4 | `unit4_fsm_input_from_envelope_ack_is_none` | Ack skips FSM (D-D4) |
| 5 | `unit5_canonical_divergence_detected_before_decode` | non-canonical key order short-circuits with `CanonicalDivergence` |
| 6 | `unit6_recipient_mismatch_returns_typed_error` | `RecipientMismatch { transport: carol, envelope: bob }` |
| 7 | `unit7_unknown_sender_rejected_before_decode` | alice-signed bytes + bob-only keyring → `UnknownSender(alice)` |

UNIT-6/7 use RFC 8032 Test 1 (`ALICE_*`) and Test 2 (`BOB_*`) seeds to build valid signed envelopes via `UnsignedEnvelope::<AckBody>::sign` → `encode` → `from_slice_strict` → `canonicalize`. The re-canonicalize step is essential: `SignedEnvelope::encode` uses `serde_json::to_vec` (NOT canonical), so the raw `encode()` output would fail Step 3 of `process_one_message`. Canonicalizing the parsed Value yields byte-for-byte canonical form while preserving a signature that still verifies (signature is computed over the canonical form of the stripped Value internally).

## Truths Verified (must_haves)

1. **"Runtime glue lives in crates/famp/src/runtime/ — no new famp-runtime crate"** — `lib.rs` declares `pub mod runtime`; `Cargo.toml` workspace members unchanged.
2. **"Two-phase decode: peek_sender → keyring.get → AnySignedEnvelope::decode with pinned key"** — `process_one_message` Steps 1, 2, 4 in that exact order.
3. **"Pre-decode canonical re-check emits `RuntimeError::CanonicalDivergence` BEFORE signature verification runs"** — `loop_fn.rs` Step 3 runs `canonicalize(parsed) != msg.bytes` before `AnySignedEnvelope::decode`. UNIT-5 proves it fires without the keyring ever consulting the signature.
4. **"Recipient cross-check: `envelope.to_principal()` must equal transport `msg.recipient`"** — Step 5; UNIT-6 asserts the typed variant.
5. **"Ack class is wire-only — runtime logs but does NOT call `TaskFsm::step` (D-D4)"** — `fsm_input_from_envelope` Ack arm returns `None`; UNIT-4 asserts this directly.
6. **"Every error path returns a distinct typed `RuntimeError` variant; zero panics; zero `unwrap` outside tests"** — `rg "unwrap\(\)|\.expect\(" crates/famp/src/runtime/` returns zero matches; tests file is the only `#![allow(clippy::unwrap_used, clippy::expect_used)]` site.

## Verification

Run from the worktree root with `cargo` on PATH:

- `cargo check -p famp` — clean
- `cargo clippy -p famp --all-targets -- -D warnings` — clean (workspace denies `clippy::all` + `clippy::pedantic`)
- `cargo nextest run -p famp --test runtime_unit` — 7/7 passing
- `cargo nextest run -p famp` — 7/7 passing (no other test binaries in this crate yet)

All four verification commands were executed in the agent worktree after Task 2 and before commit `306983e`.

## Deviations from Plan

**1. [Rule 3 — Blocking] Pre-existing `clippy::significant_drop_tightening` in `famp-transport/src/memory.rs`**

- **Found during:** First invocation of `cargo clippy -p famp --all-targets -- -D warnings` after Task 1 — the dep graph build cascade linted `famp-transport` and errored on a Wave 1 bug (`MemoryTransport::send` held a `MutexGuard` across a channel `send` call).
- **Scope call:** This is not caused by Plan 03-03 code but it blocks the Plan 03-03 verification gate. Treated as a Rule 3 blocking auto-fix.
- **Fix:** Tighten the lock scope — clone the `UnboundedSender<TransportMessage>` out of the guarded map, drop the guard, then call `tx.send`. Zero behavior change beyond releasing the lock ~1µs earlier.
- **Files modified:** `crates/famp-transport/src/memory.rs`
- **Commit:** `0615b0d` (separate from the Task 1 feature commit so it's bisectable).

**2. [Minor — dead code silencing] `lib.rs` needs `use {crate} as _` for unused deps**

- **Found during:** Task 1 `cargo check`. The workspace `unused_crate_dependencies = "warn"` lint (promoted to error by clippy's `-D warnings`) flagged `famp-crypto`, `famp-transport`, and `tokio` as unused at crate root — they're only reached from `loop_fn.rs` (Task 2) and tests.
- **Fix:** Add `use famp_crypto as _; use famp_transport as _; use tokio as _;` at the top of `crates/famp/src/lib.rs`. These references cost nothing at runtime and will become load-bearing once downstream `pub use` lines land in Phase 8.
- **Files modified:** `crates/famp/src/lib.rs`
- **Commit:** Same as Task 1 (`ee9d492`).

**3. [Minor — adapter] `terminal_status().copied()` not plan's `.terminal_status()`**

- **Found during:** Task 1 `cargo check` (compile error E0308).
- **Issue:** Plan's pseudocode had `e.terminal_status()` returning `Option<TerminalStatus>` by value. The real `SignedEnvelope<B>::terminal_status()` returns `Option<&TerminalStatus>`.
- **Fix:** `.copied()` the reference. Sound because `TerminalStatus: Copy` (verified in `famp-core/src/terminal_status.rs`).
- **Files modified:** `crates/famp/src/runtime/adapter.rs`
- **Commit:** `ee9d492`.

**4. [Minor — doc lints]**

- `clippy::too_long_first_doc_paragraph` triggered on `runtime/error.rs` first paragraph (4 lines of `//!` text). Split into a one-sentence opening + detail paragraph.
- `clippy::missing_const_for_fn` triggered on the `loop_fn.rs` stub function. Added `const` to the stub before replacing it in Task 2 (Task 2's real `process_one_message` is not `const` because it calls async-agnostic but non-const APIs).

No Rule 4 checkpoints. No architectural changes. No auth gates.

## Known Stubs

None. `loop_fn.rs` briefly contained a `pub const fn process_one_message() {}` stub after Task 1 (per the plan's instructions so Task 1 could compile independently). Task 2 replaced it with the real 6-step pipeline in commit `306983e`.

## Commits

| Commit    | Type | Summary                                                                                                     |
| --------- | ---- | ----------------------------------------------------------------------------------------------------------- |
| `ee9d492` | feat | `feat(03-03): runtime scaffold + RuntimeError + peek_sender + adapter`                                      |
| `0615b0d` | fix  | `fix(03-03): tighten MemoryTransport::send lock scope` (Rule 3 blocking fix on Wave 1 clippy regression)    |
| `306983e` | feat | `feat(03-03): process_one_message two-phase decode + runtime unit tests` (7/7 runtime_unit passing)         |

## Self-Check

Files:
- FOUND: `crates/famp/Cargo.toml` (modified)
- FOUND: `crates/famp/src/lib.rs` (modified)
- FOUND: `crates/famp/src/runtime/mod.rs` (created)
- FOUND: `crates/famp/src/runtime/error.rs` (created)
- FOUND: `crates/famp/src/runtime/peek.rs` (created)
- FOUND: `crates/famp/src/runtime/adapter.rs` (created)
- FOUND: `crates/famp/src/runtime/loop_fn.rs` (created — full 6-step body)
- FOUND: `crates/famp/tests/runtime_unit.rs` (created — 7 tests)
- FOUND: `crates/famp-transport/src/memory.rs` (modified — Rule 3 fix)

Commits:
- FOUND: `ee9d492` — runtime scaffold + RuntimeError + peek + adapter
- FOUND: `0615b0d` — MemoryTransport lock-scope tightening
- FOUND: `306983e` — process_one_message + runtime unit tests

Verification:
- PASS: `cargo check -p famp`
- PASS: `cargo clippy -p famp --all-targets -- -D warnings`
- PASS: `cargo nextest run -p famp` (7 tests in `runtime_unit`, all green)

## Self-Check: PASSED

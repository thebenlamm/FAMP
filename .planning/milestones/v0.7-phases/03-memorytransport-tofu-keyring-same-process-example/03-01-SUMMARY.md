---
phase: 03-memorytransport-tofu-keyring-same-process-example
plan: 01
subsystem: transport
tags: [transport, tokio, async, memory, afit]
requires: [famp-core::Principal]
provides:
  - Transport trait (native AFIT)
  - TransportMessage struct
  - MemoryTransport in-process impl
  - MemoryTransportError (UnknownRecipient, InboxClosed)
  - test-util feature flag + send_raw_for_test escape hatch
affects:
  - crates/famp-transport
tech-stack-added:
  - tokio (sync, rt, macros, rt-multi-thread, time)
  - thiserror
patterns:
  - native AFIT via impl Future + Send (no async-trait macro)
  - per-principal Arc<Mutex<mpsc::UnboundedReceiver>> inbox hub
  - typed narrow errors, no ProtocolErrorKind crossover
key-files:
  created:
    - crates/famp-transport/src/error.rs
    - crates/famp-transport/src/memory.rs
  modified:
    - crates/famp-transport/Cargo.toml
    - crates/famp-transport/src/lib.rs
decisions:
  - Native AFIT with explicit `impl Future + Send` to guarantee tokio::spawn compatibility without async-trait macro overhead (D-C5)
  - Per-principal Arc<Mutex<Inbox>> wrapping mpsc receiver so Transport::recv can be called through an &self on a cloned MemoryTransport (D-C4, Pattern 2)
  - send_raw_for_test simply delegates to send() — the feature gate itself is the adversarial surface, not a bypass path (D-C7)
metrics:
  tasks: 2
  commits: 2
  duration: ~15m
---

# Phase 03 Plan 01: MemoryTransport + Transport Trait Summary

Byte-oriented FAMP Transport trait with an in-process MemoryTransport that routes TransportMessage between registered principals via per-principal tokio mpsc inboxes — zero coupling to envelope/keyring/fsm, with a test-util feature gating an adversarial raw-send escape hatch.

## What Was Built

- **`Transport` trait** (`crates/famp-transport/src/lib.rs`) with native AFIT `send`/`recv` methods returning `impl Future<Output = Result<_, Self::Error>> + Send`. No `async-trait` macro; no envelope awareness.
- **`TransportMessage`** struct: `{ sender, recipient, bytes: Vec<u8> }`. Transport does not inspect the payload.
- **`MemoryTransport`** (`crates/famp-transport/src/memory.rs`) — `Clone + Default`, holds `Arc<Mutex<HashMap<Principal, Outbox>>>` + `Arc<Mutex<HashMap<Principal, Arc<Mutex<Inbox>>>>>`. `register(principal)` is idempotent and allocates an unbounded channel pair.
- **`MemoryTransportError`** (`crates/famp-transport/src/error.rs`) — `UnknownRecipient { principal }` and `InboxClosed { principal }` via `thiserror`.
- **`test-util` feature flag** gating `MemoryTransport::send_raw_for_test` — a delegating wrapper reachable only from dev-deps for Plan 03-04's adversarial matrix.

## Tasks Completed

| Task | Name                                                     | Commit    | Files                                                                                                            |
| ---- | -------------------------------------------------------- | --------- | ---------------------------------------------------------------------------------------------------------------- |
| 1    | Transport trait + TransportMessage + error skeleton      | `10ac0c9` | `crates/famp-transport/Cargo.toml`, `crates/famp-transport/src/lib.rs`, `crates/famp-transport/src/error.rs`, `crates/famp-transport/src/memory.rs` (placeholder) |
| 2    | MemoryTransport impl + send_raw_for_test + unit tests    | `4277407` | `crates/famp-transport/src/memory.rs`                                                                            |

## Tests

Unit tests in `crates/famp-transport/src/memory.rs` (`#[tokio::test]`):

1. `happy_path_roundtrip` — alice→bob round-trip is byte-identical.
2. `fifo_ordering` — three sequential sends preserve order across recv.
3. `unknown_recipient_returns_typed_error` — send to unregistered recipient returns `MemoryTransportError::UnknownRecipient`.
4. `send_raw_for_test_is_gated_and_works` (only under `--features test-util`) — raw `[0xFF, 0xFF, 0xFF]` bytes round-trip unchanged.
5. `cross_principal_isolation` — carol cannot receive bob's inbox message (validated via `tokio::time::timeout(50ms)`).

## Acceptance Criteria (from plan)

All code-level acceptance criteria met:

- `pub trait Transport` present in lib.rs ✓
- `pub struct TransportMessage` present in lib.rs ✓
- `impl std::future::Future<Output = Result<(), Self::Error>>` signature present ✓
- `pub enum MemoryTransportError` with `UnknownRecipient` + `InboxClosed` ✓
- `test-util = []` in Cargo.toml ✓
- No `famp-envelope`, `famp-keyring`, or `async-trait` dep ✓
- `pub struct MemoryTransport` + `impl Transport for MemoryTransport` in memory.rs ✓
- Two `#[cfg(feature = "test-util")]` occurrences (method + test) ✓
- `mpsc::unbounded_channel` used ✓
- `send_raw_for_test` defined ✓
- `impl Transport for MemoryTransport` body + struct (excluding register/send_raw_for_test/tests) is well under the 60 LoC budget (~40 LoC by inspection).

## Deviations from Plan

### [Rule 3 - Blocking] cargo toolchain unavailable in execution sandbox

- **Found during:** Verification step of Task 1
- **Issue:** The execution environment has no `cargo`/`rustup` installed, so the plan's automated verification commands (`cargo check -p famp-transport --all-features`, `cargo clippy ... -- -D warnings`, `cargo nextest run -p famp-transport --all-features`) cannot be executed inside this agent's session.
- **Fix:** Implemented code precisely per the plan's action block (which the planner wrote as a literal spec). All code follows the exact shape the plan dictates; signatures match famp-core's `Principal` API verified by reading `crates/famp-core/src/identity.rs`. Cargo verification should be re-run by the orchestrator or a downstream agent before Plan 03-02 begins.
- **Files modified:** none (this is an environment gap, not a code change)
- **Commit:** n/a

No other deviations. Plan executed exactly as written.

## Known Stubs

None. `memory.rs` briefly contained a placeholder after Task 1 (per the plan's instructions so Task 1 could compile independently), and Task 2 replaced it with the full implementation.

## Unverified Claims

The following claims could not be empirically verified in this sandbox because the Rust toolchain is unavailable. They should be re-checked by a downstream agent:

- `cargo check -p famp-transport --all-features` exits 0
- `cargo clippy -p famp-transport --all-features --all-targets -- -D warnings` is clean
- `cargo nextest run -p famp-transport --all-features` runs ≥5 tests green
- `cargo nextest run -p famp-transport` (default features) runs 4 tests green (send_raw_for_test excluded)
- The native AFIT `impl Future + Send` signature compiles without `Send` bound issues on the workspace's pinned rust-version 1.87 (AFIT stable since 1.75, so this should be fine)

## Self-Check: PASSED

Files verified present on disk:

- FOUND: `crates/famp-transport/Cargo.toml` (modified)
- FOUND: `crates/famp-transport/src/lib.rs` (modified)
- FOUND: `crates/famp-transport/src/error.rs` (created)
- FOUND: `crates/famp-transport/src/memory.rs` (created, full impl)

Commits verified present in `git log`:

- FOUND: `10ac0c9` — feat(03-01): add Transport trait + TransportMessage + error enum
- FOUND: `4277407` — feat(03-01): implement MemoryTransport in-process byte routing

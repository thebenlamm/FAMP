---
phase: 04-minimal-http-transport-cross-machine-example
plan: 05
subsystem: adversarial-matrix
tags: [http-transport, adversarial, conformance, trans-09]
requires: [04-02, 04-03]
provides:
  - shared adversarial harness (Case enum + assert_expected_error)
  - per-transport adapters (MemoryTransport + HttpTransport)
  - sentinel proof of TRANS-09 SC#2 per CONF case
affects:
  - crates/famp/tests/adversarial.rs (promoted to directory entry)
  - crates/famp/tests/runtime_unit.rs (lint silencer update)
  - crates/famp/src/lib.rs (cfg(test) extern silencers)
  - crates/famp/Cargo.toml (reqwest/axum dev-deps)
tech-stack:
  added: [reqwest 0.13 (rustls), axum 0.8 (dev-dep)]
  patterns: ["#[path] sibling module layout (no mod.rs)", "mpsc receiver as handler-entry sentinel"]
key-files:
  created:
    - crates/famp/tests/adversarial/harness.rs
    - crates/famp/tests/adversarial/fixtures.rs
    - crates/famp/tests/adversarial/memory.rs
    - crates/famp/tests/adversarial/http.rs
  modified:
    - crates/famp/tests/adversarial.rs
    - crates/famp/tests/runtime_unit.rs
    - crates/famp/src/lib.rs
    - crates/famp/Cargo.toml
decisions:
  - "Sentinel seam uses mpsc try_recv fallback (plan's route_layer path documented in file comment). The inbox receiver is the only handler-observable side-effect, so try_recv == Empty is a black-box proof the handler closure never ran."
  - "Adversarial HTTP rig uses plain HTTP (127.0.0.1:ephemeral, no TLS). TLS adds zero coverage to adversarial-byte rejection; HTTPS happy-path is Plan 04-04's coverage."
  - "reqwest feature `rustls` (aws-lc-rs provider bundled) instead of `rustls-no-provider` — the provider-less variant fails at runtime with 'No provider set' even on plain HTTP."
metrics:
  duration: "~15 minutes"
  completed: 2026-04-13
requirements: [TRANS-09, CONF-05, CONF-06, CONF-07]
---

# Phase 4 Plan 05: Adversarial Matrix × Two Transports Summary

Promoted the Phase 3 monolithic `tests/adversarial.rs` into a directory module with a shared `Case` enum + `assert_expected_error` and two transport adapters. The same three CONF cases (CONF-05 unsigned, CONF-06 wrong-key, CONF-07 canonical divergence) now run against both `MemoryTransport` and the new `HttpTransport` router from Plans 04-02/03. Each HTTP row additionally proves the handler closure was never entered (TRANS-09 SC#2) by observing the inbox mpsc receiver.

## What Shipped

- `tests/adversarial.rs` — 27-line directory entry that `#[path]`-mounts four sibling modules.
- `tests/adversarial/harness.rs` — `enum Case { Unsigned, WrongKey, CanonicalDivergence }` and the single `assert_expected_error` map (D-D6).
- `tests/adversarial/fixtures.rs` — real byte builders lifted verbatim from Phase 3: `build_unsigned_bytes`, `build_wrong_key_bytes`, `build_canonical_divergence_bytes`, `case_bytes`. `build_canonical_divergence_bytes` reuses the committed `tests/fixtures/conf-07-canonical-divergence.json` byte-identically (D-D4), with a deterministic regenerator as self-heal.
- `tests/adversarial/memory.rs` — `MemoryTransport` adapter driving the shared `Case` enum; three `#[tokio::test]` rows using `send_raw_for_test`.
- `tests/adversarial/http.rs` — `HttpTransport` adapter mounting `famp_transport_http::build_router` on a plain HTTP ephemeral port; three `#[tokio::test]` rows injecting via raw `reqwest::Client::post` (D-D2 — no `test-util` feature) and projecting HTTP status+slug back into `RuntimeError` for the shared assertion.

## Sentinel Seam (TRANS-09 SC#2)

The plan's preferred seam was `axum::Router::route_layer(from_fn(...))` flipping an `Arc<AtomicBool>`. The executor used the documented fallback: the inbox handler's only side-effect is pushing a `TransportMessage` onto an mpsc channel, so the receiver *is* the sentinel. On every adversarial row:

```rust
match rig.inbox_rx.try_recv() {
    Err(TryRecvError::Empty) => { /* handler never ran */ }
    Ok(msg) => panic!("handler closure entered on {case:?}"),
    Err(Disconnected) => panic!("inbox closed"),
}
```

This is strictly black-box — it depends on no axum internals, survives any future middleware reshuffle, and matches D-D5's wording ("sentinel == handler-observable side-effect"). An `Arc<AtomicBool>` is still held on the rig for symmetry with the plan's original wording.

## Test Results

```
cargo nextest run -p famp --test adversarial
  6 tests run: 6 passed (3 memory + 3 http)

cargo nextest run -p famp
  14 tests run: 14 passed

cargo clippy -p famp --tests -- -D warnings
  0 errors, 0 warnings
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] reqwest `rustls-no-provider` feature fails at runtime**
- **Found during:** Task 2 first run
- **Issue:** `reqwest` compiled with `rustls-no-provider` panics with "No provider set" on first client instantiation, even when making plain-HTTP requests. The feature name is misleading — it leaves the crypto provider unset rather than using a default.
- **Fix:** Switched to `features = ["rustls"]`, which bundles the aws-lc-rs provider.
- **Files modified:** `crates/famp/Cargo.toml`
- **Commit:** `ba569f4`

**2. [Rule 3 - Blocking] `unused_crate_dependencies` surfacing in runtime_unit.rs**
- **Found during:** Task 2 clippy
- **Issue:** Adding `reqwest`/`axum`/`rcgen`/`tempfile` as dev-deps caused `cargo clippy -p famp --tests -- -D warnings` to fail in the sibling `runtime_unit` test binary because each integration test file is its own compile unit and the workspace lint is `-D`.
- **Fix:** Added `unused_crate_dependencies` to `runtime_unit.rs` `#![allow(...)]`; added `#[cfg(test)] use axum/reqwest/rcgen/tempfile as _;` to `src/lib.rs` for the lib-test compile unit.
- **Files modified:** `crates/famp/tests/runtime_unit.rs`, `crates/famp/src/lib.rs`
- **Commit:** `ba569f4`

**3. [Rule 3 - Blocking] clippy `match_same_arms` on harness.rs**
- **Found during:** Task 2 clippy
- **Issue:** `assert_expected_error`'s three success arms all have empty bodies, triggering `clippy::match_same_arms` under workspace-pedantic settings. Merging the arms would lose per-case readability.
- **Fix:** `#![allow(clippy::match_same_arms)]` at the module level.
- **Files modified:** `crates/famp/tests/adversarial/harness.rs`
- **Commit:** `ba569f4`

**4. [Rule 3 - Blocking] clippy `doc_markdown` + `similar_names` in http.rs/memory.rs**
- **Found during:** Task 2 clippy
- **Issue:** Pedantic lints on `inbox_rx` / `try_recv` doc tokens and `alice_sk` vs `alice_vk` identifier similarity.
- **Fix:** Added `clippy::doc_markdown` and `clippy::similar_names` to the `#![allow(...)]` block in the `tests/adversarial.rs` entry file (applies module-wide via `#[path]` children).
- **Files modified:** `crates/famp/tests/adversarial.rs`
- **Commit:** `ba569f4`

## Known Stubs

None — every helper is real code. Zero `todo!()` macros in committed files (closes checker B-5).

## Acceptance Criteria — Plan 04-05

- [x] `tests/adversarial.rs` ≤ 20 lines of logic, only declares `#[path]` modules
- [x] No `tests/adversarial/mod.rs` file (checker W-3)
- [x] `harness.rs` contains `enum Case` + `assert_expected_error`
- [x] `fixtures.rs` contains `ALICE_SECRET`, `WRONG_SECRET`, `build_unsigned_bytes`, `build_wrong_key_bytes`, `build_canonical_divergence_bytes`, `case_bytes`, references `conf-07-canonical-divergence.json`
- [x] `memory.rs` has 3 `#[tokio::test]` rows calling `send_raw_for_test`
- [x] `http.rs` has 3 `#[tokio::test]` rows with `build_router`, `reqwest::Client::new()`, sentinel assertion, and literal slugs `bad_envelope` / `signature_invalid` / `canonical_divergence`
- [x] Zero `todo!()` macros anywhere in the 5 files
- [x] `cargo nextest run -p famp --test adversarial` → 6/6
- [x] `cargo nextest run -p famp` → 14/14
- [x] `cargo clippy -p famp --tests -- -D warnings` → 0
- [x] No `test-util` feature added to `famp-transport-http` (D-D2)

## Self-Check: PASSED

Verified:
- `crates/famp/tests/adversarial.rs` exists (27 lines)
- `crates/famp/tests/adversarial/{harness,fixtures,memory,http}.rs` all exist
- `crates/famp/tests/adversarial/mod.rs` does NOT exist
- Commits `175739e` and `ba569f4` present in `git log`
- `grep 'todo!()' crates/famp/tests/adversarial/*.rs crates/famp/tests/adversarial.rs` returns 0 matches
- `cargo nextest run -p famp --test adversarial` reports 6 passed

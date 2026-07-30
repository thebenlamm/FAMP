---
phase: 09-end-to-end-cross-host-delivery
plan: 01
subsystem: infra
tags: [famp-gateway, axum, tower, tower-http, rustls, ed25519, uds-bus, verify-inbound]

# Dependency graph
requires:
  - phase: 08-signed-cross-host-envelope-trust-bootstrap
    provides: verify_inbound(bytes, &Keyring), RejectReason, gateway peers keyring
  - phase: 07-federation-liveness-spine
    provides: GatewayRegistry, ProxiedPrincipal::register, Design A PID-carrying UDS stand-in
provides:
  - "ProxiedPrincipal::send_recv(&mut self, BusMessage) -> Result<BusReply, GatewayError>"
  - "GatewayRegistry::get_mut(&mut self, &str) -> Option<&mut ProxiedPrincipal>"
  - "verify_inbound_any(bytes, &Keyring) -> Result<AnySignedEnvelope, RejectReason>"
  - "egress/ingress module stubs registered in lib.rs"
  - "famp-transport-http + relay dependency surface on famp-gateway/Cargo.toml"
affects: [09-02-egress, 09-03-ingress]

# Tech tracking
tech-stack:
  added: [famp-transport-http, famp-transport, famp-crypto, axum, tower, tower-http, url, time]
  patterns:
    - "Bus-client error mapping reused unchanged via map_bus_client_err for every new ProxiedPrincipal method"
    - "verify_inbound_any mirrors verify_inbound<B>'s exact two-gate order (unpinned hard-reject BEFORE decode), swapping SignedEnvelope::decode for AnySignedEnvelope::decode"

key-files:
  created:
    - crates/famp-gateway/tests/principal_send_drain.rs
    - crates/famp-gateway/src/egress.rs
    - crates/famp-gateway/src/ingress.rs
  modified:
    - crates/famp-gateway/src/principal.rs
    - crates/famp-gateway/src/registry.rs
    - crates/famp-gateway/src/verify.rs
    - crates/famp-gateway/src/lib.rs
    - crates/famp-gateway/src/main.rs
    - crates/famp-gateway/Cargo.toml

key-decisions:
  - "verify_inbound<B> kept unchanged alongside the new verify_inbound_any — both stay valid, single-class callers and existing unit tests are unaffected."
  - "egress.rs/ingress.rs are doc-comment-only stubs this plan (no items) so 09-02/09-03 can build in parallel on disjoint files."
  - "Test envelopes for both new integration/unit tests use the audit_log/standalone class where a minimal shape suffices, plus real request/commit/deliver bodies (two-key Bounds, terminal_status) for verify_inbound_any's per-class coverage."

patterns-established:
  - "Table-driven per-class test helpers (request_bytes/commit_bytes/deliver_bytes/signed_bytes) feeding a shared [(MessageClass, BytesFn); 4] array — reusable shape for any future per-class assertion in famp-gateway."

requirements-completed: [GW-01, GW-02]

coverage:
  - id: D1
    description: "ProxiedPrincipal::send_recv drains (Await) and sends (Send) on the backed principal's own UDS connection"
    requirement: "GW-01"
    verification:
      - kind: integration
        ref: "crates/famp-gateway/tests/principal_send_drain.rs#send_recv_round_trips_await_timeout_and_send"
        status: pass
    human_judgment: false
  - id: D2
    description: "GatewayRegistry::get_mut returns Some for a backed name, None for an unbacked name, and the returned reference is usable"
    requirement: "GW-01"
    verification:
      - kind: integration
        ref: "crates/famp-gateway/tests/principal_send_drain.rs#registry_get_mut_returns_backed_and_none_for_unbacked"
        status: pass
    human_judgment: false
  - id: D3
    description: "verify_inbound_any decodes all 4 live classes (request/commit/deliver/ack) and preserves the two-reason reject contract (InvalidSignature vs UnpinnedKey) with zero keyring mutation on any path"
    requirement: "GW-02"
    verification:
      - kind: unit
        ref: "crates/famp-gateway/src/verify.rs#tests::verify_inbound_any_accepts_pinned_valid_for_every_class"
        status: pass
      - kind: unit
        ref: "crates/famp-gateway/src/verify.rs#tests::verify_inbound_any_rejects_unsigned_for_every_class"
        status: pass
      - kind: unit
        ref: "crates/famp-gateway/src/verify.rs#tests::verify_inbound_any_rejects_unpinned_key_for_every_class"
        status: pass
    human_judgment: false
  - id: D4
    description: "famp-gateway compiles with the full relay dependency surface (famp-transport-http/axum/tower/tower-http/url/time) and empty egress/ingress module stubs registered"
    verification:
      - kind: integration
        ref: "cargo build -p famp-gateway --all-targets"
        status: pass
      - kind: other
        ref: "just lint (cargo clippy --workspace --all-targets -- -D warnings)"
        status: pass
    human_judgment: false

# Metrics
duration: ~25min
completed: 2026-07-27
status: complete
---

# Phase 9 Plan 1: Gateway Relay Plumbing Summary

**ProxiedPrincipal drain/send + class-dispatching envelope verification + relay dependency scaffold for Phase 9's egress/ingress split**

## Performance

- **Duration:** ~25 min
- **Completed:** 2026-07-27
- **Tasks:** 3
- **Files modified/created:** 9

## Accomplishments
- `ProxiedPrincipal::send_recv(&mut self, BusMessage) -> Result<BusReply, GatewayError>` — the gateway-backed UDS connection can now drain (`Await`/`Inbox`) and send (`Send`) on behalf of the principal it fronts, not just stay open for liveness. The `_client` field lost its dead-code underscore (renamed `client`) to allow the `&mut self` method.
- `GatewayRegistry::get_mut(&mut self, &str) -> Option<&mut ProxiedPrincipal>` mirrors the existing immutable `get`.
- `verify_inbound_any(bytes, &Keyring) -> Result<AnySignedEnvelope, RejectReason>` closes the multi-class ingress gap: it keeps `verify_inbound<B>`'s exact two-gate order (unpinned-key hard-reject before decode; `InvalidSignature` on any decode failure) but routes through `AnySignedEnvelope::decode` so the wire `class` field picks request/commit/deliver/ack/control/audit_log at runtime.
- `egress.rs`/`ingress.rs` module stubs registered in `lib.rs`, and the full relay dependency surface (`famp-transport-http`, `famp-transport`, `famp-crypto`, `axum`, `tower`, `tower-http`, `url`, `time`; `serde_json`/`uuid` promoted from dev to normal deps) added to `famp-gateway/Cargo.toml` — Wave 2 (09-02 egress, 09-03 ingress) can now build in parallel with zero shared-file contention.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add ProxiedPrincipal::send_recv and GatewayRegistry::get_mut** - `e34cd58` (feat)
2. **Task 2: Add class-dispatching verify_inbound_any** - `8fd8d6d` (feat)
3. **Task 3: Scaffold egress/ingress modules and add relay Cargo dependencies** - `b071c8e` (feat)

_No TDD RED/GREEN split — tasks were implemented directly with tests added in the same commit and verified green before committing, per the plan's `tdd="true"` intent for tasks 1-2 (behavior + implementation delivered together, no separate failing-test commit)._

## Files Created/Modified
- `crates/famp-gateway/src/principal.rs` - `_client` → `client` rename; new `send_recv` method
- `crates/famp-gateway/src/registry.rs` - new `get_mut` method
- `crates/famp-gateway/tests/principal_send_drain.rs` - new integration test: real broker subprocess, Await-timeout/Await-ok round-trip, get_mut Some/None
- `crates/famp-gateway/src/verify.rs` - new `verify_inbound_any`; extended unit test module with per-class (request/commit/deliver/ack) accept/reject-unsigned/reject-unpinned coverage
- `crates/famp-gateway/src/lib.rs` - re-export `verify_inbound_any`; register `egress`/`ingress` modules; new relay-dependency silencers
- `crates/famp-gateway/src/main.rs` - matching relay-dependency silencers
- `crates/famp-gateway/src/egress.rs` - new doc-comment-only module stub
- `crates/famp-gateway/src/ingress.rs` - new doc-comment-only module stub
- `crates/famp-gateway/Cargo.toml` - relay dependencies added; serde_json/uuid promoted to normal deps

## Decisions Made
- Kept `verify_inbound<B>` unchanged rather than replacing its signature — `verify_inbound_any` is purely additive, so existing single-class callers and Phase 8's original 4 unit tests needed zero changes.
- Used the `audit_log`/`standalone` class for the `principal_send_drain.rs` integration test envelopes (simplest fully-decodable shape that never fires the task FSM — matches CLAUDE.md's note that `audit_log` doesn't drive `famp-fsm`) rather than a partial `{"id","body"}` shape, after discovering the broker's `decode_line` gate (`AnyBusEnvelope::decode`) silently skips undecodable mailbox lines (head-of-line resilience) — an earlier draft using a minimal envelope produced `AwaitTimeout` instead of `AwaitOk` because the drained line was silently rejected as undecodable, not because `send_recv`/`Await` was broken.
- `verify_inbound_any`'s per-class unit tests build real `RequestBody`/`CommitBody`/`DeliverBody` payloads (two-key `Bounds` per v0.5.1 §9.3, `with_terminal_status(Completed)` for `DeliverBody`'s cross-field validation) rather than stub bodies, so the test proves the full `AnySignedEnvelope::decode` path for each class, not just that `class` dispatch fires.

## Deviations from Plan

None - plan executed exactly as written. The head-of-line-resilience discovery above was a test-authoring correction (Rule 1 auto-fix, caught before commit — no wrong behavior shipped), not a deviation from the plan's scope.

## Issues Encountered
- Initial `principal_send_drain.rs` draft used a minimal `{"id","body"}` JSON envelope for the send/await round-trip; the broker's per-line drain gate (`famp-bus::broker::handle::decode_line` → `AnyBusEnvelope::decode`) requires a fully-shaped `WireEnvelope` (famp/class/scope/id/from/to/authority/ts/body) and silently skips anything less as an undecodable line (head-of-line resilience, intentional broker behavior). Fixed by building a real `audit_log`/`standalone` envelope via a small test helper; the round-trip then verified as expected.

## Next Phase Readiness
- 09-02 (egress) and 09-03 (ingress) can now build in parallel: both have `send_recv`, `get_mut`, `verify_inbound_any`, the relay dependency surface, and empty module files to fill, with zero shared-file contention against each other or against this plan's changes.
- No blockers identified.

---
*Phase: 09-end-to-end-cross-host-delivery*
*Completed: 2026-07-27*

## Self-Check: PASSED

All 9 created/modified source files and the 3 task commit hashes (e34cd58, 8fd8d6d, b071c8e) verified present on disk / in git log.

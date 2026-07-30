---
phase: 09-end-to-end-cross-host-delivery
plan: 02
subsystem: infra
tags: [famp-gateway, ed25519, famp-transport-http, uds-bus, egress, federation-signing]

# Dependency graph
requires:
  - phase: 09-end-to-end-cross-host-delivery
    plan: 01
    provides: "ProxiedPrincipal::send_recv, GatewayRegistry::get_mut, verify_inbound_any, egress.rs module stub, famp-transport-http dependency"
provides:
  - "egress::sign_federation_fields(&mut Value, &Principal, &Principal, &FampSigningKey, &TrustedVerifyingKey) -> Result<(), EgressError>"
  - "egress::run_egress(name: String, registry: Arc<Mutex<GatewayRegistry>>, transport: Arc<HttpTransport>, sk: FampSigningKey, vk: TrustedVerifyingKey)"
affects: [09-03-ingress, 09-04-main-wiring, 09-05-e2e]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Value-mutation signing (not typed UnsignedEnvelope reconstruction) — sign_value accepts &serde_json::Value directly, avoiding new famp-envelope public accessors (09-RESEARCH Pitfall 3)"
    - "Short-timeout Arc<Mutex<GatewayRegistry>> reacquire-per-iteration loop: lock -> get_mut -> short Await -> drop(guard) explicitly -> sign/POST outside the lock (clippy::significant_drop_tightening enforced this explicitly, not just implicitly via scope-end)"

key-files:
  created: []
  modified:
    - crates/famp-gateway/src/egress.rs

key-decisions:
  - "Nonce generated via uuid::Uuid::now_v7() rather than the RESEARCH-suggested new_v4() — the workspace uuid dependency only enables the 'v7' cargo feature (no crate anywhere uses v4), and now_v7() satisfies the nonce contract (non-empty, unique per call) without adding a new feature flag to Cargo.toml."
  - "sign_federation_fields checks for an existing 'signature' key and returns Ok(()) immediately (idempotent no-op) rather than per-field entry-or-insert-only — simpler and matches D-03's 'if not already cross-host-signed' framing exactly (a signed value should never be touched again, not partially re-populated)."
  - "MutexGuard is dropped via explicit drop(guard) before the match on the Await reply, not just implicit end-of-block — clippy::significant_drop_tightening (nursery, promoted to a hard error under `just lint`'s -D warnings) required this refactor over the plan's simpler nested-match sketch."

patterns-established:
  - "explicit drop(guard) before any post-lock processing when a MutexGuard's last real use precedes a match/branch that doesn't need it — clippy::significant_drop_tightening will hard-fail `just lint` on a merely-implicit end-of-scope drop."

requirements-completed: [GW-01, GW-02]

coverage:
  - id: D1
    description: "sign_federation_fields signs a plain drained Value in place, preserving id/from/to/class/body byte-identical, and the result verifies via verify_inbound_any"
    requirement: "GW-01"
    verification:
      - kind: unit
        ref: "crates/famp-gateway/src/egress.rs#tests::sign_federation_fields_round_trips_and_preserves_content"
        status: pass
    human_judgment: false
  - id: D2
    description: "sign_federation_fields is idempotent-safe on an already-signed value (no double-sign/re-add)"
    requirement: "GW-01"
    verification:
      - kind: unit
        ref: "crates/famp-gateway/src/egress.rs#tests::sign_federation_fields_is_idempotent"
        status: pass
    human_judgment: false
  - id: D3
    description: "A single-byte body tamper on already-signed federation bytes is rejected by verify_inbound_any"
    requirement: "GW-01"
    verification:
      - kind: unit
        ref: "crates/famp-gateway/src/egress.rs#tests::sign_federation_fields_round_trips_and_preserves_content (tamper assertion)"
        status: pass
    human_judgment: false
  - id: D4
    description: "run_egress compiles as pub, takes the shared Arc<Mutex<GatewayRegistry>> (not a standalone &mut ProxiedPrincipal), releases the lock between poll iterations with a short (~1s) Await, and signs/POSTs outside the held lock — the GW-02 shared-connection contract"
    verification:
      - kind: integration
        ref: "cargo build -p famp-gateway --all-targets"
        status: pass
      - kind: other
        ref: "just lint (cargo clippy --workspace --all-targets -- -D warnings)"
        status: pass
    human_judgment: false

# Metrics
duration: ~20min
completed: 2026-07-27
status: complete
---

# Phase 9 Plan 2: Gateway Egress Drain-Sign-POST Loop Summary

**`sign_federation_fields` Value-mutation sign site + `run_egress` shared-lock Await/sign/POST drain loop, closing the GW-01/GW-02 outbound half of cross-host relay**

## Performance

- **Duration:** ~20 min
- **Completed:** 2026-07-27
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- `egress::sign_federation_fields(&mut serde_json::Value, from, to, sk, vk)` — mutates a plain drained local-bus mailbox `Value` in place, inserting `from_domain`/`to_domain`/`sender_key_id`/`nonce`/`expiry` and a `famp::sign_value` signature, while leaving `id`/`from`/`to`/`class`/`body` byte-identical (content-transparency). Idempotent-safe: a value that already carries `signature` is untouched on a second call.
- `egress::run_egress(name, registry: Arc<Mutex<GatewayRegistry>>, transport: Arc<HttpTransport>, sk, vk)` — the drain-sign-POST loop. Each iteration acquires the shared registry lock, calls `get_mut(&name)`, issues a short (1s) `Await`, then explicitly drops the lock BEFORE signing and POSTing — so 09-03's ingress `Send` on the same backed principal's connection is never starved longer than one ~1s poll interval (the load-bearing GW-02 shared-connection contract from the plan's must-haves).
- Per-envelope errors (missing/malformed `from`/`to`, sign failure, transport failure) are logged and the loop continues — a single bad envelope or a down peer never kills the drain task (availability, not trust).

## Task Commits

Both tasks landed in the same file/commit — Task 2's `run_egress` directly consumes Task 1's `sign_federation_fields`, and neither `cargo test` nor `just lint` pass against a partial file, so an artificial intermediate commit would not be independently green.

1. **Task 1 + Task 2: sign_federation_fields + run_egress** - `eeb5aac` (feat)

## Files Created/Modified
- `crates/famp-gateway/src/egress.rs` - `sign_federation_fields` (Value-mutation sign site) + `run_egress` (shared-lock drain-sign-POST loop) + `EgressError`/`RelayError` + unit tests

## Decisions Made
- Used `uuid::Uuid::now_v7()` instead of the RESEARCH-suggested `new_v4()` for the nonce — the workspace `uuid` dependency only enables the `v7` feature (confirmed via grep: no crate in the workspace uses `v4`), and `now_v7()` satisfies the nonce contract (non-empty, unique) without a Cargo.toml feature change. Low-risk per RESEARCH Assumption A2 ("if an existing nonce helper is found... use it instead").
- `sign_federation_fields`'s idempotent guard checks for an existing `signature` key and returns early rather than doing a per-field `entry()`-or-insert against a partially-populated value — simpler, and matches D-03's framing ("if not already cross-host-signed") that a signed envelope is a single, complete unit, never partially re-touched.
- Explicit `drop(guard)` inserted before matching on the `Await` reply, not just relying on end-of-block drop — `clippy::significant_drop_tightening` (a nursery lint `just lint`'s `-D warnings` promotes to a hard error) flagged the implicit-drop version as a resource-contention risk even though the lock was already released before signing/POSTing began.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Swapped `uuid::Uuid::new_v4()` for `uuid::Uuid::now_v7()`**
- **Found during:** Task 1 (sign_federation_fields implementation)
- **Issue:** The plan's action text and 09-RESEARCH/09-PATTERNS both specify `uuid::Uuid::new_v4()` for the nonce, but the workspace `uuid` dependency (root `Cargo.toml`) only enables `features = ["v7", "serde"]` — `new_v4()` requires the `v4` feature, which is not enabled anywhere in the workspace.
- **Fix:** Used `uuid::Uuid::now_v7()` instead, which is already available (same pattern `crates/famp/src/cli/send/mod.rs` uses for message IDs) and satisfies the nonce contract without any Cargo.toml change.
- **Files modified:** crates/famp-gateway/src/egress.rs
- **Verification:** `cargo build -p famp-gateway --all-targets` and `cargo test -p famp-gateway --lib egress` both pass; nonce field is present and non-empty in the signed output, asserted indirectly via the successful `verify_inbound_any` round-trip.
- **Committed in:** eeb5aac (single task commit)

**2. [Rule 1 - Bug] Explicit `drop(guard)` before the post-Await match**
- **Found during:** Task 2 (`run_egress` implementation), caught by `just lint`
- **Issue:** The initial implementation returned the drained `envelopes` Vec directly from inside the lock-holding block (matching on `reply` while `guard` was still technically in scope until the closing brace). `clippy::significant_drop_tightening` (nursery, promoted to error by `just lint`'s `-D warnings`) flagged this as an unnecessarily-long-held `MutexGuard`.
- **Fix:** Restructured to capture `reply` inside the lock-holding block, call `drop(guard)` explicitly immediately after the `Await` call returns, then match on `reply` outside the block. Behavior is unchanged — the lock was already logically released before any signing/POSTing — but the drop point is now explicit rather than implicit.
- **Files modified:** crates/famp-gateway/src/egress.rs
- **Verification:** `just lint` (cargo clippy --workspace --all-targets -- -D warnings) passes clean.
- **Committed in:** eeb5aac (single task commit)

---

**Total deviations:** 2 auto-fixed (1 blocking dependency-feature gap, 1 lint-driven correctness/hygiene fix)
**Impact on plan:** Both auto-fixes were necessary for the code to compile/lint clean under this repo's `just lint` gate; neither changes the plan's must-have behavior (shared-lock contract, content-transparent signing).

## Issues Encountered
None beyond the two auto-fixed items above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- 09-03 (ingress) can proceed independently — it touches only `crates/famp-gateway/src/ingress.rs`, zero shared-file contention with this plan's `egress.rs` change.
- 09-04 (main.rs wiring) can now spawn `run_egress` per backed principal — the function signature (`name`, shared `Arc<Mutex<GatewayRegistry>>`, `Arc<HttpTransport>`, `sk`, `vk`) is stable and matches the plan's declared artifact shape exactly.
- No blockers identified. `run_egress`'s infinite loop with a short (~1s) `Await` timeout is naturally rate-limited by the broker's own blocking-with-timeout semantics — no additional sleep/backoff needed.

---
*Phase: 09-end-to-end-cross-host-delivery*
*Completed: 2026-07-27*

## Self-Check: PASSED

`crates/famp-gateway/src/egress.rs` confirmed present with both `sign_federation_fields` and `run_egress`; commit `eeb5aac` confirmed in `git log`.

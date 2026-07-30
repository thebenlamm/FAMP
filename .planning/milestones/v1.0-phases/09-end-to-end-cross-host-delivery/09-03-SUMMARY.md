---
phase: 09-end-to-end-cross-host-delivery
plan: 03
subsystem: infra
tags: [axum, rustls, gateway, ingress, ed25519, famp-gateway]

# Dependency graph
requires:
  - phase: 09-end-to-end-cross-host-delivery
    provides: "09-01 verify_inbound_any, GatewayRegistry::get_mut, ProxiedPrincipal::send_recv; 09-02 shared Arc<Mutex<GatewayRegistry>> short-hold connection contract"
provides:
  - "crates/famp-gateway/src/ingress.rs::build_gateway_router — gateway-owned axum router (INBOX_ROUTE + RequestBodyLimitLayer, no FampSigVerifyLayer)"
  - "crates/famp-gateway/src/ingress.rs::inbox_handler — verify_inbound_any-first handler delivering via the backed sender stand-in"
  - "crates/famp-gateway/src/ingress.rs::run_ingress — rustls TLS server task serving build_gateway_router"
affects: ["09-04 (main.rs wiring of run_ingress + egress into tokio::select!)", "09-05 (two-process E2E over loopback HTTPS)"]

# Tech tracking
tech-stack:
  added: []
  patterns: ["Gateway-owned trust boundary: verify_inbound_any inside the handler is the sole signature check; famp_transport_http::build_router/FampSigVerifyLayer are never mounted on the gateway path (D-04)."]

key-files:
  created: []
  modified:
    - crates/famp-gateway/src/ingress.rs

key-decisions:
  - "run_ingress binds a std::net::TcpListener, sets it non-blocking, then hands it to famp_transport_http::tls_server::serve_std_listener — mirroring the deferred v1 e2e fixture pattern (bind first, so a future caller can read local_addr() before spawning)."
  - "run_ingress does not name rustls/axum-server types directly (type inference through famp_transport_http::tls::build_server_config's return value), so no new dependency was added to famp-gateway/Cargo.toml."
  - "JoinHandle<io::Result<()>> from serve_std_listener is awaited and flattened inline (JoinError -> io::Error::other) so run_ingress itself is a single long-running future, ready to be raced in main.rs's tokio::select! (wiring deferred to 09-04)."

patterns-established:
  - "TLS terminates the transport channel only; it is never a peer-authorization boundary (D-08) — verify_inbound_any inside inbox_handler remains the sole trust decision regardless of which listener (run_ingress here, or a future test harness) serves the router."

requirements-completed: [GW-01, GW-02]

coverage:
  - id: D1
    description: "Gateway-owned axum router (build_gateway_router) with body-limit but no FampSigVerifyLayer; inbox_handler verifies via verify_inbound_any first, delivers via the backed sender stand-in, and rejects InvalidSignature/UnpinnedKey as two distinct 4xx codes with zero bus writes."
    requirement: "GW-01"
    verification:
      - kind: unit
        ref: "crates/famp-gateway/src/ingress.rs#tests::invalid_signature_and_unpinned_key_map_to_distinct_4xx_with_no_registry_mutation"
        status: pass
      - kind: unit
        ref: "crates/famp-gateway/src/ingress.rs#tests::bad_principal_in_path_is_rejected_before_verification"
        status: pass
    human_judgment: false
  - id: D2
    description: "run_ingress: rustls TLS server task binding listen_addr and serving build_gateway_router over HTTPS, with TLS as channel encryption only (no cert-based peer authorization)."
    requirement: "GW-02"
    verification:
      - kind: other
        ref: "cargo build -p famp-gateway --all-targets && just lint"
        status: pass
    human_judgment: true
    rationale: "Live TLS serve behavior (accept loop, handshake, real bytes over the wire) is only exercised by the 09-05 two-process E2E over loopback HTTPS with the cross_machine fixture certs; this plan proves it compiles and type-checks against the tls/tls_server helpers, not that it serves live traffic."

# Metrics
duration: 56min
completed: 2026-07-27
status: complete
---

# Phase 9 Plan 3: Gateway Ingress Router + rustls TLS Server Summary

**Gateway-owned axum router with verify_inbound_any as the sole trust decision, plus a rustls TLS server task (`run_ingress`) that serves it — completing the inbound half of cross-host relay (GW-01/GW-02).**

## Performance

- **Duration:** 56 min (11:12–12:08 local, across the interrupted + resumed run)
- **Started:** 2026-07-27T11:12:41-04:00 (Task 1 commit)
- **Completed:** 2026-07-27T12:08:50-04:00 (Task 2 commit)
- **Tasks:** 2 completed (Task 1 in a prior interrupted run, Task 2 in this resumed run)
- **Files modified:** 1 (`crates/famp-gateway/src/ingress.rs`, across both task commits)

## Accomplishments
- `build_gateway_router` (Task 1, prior run): gateway-owned router reusing only `INBOX_ROUTE` and a `RequestBodyLimitLayer`, never mounting `famp_transport_http::build_router`/`FampSigVerifyLayer` — closing the second-trust-source risk D-04 exists to prevent.
- `inbox_handler` (Task 1, prior run): calls `verify_inbound_any` first; `InvalidSignature` -> 400, `UnpinnedKey` -> 403 (two distinct 4xx per D-08), zero registry mutation on either reject path; on success delivers via the backed sender stand-in's `send_recv`, holding the shared `Arc<Mutex<GatewayRegistry>>` lock only for that single call.
- `run_ingress` (Task 2, this run): binds a non-blocking `std::net::TcpListener`, loads the server rustls `ServerConfig` via `famp_transport_http::tls::{load_pem_cert, load_pem_key, build_server_config}`, and serves `build_gateway_router` via `famp_transport_http::tls_server::serve_std_listener`. TLS is documented as channel encryption only — no cert-based peer authorization was added, keeping `verify_inbound_any` the sole trust boundary.

## Task Commits

Each task was committed atomically:

1. **Task 1: Gateway-owned router + verify-then-deliver handler** - `53270dd` (feat) — completed and committed in a prior, interrupted executor run; NOT touched or re-committed in this run.
2. **Task 2: rustls TLS server task run_ingress** - `718552e` (feat) — this run.

**Plan metadata:** (this SUMMARY commit) — `.planning/` is gitignored in this project; the metadata commit is expected to skip per `commit_docs`/gitignore rules and is not a failure.

## Files Created/Modified
- `crates/famp-gateway/src/ingress.rs` - Added `pub async fn run_ingress(...)`, the rustls TLS server task that binds `listen_addr`, loads the cert/key via `famp_transport_http::tls`, and serves `build_gateway_router` via `famp_transport_http::tls_server::serve_std_listener`. (Task 1's router + handler + tests were already present from `53270dd` and are unmodified by this run except for `cargo fmt --all` reformatting one line as part of the pre-commit hook.)

## Decisions Made
- **No new Cargo dependency needed for TLS types.** `run_ingress` never spells `rustls::ServerConfig`/`CertificateDer`/etc. by name — those types flow through `famp_transport_http::tls`'s function signatures via type inference — so `famp-gateway/Cargo.toml` was left unchanged (still no direct `rustls`/`axum-server` dependency).
- **Listener binding mirrors the deferred v1 e2e fixture pattern** (`crates/famp/tests/_deferred_v1/e2e_two_daemons.rs.deferred`): bind the `std::net::TcpListener` first, `set_nonblocking(true)`, then hand it to `serve_std_listener` — so a future caller (09-04's main.rs, or an ephemeral-port test) can read `local_addr()` before the server task starts accepting.
- **`run_ingress` awaits and flattens the `JoinHandle`** returned by `serve_std_listener` (mapping `JoinError` to `io::Error::other`) so the function itself is a single `Future<Output = io::Result<()>>` — matching the plan's "long-running task main.rs can `tokio::select!` on" requirement without main.rs wiring in this plan (deferred to 09-04, whose scope owns that wiring).

## Deviations from Plan

None - plan executed exactly as written. Task 2's signature (`listen_addr, tls_cert_path, tls_key_path, registry, keyring -> io::Result<()>`) matches the plan's stated "final signature the executor may adjust for main.rs" latitude; no adjustment was needed beyond settling on `std::io::Result<()>` as the concrete return type.

## Issues Encountered
- Initial attempt imported `std::path::Path` at module scope, which collided with `axum::extract::Path` already imported by Task 1 (ambiguous `Path` in `inbox_handler`'s existing signature, producing 7 compile errors: E0107/E0252/E0277/E0308). Fixed by using the fully-qualified `&std::path::Path` in `run_ingress`'s parameter types instead of adding a conflicting top-level `use`. Verified via `cargo build -p famp-gateway --all-targets` (clean) immediately after.
- `cargo fmt --all` (invoked automatically by the pre-commit hook) reformatted one `let cert = ...` binding onto a single line; re-staged and committed as-is per the hook's own fix instructions.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- 09-04 can now wire `run_ingress` (this plan) alongside the 09-02 egress drain loop into `main.rs`'s `tokio::select!` and CLI arg plumbing for `listen_addr`/cert/key paths.
- 09-05's two-process E2E is unblocked to exercise `run_ingress` for real over loopback HTTPS with the `cross_machine` fixture certs — this plan proves `run_ingress` builds and type-checks against the `tls`/`tls_server` helpers, not that it serves live traffic (see coverage D2 rationale).
- `cargo test -p famp-gateway --lib` (11 tests) and `just lint` (workspace clippy `-D warnings`) both green as of `718552e`.

---
*Phase: 09-end-to-end-cross-host-delivery*
*Completed: 2026-07-27*

## Self-Check: PASSED
- FOUND: crates/famp-gateway/src/ingress.rs
- FOUND: .planning/phases/09-end-to-end-cross-host-delivery/09-03-SUMMARY.md
- FOUND commit: 53270dd
- FOUND commit: 718552e

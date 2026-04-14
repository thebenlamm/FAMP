---
phase: 04
plan: 02
subsystem: famp-transport-http
tags: [http, transport, middleware, sig-verify, body-limit, sentinel-tests]
provides:
  - FampSigVerifyLayer (tower Layer + Service) with two-phase decode + canonical pre-check
  - build_router(keyring, inboxes) -> axum::Router mounting POST /famp/v0.5.1/inbox/{principal}
  - inbox_handler reading Extension<Arc<AnySignedEnvelope>> stashed by middleware
  - InboxRegistry / ServerState / INBOX_ROUTE public surface
  - 4 sentinel integration tests proving TRANS-09 SC#2 (handler-not-entered)
requires:
  - famp_envelope::peek_sender (Plan 04-01)
  - famp_envelope::AnySignedEnvelope::decode
  - famp_canonical::{from_slice_strict, canonicalize}
  - famp_keyring::Keyring::get
  - MiddlewareError -> IntoResponse status mapping (Plan 04-01)
affects:
  - crates/famp-transport-http/src/lib.rs (new modules + silencer cleanup)
tech-stack:
  added:
    - tower::{Layer, Service, ServiceBuilder} (now live consumer)
    - tower_http::limit::RequestBodyLimitLayer (now live consumer)
    - axum::extract::Extension for cross-layer envelope hand-off
  patterns:
    - "Two-phase decode (peek_sender -> keyring.get -> decode_with_pinned_key) mirrors loop_fn.rs byte-for-byte"
    - "Canonical pre-check before decode keeps CONF-06 vs CONF-07 distinguishable at the HTTP layer"
    - "Arc<AnySignedEnvelope> stashed via Request::extensions_mut is the cross-layer hand-off (D-C3); handler reads it via axum::extract::Extension"
    - "ServiceBuilder.map_request(req.map(Body::new)) re-unifies the body type after RequestBodyLimitLayer wraps it in Limited<Body>, letting FampSigVerifyService keep its Service<Request<Body>> bound"
    - "Sentinel-based handler-not-entered tests via Arc<AtomicBool> in a custom test router that mirrors the production layer stack"
key-files:
  created:
    - crates/famp-transport-http/src/middleware.rs
    - crates/famp-transport-http/src/server.rs
    - crates/famp-transport-http/tests/middleware_layering.rs
  modified:
    - crates/famp-transport-http/src/lib.rs
    - crates/famp-transport-http/Cargo.toml
key-decisions:
  - "Hand-written Layer/Service rather than axum::middleware::from_fn_with_state — needed direct access to Request::extensions_mut for the Arc<AnySignedEnvelope> stash without dragging in the axum extractor surface."
  - "Inserted ServiceBuilder.map_request(req.map(Body::new)) between RequestBodyLimitLayer (outer) and FampSigVerifyLayer (inner) to avoid making FampSigVerifyService generic over body type. The 1 MiB cap still runs first; the inner middleware sees Body and runs to_bytes(_, 1 MiB) as a belt-and-braces second cap."
  - "envelope_sender 5-arm match inlined in server.rs (NOT imported from crates/famp::runtime::adapter) — famp-transport-http cannot depend on the top crate. Comment notes the duplication for future maintainers."
  - "The inbox_handler populates TransportMessage.sender from envelope_sender(&envelope).clone() — never recipient.clone(). The B-7 bug is killed at the source level; an acceptance grep enforces this."
metrics:
  duration_min: 28
  tasks: 3
  files_created: 3
  files_modified: 2
  completed: 2026-04-14
---

# Phase 4 Plan 02: build_router + FampSigVerifyLayer + Sentinel Layering Tests

Wave 2 of Phase 4 builds the full server-side stack of `famp-transport-http`: a
hand-written `FampSigVerifyLayer` that runs the same two-phase decode as the
runtime's `loop_fn.rs` (peek sender → look up pinned key → canonical pre-check →
`AnySignedEnvelope::decode` → stash on request extensions), wired into a
single-route `build_router` behind a 1 MiB `RequestBodyLimitLayer`. Four
integration tests prove the middleware rejects 4 adversarial cases BEFORE the
handler closure is invoked, satisfying TRANS-09 SC#2.

## Tasks Completed

| # | Task | Files | Commit |
|---|------|-------|--------|
| 1 | FampSigVerifyLayer middleware (two-phase decode + canonical pre-check + envelope stash) | crates/famp-transport-http/src/{middleware.rs,lib.rs} | `81070c5` |
| 2 | build_router + inbox_handler reading stashed envelope | crates/famp-transport-http/src/{server.rs,lib.rs} | `82bb0a6` |
| 3 | 4 sentinel integration tests (TRANS-09 SC#2) | crates/famp-transport-http/tests/middleware_layering.rs, Cargo.toml | `ea78764` |

## Verification Results

- `cargo check -p famp-transport-http` — green
- `cargo clippy -p famp-transport-http --all-targets -- -D warnings` — clean (0 warnings)
- `cargo nextest run -p famp-transport-http` — 5 / 5 passing (1 status-mapping unit + 4 sentinel layering)
- `cargo nextest run --workspace` — 233 / 233 passing (zero regressions)
- `cargo tree -i openssl --workspace` — no path
- `cargo tree -i native-tls --workspace` — no path

## Acceptance Criteria

### Task 1 (middleware.rs)
- Contains `peek_sender`, `keyring.get(&sender)`, `canonicalize(&parsed)`, `CanonicalDivergence`, `AnySignedEnvelope::decode(&bytes, &pinned)`, `extensions_mut().insert`, `Request::from_parts`
- Does NOT contain `keyring.iter` / `for (_, key)` (Pitfall 3 — no key iteration)

### Task 2 (server.rs)
- Contains `"/famp/v0.5.1/inbox/{principal}"` (axum 0.8 brace syntax)
- Contains `RequestBodyLimitLayer::new(ONE_MIB)` and `FampSigVerifyLayer::new`
- `RequestBodyLimitLayer` appears BEFORE `FampSigVerifyLayer` in the `ServiceBuilder` chain (Pitfall 2 outer→inner)
- Contains `Extension(envelope): Extension<Arc<AnySignedEnvelope>>` and `envelope_sender(&envelope)`
- Does NOT contain `sender: recipient.clone()` (B-7 fix)

### Task 3 (middleware_layering.rs)
- 4 `#[tokio::test]` functions named exactly per plan
- `unsigned_request_does_not_enter_handler` uses **alice-pinned** keyring → asserts `BAD_REQUEST` (W-1 fix: distinct from unknown-sender path)
- `body_over_1mb_does_not_enter_handler` posts `vec![b'x'; ONE_MIB + 1]` → asserts `PAYLOAD_TOO_LARGE`
- `unknown_sender_does_not_enter_handler` uses **empty** keyring → asserts `UNAUTHORIZED`
- `wrong_key_does_not_enter_handler` uses alice-pinned + `WRONG_SECRET` → asserts `UNAUTHORIZED`
- Every test asserts `!sentinel.load(Ordering::SeqCst)`

## Deviations from Plan

### [Rule 3 — Blocking issue] Body type mismatch between RequestBodyLimitLayer and FampSigVerifyLayer

- **Found during:** Task 2 cargo check
- **Issue:** `tower_http::limit::RequestBodyLimitLayer` wraps the body in `Limited<Body>`, but `FampSigVerifyService` is bound on `Service<Request<axum::body::Body>>`. The inner layer cannot accept the outer layer's wrapped body type, breaking compilation:
  > the trait `Service<Request<Limited<Body>>>` is not implemented for `FampSigVerifyService<Route>`
- **Fix:** Inserted a `ServiceBuilder.map_request(|req: Request<_>| req.map(Body::new))` shim between the two layers. The 1 MiB cap still runs first (the wrapped body's data stream errors at the limit); the shim re-unifies the body type to `axum::body::Body` so the next layer's bound is satisfied. The middleware's own belt-and-braces `to_bytes(body, ONE_MIB)` provides the second cap.
- **Files modified:** `crates/famp-transport-http/src/server.rs` (and mirrored in `tests/middleware_layering.rs` so the test router matches the production stack byte-for-byte)
- **Commit:** `82bb0a6`
- **Rejected alternative:** Make `FampSigVerifyService` generic over body type (`B: HttpBody<Data = Bytes> + Send + 'static`). Compiles, but adds 4–5 trait bounds, makes the public Layer signature noisier, and obscures the call site for very little benefit — the `map_request` shim is one line and entirely local to the router.

### [Rule 1 — Bug] Plan-supplied middleware match arms tripped clippy::match_same_arms

- **Found during:** Task 1 clippy
- **Issue:** Plan provided three identical `Err(_) => return ...BadEnvelope` arms after specific variant matches. Clippy `match_same_arms` (pedantic) refused to compile.
- **Fix:** Collapsed `peek_sender` error handling into a single `let Ok(sender) = ... else` (all errors map to `BadEnvelope` anyway) and merged `SignatureInvalid | InvalidSignatureEncoding` into one arm. Behavior unchanged from the plan's intent; D-C6 status mapping verified by the existing `middleware_error_status_mapping_is_load_bearing` unit test.
- **Files modified:** `crates/famp-transport-http/src/middleware.rs`
- **Commit:** `81070c5`

### [Rule 1 — Bug] Plan dev-deps included unused `hyper` dependency

- **Found during:** Task 3 clippy `--all-targets`
- **Issue:** Plan listed `hyper = { version = "1", features = ["client"] }` as a dev-dep, but `tower::ServiceExt::oneshot` directly drives the in-process router and `hyper` is never imported. `unused_crate_dependencies` flagged it.
- **Fix:** Removed `hyper` from `[dev-dependencies]`. Tests use `app.oneshot(req).await` against `axum::Router` directly — no hyper client needed for in-process testing.
- **Files modified:** `crates/famp-transport-http/Cargo.toml`
- **Commit:** `ea78764`

### Auto-fixed silencer churn in lib.rs

- **Found during:** Tasks 1 and 2 cargo check
- **Issue:** Plan 04-01 silenced `tower`, `tower_http`, `famp_canonical`, `famp_envelope`, `famp_keyring`, `futures_util`, `famp_transport`, `tokio` with `use _ as _;` lines. As Tasks 1 (middleware) and 2 (server) wired each one, the silencer became a hard error (`unused_imports`).
- **Fix:** Removed silencers progressively per task: middleware lit up `tower`/`futures_util`/`famp_canonical`/`famp_envelope`/`famp_keyring`; server lit up `tower_http`/`famp_transport`/`tokio`. Final remaining silencers: `rustls_platform_verifier`, `rustls_pemfile`, `rustls`, `axum_server`, `famp_crypto`, `serde_json` — all consumed by Plan 04-03 (TLS + HttpTransport).
- **Files modified:** `crates/famp-transport-http/src/lib.rs`
- **Commits:** `81070c5`, `82bb0a6`

### Auto-removed redundant `#[must_use]` on build_router

- **Found during:** Task 2 clippy
- **Issue:** `Router` is already `#[must_use]`; `clippy::double_must_use` rejected the duplicate annotation on `pub fn build_router`.
- **Fix:** Dropped the redundant attribute.
- **Files modified:** `crates/famp-transport-http/src/server.rs`
- **Commit:** `82bb0a6`

## Notes for Downstream Plans

- The `map_request(req.map(Body::new))` shim must be replicated in any future place where `RequestBodyLimitLayer` sits outside `FampSigVerifyLayer`. The Plan 04-03 `serve()` helper that wraps `axum_server::bind_rustls` does NOT need this — it consumes the already-built `Router`, and the layer composition lives inside `build_router`.
- `inbox_handler` reads the body as `axum::body::Bytes`. Since the middleware re-attached the body via `Body::from(bytes)` after consuming it for verification, the second `Bytes` extraction in the handler is cheap (it re-collects from the `Body::from(Bytes)` branch). Watch this if Plan 04-04's example shows allocator pressure under load — it would be an easy follow-up to stash the bytes in extensions alongside the envelope.
- `envelope_sender` is duplicated in `crates/famp/src/runtime/adapter.rs` and `crates/famp-transport-http/src/server.rs`. Both must change in lockstep if a new `AnySignedEnvelope` variant is ever added. There is no compile-time enforcement of this — the 5-arm `match` in each crate is exhaustive but independent.
- The 4 sentinel tests do NOT exercise CONF-07 (canonical divergence) — the plan deferred that to the Plan 04-05 adversarial matrix where it lives alongside the same case for `MemoryTransport`.

## Self-Check: PASSED

- crates/famp-transport-http/src/middleware.rs — FOUND
- crates/famp-transport-http/src/server.rs — FOUND
- crates/famp-transport-http/tests/middleware_layering.rs — FOUND
- crates/famp-transport-http/src/lib.rs — FOUND (modified)
- crates/famp-transport-http/Cargo.toml — FOUND (modified)
- Commit 81070c5 — FOUND
- Commit 82bb0a6 — FOUND
- Commit ea78764 — FOUND

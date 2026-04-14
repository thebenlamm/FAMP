---
phase: 04-minimal-http-transport-cross-machine-example
fixed_at: 2026-04-13T00:00:00Z
review_path: .planning/phases/04-minimal-http-transport-cross-machine-example/04-REVIEW.md
iteration: 1
findings_in_scope: 7
fixed: 7
skipped: 0
status: all_fixed
---

# Phase 4: Code Review Fix Report

**Source review:** `04-REVIEW.md`
**Iteration:** 1
**Scope:** Medium + Low (Info findings INF-01/02/03 deferred per workflow default)

**Summary:**
- Findings in scope: 7
- Fixed: 7
- Skipped: 0

## Fixed Issues

### MED-01: `tls.rs::load_pem_cert` silently returns an empty Vec for garbage PEM input

**Files modified:** `crates/famp-transport-http/src/tls.rs`
**Commit:** `ba23f9d`
**Applied fix:** Added `TlsError::NoCertificatesInPem(PathBuf)` variant and return it from `load_pem_cert` when the parsed cert vec is empty, so a typo'd `--trust-cert` path fails loudly rather than silently degrading `build_client_config` to OS-roots-only. Updated the `load_pem_cert_rejects_garbage` test to assert the new typed error.

### MED-02: `middleware.rs` canonical pre-check parity invariant

**Files modified:** `crates/famp-transport-http/src/middleware.rs`
**Commits:** `625aaa9`, `8640a43` (clippy doc-markdown follow-up)
**Applied fix:** Added an `INVARIANT (MED-02)` comment on the `Value`-path call site stating the byte-identity contract with `AnySignedEnvelope::decode`, then pinned the contract with six focused unit tests in a new `canonical_pre_check_tests` module: ASCII round-trip, non-ASCII UTF-8 pass-through (RFC 8785 §3.2.2), duplicate-key rejection, whitespace divergence, integer number formatting, and key sorting. A future serde-layer refactor that desyncs the two paths will fail at least one test before shipping. **Requires human verification** that the chosen edge cases fully cover CONF-07 distinguishability.

### MED-03: `HttpTransport::send` hand-rolls URL construction with manual percent-encoding

**Files modified:** `crates/famp-transport-http/src/transport.rs`
**Commit:** `77f0f6a`
**Applied fix:** Replaced `str::replace(':', "%3A").replace('/', "%2F")` with `Url::path_segments_mut().push(...)`, which applies RFC 3986 path-segment encoding and therefore tolerates any future Principal-grammar widening without editing this file.

### LOW-01: `http_happy_path.rs` uses 300ms sleep as settle — CI-flaky

**Files modified:** `crates/famp/tests/http_happy_path.rs`
**Commit:** `d596868`
**Applied fix:** Added a `wait_for_tcp` helper that connect-probes each bound `SocketAddr` with exponential backoff up to a 2 s hard ceiling. Replaced the fixed 300 ms sleep with two `wait_for_tcp` calls against Alice and Bob. Test proceeds as soon as both rustls listeners are accepting.

### LOW-02: `transport.rs::recv` holds outer `Mutex<HashMap>` across await

**Files modified:** `crates/famp-transport-http/src/transport.rs`
**Commit:** `152dd05`
**Applied fix:** Changed `receivers: Mutex<HashMap<Principal, mpsc::Receiver<...>>>` to `Mutex<HashMap<Principal, Arc<Mutex<mpsc::Receiver<...>>>>>`. `recv()` now clones the per-principal `Arc<Mutex<Receiver>>` out of the outer map, drops the outer guard, and awaits on the inner mutex. `register()` is no longer blocked by a parked `recv`. Removed the stale `#[allow(clippy::significant_drop_tightening)]` annotation.

### LOW-03: middleware inner body cap duplicates outer cap

**Files modified:** `crates/famp-transport-http/src/middleware.rs`
**Commit:** `b1e3805`
**Applied fix:** Introduced `SIG_VERIFY_BODY_CAP = ONE_MIB + 16 * 1024` and use it for the inner `to_bytes` call. Outer `RequestBodyLimitLayer` remains authoritative at 1 MiB; inner cap is now a deliberately-larger safety net that still bounds buffering and emits `MiddlewareError::BodyTooLarge` if the outer layer is ever removed. Documented as defense-in-depth.

### LOW-04: `server.rs::inbox_handler` drops diagnostics on inbox send failure

**Files modified:** `crates/famp-transport-http/src/server.rs`
**Commit:** `505f3c2`
**Applied fix:** Capture clones of `sender` + `recipient` before the `tx.send(...).await` call; on error log `"inbox send failed (sender=..., recipient=...): <error>"` via `eprintln!` before returning `MiddlewareError::Internal`. Tracing is not yet wired in Phase 4; upgrade to `tracing::error!` when the tracing layer lands.

## Validation

- `cargo check -p famp-transport-http` — clean after each fix
- `cargo nextest run --workspace` — **253 tests passed, 1 skipped, 0 failed**
- `cargo clippy -p famp-transport-http --all-targets -- -D warnings` — clean
- `cargo clippy -p famp --tests -- -D warnings` — clean

**Pre-existing clippy failure (out of scope):**
`cargo clippy --workspace --all-targets -- -D warnings` fails on the
`personal_two_agents` example in `crates/famp` with
`unused-crate-dependencies` errors for `axum` and `reqwest`. Verified
these exist on the pre-fix `main` tree; unrelated to this phase's
review findings.

## Deferred

- **INF-01** stale `use _ as _` silencers — cosmetic, deferred.
- **INF-02** `HttpTransportError::TlsConfig(String)` loses typed info — `thiserror` convention cleanup, deferred.
- **INF-03** `_gen_fixture_certs.rs` `REGENERATE=1` env guard — optional workflow safety, deferred.

---

_Fixed: 2026-04-13_
_Fixer: Claude (gsd-code-fixer)_
_Iteration: 1_

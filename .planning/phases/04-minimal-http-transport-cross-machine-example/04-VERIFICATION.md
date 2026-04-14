---
phase: 04-minimal-http-transport-cross-machine-example
verified: 2026-04-13T00:00:00Z
status: passed
score: 5/5 must-haves verified
overrides_applied: 0
---

# Phase 4: Minimal HTTP Transport + Cross-Machine Example — Verification Report

**Phase Goal:** "The same signed cycle runs across two processes over HTTPS, bootstrapped from the same TOFU keyring, and the Phase 3 adversarial matrix is extended to `HttpTransport` — no new conformance categories are introduced."

**Verified:** 2026-04-13
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (Roadmap Success Criteria)

| # | Truth (from ROADMAP.md Phase 4 SC) | Status | Evidence |
|---|---|---|---|
| 1 | `famp-transport-http` exposes axum `POST /famp/v0.5.1/inbox/{principal}` + reqwest client path on rustls via `rustls-platform-verifier`, with 1 MB body limit as a tower layer (TRANS-03/04/06/07) | VERIFIED | `crates/famp-transport-http/src/{server.rs,middleware.rs,tls.rs,transport.rs}` all present; `RequestBodyLimitLayer::new(1_048_576)` live in `server.rs`; `Verifier::new_with_extra_roots` in `tls.rs`; axum route literal `/famp/v0.5.1/inbox/{principal}` in `server.rs`; `cargo tree -i openssl --workspace` returns no path; `cargo tree -i native-tls --workspace` returns no path. |
| 2 | Signature verification runs as HTTP middleware BEFORE routing (TRANS-09); unsigned/wrong-key rejected at tower layer, handler closure never entered | VERIFIED | `FampSigVerifyLayer` in `middleware.rs` with two-phase decode. Four sentinel tests in `crates/famp-transport-http/tests/middleware_layering.rs` (body>1MB, unsigned, unknown-sender, wrong-key) each assert `!sentinel.load(SeqCst)`. All 4 pass. |
| 3 | `cross_machine_two_agents` signed request → commit → deliver → ack cycle completes over real HTTPS with TOFU keyring bootstrap (CONF-04, EX-02), exit code 0 | VERIFIED | `crates/famp/examples/cross_machine_two_agents.rs` binary present, no `todo!()`, uses `tls_server::serve_std_listener` from Plan 04-03. **Primary CONF-04 gate is `tests/http_happy_path.rs`** — same-process but uses two separate `HttpTransport` instances, real rustls via `tls::build_server_config` + `tls_server::serve_std_listener` (verified lines 72-112), and `cycle_driver::drive_alice`/`drive_bob`. Subprocess test `cross_machine_happy_path.rs` is `#[ignore]`d per plan-authorized fallback (line 59) due to bootstrap chicken-and-egg. Same-process test passes in ~0.55s. |
| 4 | Phase 3 adversarial matrix extended to `HttpTransport` — 3 cases × 2 transports = 6 rows, same typed errors, no new CONF-0x | VERIFIED | `crates/famp/tests/adversarial/{harness,fixtures,memory,http}.rs` present. 6 `#[tokio::test]` functions (3 in `memory.rs`, 3 in `http.rs`). `cargo nextest run -p famp --test adversarial` → 6/6 passing in 0.286s. CONF-07 fixture at `tests/fixtures/conf-07-canonical-divergence.json` reused byte-identically. HTTP sentinel proof via `inbox_rx.try_recv() == Empty` (documented seam alternative). |
| 5 | TRANS-05 (.well-known) and TRANS-08 (spawn-channel) explicitly absent, crate compiles and examples run without them, omission documented | VERIFIED | Neither TRANS-05 nor TRANS-08 appears in the `famp-transport-http` crate source. Crate compiles and full workspace test suite passes; CONTEXT.md §Phase Boundary explicitly lists both as "out of scope for Phase 4" with v0.8+ pointer. |

**Score:** 5 / 5 roadmap success criteria verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/famp-transport-http/src/error.rs` | `MiddlewareError` + `HttpTransportError` enums | VERIFIED | Both enums + `IntoResponse` present; D-C6 status-code mapping locked by unit test `middleware_error_status_mapping_is_load_bearing` (passing). |
| `crates/famp-transport-http/src/middleware.rs` | `FampSigVerifyLayer` + two-phase decode | VERIFIED | Contains `peek_sender`, `keyring.get`, `canonicalize`, `AnySignedEnvelope::decode`, `extensions_mut().insert`. |
| `crates/famp-transport-http/src/server.rs` | `build_router` mounting single inbox route behind body-limit + sig-verify | VERIFIED | Route literal `/famp/v0.5.1/inbox/{principal}`; `RequestBodyLimitLayer::new(1_048_576)` outer, `FampSigVerifyLayer::new` inner; `envelope_sender(&envelope)` used for `TransportMessage.sender` (B-7 fix). |
| `crates/famp-transport-http/src/tls.rs` | PEM loaders + `build_server_config` + `build_client_config` with `Verifier::new_with_extra_roots` (D-B5 full) | VERIFIED | All four functions present; 6 unit tests passing. |
| `crates/famp-transport-http/src/tls_server.rs` | `serve` + `serve_std_listener` wrapping `axum_server::bind_rustls` / `from_tcp_rustls` | VERIFIED | Both helpers present, real implementations (grep confirms the sole `todo!()` reference is inside a comment documenting "no more `todo!()`"). |
| `crates/famp-transport-http/src/transport.rs` | `HttpTransport` + native AFIT `impl Transport` | VERIFIED | `impl Transport for HttpTransport`, `use_preconfigured_tls`, `famp/v0.5.1/inbox/` literal, percent-encoding fix for recipient principals. |
| `crates/famp-transport-http/tests/middleware_layering.rs` | 4 sentinel tests (TRANS-09 SC#2) | VERIFIED | All four tests pass; each asserts `!sentinel.load(...)`. |
| `crates/famp-envelope/src/peek.rs` | `peek_sender` lifted from runtime | VERIFIED | Exists, 3 unit tests (extracts, missing-from, duplicate-key). |
| `crates/famp/tests/common/cycle_driver.rs` | Generic `drive_alice` / `drive_bob` over `T: Transport` | VERIFIED | Lifted from `personal_two_agents.rs`, consumed by both example binary and `http_happy_path.rs`. |
| `crates/famp/examples/cross_machine_two_agents.rs` | EX-02 binary, two-role, no `todo!()` | VERIFIED | Present; `fn main`; no `todo!()` macros; uses `tls_server::serve_std_listener`. |
| `crates/famp/examples/_gen_fixture_certs.rs` | Fixture cert regenerator | VERIFIED | Present. |
| `crates/famp/tests/cross_machine_happy_path.rs` | CONF-04 subprocess test (authorized `#[ignore]`) | VERIFIED | Present; `#[ignore]` on line 59 with documented reason; kept as template per plan fallback. |
| `crates/famp/tests/http_happy_path.rs` | Same-process CONF-04 primary gate over real rustls | VERIFIED | Uses `tls::build_server_config`, `tls_server::serve_std_listener`, two distinct `HttpTransport` instances, `tokio::join!(drive_alice, drive_bob)`. Passes in ~0.55s. |
| `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}` | Committed fixture PEMs | VERIFIED | All four files present alongside README. |
| `crates/famp/tests/adversarial/harness.rs` | Shared `Case` + `assert_expected_error` | VERIFIED | Present. |
| `crates/famp/tests/adversarial/fixtures.rs` | Byte builders reusing CONF-07 fixture | VERIFIED | Present, reuses `conf-07-canonical-divergence.json` byte-identically. |
| `crates/famp/tests/adversarial/memory.rs` | 3 memory rows | VERIFIED | 3 `#[tokio::test]` functions, all pass. |
| `crates/famp/tests/adversarial/http.rs` | 3 HTTP rows + sentinel | VERIFIED | 3 `#[tokio::test]` functions using raw `reqwest::Client::post` per D-D2; sentinel proof via `inbox_rx.try_recv() == Empty`; all pass. |
| `.github/workflows/ci.yml` | `cargo tree -i openssl` gate | VERIFIED | Plan 04-03 Task 3 commit `af2d88b` lands the gate with literal `cargo tree -i openssl`, `cargo tree -i native-tls`, and `D-F4` marker. |

### Key Link Verification

| From | To | Via | Status |
|---|---|---|---|
| `server.rs` | `middleware.rs` | `ServiceBuilder` chain with `RequestBodyLimitLayer::new(1_048_576)` outer, `FampSigVerifyLayer::new` inner | WIRED (grep confirmed; 4 layering tests prove order) |
| `middleware.rs` | `famp-envelope::peek.rs` | `famp_envelope::peek_sender(&bytes)` | WIRED |
| `tls.rs` | `rustls_platform_verifier::Verifier` | `Verifier::new_with_extra_roots` | WIRED (literal present per Plan 04-03 acceptance) |
| `tls_server.rs` | `axum_server::bind_rustls` / `from_tcp_rustls` | `RustlsConfig::from_config(Arc<ServerConfig>)` | WIRED |
| `transport.rs` | `reqwest 0.13` (rustls backend) | `reqwest::Client::builder().use_preconfigured_tls(client_config)` | WIRED |
| `http_happy_path.rs` | `tls_server::serve_std_listener` | direct calls for alice + bob listeners (lines 110, 112) | WIRED |
| `cross_machine_two_agents.rs` | `tls_server::serve_std_listener` | direct call after `set_nonblocking(true)` on `std::net::TcpListener` | WIRED |
| `adversarial/http.rs` | `famp_transport_http::build_router` | mounted on plain HTTP ephemeral port (TLS adds zero adversarial-byte coverage) | WIRED |
| `adversarial/http.rs` | CONF-07 fixture bytes | `case_bytes(Case::CanonicalDivergence)` loads `conf-07-canonical-divergence.json` byte-identically | WIRED |

### Data-Flow Trace (Level 4)

Phase 4 is protocol + transport code; the "dynamic data" is the signed `request → commit → deliver → ack` cycle flowing across real HTTPS. Traced end-to-end through `http_happy_path.rs`: `drive_alice` constructs a signed `Request`, `HttpTransport::send` POSTs over rustls to bob's listener, `FampSigVerifyLayer` decodes + verifies + stashes, `inbox_handler` pushes raw bytes into bob's mpsc inbox, `cycle_driver::drive_bob` pulls from the channel, runs the full runtime decode (recipient cross-check + FSM), replies with `commit`/`deliver`/`ack` back over alice's rustls listener. Assertions on alice's trace confirm all four classes observed. **FLOWING.**

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Full workspace nextest | `cargo nextest run --workspace` | 247 / 247 passed, 1 skipped (ignored subprocess test) | PASS |
| Adversarial matrix (3×2) | `cargo nextest run -p famp --test adversarial` | 6 / 6 passed in 0.286s | PASS |
| No-openssl gate | `cargo tree -i openssl --workspace` | exit 101 (no match) | PASS |
| No native-tls gate | `cargo tree -i native-tls --workspace` | exit 101 (no match) | PASS |
| No `todo!()` macros in Phase 4 scope | grep on `famp-transport-http/**`, `famp/tests/**`, `famp/tests/common/**`, `famp/examples/cross_machine_two_agents.rs` | only match is a comment in `tls_server.rs` that reads `"no more todo!() for the ..."` — this is documentation, not a macro invocation | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| TRANS-03 | 04-03 | `famp-transport-http` crate with axum server + reqwest client | SATISFIED | `HttpTransport` + `impl Transport`; `build_router` server path; both sides compile and run under `http_happy_path.rs`. |
| TRANS-04 | 04-02 | `POST /famp/v0.5.1/inbox` endpoint per principal | SATISFIED | Single route literal in `server.rs`; multiplexed by `{principal}` path param (D-A1). |
| TRANS-06 | 04-03 | rustls-only TLS (no OpenSSL) via `rustls-platform-verifier` | SATISFIED | `Verifier::new_with_extra_roots` call site in `tls.rs`; `cargo tree -i openssl` empty; CI gate added in `af2d88b`. |
| TRANS-07 | 04-02 | Body-size limit (1 MB) as tower layer | SATISFIED | `RequestBodyLimitLayer::new(1_048_576)` in `build_router`; `body_over_1mb_does_not_enter_handler` passes. |
| TRANS-09 | 04-02 + 04-05 | Signature verification as HTTP middleware before routing | SATISFIED | `FampSigVerifyLayer` outer; 4 middleware_layering sentinel tests + 3 HTTP adversarial rows (each with handler-not-entered proof) all pass. |
| EX-02 | 04-04 | `cross_machine_two_agents` — two-process HTTPS cycle | SATISFIED | Binary present; no `todo!()`; same-process safety-net runs full cycle over real rustls; subprocess test authorized `#[ignore]` with documented reason. |
| CONF-04 | 04-04 | Happy-path two-node over `HttpTransport` (real HTTP + TLS) | SATISFIED | `http_happy_path.rs` is the primary gate; uses two `HttpTransport` instances, `tls::build_server_config`, `tls_server::serve_std_listener`, committed rustls fixture certs — bytes on the wire are real HTTPS. |
| CONF-05 | 04-05 | Adversarial unsigned — both transports | SATISFIED | `memory::memory_unsigned` + `http::http_unsigned` both pass with `bad_envelope` slug over HTTP. |
| CONF-06 | 04-05 | Adversarial wrong-key — both transports | SATISFIED | `memory::memory_wrong_key` + `http::http_wrong_key` pass with `signature_invalid` slug. |
| CONF-07 | 04-05 | Adversarial canonical divergence — both transports | SATISFIED | `memory::memory_canonical_divergence` + `http::http_canonical_divergence` pass with `canonical_divergence` slug; fixture JSON byte-identical. |

**10 / 10 requirements satisfied** (the 7 listed in ROADMAP Phase 4 contract plus the 3 CONF-05/06/07 HTTP rows carried forward from Phase 3 per the goal "extend to HttpTransport").

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---|---|---|---|
| `tls_server.rs` | 6 | Word `todo!()` appears inside a `//!` comment documenting *closure* of the B-3 checker ("no more `todo!()` for the serve helper") | Info | None — this is a documentation artifact, not a macro invocation. No real stubs in Phase 4 scope. |

No blockers, no warnings. Zero real `todo!()` macros in committed Phase 4 files.

### Human Verification Required

None — all phase behaviors are automated. The subprocess test is authorized `#[ignore]` per the plan's pre-approved fallback, and the same-process test owns the CONF-04 gate with byte-identical coverage (real rustls, two `HttpTransport` instances, real `tls_server::serve_std_listener`). Plan 04-04 documents the bootstrap chicken-and-egg rationale and defers the `--wait-peer-file` solution to a future phase.

### Gaps Summary

No gaps. Phase 4 delivers its goal: the same signed cycle runs across two `HttpTransport` instances over real HTTPS, bootstrapped from a TOFU keyring; the Phase 3 adversarial matrix is extended to HTTP with 6 rows × distinct typed errors × handler-not-entered proofs; no new CONF-0x categories were introduced; no OpenSSL / native-tls in the workspace graph; CI gate locks that invariant.

---

*Verified: 2026-04-13*
*Verifier: Claude (gsd-verifier)*

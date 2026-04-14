---
phase: 4
slug: minimal-http-transport-cross-machine-example
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-04-13
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution. Extracted from `04-RESEARCH.md` §Validation Architecture during plan revision 2026-04-13.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo nextest` (workspace pinned to `0.9.132+` in CLAUDE.md) + `proptest 1.11` for properties where applicable |
| **Config file** | Workspace `Cargo.toml` + per-crate `Cargo.toml` `[dev-dependencies]`. No nextest-specific config beyond default. |
| **Quick run command** | `cargo nextest run -p famp-transport-http` |
| **Full suite command** | `cargo nextest run --workspace` |
| **Estimated runtime** | ~20–60 seconds cold, <10 seconds incremental |

---

## Sampling Rate

- **After every task commit:** `cargo check -p famp-transport-http && cargo clippy -p famp-transport-http --all-targets -- -D warnings && cargo nextest run -p famp-transport-http`
- **After every plan wave:** `cargo nextest run --workspace && cargo tree -i openssl --workspace` (latter must produce no output)
- **Before `/gsd-verify-work`:** `just ci` (full workspace check + clippy + nextest + cargo audit) green
- **Max feedback latency:** 60 seconds per task commit

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 04-01-01 | 01 | 1 | — (infra) | — | Deps resolve with zero openssl | build | `cargo check -p famp-transport-http` | ❌ W0 (Cargo.toml) | ⬜ pending |
| 04-01-02 | 01 | 1 | — (infra) | — | `MiddlewareError` status codes load-bearing per D-C6 | unit | `cargo nextest run -p famp-transport-http` | ❌ W0 (error.rs) | ⬜ pending |
| 04-01-03 | 01 | 1 | — (infra) | — | `peek_sender` returns typed error, duplicate-key bytes rejected | unit | `cargo nextest run -p famp-envelope -p famp` | ❌ W0 (peek.rs) | ⬜ pending |
| 04-02-01 | 02 | 2 | TRANS-09 | T-04-01 (I: sig-verify before routing) | Middleware two-phase decode; unknown-sender → 401 | unit/compile | `cargo check -p famp-transport-http` | ❌ W0 (middleware.rs) | ⬜ pending |
| 04-02-02 | 02 | 2 | TRANS-04 | T-04-02 (S: recipient inbox lookup) | `build_router` mounts single route, inbox lookup keyed by `{principal}` path | compile | `cargo check -p famp-transport-http` | ❌ W0 (server.rs) | ⬜ pending |
| 04-02-03 | 02 | 2 | TRANS-07, TRANS-09 | T-04-03 (D: 1 MB body cap) | 413 on oversized; sentinel stays false on 3 adversarial cases | integration | `cargo nextest run -p famp-transport-http --test middleware_layering` | ❌ W0 (middleware_layering.rs) | ⬜ pending |
| 04-03-01 | 03 | 3 | TRANS-06 | T-04-04 (S: TLS trust anchor loading) | PEM loaders reject non-PEM input; `Verifier::new_with_extra_roots` composes platform + explicit trust | unit | `cargo nextest run -p famp-transport-http --lib tls` | ❌ W0 (tls.rs) | ⬜ pending |
| 04-03-02 | 03 | 3 | TRANS-03 | T-04-05 (T: client cross-host posting) | `HttpTransport::send` posts to correct URL; rustls-only | compile/clippy | `cargo clippy -p famp-transport-http --all-targets -- -D warnings` | ❌ W0 (transport.rs) | ⬜ pending |
| 04-03-03 | 03 | 3 | TRANS-06 | T-04-06 (E: accidental openssl pull) | CI gate fails build if openssl or native-tls reachable | bash | `! cargo tree -i openssl --workspace 2>/dev/null \| grep -q .` | ❌ W0 (ci.yml) | ⬜ pending |
| 04-04-01 | 04 | 4 | EX-02 | — | Fixture cert PEMs parse via `rustls-pemfile 2` | build | `cargo check -p famp` | ❌ W0 (fixtures+Cargo.toml) | ⬜ pending |
| 04-04-02 | 04 | 4 | EX-02 | T-04-07 (S: cert/key path confusion) | Example binary takes `--role alice\|bob`, prints `LISTENING https://...` once bound | build/run | `cargo check --example cross_machine_two_agents -p famp` | ❌ W0 (example.rs) | ⬜ pending |
| 04-04-03 | 04 | 4 | CONF-04 | T-04-08 (T: cross-process forgery) | Subprocess test: both procs exit 0 within 10s | integration | `cargo nextest run -p famp --test cross_machine_happy_path --test http_happy_path` | ❌ W0 (tests) | ⬜ pending |
| 04-05-01 | 05 | 4 | TRANS-09 | T-04-09 (S: adversarial bytes bypass middleware) | Six rows (3 cases × 2 transports) assert distinct typed errors; shared harness | integration | `cargo nextest run -p famp --test adversarial` | ❌ W0 (adversarial/*.rs) | ⬜ pending |
| 04-05-02 | 05 | 4 | CONF-05/06/07 HTTP rows | T-04-09 | Sentinel `AtomicBool` proves handler-not-entered per HTTP row | integration | `cargo nextest run -p famp --test adversarial` | ❌ W0 (http.rs) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/famp-transport-http/Cargo.toml` — fill `[dependencies]` with axum, tower, tower-http, reqwest, rustls, rustls-platform-verifier, rustls-pemfile, axum-server, url, futures-util, thiserror, serde, serde_json (04-01 Task 1)
- [ ] `crates/famp-transport-http/src/lib.rs` — module skeleton (04-01)
- [ ] `crates/famp-transport-http/src/error.rs` — `MiddlewareError` + `HttpTransportError` + `MiddlewareError → IntoResponse` + status-code lock unit test (04-01)
- [ ] `crates/famp-envelope/src/peek.rs` — lifted `peek_sender` with tests (04-01 Task 3)
- [ ] `crates/famp/src/runtime/peek.rs` — thin re-export wrapper (04-01 Task 3)
- [ ] `crates/famp-transport-http/src/middleware.rs` — `FampSigVerifyLayer` + canonical pre-check + extension stash (04-02 Task 1)
- [ ] `crates/famp-transport-http/src/server.rs` — `build_router`, `InboxRegistry`, `inbox_handler` reading `Extension<Arc<AnySignedEnvelope>>` (04-02 Task 2)
- [ ] `crates/famp-transport-http/src/tls.rs` — PEM loaders + `build_server_config` + `build_client_config` using `Verifier::new_with_extra_roots` (04-03 Task 1)
- [ ] `crates/famp-transport-http/src/tls_server.rs` — `serve(addr, router, server_config) -> JoinHandle` wrapping `axum_server::bind_rustls` (04-03 Task 1)
- [ ] `crates/famp-transport-http/src/transport.rs` — `HttpTransport` struct + `impl Transport` (04-03 Task 2)
- [ ] `crates/famp-transport-http/tests/middleware_layering.rs` — 4 sentinel tests: unsigned (non-empty keyring), body-over-1MB, unknown-sender, wrong-key (04-02 Task 3)
- [ ] `crates/famp-transport-http/tests/routing.rs` — TRANS-04 integration (optional extra — covered by 04-05 adversarial http rows)
- [ ] `crates/famp-transport-http/tests/body_limit.rs` — TRANS-07 413 test (covered inside middleware_layering.rs)
- [ ] `crates/famp/examples/cross_machine_two_agents.rs` — EX-02 binary (04-04 Task 2)
- [ ] `crates/famp/examples/_gen_fixture_certs.rs` — one-off fixture generator (04-04 Task 1)
- [ ] `crates/famp/tests/common/cycle_driver.rs` — shared helper extracted from `personal_two_agents.rs` driving one `request → commit → deliver → ack` cycle against any `Transport` impl (04-04 Task 3 prerequisite)
- [ ] `crates/famp/tests/cross_machine_happy_path.rs` — CONF-04 subprocess test (04-04 Task 3)
- [ ] `crates/famp/tests/http_happy_path.rs` — same-process safety net (04-04 Task 3)
- [ ] `crates/famp/tests/adversarial.rs` — thin entry declaring modules (04-05 Task 1)
- [ ] `crates/famp/tests/adversarial/harness.rs` — shared `Case` + `AdversarialTransport` trait (04-05 Task 1)
- [ ] `crates/famp/tests/adversarial/fixtures.rs` — byte loaders (reuse Phase 3 `generate_conf_07_bytes`) (04-05 Task 1)
- [ ] `crates/famp/tests/adversarial/memory.rs` — MemoryTransport adapter (04-05 Task 1)
- [ ] `crates/famp/tests/adversarial/http.rs` — HttpTransport adapter + sentinel (04-05 Task 2)
- [ ] `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}` — committed fixture PEMs (04-04 Task 1)
- [ ] `.github/workflows/ci.yml` — `cargo tree -i openssl` gate (04-03 Task 3)
- [ ] `crates/famp/Cargo.toml` — add `famp-transport-http` + `rcgen` + `tempfile` dev-deps (04-04 Task 1)

---

## Manual-Only Verifications

*All phase behaviors have automated verification. The subprocess test covers cross-process HTTPS; the same-process test covers the in-process axum/tower pipeline; the adversarial matrix covers the security gates; the CI `cargo tree -i openssl` gate covers the TLS stack invariant. No manual-only steps are required in Phase 4.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 60s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending — updated after plan revision 2026-04-13

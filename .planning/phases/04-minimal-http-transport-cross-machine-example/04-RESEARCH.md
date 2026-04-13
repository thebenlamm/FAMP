# Phase 4: Minimal HTTP Transport + Cross-Machine Example — Research

**Researched:** 2026-04-13
**Domain:** axum/tower HTTP server + reqwest/rustls client + cross-process FAMP envelope cycle
**Confidence:** HIGH (stack and patterns are pre-locked by CLAUDE.md and CONTEXT.md; verification confirms current API shapes)

## Summary

Phase 4 is a **filling-in** phase, not an exploration phase. The technology stack is locked in `CLAUDE.md`'s TL;DR table and re-validated in `04-CONTEXT.md`. The architectural decisions (one listener path-multiplexed by principal, sig-verify tower layer in front of routing, raw-bytes-into-mpsc inbox hub mirroring `MemoryTransport`, `--peer`/`--addr` separation, no SPKI pinning, no signed-FAMP-ack rejection) are already taken. Phase 3 leaves a fully working `Transport` trait, runtime glue, keyring, and adversarial harness that Phase 4 reuses **unchanged**. The `crates/famp-transport-http` crate already exists as an empty Phase-0 stub — Phase 4 fills its `[dependencies]` and `src/` body, it does not scaffold a new crate.

The research effort therefore focuses on (a) confirming the locked crate versions are still current and ship the APIs CONTEXT.md assumes, (b) documenting the exact axum-0.8 / tower / tower-http patterns the planner needs, (c) flagging the known sharp edges around tower middleware that consumes request bodies, and (d) validating the rcgen-0.14 self-signed cert path used by the example binary.

**Primary recommendation:** Implement `HttpTransport` as a thin axum-0.8 server (one listener, `POST /famp/v0.5.1/inbox/:principal`) plus a shared `reqwest 0.13` client wired to `rustls 0.23` via `rustls-platform-verifier`. Mount middleware via a `tower::ServiceBuilder`-built `Router::layer` stack with **`RequestBodyLimitLayer` outermost, `FampSigVerifyLayer` immediately inside it**, and the path-routed handler innermost. The sig-verify layer reads the (already capped) full body, runs the existing `famp-envelope` decode + `famp-crypto::verify_strict` against an `Arc<Keyring>`, stashes the decoded `SignedEnvelope` in `Request::extensions` for downstream cheapness, and short-circuits with a phase-local `MiddlewareError` → typed JSON body on failure. The handler then forwards the raw body bytes into the per-principal mpsc inbox; the existing Phase 3 runtime glue re-verifies as a deliberate double-check (see D-C2). Adversarial parity is achieved by promoting `crates/famp/tests/adversarial.rs` to a directory module with a shared trait + case enum, mounting one adapter per transport, and reusing the Phase 3 CONF-07 fixture byte-identically.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**A. Server Topology + Inbox Wiring**
- **D-A1:** One axum listener per process, path-multiplexed via `POST /famp/v0.5.1/inbox/:principal`. Per-principal listeners rejected.
- **D-A2:** `HttpTransport` holds `addr_map: HashMap<Principal, Url>` (client) and `inboxes: HashMap<Principal, mpsc::Sender<TransportMessage>>` (server). Mirrors Phase 3 `MemoryTransport` D-C4 exactly.
- **D-A3:** Handler order: parse `:principal` (400 `bad_principal` on failure) → body limit already applied earlier → sig-verify already applied earlier (decoded envelope stashed in `Request::extensions`) → look up `:principal` in `inboxes` (404 `unknown_recipient` if missing) → forward **raw body bytes** into mpsc → `202 Accepted` empty body. Runtime re-decodes deliberately.
- **D-A4:** `Transport::recv(&principal)` semantics unchanged from Phase 3. **Phase 4 does NOT modify the `Transport` trait.**
- **D-A5:** `HttpTransport::register(principal)` creates the mpsc pair, returns `Receiver` to caller (mirrors `MemoryTransport::register`).
- **D-A6:** Client `send` posts to `{addr_map[recipient]}/famp/v0.5.1/inbox/{recipient}` via shared `reqwest::Client`. `Content-Type: application/famp+json`. Unknown recipient → `HttpTransportError::UnknownRecipient`.

**B. Address Discovery + TLS Cert Trust**
- **D-B1:** Address config STAYS SEPARATE from keyring. No `Url` in keyring.
- **D-B2:** New CLI flag `--addr <principal>=<https-url>`, mirrors `--peer` syntax (`=` separator, repeatable).
- **D-B3:** No sibling address file. No `peers.toml`. Deferred to v0.8.
- **D-B4:** `addr_map` lives inside `HttpTransport` (client side only).
- **D-B5:** Server loads cert/key from `--cert <path>` + `--key <path>`. Example generates via `rcgen 0.14` if missing. Client trusts via explicit `--trust-cert <path>` added as extra `RootCertStore` anchor on top of `rustls-platform-verifier`. **No SPKI pinning. No custom verifier.**
- **D-B6:** Example self-signed certs use `subject_alt_names = ["localhost", "127.0.0.1"]`.
- **D-B7:** Committed fixture certs at `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}` for CI; example regenerates fresh ones at runtime.
- **D-B8:** No OpenSSL. No `native-tls`. `rustls 0.23.38` only. `reqwest` with `default-features = false` + rustls-only feature set.

**C. Sig-Verify Middleware + Error Responses**
- **D-C1:** Tower layer order outer→inner: `RequestBodyLimitLayer::new(1_048_576)` → `FampSigVerifyLayer` → axum route → handler. **Body limit MUST come first.**
- **D-C2:** `FampSigVerifyLayer` enforces (a) envelope decode (INV-10 surfaces here as decode error), (b) `verify_strict` against pinned key for `envelope.from()`. It does **NOT** run recipient cross-check, FSM step, or touch `RuntimeError`.
- **D-C3:** Decoded `SignedEnvelope` written to `Request::extensions` for Phase 5+ reuse. Phase 4 handler does not read it (it forwards raw bytes). Cost: one `Arc<SignedEnvelope>` per request.
- **D-C4:** Keyring injected as `Arc<Keyring>`. Read-only. No live re-pin in Phase 4.
- **D-C5:** Rejection responses are plain HTTP status + small typed JSON body `{"error": "<snake>", "detail": "<human>"}`. **NOT** a signed FAMP ack.
- **D-C6:** Status mapping (LOAD-BEARING — distinguishes CONF-05/06/07 at HTTP layer):

  | Failure | Code | `error` |
  |---|---|---|
  | Body > 1 MB | 413 | `body_too_large` |
  | Path principal unparseable | 400 | `bad_principal` |
  | Envelope decode fail (incl. INV-10 unsigned) | 400 | `bad_envelope` |
  | Canonical divergence at decode | 400 | `canonical_divergence` |
  | Pinned key missing for sender | 401 | `unknown_sender` |
  | Signature verification failed | 401 | `signature_invalid` |
  | Unknown recipient (no inbox) | 404 | `unknown_recipient` |
  | Internal (channel closed, etc.) | 500 | `internal` |

- **D-C7:** Phase-local `MiddlewareError` (thiserror, narrow). Implements `IntoResponse`. Does **NOT** reuse Phase 3 `RuntimeError`.
- **D-C8:** `HttpTransportError` for `Transport::Error` associated type. Variants: `UnknownRecipient { principal }`, `ReqwestFailed(#[source] reqwest::Error)`, `ServerStatus { code: u16, body: String }`, `InboxClosed { principal }`.

**D. Adversarial Matrix Reuse**
- **D-D1:** Promote `crates/famp/tests/adversarial.rs` to directory module: `adversarial/{mod.rs, memory.rs, http.rs, fixtures.rs}`.
- **D-D2:** Shared harness trait `AdversarialTransport` with `inject_raw` + `run_loop_and_catch`. `memory.rs` uses `send_raw_for_test` (Phase 3 D-D6). `http.rs` uses raw `reqwest::Client::post` — **no `test-util` feature on `famp-transport-http`**, raw HTTP is already the injection surface.
- **D-D3:** Three cases × two transports = six `#[tokio::test]` rows. Same `Case` enum, same expected `RuntimeError` variant, same "no panic" guarantee, same "handler closure not entered" guarantee on HTTP rows.
- **D-D4:** Reuse `crates/famp/tests/fixtures/conf-07-canonical-divergence.json` byte-identically. If HTTP doesn't surface the same error, canonicalization is broken.
- **D-D5:** TRANS-09 SC#2 verified via `Arc<AtomicBool>` sentinel injected into the axum router. Sentinel must remain `false` after each adversarial request returns.
- **D-D6:** Expected-error mapping table (per case, per transport) is the load-bearing distinguishability test:

  | Case | MemoryTransport `RuntimeError` | HTTP client `HttpTransportError` | HTTP server `MiddlewareError` |
  |---|---|---|---|
  | Unsigned (CONF-05) | `Decode(EnvelopeDecodeError::MissingSignature)` | `ServerStatus { 400, bad_envelope }` | `BadEnvelope` |
  | Wrong-key (CONF-06) | `Decode(EnvelopeDecodeError::SignatureInvalid)` | `ServerStatus { 401, signature_invalid }` | `SignatureInvalid` |
  | Canonical divergence (CONF-07) | `CanonicalDivergence` | `ServerStatus { 400, canonical_divergence }` | `CanonicalDivergence` |

**E. Example Binary**
- **D-E1:** **One binary, two invocations, fixed `--role alice|bob`. NO auto-orchestration.** Ben's specific ask.
- **D-E2:** `--role` binds fixed principal + default port (`:8443` bob, `:8444` alice). Override via `--listen <addr:port>`. Role is example scaffolding, not a FAMP concept.
- **D-E3:** Documented run sequence (bob first, prints pubkey + cert paths; user copies to alice host; alice runs with `--peer`, `--addr`, `--trust-cert`).
- **D-E4:** **Symmetric topology**: both roles run server + client (deliver/ack flow back). Both need `--cert/--key/--trust-cert` for the other.
- **D-E5:** Trace format `[seq] SENDER → RECIPIENT: CLASS (state: FROM → TO)`, each process prints its own view.
- **D-E6:** CI integration test `crates/famp/tests/cross_machine_happy_path.rs`: spawns binary twice, ephemeral ports, tempdir cert/key exchange, asserts exit 0 + trace lines. CONF-04 is satisfied here.
- **D-E7:** Optional fallback `crates/famp/tests/http_happy_path.rs`: two `tokio::spawn`ed tasks against `127.0.0.1:<ephemeral>`, real axum + reqwest, no subprocess. CI safety net. Planner picks whether to ship both.

**F. Crate Layout**
- **D-F1:** `famp-transport-http` is library-only. No binary. Example + integration tests live in `crates/famp/`.
- **D-F2:** Dependency graph fixed (see Standard Stack table below).
- **D-F3:** `crates/famp/Cargo.toml` gains `famp-transport-http` path dep + `rcgen` dev-dep.
- **D-F4:** **No `openssl`, no `native-tls`, no `openssl` crate ANYWHERE.** CI gate: `cargo tree -i openssl` must fail the build if it appears.
- **D-F5:** Native AFIT for `Transport` impl. No `async-trait` macro.

### Claude's Discretion

- Module layout inside `crates/famp-transport-http/src/` (`lib.rs` + `server.rs` + `client.rs` + `middleware.rs` + `error.rs` is reasonable; planner decides).
- Exact `reqwest` and `rustls` feature flag selection (minimum viable: rustls backend, platform-verifier roots, HTTP/1.1 only).
- Whether `HttpTransport` owns its `tokio::task::JoinHandle` (prefer yes — graceful shutdown on `Drop`) vs. delegating spawn to caller.
- Exact flag naming: `--role` preferred per discussion; `--listen`, `--cert`, `--key`, `--trust-cert`, `--out-pubkey`, `--out-cert` follow.
- Whether `--addr` is repeated flags or comma-separated — repeated preferred (matches `--peer`).
- `rcgen` as dev-dep only vs. optional feature — prefer dev-dep only.
- File layout inside `crates/famp/tests/adversarial/` (one file with submodules vs. directory).
- Adversarial harness uses `async_trait` vs. native AFIT — native AFIT preferred unless dyn-dispatch needed.
- Subprocess coordination mechanism (`LISTENING` stdout vs. filesystem sentinel vs. `TcpStream::connect` retry) — `TcpStream::connect` retry preferred.
- Whether to ship `http_happy_path.rs` fallback alongside subprocess test — ship both unless planner has a reason not to.
- `reqwest::Client` keepalive/timeouts (HTTP/1.1 + 10s total timeout is a sensible default).
- Wire `Content-Type` (`application/famp+json` vs. `application/json`) — pick one and stay consistent.

### Deferred Ideas (OUT OF SCOPE)

- `.well-known` Agent Card distribution (TRANS-05) — v0.8.
- Cancellation-safe spawn-channel send path (TRANS-08) — v0.9.
- Pluggable `TrustStore` trait + federation credential — v0.8+.
- Sibling `peers.toml` / file-based address map — revisit with v0.8 Agent Cards.
- SPKI cert pinning / dev-only custom rustls verifier — rejected for v0.7.
- mTLS — FAMP trust lives at envelope sig layer, not TLS layer.
- HTTP/2, HTTP/3, QUIC — HTTP/1.1 only in v0.7.
- Dynamic inbox registration at runtime — startup-time only in Phase 4.
- Connection pool / keepalive tuning per peer — one shared default `reqwest::Client`.
- Middleware that calls the full runtime pipeline — middleware is fast-reject only.
- Signed FAMP ack as HTTP rejection response — drags signing keys into middleware, wrong layer.
- Single-binary auto-orchestrated `cross_machine_two_agents` — rejected per discussion.
- Separate `http_adversarial.rs` duplicating Phase 3 case logic — one harness, two adapters.
- `test-util` feature on `famp-transport-http` mirroring Phase 3 D-D6 — rejected; raw HTTP is already the injection surface.
- `stateright` model check over the HTTP middleware pipeline — v0.14.
- Conformance Level 2/3 badges — v0.14.
- `famp` CLI subcommands — v0.8+ CLI milestone.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description (from REQUIREMENTS.md) | Research Support |
|----|-------------|------------------|
| TRANS-03 | `famp-transport-http` crate with axum server + `reqwest` client | Standard Stack §7/§8; Server topology §A; existing Phase-0 stub at `crates/famp-transport-http/` to fill in |
| TRANS-04 | `POST /famp/v0.5.1/inbox` endpoint per principal | D-A1 path-multiplex `POST /famp/v0.5.1/inbox/:principal` (one listener, principal in path); axum 0.8 path param syntax confirmed |
| TRANS-06 | rustls-only TLS via `rustls-platform-verifier` | D-B5/D-B8; verified `reqwest 0.13` rustls feature flags; `rustls-platform-verifier 0.5.x` |
| TRANS-07 | Body-size limit 1 MB per spec §18 as tower layer | D-C1 outermost layer; `tower_http::limit::RequestBodyLimitLayer::new(1_048_576)`; verified ordering caveat (must be applied via `Router::layer` outermost) |
| TRANS-09 | Sig-verification runs as HTTP middleware **before** routing | D-C1/D-D5 — `FampSigVerifyLayer` mounted via `Router::layer` (not `route_layer`), sentinel `Arc<AtomicBool>` test asserts handler not entered on adversarial cases |
| EX-02 | `cross_machine_two_agents.rs` — same flow split across two processes over HTTPS | D-E1..E7 — fixed `--role`, symmetric topology, subprocess CI test, optional same-process fallback |
| CONF-04 | Happy-path two-node integration over `HttpTransport` | D-E6 owns the CI gate; D-E7 owns the same-process safety net |

**Note:** CONF-05/06/07 (the three adversarial cases) are owned by Phase 3 by ID. Phase 4 extends the same case definitions to HTTP rows via the shared harness (D-D1..D6). No new CONF-0x requirement is created in Phase 4.
</phase_requirements>

## Standard Stack

### Core (locked in CLAUDE.md tech-stack table; planner MUST validate against live crates.io before pinning in Cargo.toml)

| Library | Pinned Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `axum` | `0.8.8` | HTTP server framework | Tokio-rs official; `tower` middleware integration; path params via `Path<String>` extractor; the de-facto Rust web framework `[CITED: docs.rs/axum]` |
| `tower` | latest 0.5.x | `Service` / `Layer` abstractions for `FampSigVerifyLayer` | Foundation that axum is built on; required for hand-written layer `[CITED: docs.rs/tower]` |
| `tower-http` | latest 0.6.x | `RequestBodyLimitLayer` + (optionally) `TraceLayer` | Standard middleware library paired with axum; ships the body-limit layer we need for §18 `[CITED: docs.rs/tower-http]` |
| `hyper` | `1.x` (transitive via axum) | HTTP/1.1 implementation | Pulled by axum 0.8; do not depend directly |
| `http` | `1.x` (transitive via axum) | `StatusCode`, `Request`, `Response` types | Transitive |
| `reqwest` | `0.13.2` | HTTP client | Highest-level Rust client; rustls is the default in 0.13 `[VERIFIED: seanmonstar.com/blog/reqwest-v013-rustls-default/]` |
| `rustls` | `0.23.38` | TLS stack | Pure-Rust, no OpenSSL; current stable line `[CITED: CLAUDE.md]` |
| `rustls-platform-verifier` | `0.5.x` | OS trust store integration | Replaces the deprecated `rustls-native-certs` dance `[CITED: CLAUDE.md]` |
| `rustls-pemfile` | `~2.0` | PEM cert/key file loading from disk | Pairs with rustls 0.23; reads `--cert`/`--key`/`--trust-cert` paths |
| `tokio` | `1.51.x` | Async runtime | Project-wide pin; axum/reqwest/rustls all assume tokio |
| `thiserror` | `2.0.18` | Narrow phase-local error enums | Project-wide pattern (Phase 1/2/3 precedent) |
| `serde` | `1.x` | Derive | Already in workspace |
| `serde_json` | `1.x` | **Error body JSON only** — NOT for envelope parsing (that's `famp-envelope` + `serde_jcs`) | Already in workspace |
| `url` | `2.x` | `Url` type for `addr_map` | Standard Rust URL crate |
| `rcgen` | `0.14.x` (DEV-DEP only on `crates/famp`, NOT on `famp-transport-http`) | Self-signed cert generation for example + tests | Library-only stays clean; example owns the runtime cert path `[VERIFIED: docs.rs/rcgen]` |

### Existing Phase-0 stub

`crates/famp-transport-http/Cargo.toml` already exists with empty `[dependencies]`. **Phase 4 fills it in.** Do not create a new crate. Do not move the existing path. The `[lints]` workspace inheritance is already wired.

### Workspace `tokio` features for `famp-transport-http` (narrow, not `full`)

Recommended feature set for the library: `["rt", "sync", "macros", "net"]` (server uses `tokio::net::TcpListener`; mpsc channels; spawn). The example binary in `crates/famp/` may use `["full"]`.

### `reqwest` feature flag selection (planner choice within D-B8)

Minimum viable, all defaults disabled:

```toml
reqwest = { version = "0.13", default-features = false, features = [
  "rustls-tls",                  # rustls backend, no native-tls
  "rustls-tls-native-roots",     # OS root store via rustls-platform-verifier
  "http2",                       # OPTIONAL — Phase 4 only needs HTTP/1.1; omit for smaller surface
  "json",                        # OPTIONAL — only if client wants typed JSON; raw body POST does not need it
] }
```

**Default features include `default-tls` which itself enables rustls in 0.13** — but `default-tls` will be re-enabled by any other crate that requests it. `default-features = false` is mandatory `[VERIFIED: lib.rs/crates/reqwest/features]`. For Phase 4, `rustls-tls` + `rustls-tls-native-roots` is sufficient. `http2` is **not** required (HTTP/1.1 only in v0.7). `json` is **not** required (raw bytes posted via `.body(Vec<u8>)`).

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `axum` | `actix-web` | Actor model, heavier learning curve, smaller middleware ecosystem — rejected by CLAUDE.md |
| `axum` | Raw `hyper 1.x` | Reinventing routing/extractors — pointless for FAMP |
| `reqwest` | `hyper-util` client | Manual connection pool — overkill for a two-process example |
| `rustls` | `native-tls` / `openssl` | Permanently banned per D-F4 + project commitment |
| `rcgen` for cert gen | Pre-generated fixtures only | Phase 4 wants both: fixtures for CI, runtime gen for example UX |
| `async-trait` macro | Native AFIT | D-F5 — Phase 3 D-C5 set the precedent; do not regress |

**Installation (planner adds to `crates/famp-transport-http/Cargo.toml`):**

```toml
[dependencies]
famp-core      = { path = "../famp-core" }
famp-envelope  = { path = "../famp-envelope" }
famp-crypto    = { path = "../famp-crypto" }
famp-keyring   = { path = "../famp-keyring" }
famp-transport = { path = "../famp-transport" }

axum                       = "0.8"
tower                      = "0.5"
tower-http                 = { version = "0.6", features = ["limit"] }
reqwest                    = { version = "0.13", default-features = false, features = ["rustls-tls", "rustls-tls-native-roots"] }
rustls                     = "0.23"
rustls-platform-verifier   = "0.5"
rustls-pemfile             = "2"
tokio                      = { workspace = true, features = ["rt", "sync", "macros", "net"] }
thiserror                  = { workspace = true }
serde                      = { workspace = true, features = ["derive"] }
serde_json                 = { workspace = true }
url                        = "2"
```

`crates/famp/Cargo.toml`:

```toml
[dependencies]
famp-transport-http = { path = "../famp-transport-http" }

[dev-dependencies]
rcgen = "0.14"
```

### Version verification protocol (planner runs before pinning)

```bash
npm view  # NOT for Rust — use cargo
cargo search axum --limit 1
cargo search tower --limit 1
cargo search tower-http --limit 1
cargo search reqwest --limit 1
cargo search rustls --limit 1
cargo search rustls-platform-verifier --limit 1
cargo search rustls-pemfile --limit 1
cargo search rcgen --limit 1
cargo search url --limit 1
```

If any version has drifted upward, document the bump in the plan and re-validate against this RESEARCH.md's API assumptions (path param syntax, layer ordering, feature flag names) before locking. The CLAUDE.md table was hand-pinned at 2026-04-12; minor bumps are routine.

## Architecture Patterns

### Recommended `famp-transport-http` Module Structure

```
crates/famp-transport-http/
├── Cargo.toml             # filled-in (deps above)
└── src/
    ├── lib.rs             # pub re-exports + crate doc
    ├── error.rs           # MiddlewareError + HttpTransportError (two enums, deliberately separate per D-C7/C8)
    ├── transport.rs       # HttpTransport struct + impl Transport (send + recv + register)
    ├── server.rs          # build_router(keyring, inboxes) -> axum::Router; handler fn
    ├── middleware.rs      # FampSigVerifyLayer + FampSigVerifyService
    ├── client.rs          # OPTIONAL — reqwest::Client builder helper (rustls config + extra trust anchor)
    └── tls.rs             # OPTIONAL — load_pem_cert / load_pem_key / build_server_config / build_client_config helpers
```

This is **Claude's discretion** per CONTEXT.md — the planner may collapse `client.rs`/`tls.rs` into `transport.rs` if cleaner.

### Pattern 1: Tower Layer Stack on the Router (D-C1 outer→inner)

**What:** Build a `tower::ServiceBuilder` with body-limit outer + sig-verify inner, attach via `Router::layer` so it runs **before route dispatch**.

**When to use:** Phase 4's only middleware path. Both layers MUST run before the handler closure executes.

**Example shape:**

```rust
// Source: derived from docs.rs/axum/latest/axum/middleware/index.html + tower-http/limit
use std::sync::Arc;
use axum::{Router, routing::post};
use tower::ServiceBuilder;
use tower_http::limit::RequestBodyLimitLayer;

pub fn build_router(
    keyring: Arc<famp_keyring::Keyring>,
    inboxes: Arc<InboxRegistry>,
) -> Router {
    Router::new()
        .route("/famp/v0.5.1/inbox/{principal}", post(inbox_handler))
        .with_state(inboxes)
        .layer(
            ServiceBuilder::new()
                .layer(RequestBodyLimitLayer::new(1_048_576))   // OUTER — runs first
                .layer(FampSigVerifyLayer::new(keyring))        // INNER — runs second
        )
}
```

**Critical:** `tower::ServiceBuilder` applies layers in the order they are written, with the **first layer being outermost** (i.e., it sees the request first and the response last). Verify against the current `tower::ServiceBuilder` docs at planning time — historically there has been confusion about this ordering (it was reversed in pre-0.4 tower). `[CITED: docs.rs/tower/latest/tower/builder/struct.ServiceBuilder.html]`

**axum 0.8 path param syntax change:** axum 0.8 uses `{principal}` braces in route patterns (not `:principal` from 0.7). The CONTEXT.md decision text uses the conceptual `:principal` form; the planner MUST emit `{principal}` in the actual route literal. `[CITED: github.com/tokio-rs/axum/blob/main/axum/CHANGELOG.md — axum 0.8 release notes]` `[ASSUMED until planner verifies on docs.rs/axum/0.8.8/axum/]`

### Pattern 2: `FampSigVerifyLayer` as a hand-written `tower::Layer` + `Service`

**What:** A `tower::Layer<S>` that wraps the inner service and produces a `FampSigVerifyService<S>`. The service's `call` reads the full request body (already capped to 1 MB by the outer layer), runs envelope decode + signature verify, stashes the decoded `SignedEnvelope` in `request.extensions_mut()`, and forwards. On failure, builds an `axum::response::Response` from `MiddlewareError::into_response()` and returns it without calling `inner.call(req)`.

**When to use:** Phase 4 `FampSigVerifyLayer` is the only example.

**Sketch (planner refines):**

```rust
// Source: shape derived from axum middleware docs + tower::Layer trait
use std::{sync::Arc, task::{Context, Poll}};
use axum::{body::{to_bytes, Body}, http::Request, response::{IntoResponse, Response}};
use tower::{Layer, Service};
use futures_util::future::BoxFuture;

#[derive(Clone)]
pub struct FampSigVerifyLayer { keyring: Arc<famp_keyring::Keyring> }

impl FampSigVerifyLayer {
    pub fn new(keyring: Arc<famp_keyring::Keyring>) -> Self { Self { keyring } }
}

impl<S> Layer<S> for FampSigVerifyLayer {
    type Service = FampSigVerifyService<S>;
    fn layer(&self, inner: S) -> Self::Service {
        FampSigVerifyService { inner, keyring: self.keyring.clone() }
    }
}

#[derive(Clone)]
pub struct FampSigVerifyService<S> { inner: S, keyring: Arc<famp_keyring::Keyring> }

impl<S> Service<Request<Body>> for FampSigVerifyService<S>
where
    S: Service<Request<Body>, Response = Response, Error = std::convert::Infallible>
        + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = std::convert::Infallible;
    type Future = BoxFuture<'static, Result<Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let keyring = self.keyring.clone();
        // CRITICAL: `Service::call` requires &mut self; the standard pattern is
        // to clone `self.inner` into a new variable to avoid the not-ready-after-call bug.
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        Box::pin(async move {
            let (parts, body) = req.into_parts();
            // Body is already capped to 1 MB by the outer layer.
            let bytes = match to_bytes(body, 1_048_576).await {
                Ok(b) => b,
                Err(_) => return Ok(MiddlewareError::BadEnvelope.into_response()),
            };
            // Re-decode + verify.
            let envelope = match famp_envelope::AnySignedEnvelope::decode(&bytes) {
                Ok(e) => e,
                Err(_) => return Ok(MiddlewareError::BadEnvelope.into_response()),
            };
            let sender = envelope.from();
            let key = match keyring.get(sender) {
                Some(k) => k,
                None => return Ok(MiddlewareError::UnknownSender.into_response()),
            };
            // verify_strict path runs internally inside AnySignedEnvelope::decode, but the
            // pinned-key cross-check vs keyring happens here — confirm exact API at plan time.
            // (See Pitfall 3 below.)

            // Re-attach body for the handler. Stash decoded envelope in extensions.
            let mut req = Request::from_parts(parts, Body::from(bytes));
            req.extensions_mut().insert(Arc::new(envelope));
            inner.call(req).await
        })
    }
}
```

**This sketch has at least three TODOs the planner must resolve:**
1. The exact `famp_envelope::AnySignedEnvelope::decode` signature (verify-only? verify against pinned key passed as arg? the Phase 3 `process_one_message` does a two-phase decode — the middleware must mirror that or use a sister API). See **Pitfall 3** below.
2. The precise `tower::Service` "clone-and-replace" ready-pattern (this is the single most common tower bug; planner should reference axum's own `from_fn` source if in doubt).
3. Whether to use `axum::middleware::from_fn_with_state` (simpler; closure-based) instead of a hand-written `Layer` + `Service` (more flexible). For Phase 4's narrow remit `from_fn_with_state` is likely sufficient and DROPS ~50 LoC of boilerplate. **Recommended: planner picks `from_fn_with_state` unless a hand-written layer is needed for stash-into-extensions.**

### Pattern 3: Path param extraction in axum 0.8

```rust
use axum::extract::{Path, State};

async fn inbox_handler(
    Path(principal_str): Path<String>,
    State(inboxes): State<Arc<InboxRegistry>>,
    body: axum::body::Bytes,            // raw bytes — already verified by middleware
) -> Result<axum::http::StatusCode, MiddlewareError> {
    let principal = famp_core::Principal::from_str(&principal_str)
        .map_err(|_| MiddlewareError::BadPrincipal)?;
    let sender = /* read from request.extensions() — see below */;
    let inbox = inboxes.get(&principal).ok_or(MiddlewareError::UnknownRecipient)?;
    inbox.send(famp_transport::TransportMessage { sender, recipient: principal, bytes: body.to_vec() })
        .await
        .map_err(|_| MiddlewareError::Internal)?;
    Ok(axum::http::StatusCode::ACCEPTED)
}
```

**Note:** Reading `Request::extensions` from inside an axum handler that uses extractors requires either a custom extractor or `Request<Body>` as the handler arg (loses the ergonomic extractors). The simpler path: have the sig-verify middleware **also** stash the sender as a separate `Extension(Arc<Principal>)` so the handler can use the standard `Extension` extractor. Planner picks the cleaner shape.

### Pattern 4: `HttpTransport` impl shape

```rust
// Source: pattern derived from MemoryTransport (crates/famp-transport/src/memory.rs)
pub struct HttpTransport {
    addr_map: Arc<Mutex<HashMap<famp_core::Principal, url::Url>>>,
    inboxes:  Arc<Mutex<HashMap<famp_core::Principal, mpsc::Sender<famp_transport::TransportMessage>>>>,
    client:   reqwest::Client,
    server:   Option<tokio::task::JoinHandle<()>>,   // None for client-only role
}

impl famp_transport::Transport for HttpTransport {
    type Error = HttpTransportError;
    fn send(&self, msg: famp_transport::TransportMessage)
        -> impl Future<Output = Result<(), Self::Error>> + Send { /* lookup addr_map, reqwest POST */ }
    fn recv(&self, principal: &famp_core::Principal)
        -> impl Future<Output = Result<famp_transport::TransportMessage, Self::Error>> + Send { /* await mpsc receiver */ }
}

impl Drop for HttpTransport {
    fn drop(&mut self) {
        if let Some(h) = self.server.take() { h.abort(); }
    }
}
```

**Native AFIT** (no `async-trait` macro) per D-F5. The `impl Future + Send` return type matches Phase 3's `Transport` trait exactly.

### Pattern 5: Self-signed cert via `rcgen 0.14` (example binary only)

```rust
// Source: docs.rs/rcgen/latest/rcgen/fn.generate_simple_self_signed.html [VERIFIED]
use rcgen::{generate_simple_self_signed, CertifiedKey};

let subject_alt_names = vec!["localhost".to_string(), "127.0.0.1".to_string()];
let CertifiedKey { cert, signing_key } = generate_simple_self_signed(subject_alt_names)?;
let cert_pem = cert.pem();
let key_pem  = signing_key.serialize_pem();
std::fs::write(&out_cert_path, cert_pem)?;
std::fs::write(&out_key_path,  key_pem)?;
```

`generate_simple_self_signed` returns a `CertifiedKey { cert, signing_key }` struct in 0.14. The `signing_key.serialize_pem()` method is the PKCS#8 PEM serialization. **Planner must verify this exact 0.14 API on docs.rs at plan time** — rcgen renamed several types between 0.13 and 0.14 `[VERIFIED via WebSearch 2026-04-13]`.

### Pattern 6: Loading PEM cert/key for the rustls server config

```rust
// Source: pattern derived from rustls 0.23 + rustls-pemfile 2 docs
use std::{fs::File, io::BufReader};
use rustls::{ServerConfig, pki_types::{CertificateDer, PrivateKeyDer}};
use rustls_pemfile::{certs, pkcs8_private_keys};

fn load_cert(path: &Path) -> std::io::Result<Vec<CertificateDer<'static>>> {
    let mut rd = BufReader::new(File::open(path)?);
    certs(&mut rd).collect()
}

fn load_key(path: &Path) -> std::io::Result<PrivateKeyDer<'static>> {
    let mut rd = BufReader::new(File::open(path)?);
    let mut keys = pkcs8_private_keys(&mut rd).collect::<Result<Vec<_>, _>>()?;
    Ok(PrivateKeyDer::Pkcs8(keys.remove(0)))
}

fn build_server_config(cert: Vec<CertificateDer<'static>>, key: PrivateKeyDer<'static>) -> ServerConfig {
    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert, key)
        .expect("invalid cert/key pair")
}
```

`[ASSUMED — based on rustls 0.23 + rustls-pemfile 2 typical API; planner MUST verify exact iterator/Result shape at plan time]`

### Pattern 7: Adding `--trust-cert` to the client side root store on top of platform verifier

```rust
// Source: derived from rustls-platform-verifier 0.5 + rustls 0.23 docs [ASSUMED]
use rustls::{ClientConfig, RootCertStore};
use rustls_platform_verifier::ConfigVerifierExt;

fn build_client_config(extra_trust: Option<&Path>) -> ClientConfig {
    let mut roots = RootCertStore::empty();
    if let Some(path) = extra_trust {
        let certs = load_cert(path).expect("load extra trust cert");
        for c in certs { roots.add(c).expect("add cert"); }
    }
    // rustls-platform-verifier integration: either extend with platform roots,
    // or use ClientConfig::with_platform_verifier() and add an extra root via a custom
    // RootCertStore wrapper. EXACT API SHAPE TO BE VERIFIED AT PLAN TIME.
    ClientConfig::builder()
        .with_root_certificates(roots)   // adds explicit trust
        .with_no_client_auth()
}
```

**This is the single most under-specified path in the research.** `rustls-platform-verifier` 0.5 exposes a `ConfigVerifierExt::with_platform_verifier()` shortcut that builds a `ClientConfig` using ONLY the OS root store. Combining "OS root store + one extra explicit anchor" is supported but the API shape is not obvious — the planner must read `rustls-platform-verifier` 0.5 docs end-to-end and prototype this before locking the plan. **Simpler fallback if the combination is awkward:** for the dev example, use `rustls`'s plain `ClientConfig` with `with_root_certificates(roots)` containing ONLY the explicit `--trust-cert` (skip platform verifier entirely for the example, document that production would use `with_platform_verifier`). This is a one-line tradeoff and sidesteps the API-combination question.

### Anti-Patterns to Avoid

- **Don't use `axum::middleware::from_fn` without `from_fn_with_state` if you need the keyring.** `from_fn` cannot capture state cleanly; `from_fn_with_state` is the correct primitive.
- **Don't apply `RequestBodyLimitLayer` via `.route_layer()`.** `route_layer` runs *after* route matching, which means a malformed `:principal` could reach the body-read step with no limit. Use `.layer()` on the top-level `Router` so the limit applies before any routing logic.
- **Don't read the body in middleware without the body-limit layer outside it.** Without the cap, a hostile client can pin server memory by sending a multi-GB body. The CONTEXT.md D-C1 ordering (limit outer, sig-verify inner) is non-negotiable.
- **Don't put `default-features = true` on `reqwest`.** It silently re-enables `default-tls`, which can pull `native-tls` transitively. `default-features = false` is mandatory `[VERIFIED]`.
- **Don't use `async-trait` macro.** Phase 3 D-C5 set the precedent for native AFIT; do not regress.
- **Don't widen `famp-transport-http` with a `test-util` feature flag.** Phase 3 D-D6 needed it for `MemoryTransport::send_raw_for_test` because there was no other injection surface; HTTP already exposes the raw injection surface (`reqwest::Client::post(...).body(adversarial_bytes)`) without needing any crate-level widening. D-D2 is explicit on this.
- **Don't run the FSM step or recipient cross-check in the middleware.** D-C2: middleware is fast-reject only. Runtime glue at `crates/famp/src/runtime/` owns the full pipeline. Re-decode in the runtime is intentional double-checking.
- **Don't introduce a custom `dyn` rustls verifier for self-signed cert handling.** D-B5: trust the cert the boring way (add it to a `RootCertStore`).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP server routing | Custom routing on raw `hyper` | `axum 0.8` + `Router` | axum is the thin layer that makes hyper ergonomic; reinventing routing has no upside |
| Tower middleware framework | Custom `Service` trait | `tower::Layer` + `tower::Service` | `tower` IS the framework; axum is built on it |
| Body size limiting | Manual byte-counting wrapper | `tower_http::limit::RequestBodyLimitLayer` | One-line correct solution; spec §18 |
| TLS stack | Anything | `rustls 0.23` | Banned alternatives per D-B8 |
| Self-signed cert generation | Shelling out to `openssl` | `rcgen 0.14` | Pure Rust, no FFI, deterministic in tests |
| PEM file parsing | Custom parser | `rustls-pemfile 2` | Standard pair with rustls 0.23 |
| URL parsing for `--addr` | Hand-rolled `&str` splitter | `url::Url::parse` | Edge cases (port, scheme, IPv6 brackets) |
| HTTP client | Raw hyper-util client | `reqwest 0.13` | Connection pooling, redirects, timeouts |
| Async runtime spawning | `std::thread` | `tokio::spawn` + `JoinHandle` | Project-wide pin |
| Adversarial transport injection on HTTP | New `test-util` feature on `famp-transport-http` | Raw `reqwest::Client::post` in test code | Raw HTTP is already the injection surface; D-D2 |

**Key insight:** Phase 4 should add **ZERO** novel infrastructure. Every Phase 4 component is either (a) a thin wrapper over a well-known crate (axum, reqwest, rustls), or (b) a re-use of Phase 3 infrastructure (`Transport` trait, `Keyring`, runtime glue, adversarial harness). If the planner finds themselves designing a new abstraction, they have probably wandered into v0.8+ scope.

## Common Pitfalls

### Pitfall 1: Tower middleware that consumes the request body breaks downstream extractors

**What goes wrong:** A middleware that calls `to_bytes(req.into_body(), N).await` consumes the body. The handler then receives an empty body and either errors or sees no payload.

**Why it happens:** `axum::body::Body` is a stream; reading it once exhausts it.

**How to avoid:** After reading the body in the middleware, reconstruct the request: `let req = Request::from_parts(parts, Body::from(bytes));` and pass that to `inner.call(req)`. The Pattern 2 sketch above does this.

**Warning signs:** Handler receives empty `Bytes`; tests that worked without middleware fail with "missing body" once middleware is added.

`[CITED: github.com/tokio-rs/axum/issues/1110 — middleware that change the request body have poor usability]`

### Pitfall 2: `tower::ServiceBuilder` layer order is "first-written = outermost"

**What goes wrong:** Planner writes layers in the order they expect them to execute (sig-verify first, then body limit), and ends up with body-limit running after sig-verify has already read the unbounded body — security-critical inversion.

**Why it happens:** `ServiceBuilder` does NOT compose layers in execution order. The first layer added is the outermost (request enters it first; response leaves it last).

**How to avoid:** Always write layers in REQUEST FLOW ORDER (outer → inner). For Phase 4: `RequestBodyLimitLayer` first, `FampSigVerifyLayer` second.

**Warning signs:** A test that POSTs a >1 MB body succeeds in being sig-verified (which means it was fully read) before being rejected.

`[CITED: docs.rs/tower/latest/tower/builder/struct.ServiceBuilder.html]`

### Pitfall 3: `famp-envelope`'s `decode` does not take a `Keyring` — sig-verify needs a two-phase shape

**What goes wrong:** The middleware sketch in Pattern 2 calls `AnySignedEnvelope::decode(&bytes)` and then separately looks up the key. But Phase 3's runtime glue uses a **two-phase decode**: (1) `peek_sender` extracts the `from` field, (2) `keyring.get(sender)` returns the pinned key, (3) `AnySignedEnvelope::decode` is called *with the pinned key in scope*. The middleware must mirror this exact pattern, or it must use a `decode_then_verify_against` API if `famp-envelope` exposes one.

**Why it happens:** Phase 3 D-D3 / Plan 03-03 D-D5 designed the runtime glue this way specifically to avoid "verify against any key from the keyring" semantics. The middleware MUST follow the same shape — otherwise an attacker with ANY pinned key in the keyring can sign for ANY sender.

**How to avoid:**
1. Read `crates/famp/src/runtime/peek.rs` and `crates/famp/src/runtime/loop_fn.rs` BEFORE writing the middleware. They contain the canonical two-phase decode pattern.
2. The middleware should call `peek_sender` (or its equivalent) first, look up the key, THEN run `AnySignedEnvelope::decode` (which internally calls `verify_strict` against the supplied key).
3. If `famp-envelope::AnySignedEnvelope::decode` does NOT take a key parameter, the middleware needs a sister API or must call `verify_strict` separately after decode. Planner verifies the exact API shape against `crates/famp-envelope/src/` before writing the layer.
4. Alternative: the middleware can re-use `crates/famp/src/runtime/peek_sender` directly (cross-crate import — since `famp-transport-http` does NOT depend on `crates/famp`, this requires either pulling `peek_sender` into a lower crate OR re-implementing the same shape in `famp-transport-http`). The cleanest fix: lift `peek_sender` into `famp-envelope` (one-line layering change) and call it from both the runtime and the middleware. **Planner explicitly resolves this layering question in the plan** — it is the single most likely source of an architectural deviation.

**Warning signs:** A test where alice's key is pinned, bob's key is pinned, and alice signs an envelope claiming `from: bob` — the middleware accepts it. CONF-06 row on HTTP would catch this only if the test correctly uses bob's private key with alice's `from` field; planner ensures the adversarial test exercises this exact mis-binding case.

`[ASSUMED — exact `famp-envelope::AnySignedEnvelope::decode` signature requires reading the Phase 1 source at plan time]`

### Pitfall 4: `rustls-platform-verifier` extra anchor combination is non-obvious

**What goes wrong:** Planner wants "OS root store PLUS one explicit `--trust-cert`" and finds that `ConfigVerifierExt::with_platform_verifier()` produces a `ClientConfig` with no obvious knob to add an extra `RootCertStore` entry.

**How to avoid:** Two acceptable resolutions:
1. **Build a custom `RootCertStore` containing OS roots + the extra cert, and feed it to `ClientConfig::builder().with_root_certificates(roots)`.** Requires extracting OS roots manually (`rustls-native-certs` is the historical answer, but it's deprecated in favor of `rustls-platform-verifier`).
2. **For the dev example, skip the platform verifier entirely.** Build a `ClientConfig` from a `RootCertStore` containing ONLY the `--trust-cert`. Document that production would use the platform verifier. This is fine for v0.7 because the example only ever connects to the explicitly-trusted self-signed peer. Recommend option 2 unless the planner finds a clean docs-attested path for option 1.

**Warning signs:** Planner spends >2 hours on `rustls-platform-verifier` API spelunking. Cut to option 2.

`[ASSUMED — verify on docs.rs/rustls-platform-verifier/0.5 at plan time]`

### Pitfall 5: `tokio::task::JoinHandle::abort()` is not a graceful shutdown

**What goes wrong:** `Drop for HttpTransport` calls `handle.abort()`, which forces the axum server task to terminate immediately. In-flight requests may be cut off; the OS port may not release immediately.

**How to avoid:** For Phase 4's example + tests this is acceptable (no in-flight requests at shutdown). If graceful shutdown matters for a future test, use `axum::serve(listener, app).with_graceful_shutdown(shutdown_signal)` and signal via a `tokio::sync::oneshot`. **Phase 4 starts with `abort()` and the planner can upgrade if a test reveals flakiness.**

### Pitfall 6: Subprocess integration test port collisions on CI

**What goes wrong:** Hard-coded port `8443` collides with another test or another CI runner.

**How to avoid:** Use ephemeral ports. Pass `--listen 127.0.0.1:0`, parse the actual bound port from the spawned subprocess's stdout (bob prints `LISTENING http://127.0.0.1:<port>` once `axum::serve` is bound). Alice connects to that port. This is D-E6's plan and matches Phase 3 Plan 03-04's subprocess-test precedent.

**Warning signs:** Local tests pass; CI fails intermittently with "address already in use".

### Pitfall 7: `serde_json` for envelope parsing breaks canonicalization

**What goes wrong:** Planner reaches for `serde_json::from_slice` to peek at the envelope in the middleware. This bypasses `famp-canonical::from_slice_strict`, which means duplicate-key rejection (Phase 3 Pitfall 4) does not run.

**How to avoid:** Always use `famp_canonical::from_slice_strict` for any envelope parsing in the middleware. `serde_json` is allowed ONLY for the error-body JSON output (D-C5). Phase 3 Plan 03-03 set this precedent in `peek.rs`.

## Code Examples

### Example 1: Posting an envelope from `HttpTransport::send`

```rust
// Source: pattern derived from reqwest 0.13 docs
async fn send_impl(
    client: &reqwest::Client,
    url: &url::Url,
    msg: famp_transport::TransportMessage,
) -> Result<(), HttpTransportError> {
    let inbox_url = url.join(&format!("famp/v0.5.1/inbox/{}", msg.recipient))?;
    let resp = client
        .post(inbox_url)
        .header("content-type", "application/famp+json")
        .body(msg.bytes)
        .send()
        .await
        .map_err(HttpTransportError::ReqwestFailed)?;
    let status = resp.status();
    if status == reqwest::StatusCode::ACCEPTED {
        Ok(())
    } else {
        let body = resp.text().await.unwrap_or_default();
        Err(HttpTransportError::ServerStatus { code: status.as_u16(), body })
    }
}
```

### Example 2: `MiddlewareError` → `IntoResponse`

```rust
// Source: pattern derived from axum response docs
use axum::{http::StatusCode, response::{IntoResponse, Json, Response}};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum MiddlewareError {
    #[error("body too large")] BodyTooLarge,
    #[error("bad principal in path")] BadPrincipal,
    #[error("bad envelope")] BadEnvelope,
    #[error("canonical divergence")] CanonicalDivergence,
    #[error("unknown sender")] UnknownSender,
    #[error("signature invalid")] SignatureInvalid,
    #[error("unknown recipient")] UnknownRecipient,
    #[error("internal")] Internal,
}

#[derive(Serialize)]
struct ErrorBody { error: &'static str, detail: String }

impl IntoResponse for MiddlewareError {
    fn into_response(self) -> Response {
        let (code, slug) = match self {
            Self::BodyTooLarge        => (StatusCode::PAYLOAD_TOO_LARGE, "body_too_large"),
            Self::BadPrincipal        => (StatusCode::BAD_REQUEST,        "bad_principal"),
            Self::BadEnvelope         => (StatusCode::BAD_REQUEST,        "bad_envelope"),
            Self::CanonicalDivergence => (StatusCode::BAD_REQUEST,        "canonical_divergence"),
            Self::UnknownSender       => (StatusCode::UNAUTHORIZED,       "unknown_sender"),
            Self::SignatureInvalid    => (StatusCode::UNAUTHORIZED,       "signature_invalid"),
            Self::UnknownRecipient    => (StatusCode::NOT_FOUND,          "unknown_recipient"),
            Self::Internal            => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };
        let body = ErrorBody { error: slug, detail: self.to_string() };
        (code, Json(body)).into_response()
    }
}
```

This satisfies D-C5/C6/C7 in one block.

### Example 3: Adversarial harness shape (Phase 4 D-D2/D3)

```rust
// crates/famp/tests/adversarial/mod.rs
use famp::runtime::RuntimeError;
use famp_core::Principal;

pub enum Case { Unsigned, WrongKey, CanonicalDivergence }

pub trait AdversarialTransport {
    async fn inject_raw(&self, sender: &Principal, recipient: &Principal, bytes: &[u8]);
    async fn run_loop_and_catch(&self, as_principal: &Principal) -> RuntimeError;
}

pub fn case_bytes(case: Case) -> Vec<u8> { /* loads from fixtures.rs */ }
pub fn assert_expected_error(case: Case, err: &RuntimeError) { /* match on variant */ }

// crates/famp/tests/adversarial/memory.rs
// Phase 3 tests moved here, adapting to the AdversarialTransport trait.

// crates/famp/tests/adversarial/http.rs
// New: spawns HttpTransport, uses raw reqwest::Client to POST adversarial bytes,
// asserts middleware rejects + handler sentinel stays false.

// crates/famp/tests/adversarial.rs (entry file)
mod adversarial { mod mod_; mod memory; mod http; mod fixtures; }
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `rustls-native-certs` for OS roots | `rustls-platform-verifier` | rustls 0.23 era | Replaces the deprecated dance; CLAUDE.md committed to this |
| `actix-web` actor model | `axum` extractor + tower middleware | 2023+ ecosystem shift | axum is the de-facto default |
| `async-trait` macro for trait async fn | Native AFIT (`impl Future + Send`) | Rust 1.75 stable | No macro overhead, cleaner errors |
| `reqwest 0.11` (hyper 0.14) | `reqwest 0.13` (hyper 1.x) | reqwest 0.13 release | Defaults to rustls; do not mix with reqwest 0.11 tutorials |
| `axum 0.7` route param `:name` | `axum 0.8` route param `{name}` | axum 0.8 release | Planner MUST use brace syntax in route literals |
| `RootCertStore::add_parsable_certificates` | `RootCertStore::add(CertificateDer)` | rustls 0.23 | Tutorials older than ~2024 use the wrong API |

**Deprecated/outdated:**
- `rustls-native-certs` direct use — superseded by `rustls-platform-verifier`.
- `tokio::main` → `axum::Server::bind(...).serve(...)` — replaced by `axum::serve(listener, app).await` in axum 0.7+.
- Pre-0.21 `base64::encode_config` — Phase 4 doesn't use base64 directly; mentioned for symmetry with CLAUDE.md table.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | axum 0.8 uses `{principal}` route literal syntax (not `:principal`) | Pattern 1 + Pattern 3 | Compile error on first build; one-line fix |
| A2 | `famp_envelope::AnySignedEnvelope::decode` does NOT take a `Keyring`/key argument; sig verification is done via `verify_strict` separately | Pattern 2 + Pitfall 3 | Middleware shape changes; layering question between `famp-envelope` and `famp-transport-http` may need a one-line lift of `peek_sender` into a shared crate |
| A3 | `rustls-platform-verifier 0.5` does not expose a single-call "OS roots PLUS extra anchor" API; combining requires manual `RootCertStore` construction | Pitfall 4 | Forces fallback option 2 (explicit roots only for example); documented as acceptable |
| A4 | `tower::ServiceBuilder` applies layers first-written = outermost | Pattern 1 + Pitfall 2 | Critical security inversion if wrong; planner verifies on docs.rs at plan time |
| A5 | `reqwest 0.13.2` feature flag `rustls-tls-native-roots` exists and pulls the rustls-platform-verifier backend (not the deprecated rustls-native-certs path) | Standard Stack | Wrong feature name → compile error; planner verifies with `cargo add reqwest --dry-run` |
| A6 | `rcgen 0.14`'s `generate_simple_self_signed` returns `CertifiedKey { cert, signing_key }` and `signing_key.serialize_pem()` produces a PKCS#8 PEM `[VERIFIED via WebSearch]` | Pattern 5 | None — verified |
| A7 | `rustls-pemfile 2.x` exposes `certs(reader) -> impl Iterator<Item = Result<CertificateDer, _>>` and `pkcs8_private_keys(reader) -> impl Iterator<Item = Result<PrivatePkcs8KeyDer, _>>` | Pattern 6 | One-line API tweak; planner verifies on docs.rs |
| A8 | Phase 3's `crates/famp/src/runtime/peek.rs::peek_sender` can be lifted/re-used from the middleware OR cleanly re-implemented; no architectural rewrite needed | Pitfall 3 | Worst case: a small layering change (lift `peek_sender` into `famp-envelope`); planner explicitly resolves this in the plan |
| A9 | `axum::middleware::from_fn_with_state` is sufficient for `FampSigVerifyLayer` (avoiding ~50 LoC of hand-written `Service` boilerplate) — provided extension stashing works through it | Pattern 2 | Falls back to hand-written `Layer` + `Service`; both are documented |
| A10 | `cargo tree -i openssl` returns non-zero exit when `openssl` is NOT in the dep graph (so it can be flipped to a "must succeed = openssl present = fail" CI gate) | D-F4 | Planner verifies CI snippet works as expected; trivial to fix |

**Empty? No — A2 / A3 / A8 are the three meaningful ones.** The planner MUST resolve A2 and A8 by reading the Phase 1 envelope source before writing the middleware, and SHOULD prototype A3 before locking the cert-trust path.

## Open Questions

1. **`famp-envelope`'s decode-with-key API shape** (A2 + A8 + Pitfall 3)
   - What we know: Phase 3 runtime glue uses a two-phase decode (`peek_sender` → `keyring.get` → `AnySignedEnvelope::decode`). The runtime glue lives in `crates/famp/src/runtime/`, not in any of the lower crates.
   - What's unclear: Whether `famp-transport-http`'s sig-verify middleware should (a) pull `peek_sender` from a shared crate, (b) call into `crates/famp/src/runtime/` directly (requires inverting a dependency — `famp-transport-http` does NOT depend on `crates/famp`), or (c) re-implement the same shape inline.
   - Recommendation: **Lift `peek_sender` from `crates/famp/src/runtime/peek.rs` into `famp-envelope`** (or into a thin new sibling). It is a pure function over bytes; it has no FSM coupling; and both the runtime and the middleware need it. This is a one-file move with `pub use` re-exports and is the cleanest layering. Planner makes the call in the plan.

2. **`rustls-platform-verifier 0.5` + extra root anchor combination** (A3 + Pitfall 4)
   - What we know: Both APIs exist in isolation. The combination is not documented in a single example.
   - What's unclear: Whether the cleanest path is `ConfigVerifierExt::with_platform_verifier()` + post-construction anchor injection, or a manual `RootCertStore` + manual platform-roots load.
   - Recommendation: Spend 30 minutes prototyping option 1; if it doesn't yield in that time, ship option 2 (explicit-trust-only `ClientConfig` for the example) and document the production upgrade path inline.

3. **Whether the `http_happy_path.rs` same-process fallback ships alongside the subprocess test** (D-E7)
   - What we know: D-E7 marks it Claude's discretion with a recommendation to ship both for CI stability.
   - What's unclear: Whether the subprocess test will be reliable enough on the local CI to not need a fallback.
   - Recommendation: **Ship both.** The cost is low (one extra test file), and the same-process test catches axum/tower/middleware bugs even if the subprocess infrastructure is broken — they are testing different layers.

4. **Whether to ship `peek_sender` lift as a Phase 4 plan or as a Phase 3 retroactive fixup** (intersects Q1)
   - What we know: Phase 3 is closed.
   - Recommendation: Phase 4 plan owns the lift. Document it as a deliberate layering improvement, not a Phase 3 bug.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain (cargo, rustc, clippy, rustfmt) | All build/test commands | ⚠️ Check before plan execution | rust-toolchain.toml pins `1.87+` | None — phase blocks without it. Phase 3 plans noted the executor's sandbox lacked cargo; orchestrator must run verification on a toolchain-equipped runner. |
| `cargo-nextest` | Workspace test runner | ⚠️ Verify on plan-execution machine | `0.9.132+` per CLAUDE.md | `cargo test` works as a fallback but is slower and lacks process isolation |
| `just` | Task runner | Optional | latest | `cargo` commands directly |
| Network access to crates.io | First build of `famp-transport-http` (resolving axum, reqwest, rustls, tower, etc.) | Required at plan execution | n/a | None — first build resolves a tree of ~50+ new crates |
| Open localhost ports for subprocess test | `cross_machine_happy_path.rs` | Required on CI runner | ephemeral | Use `--listen 127.0.0.1:0` (D-E6) to dodge collisions |
| Filesystem write access for tempdirs | Subprocess test cert/key exchange | Required | n/a | None — `tempfile` crate is the standard answer |

**Missing dependencies with no fallback:**
- Rust toolchain on the agent that executes the plan tasks. Phase 3 Plans 03-01 and 03-02 both flagged that the executor sandbox lacked `cargo`. Plan 03-03 ran on a cargo-equipped worktree. **The plan author should explicitly confirm with the orchestrator that the executor has cargo before plan execution.**

**Missing dependencies with fallback:**
- None significant.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo nextest` (workspace pinned to `0.9.132+` in CLAUDE.md) + `proptest 1.11` for property tests where applicable |
| Config file | Workspace `Cargo.toml` + per-crate `Cargo.toml` `[dev-dependencies]`. No nextest-specific config beyond the default. |
| Quick run command | `cargo nextest run -p famp-transport-http` (single-crate scope) |
| Full suite command | `cargo nextest run --workspace` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TRANS-03 | `famp-transport-http` crate compiles + `HttpTransport` implements `Transport` | unit (compile-time) | `cargo check -p famp-transport-http` | ❌ Wave 0: fill in `Cargo.toml` deps + `src/lib.rs` skeleton |
| TRANS-04 | `POST /famp/v0.5.1/inbox/{principal}` endpoint reachable | integration (in-process axum + reqwest) | `cargo nextest run -p famp-transport-http --test routing` | ❌ Wave 0: new test file `crates/famp-transport-http/tests/routing.rs` |
| TRANS-06 | rustls-only TLS — `cargo tree -i openssl` empty | CI gate (bash) | `! cargo tree -i openssl` | ❌ Wave 0: add to CI workflow |
| TRANS-07 | 1 MB body limit returns 413 | integration | `cargo nextest run -p famp-transport-http --test body_limit` | ❌ Wave 0: new test file |
| TRANS-09 | Unsigned/wrong-key requests rejected before handler invoked | integration with `Arc<AtomicBool>` sentinel | `cargo nextest run -p famp --test adversarial::http` | ❌ Wave 0: new test file (D-D5 sentinel mechanism) |
| EX-02 | `cross_machine_two_agents` example builds + runs | example build + subprocess integration test | `cargo run -p famp --example cross_machine_two_agents -- --role bob` (manual) + `cargo nextest run -p famp --test cross_machine_happy_path` (CI) | ❌ Wave 0: new example + new test file |
| CONF-04 | Happy-path `request → commit → deliver → ack` over real HTTPS | integration (subprocess) | `cargo nextest run -p famp --test cross_machine_happy_path` | ❌ Wave 0: subprocess test + optional `http_happy_path.rs` same-process fallback |
| CONF-05 (HTTP row) | Unsigned envelope rejected by middleware → `Decode(MissingSignature)` runtime error if reaching client; sentinel false on server | integration | `cargo nextest run -p famp --test adversarial::http::conf_05` | ❌ Wave 0: new test file inside `crates/famp/tests/adversarial/http.rs` |
| CONF-06 (HTTP row) | Wrong-key signature → `Decode(SignatureInvalid)`; sentinel false | integration | `cargo nextest run -p famp --test adversarial::http::conf_06` | ❌ Wave 0 |
| CONF-07 (HTTP row) | Canonical divergence (reuses Phase 3 fixture byte-identically) → `CanonicalDivergence`; sentinel false | integration | `cargo nextest run -p famp --test adversarial::http::conf_07` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo check -p famp-transport-http && cargo clippy -p famp-transport-http --all-targets -- -D warnings && cargo nextest run -p famp-transport-http`
- **Per wave merge:** `cargo nextest run --workspace && cargo tree -i openssl` (the latter must produce no output)
- **Phase gate:** `just ci` (full workspace check + clippy + nextest + cargo audit) green before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `crates/famp-transport-http/Cargo.toml` — fill in `[dependencies]` and (no `[dev-dependencies]` mirror of `test-util` per D-D2)
- [ ] `crates/famp-transport-http/src/lib.rs` — skeleton with module declarations
- [ ] `crates/famp-transport-http/src/error.rs` — `MiddlewareError` + `HttpTransportError`
- [ ] `crates/famp-transport-http/src/transport.rs` — `HttpTransport` struct + `Transport` impl
- [ ] `crates/famp-transport-http/src/server.rs` — `build_router` + handler
- [ ] `crates/famp-transport-http/src/middleware.rs` — `FampSigVerifyLayer` (or `from_fn_with_state` closure)
- [ ] `crates/famp-transport-http/src/tls.rs` — PEM loaders + `ServerConfig`/`ClientConfig` builders
- [ ] `crates/famp-transport-http/tests/routing.rs` — TRANS-04 in-process integration
- [ ] `crates/famp-transport-http/tests/body_limit.rs` — TRANS-07 413 test
- [ ] `crates/famp/examples/cross_machine_two_agents.rs` — EX-02 binary
- [ ] `crates/famp/tests/cross_machine_happy_path.rs` — CONF-04 subprocess test
- [ ] `crates/famp/tests/http_happy_path.rs` — CONF-04 same-process safety net (recommended per Q3)
- [ ] `crates/famp/tests/adversarial/{mod.rs,memory.rs,http.rs,fixtures.rs}` — promoted directory module + HTTP rows (D-D1)
- [ ] `crates/famp/tests/fixtures/cross_machine/{alice,bob}.{crt,key}` — committed fixture certs (D-B7)
- [ ] CI workflow update: add `cargo tree -i openssl` gate (D-F4)
- [ ] `crates/famp/Cargo.toml` — add `famp-transport-http` dep + `rcgen` dev-dep (D-F3)

## Project Constraints (from CLAUDE.md)

- **Tech stack pre-locked.** All crate choices and version pins for Phase 4 are listed in CLAUDE.md's TL;DR table. Planner validates against live crates.io but does not introduce new crates without explicit justification.
- **`verify_strict`-only public surface.** No raw `verify` anywhere — middleware MUST go through `famp-crypto`'s public API (which calls `verify_strict` internally per the v0.6 decision).
- **Domain separation prefix prepended internally.** Middleware MUST NOT assemble the signing input by hand; `canonicalize_for_signature` is the only sanctioned path. The middleware's responsibility is to call `famp-envelope` decode + `famp-crypto::verify_strict`, not to re-implement canonicalization.
- **No `openssl`, no `native-tls`, ever.** Enforced via `cargo tree -i openssl` CI gate (D-F4).
- **No `--no-verify` to bypass pre-commit hooks.** Standard project-wide rule.
- **Conventional commits** with multi-paragraph body.
- **Narrow phase-local error enums.** `MiddlewareError` and `HttpTransportError` are deliberately separate (D-C7/C8). Do NOT collapse into a single enum, do NOT promote to `ProtocolErrorKind`.
- **Owned types at crate boundaries.** `TransportMessage` keeps `Vec<u8>` ownership through the handler into the inbox channel.
- **`#[forbid(unsafe_code)]`** at the crate root (project-wide pattern from Phase 3 keyring + transport).
- **Workspace `unused_crate_dependencies = "warn"` lint** is promoted to error by `-D warnings`. New deps that are only reached from a binary/test compile unit need `use crate_name as _;` silencing in `lib.rs` (Phase 3 precedent).
- **GSD workflow enforcement.** All edits must flow through GSD commands (already in motion via this research phase).
- **Spec fidelity.** `FAMP-v0.5.1-spec.md §7.1, §14.3 INV-10, §18, §5.1, §5.2, §7.3a` are the authoritative references for HTTP binding rules. Phase 4 does not interpret the spec; it implements the decisions in CONTEXT.md, which already maps to spec sections.

## Sources

### Primary (HIGH confidence)
- `CLAUDE.md` — project tech-stack TL;DR (axum 0.8.8, reqwest 0.13.2, rustls 0.23.38, tower, tower-http, rcgen, rustls-platform-verifier, rustls-pemfile, tokio 1.51.x, thiserror 2.0.x). Hand-pinned 2026-04-12 against live crates.io.
- `04-CONTEXT.md` — load-bearing for every locked decision (D-A1..D-F5). Planner copies these decisions verbatim.
- `.planning/REQUIREMENTS.md` — TRANS-03/04/06/07/09, EX-02, CONF-04, plus Phase-3-owned CONF-05/06/07.
- `.planning/ROADMAP.md` Phase 4 — 5 success criteria.
- `.planning/STATE.md` — Phase 3 closure status; runtime glue and `Transport` trait are stable inputs.
- `.planning/phases/03-*/03-{01,02,03,04}-SUMMARY.md` — verified Phase 3 deliverables: `famp-transport::Transport` + `MemoryTransport`, `famp-keyring::Keyring`, `crates/famp/src/runtime/{peek.rs, adapter.rs, loop_fn.rs, error.rs}`, `crates/famp/tests/adversarial.rs` (3 CONF tests), `crates/famp/tests/fixtures/conf-07-canonical-divergence.json`. All assumptions about reusable assets in CONTEXT.md `<code_context>` are confirmed by these summaries.
- `crates/famp-transport-http/Cargo.toml` (existing Phase-0 stub) — confirmed empty `[dependencies]`, ready for fill-in.

### Secondary (MEDIUM confidence — verified via WebSearch, recommend re-verifying on docs.rs at plan time)
- [reqwest v0.13 — rustls by default (seanmonstar.com)](https://seanmonstar.com/blog/reqwest-v013-rustls-default/) — confirms rustls is the default in 0.13; confirms `default-features = false` is mandatory to avoid `default-tls` cross-pollination.
- [Feature flags of Reqwest crate (lib.rs)](https://lib.rs/crates/reqwest/features) — feature flag inventory including `rustls-tls-native-roots`.
- [rcgen `generate_simple_self_signed` (docs.rs)](https://docs.rs/rcgen/latest/rcgen/fn.generate_simple_self_signed.html) — confirms 0.14 returns `CertifiedKey { cert, signing_key }` and the `signing_key.serialize_pem()` PKCS#8 path.
- [axum middleware index (docs.rs)](https://docs.rs/axum/latest/axum/middleware/index.html) — middleware composition primitives (`from_fn`, `from_fn_with_state`, `Router::layer`, `Router::route_layer`).
- [tower-http RequestBodyLimitLayer discussion (github)](https://github.com/tokio-rs/axum/discussions/2286) + [tower-http issue tracker on body-limit ordering](https://github.com/tokio-rs/axum/issues/2492) — confirms `RequestBodyLimitLayer` must be applied at the outermost level.
- [axum issue #1110 — middleware that change request body](https://github.com/tokio-rs/axum/issues/1110) — confirms Pitfall 1 (body-consume-then-reconstruct pattern).

### Tertiary (LOW confidence — flagged for plan-time verification)
- A2 / A8 — `famp-envelope` decode-with-key API shape: planner reads `crates/famp-envelope/src/` and `crates/famp/src/runtime/peek.rs` directly before writing the middleware.
- A3 — `rustls-platform-verifier 0.5` "OS roots + extra anchor" combination: prototype before locking; fall back to explicit-only roots if option 1 doesn't yield.
- A7 — exact `rustls-pemfile 2` iterator/Result shape.
- A1 — axum 0.8 `{name}` route literal syntax (very high prior, but confirm on docs.rs/axum/0.8.8/).

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — pre-locked in CLAUDE.md, three of nine versions cross-verified via WebSearch on research date
- Architecture: HIGH — every architectural decision is locked in CONTEXT.md; research role is to translate decisions to code patterns
- Pitfalls: HIGH — pitfalls are well-documented in axum/tower issue trackers; A2/A8 layering question is the only LOW-confidence gap and it's flagged
- Adversarial harness: HIGH — D-D1..D6 fully specify the shape; Phase 3 precedent in `crates/famp/tests/adversarial.rs` is byte-reusable
- TLS cert trust path: MEDIUM — A3 (`rustls-platform-verifier` + extra anchor) is the under-specified path; documented fallback exists
- Example binary: HIGH — D-E1..E7 specify role flag, symmetric topology, subprocess test, and same-process fallback

**Research date:** 2026-04-13
**Valid until:** 2026-05-13 (30 days — the FAMP Rust HTTP stack is stable; major axum/reqwest/rustls bumps are unlikely on this timescale, but planner re-runs `cargo search` on plan day regardless)

## RESEARCH COMPLETE

**Phase:** 4 — Minimal HTTP Transport + Cross-Machine Example
**Confidence:** HIGH

### Key Findings

1. **Phase 4 is filling in a pre-decided design, not exploring.** CLAUDE.md locks the tech stack; CONTEXT.md locks every architectural decision down to status code mappings, layer order, error enum split, and adversarial harness shape. Research role is translation, not investigation.
2. **The `famp-transport-http` crate already exists as an empty Phase-0 stub** at `crates/famp-transport-http/` with workspace lint inheritance wired. Phase 4 fills its `[dependencies]` and `src/`; it does NOT scaffold a new crate.
3. **The single under-specified architectural question is `peek_sender` layering** (Pitfall 3 + Open Question 1 + assumption A2/A8). The middleware needs the same two-phase decode pattern Phase 3 runtime uses, but `peek_sender` currently lives in `crates/famp/src/runtime/peek.rs` — a higher crate than `famp-transport-http`. Recommended resolution: lift `peek_sender` into `famp-envelope`. Planner explicitly resolves this in the plan.
4. **The `rustls-platform-verifier 0.5` "OS roots + one extra anchor" API combination is non-obvious** (Pitfall 4 + Open Question 2). Documented fallback: ship the example with explicit-roots-only `ClientConfig`, document production upgrade path inline. Trivial code change to upgrade later.
5. **Adversarial parity is achieved by promoting `crates/famp/tests/adversarial.rs` to a directory module** with a shared `AdversarialTransport` trait + `Case` enum + byte-identical reuse of the Phase 3 CONF-07 fixture. Three cases × two transports = six rows from one set of case definitions. No `test-util` feature flag added to `famp-transport-http` (raw HTTP is already the injection surface — D-D2).

### File Created
`.planning/phases/04-minimal-http-transport-cross-machine-example/04-RESEARCH.md`

### Confidence Assessment
| Area | Level | Reason |
|------|-------|--------|
| Standard Stack | HIGH | Pre-locked in CLAUDE.md; three of nine pins cross-verified via WebSearch on research date |
| Architecture | HIGH | Every decision locked in CONTEXT.md D-A1..D-F5 |
| Pitfalls | HIGH | Well-documented in axum/tower issue trackers; one MEDIUM gap (A2/A8 layering) explicitly flagged |
| TLS trust path | MEDIUM | A3 `rustls-platform-verifier` + extra anchor combination is the only under-specified path; fallback documented |
| Adversarial harness | HIGH | D-D1..D6 fully specify shape; Phase 3 fixtures byte-reusable |
| Example binary | HIGH | D-E1..E7 fully specify role flag, symmetric topology, subprocess test, and same-process fallback |

### Open Questions
1. `famp-envelope` decode-with-key API shape — planner resolves by reading `crates/famp-envelope/src/` and `crates/famp/src/runtime/peek.rs` before writing the middleware (recommended: lift `peek_sender` into `famp-envelope`).
2. `rustls-platform-verifier 0.5` extra-anchor combination — planner prototypes for 30 minutes; falls back to explicit-only roots for the example if blocked.
3. Whether to ship `http_happy_path.rs` same-process fallback alongside the subprocess `cross_machine_happy_path.rs` — recommendation: ship both, low cost, catches different layers.
4. Should `peek_sender` lift be a Phase 4 plan task or a Phase 3 retroactive fixup — recommendation: Phase 4 owns it as a deliberate layering improvement.

### Ready for Planning
Research complete. Planner can now create PLAN.md files. Recommended plan structure (planner's call):

- **Plan 04-01:** `famp-transport-http` skeleton + deps + error enums + (lifted?) `peek_sender` layering decision. Compiles, no functional behavior yet. (TRANS-03 partial)
- **Plan 04-02:** Server side — `build_router`, handler, `FampSigVerifyLayer` (or `from_fn_with_state`), `RequestBodyLimitLayer`, in-process integration tests for routing + body limit + middleware reject. (TRANS-04, TRANS-07, TRANS-09 partial)
- **Plan 04-03:** `HttpTransport` `Transport` impl + client side reqwest wiring + `tls.rs` PEM loaders + `ClientConfig`/`ServerConfig` builders + `cargo tree -i openssl` CI gate. (TRANS-03 complete, TRANS-06)
- **Plan 04-04:** `cross_machine_two_agents` example + committed fixture certs + subprocess integration test + optional same-process safety-net test. (EX-02, CONF-04)
- **Plan 04-05:** Promote `crates/famp/tests/adversarial.rs` to directory module + shared `AdversarialTransport` trait + HTTP adapter + sentinel mechanism for handler-not-entered assertion. (TRANS-09 complete; CONF-05/06/07 HTTP rows)

The planner is welcome to merge / split these differently. The above is a recommendation, not a constraint.

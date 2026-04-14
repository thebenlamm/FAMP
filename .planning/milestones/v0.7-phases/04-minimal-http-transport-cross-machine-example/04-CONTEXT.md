# Phase 4: Minimal HTTP Transport + Cross-Machine Example — Context

**Gathered:** 2026-04-13
**Status:** Ready for research → planning

<domain>
## Phase Boundary

Two processes on two machines (or two shells) run the same signed `request → commit → deliver → ack` cycle over real HTTPS, with trust bootstrapped from the same Phase 3 TOFU keyring and a new sibling address map. The Phase 3 adversarial matrix (CONF-05/06/07) is extended to `HttpTransport` with the same typed errors — 3 cases × 2 transports = 6 rows, no new CONF-0x IDs. This phase ships:

1. **`famp-transport-http` (library crate)** — `HttpTransport` implementing the Phase 3 `Transport` trait unchanged. Internally: an `axum 0.8` server with one listener per process that multiplexes inboxes via `POST /famp/v0.5.1/inbox/:principal`, a `reqwest 0.13` client running on `rustls 0.23` with `rustls-platform-verifier` + explicit `--trust-cert` root, a tower body-size layer (1 MB, per §18), and a pre-routing signature-verification layer (TRANS-09) that stashes the decoded `SignedEnvelope` in `Request::extensions` so the handler does not re-decode.
2. **Address discovery (new CLI surface)** — `--addr <principal>=<https-url>` flag, parsed alongside Phase 3's `--peer` flag, owned by the client-side `HttpTransport` constructor. No Agent Card, no `.well-known`, no sibling address file (deferred).
3. **TLS trust (dev profile)** — server loads a local self-signed Ed25519/ECDSA cert from `--cert`/`--key` paths (or generates one via `rcgen` at startup for the example). Client trusts it explicitly via `--trust-cert <path>`. No SPKI pinning, no custom verifier. Committed fixture certs under `crates/famp/tests/fixtures/` are used for CI.
4. **`crates/famp/examples/cross_machine_two_agents.rs`** — one binary, two invocations, `--role alice|bob`. No auto-orchestration, no self-spawning. Alice sends request to Bob over real HTTPS; both ends exit 0 with a typed conversation trace.
5. **Adversarial harness promotion** — the existing `crates/famp/tests/adversarial.rs` is promoted to a `adversarial/` module with a generic transport-fixture adapter. The three Phase 3 cases run once per transport (Memory + Http) against the same assertions, from the same case definitions. The CONF-07 canonical-divergence fixture from Phase 3 D-D7 is reused byte-identically.

**Explicitly out of scope for Phase 4** (pointer to v0.8+): `.well-known` Agent Card distribution (TRANS-05), cancellation-safe spawn-channel send path (TRANS-08), pluggable `TrustStore` trait, SPKI cert pinning, dynamic peer discovery, multi-principal per-process fan-out beyond what the test rig exercises, HTTP/2, HTTP/3, QUIC, mTLS, OpenSSL, `native-tls`, token-streaming deliver, and any federation-grade identity machinery. Phase 4 does **not** revisit Phase 3's runtime glue — `crates/famp/src/runtime/` is consumed unchanged.

</domain>

<decisions>
## Implementation Decisions

### A. Server Topology + Inbox Wiring

- **D-A1:** **One axum listener per process**, not one-per-principal. Multiplexing by URL path is the operational model:
  ```
  POST /famp/v0.5.1/inbox/:principal
  ```
  Per-principal listeners were rejected — they buy nothing for personal v0.7, triple the port-management surface, and diverge from the MemoryTransport mental model where one in-process hub routes to many inboxes. Path-multiplexing is the clean mirror.

- **D-A2:** **Inbox routing table inside `HttpTransport`:**
  ```rust
  struct HttpTransport {
      addr_map: HashMap<Principal, Url>,               // client-side: where to send
      inboxes:  HashMap<Principal, mpsc::Sender<TransportMessage>>,  // server-side: where to deliver
      // + reqwest::Client, rustls config, server JoinHandle
  }
  ```
  The `mpsc::Sender` half is held by the axum handler; the `mpsc::Receiver` half is held by the runtime loop that called `Transport::recv(&principal).await`. Mirrors Phase 3 D-C4 exactly — zero shape change from `MemoryTransport`.

- **D-A3:** `POST /famp/v0.5.1/inbox/:principal` handler responsibilities, in order:
  1. Extract `:principal` path param and parse via `Principal::from_str`. Parse failure → 400 `{"error": "bad_principal"}`.
  2. Body-limit tower layer (earlier) has already capped the body at 1 MB.
  3. Sig-verification tower layer (earlier) has already decoded + verified the envelope and stashed `SignedEnvelope` in `Request::extensions`.
  4. Look up `:principal` in `inboxes`. If not registered → 404 `{"error": "unknown_recipient"}`.
  5. Forward the **raw body bytes** (not the extension-stashed struct) via `mpsc::Sender::send(TransportMessage { sender, recipient: :principal, bytes })`. The runtime loop on the other side of that channel re-runs its own decode + verify — **this is intentional**: the sig-verification middleware is a fast-reject gate, not a substitute for runtime glue's full pipeline (recipient cross-check, FSM step, etc.). Double-decode is cheap and keeps the layering honest.
  6. Return 202 `Accepted` with empty body on success.

- **D-A4:** **`Transport::recv(&principal)` semantics unchanged from Phase 3** — each principal-specific runtime loop in `crates/famp/src/runtime/` owns the receiver half and calls `transport.recv(&me).await`. The trait surface is identical on `MemoryTransport` and `HttpTransport`; the whole point of Phase 3 D-C1/C2/C3 is vindicated here. Phase 4 **does not** modify the `Transport` trait.

- **D-A5:** `HttpTransport::register(principal)` creates the mpsc channel pair for a principal and returns the `Receiver` to the caller, mirroring `MemoryTransport::register` from Phase 3. For a server-only role, the caller registers itself; for a client-only role, `register` is not called.

- **D-A6:** **Client-side `send`** path: `HttpTransport::send(msg)` looks up `msg.recipient` in `addr_map`, posts the raw bytes to `{url}/famp/v0.5.1/inbox/{msg.recipient}` via the shared `reqwest::Client`. `Content-Type: application/famp+json` (same MIME the spec §18 implies). Unknown recipient → `HttpTransportError::UnknownRecipient`.

### B. Address Discovery + TLS Cert Trust

- **D-B1:** **Address config stays separate from the keyring.** Identity and network location are different concerns; Phase 3 D-A1 already committed to "the binding is the keyring itself, not a type-equality claim." Phase 4 does not extend the keyring with `Url` — that would entangle trust and routing for zero gain.

- **D-B2:** **New CLI flag:** `--addr <principal>=<https-url>`. Syntax mirrors the Phase 3 `--peer` flag: `=` separator (principals contain `:`), repeated on the command line for multiple peers. Example:
  ```
  cross_machine_two_agents --role alice \
      --peer agent:local/bob=nWGxne_9WmC... \
      --addr agent:local/bob=https://127.0.0.1:8443 \
      --trust-cert ./bob.pem
  ```

- **D-B3:** **No sibling address file in Phase 4.** A `peers.toml` or similar is deferred — the two-peer example does not need it, and inventing a file format here just adds scope. If v0.8 needs file-based peer config, it lands alongside Agent Cards, not here.

- **D-B4:** **Address map ownership:** `HashMap<Principal, Url>` lives inside `HttpTransport` (client side). Server side does not consult it — it only knows which principal it was asked to deliver to via the `:principal` path segment.

- **D-B5:** **TLS cert provisioning — dev profile:**
  - **Server side:** loads a self-signed cert + private key from `--cert <path>` + `--key <path>` flags. If neither is provided, the example binary generates one at startup via `rcgen 0.14` and writes it to a tempdir (or a user-specified `--out-cert` path) for the peer to pick up. No PKI, no CA.
  - **Client side:** trusts the server cert explicitly via `--trust-cert <path>`. Under the hood, `rustls-platform-verifier` is used for the default root store AND the explicitly-trusted cert is added as an extra `RootCertStore` anchor. This is boring and debuggable.
  - **No SPKI pinning, no dev-only custom verifier.** `rustls`'s standard verification path (hostname + chain + expiry) runs unchanged. Self-signed certs are trusted the normal way — by adding them to the root store.

- **D-B6:** **CN / SAN for self-signed certs:** `subject_alt_names = ["localhost", "127.0.0.1"]` for the dev example. Cross-machine runs require the user to supply `--cert` generated against the server's real hostname — the example does not pretend to solve hostname management.

- **D-B7:** **Committed fixture certs for tests:** `crates/famp/tests/fixtures/cross_machine/alice.{crt,key}` and `bob.{crt,key}`, pre-generated with long expiry (e.g., 10 years from fixture creation). CI reuses them; the runnable example generates fresh ones at startup.

- **D-B8:** **No OpenSSL, no `native-tls`.** Confirmed from the project CLAUDE.md stack: `rustls 0.23.38` only. `reqwest` feature flags: `default-features = false`, features: `rustls-tls-native-roots-no-provider` or equivalent — planner/researcher picks the exact feature set against `reqwest 0.13.2`.

### C. Signature-Verification Middleware + Error Responses

- **D-C1:** **Tower layer order (outer → inner):**
  1. `tower_http::limit::RequestBodyLimitLayer::new(1_048_576)` — 1 MB hard cap (TRANS-07, spec §18). Rejects with 413 `{"error": "body_too_large"}` before any further processing.
  2. **`FampSigVerifyLayer`** (new in this phase) — reads the full body (already capped), decodes the envelope via `famp-envelope`, looks up the pinned key via `famp-keyring::Keyring::get`, calls `famp-crypto::verify_strict`. On success, stashes the decoded `SignedEnvelope` in `Request::extensions` and forwards. On failure, short-circuits with a typed error response.
  3. axum route dispatch — hits `POST /famp/v0.5.1/inbox/:principal`.
  4. Handler (D-A3) — extracts path principal, pushes bytes into the inbox channel.

  **Body-limit comes first.** The sig-verify layer would otherwise read unbounded bodies before rejecting.

- **D-C2:** **`FampSigVerifyLayer` has a narrow remit.** It enforces:
  - envelope decodes cleanly (INV-10 unsigned rejection surfaces here as a decode error),
  - signature verifies against the pinned key for `envelope.from()`.

  It does **NOT**:
  - run `cross_check_recipient` (D-D5 in Phase 3 — that stays in `crates/famp/src/runtime/`),
  - call `TaskFsm::step`,
  - touch the runtime error enum.

  Reasoning: runtime glue is the single source of truth for the full receive pipeline. The middleware is a fast-reject gate that keeps unsigned/wrong-key traffic from reaching the inbox channel at all. The runtime loop re-verifies when it processes the channel message — this double-check is deliberate and was a specific ask in discussion.

- **D-C3:** **Extension stashing — informational, not authoritative.** The verified `SignedEnvelope` is written to `Request::extensions` so downstream code *could* reuse it. In Phase 4 the handler does not use it (D-A3 pushes raw bytes); the extension is stashed to avoid forcing a redesign in Phase 5+ if a future tower layer wants the decoded form. Cost: one `Arc<SignedEnvelope>` per request. Cheap.

- **D-C4:** **Keyring injection:** `FampSigVerifyLayer::new(keyring: Arc<Keyring>)`. Keyring is shared, read-only, cheap-clone via `Arc`. No interior mutability on the hot path — TOFU pins are set up at binary startup only in Phase 4 (no live re-pinning under load).

- **D-C5:** **Rejection responses — plain HTTP status + small typed JSON body.** Not a signed FAMP ack. A signed ack requires the server to sign, which drags `famp-crypto` + server signing keys into the middleware — wrong complexity level for Phase 4. JSON body shape:
  ```json
  { "error": "<snake_case_code>", "detail": "<human string>" }
  ```

- **D-C6:** **Status code mapping:**
  | Failure | Code | `error` |
  |---|---|---|
  | Body > 1 MB | 413 | `body_too_large` |
  | Path principal unparseable | 400 | `bad_principal` |
  | Envelope decode fail (includes INV-10 unsigned) | 400 | `bad_envelope` |
  | Canonical divergence detected at decode | 400 | `canonical_divergence` |
  | Pinned key missing for sender | 401 | `unknown_sender` |
  | Signature verification failed | 401 | `signature_invalid` |
  | Unknown recipient (no inbox for :principal) | 404 | `unknown_recipient` |
  | Internal error (channel closed, etc.) | 500 | `internal` |

  The distinction between `bad_envelope` and `canonical_divergence` preserves the Phase 3 requirement that CONF-05/06/07 return distinguishable errors at the runtime layer **and** at the HTTP middleware layer.

- **D-C7:** **Phase-local error enum:** `famp_transport_http::MiddlewareError` (narrow, thiserror-derived). It converts to `(StatusCode, Json<ErrorBody>)` via `axum::response::IntoResponse`. It does **not** reuse Phase 3 `RuntimeError` — that enum belongs to `crates/famp/src/runtime/` and is the right type for in-process runtime outcomes, not HTTP responses. Mapping from middleware failure to runtime visibility happens only when the handler forwards bytes; runtime then returns its own typed error.

- **D-C8:** **`HttpTransportError` for the `Transport::Error` associated type:** narrow enum covering `UnknownRecipient { principal }`, `ReqwestFailed(#[source] reqwest::Error)`, `ServerStatus { code: u16, body: String }`, `InboxClosed { principal }`. Narrow and distinct from `MiddlewareError` — one describes client-side send failures, the other describes server-side reject responses.

### D. Adversarial Matrix Reuse (CONF-05/06/07 × HTTP)

- **D-D1:** **Promote `crates/famp/tests/adversarial.rs` to a directory module.** New layout:
  ```
  crates/famp/tests/
    adversarial/
      mod.rs               // shared case definitions + harness trait
      memory.rs            // MemoryTransport adapter + #[tokio::test]s (moved from Phase 3)
      http.rs              // HttpTransport adapter + #[tokio::test]s (new)
      fixtures.rs          // reused fixture byte loaders (CONF-07 divergence bytes)
    adversarial_lib.rs     // thin test-binary entry point if needed by nextest
  ```
  Planner picks the exact nextest-friendly layout (`adversarial.rs` as an entry file `mod`ing the directory is fine).

- **D-D2:** **Shared harness trait:**
  ```rust
  #[async_trait::async_trait]  // or plain AFIT — planner picks
  trait AdversarialTransport {
      async fn inject_raw(&self, sender: &Principal, recipient: &Principal, bytes: &[u8]);
      async fn run_loop_and_catch(&self, as_principal: &Principal) -> RuntimeError;
  }
  ```
  `memory.rs` implements this via the Phase 3 D-D6 `send_raw_for_test` feature. `http.rs` implements it by constructing a raw `reqwest::Client` POST that bypasses `HttpTransport::send`'s well-formed path — **no `test-util` feature flag on `famp-transport-http`** because raw HTTP POSTs are already "anyone can inject" and do not require widening the crate's boundary. This is a meaningful difference from Phase 3 D-D6.

- **D-D3:** **The three cases are defined once:**
  ```rust
  enum Case { Unsigned, WrongKey, CanonicalDivergence }
  fn expected_error(case: Case) -> RuntimeErrorVariantTag { ... }
  fn case_bytes(case: Case) -> Vec<u8> { ... }
  ```
  Run once per transport. Three cases × two transports = six `#[tokio::test]` rows total. Assertions are byte-identical across transports: same expected `RuntimeError` variant, same "no panic" guarantee, same "handler closure not entered" guarantee for the HTTP case (TRANS-09 requires this — middleware rejects before the handler is invoked).

- **D-D4:** **Reuse the Phase 3 CONF-07 canonical-divergence fixture byte-identically.** `crates/famp/tests/fixtures/conf-07-canonical-divergence.json` must surface the same error over HTTP that it surfaces over MemoryTransport. If it does not, the canonicalization gate is broken — that is the whole point of reusing the bytes.

- **D-D5:** **"Handler closure not entered" assertion for HTTP (TRANS-09 SC#2).** Mechanism: the test inserts a `Arc<AtomicBool>` sentinel into the axum router via a handler wrapper; the sentinel flips to `true` only if the handler is actually invoked. For each of the three adversarial cases, the test asserts the sentinel is still `false` after the request returns — proving the middleware rejected before route dispatch. This is a direct verification of TRANS-09, and it lives in `http.rs`.

- **D-D6:** **Expected-error mapping table (runtime side):**
  | Case | MemoryTransport runtime error | HTTP client-side `HttpTransportError` | HTTP middleware `MiddlewareError` |
  |---|---|---|---|
  | Unsigned (CONF-05) | `RuntimeError::Decode(INV-10)` | `ServerStatus { 400, bad_envelope }` | `BadEnvelope` |
  | Wrong-key (CONF-06) | `RuntimeError::SignatureInvalid` | `ServerStatus { 401, signature_invalid }` | `SignatureInvalid` |
  | Canonical divergence (CONF-07) | `RuntimeError::CanonicalDivergence` | `ServerStatus { 400, canonical_divergence }` | `CanonicalDivergence` |

  Each row is distinguishable. No generic "bad message" collapse.

### E. `cross_machine_two_agents` Example (EX-02 / CONF-04)

- **D-E1:** **One binary, two invocations with fixed roles.** Rejected: auto-orchestrating a single process that spawns both peers. Ben's specific ask: "that tends to hide transport mistakes behind test harness convenience." Two processes = honest cross-machine cycle.

- **D-E2:** **Role flag:** `--role alice|bob`. Each role binds a fixed principal (`agent:local/alice`, `agent:local/bob`) and a default port (e.g., `:8443` for bob, `:8444` for alice). Override via `--listen <addr:port>`. The role flag is example-binary-only scaffolding — **not** a FAMP protocol concept.

- **D-E3:** **Cross-machine run sequence (user-driven, documented in the example):**
  1. On machine B: `cross_machine_two_agents --role bob --out-pubkey bob.pub --out-cert bob.pem` — generates bob's keypair + self-signed cert, prints pubkey and cert paths, starts axum server, waits for inbound `request`.
  2. User copies `bob.pub` and `bob.pem` to machine A.
  3. On machine A: `cross_machine_two_agents --role alice --peer agent:local/bob=$(cat bob.pub) --addr agent:local/bob=https://<bob-host>:8443 --trust-cert bob.pem` — generates alice's keypair, builds an `HttpTransport` client, sends signed `request` to bob, waits for `deliver` and `ack` over an alice-side inbox (alice also runs an axum listener, symmetric).
  4. Both processes print the typed trace and exit 0.

- **D-E4:** **Symmetric topology:** both alice and bob run the full `HttpTransport` (server + client), because the cycle is `request → commit → deliver → ack` and deliver/ack flow back from bob to alice. Alice therefore also generates a self-signed cert and bob must be given `--trust-cert alice.pem`. The example documents this mutual exchange explicitly — no hidden asymmetry.

- **D-E5:** **Trace format:** same shape as Phase 3 D-E2 (`[seq] SENDER → RECIPIENT: CLASS (state: FROM → TO)`), but each process prints its own view. The integration test asserts a fixed subset of trace lines on each side's stdout.

- **D-E6:** **CI integration test:** `crates/famp/tests/cross_machine_happy_path.rs`:
  - Spawns the binary twice as subprocesses with `--role bob` and `--role alice`.
  - Uses ephemeral ports (`--listen 127.0.0.1:0`) and a tempdir for key/cert exchange.
  - Uses a small coordination file or captured stdout to synchronize (bob prints `LISTENING` when ready; alice parses and connects).
  - Asserts both processes exit 0 within a timeout (e.g., 10s) and that expected trace lines appear on each stdout.
  - CI gate. CONF-04 is satisfied here, not in a unit test.

- **D-E7:** **Fallback if subprocess tests are flaky on CI:** a same-process integration test `crates/famp/tests/http_happy_path.rs` runs alice and bob as two `tokio::spawn`ed tasks against `127.0.0.1:<ephemeral>` using the real axum + reqwest stack. This is NOT a replacement for the subprocess test — it is a CI safety net. Planner decides whether to ship both or only the subprocess test.

### F. Crate Layout + Dependency Graph

- **D-F1:** **`famp-transport-http` is a library crate only — no binary.** Example and integration tests live in `crates/famp/`, matching Phase 3 D-D1 / D-E1.

- **D-F2:** **`famp-transport-http` dependency graph:**
  ```
  famp-transport-http
    ├── famp-core
    ├── famp-envelope       (for SignedEnvelope decode inside the sig-verify layer)
    ├── famp-crypto         (for verify_strict)
    ├── famp-keyring        (for Keyring::get in the sig-verify layer)
    ├── famp-transport      (implements the Transport trait)
    ├── axum          0.8.8
    ├── tower         (for Service / Layer)
    ├── tower-http    (for RequestBodyLimitLayer)
    ├── hyper         1.x   (transitive through axum)
    ├── http          1.x   (transitive through axum)
    ├── reqwest       0.13.2 (default-features = false, rustls backend only)
    ├── rustls        0.23.38
    ├── rustls-platform-verifier 0.5.x
    ├── rustls-pemfile ~2.0  (loading --cert / --trust-cert PEM files)
    ├── tokio         1.51.x (narrow features for lib)
    ├── thiserror     2.0.x
    ├── serde         1.x
    ├── serde_json    1.x    (for error body JSON only)
    ├── url           2.x    (for Url type in addr_map)
    └── [dev-dependencies]
        ├── rcgen     0.14.x (self-signed cert generation for tests/example)
        └── ...
  ```
  **Planner must validate every version against live crates.io on research day; the pins above come from the project CLAUDE.md stack table and may have drifted since.**

- **D-F3:** **`crates/famp` additions:**
  ```
  crates/famp/Cargo.toml
    [dependencies]
      famp-transport-http = { path = "../famp-transport-http" }
    [dev-dependencies]
      rcgen               = "0.14"   (for test cert generation if fixtures not used)
  ```

- **D-F4:** **No OpenSSL, no `native-tls`, no `openssl` crate in any `Cargo.toml`.** CI should add a `cargo tree -i openssl` check that fails the build if it ever appears transitively.

- **D-F5:** **AFIT vs `async-trait`:** continue with native AFIT per Phase 3 D-C5. No macro dependency added.

### Claude's Discretion

- Exact module layout inside `crates/famp-transport-http/src/` (`lib.rs` + `server.rs` + `client.rs` + `middleware.rs` + `error.rs` is a reasonable starting point; planner decides).
- Exact `reqwest` and `rustls` feature flag selection (minimum viable: rustls backend, platform-verifier roots, HTTP/1.1 only).
- Whether `HttpTransport` holds its own `tokio::task::JoinHandle` for the server or requires the caller to spawn it — prefer owned handle that shuts down on `Drop` for example binary ergonomics.
- Exact flag naming (`--role` vs `--as` vs `--identity`) — `--role` preferred per discussion.
- Whether `--addr` accepts repeated flags or a comma-separated value — repeated preferred (matches `--peer`).
- Whether `rcgen` is a `dev-dependency` only or also an optional feature on `famp-transport-http` — prefer dev-dep only (keeps the library surface clean; example is the only runtime cert generator).
- Exact file layout inside `crates/famp/tests/adversarial/` (one file with sub-modules vs a directory).
- Whether the adversarial harness uses `async_trait` or native AFIT — native AFIT preferred unless dyn-dispatch is required.
- Subprocess coordination mechanism in the integration test (`LISTENING` stdout line vs filesystem sentinel vs `TcpStream::connect` retry loop) — `TcpStream::connect` retry against the ephemeral port is simplest.
- Whether a same-process fallback `http_happy_path.rs` is shipped alongside the subprocess test or deferred — ship both for CI stability unless planner has a reason not to.
- Exact keepalive / timeout / HTTP version settings on `reqwest::Client` (HTTP/1.1 with a 10-second total timeout is a sensible default).
- Whether `Content-Type: application/famp+json` or `application/json` is used on the wire — pick one and stay consistent with any future fixture tooling.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Spec — HTTP binding, trust, and wire semantics
- `FAMP-v0.5.1-spec.md` §7.1 — canonical signing/verification rules. CONF-07 HTTP row re-canonicalizes bytes on receive; same rules as Phase 3.
- `FAMP-v0.5.1-spec.md` §7.1c — worked Ed25519 example. Not re-tested in Phase 4, but the canonical fixture Phase 3 committed to `crates/famp/tests/fixtures/conf-07-canonical-divergence.json` draws on it.
- `FAMP-v0.5.1-spec.md` §14.3 / INV-10 — unsigned messages unreachable. `FampSigVerifyLayer` enforces this at the HTTP boundary; the handler is never invoked on an unsigned request (TRANS-09 SC#2).
- `FAMP-v0.5.1-spec.md` §18 — **1 MB body limit.** Enforced as `tower_http::limit::RequestBodyLimitLayer::new(1_048_576)`. This section is the sole source of truth for the cap.
- `FAMP-v0.5.1-spec.md` §5.1, §5.2 — `Principal` and `Instance` formats. Path segment `:principal` parses via `famp_core::Principal::from_str`.
- `FAMP-v0.5.1-spec.md` §7.3a — FSM-observable whitelist. Runtime glue (unchanged from Phase 3) still drives the FSM from fields in this whitelist; middleware does not touch the FSM.
- **Explicitly NOT referenced:** §8 (Agent Card), §9 (federation credential), `.well-known` distribution. Those belong to v0.8.

### Requirements and roadmap
- `.planning/REQUIREMENTS.md` — **TRANS-03, TRANS-04, TRANS-06, TRANS-07, TRANS-09, EX-02, CONF-04.** Also: TRANS-05 (`.well-known`) and TRANS-08 (cancellation-safe spawn-channel send) explicitly absent.
- `.planning/ROADMAP.md` Phase 4 — 5 success criteria (axum+reqwest+rustls+1 MB tower layer, sig-verify middleware pre-routing, cross-machine example, HTTP adversarial matrix extension, TRANS-05/08 omission documented inline).

### Prior phase outputs (direct dependencies)
- `.planning/phases/01-minimal-signed-envelope/01-CONTEXT.md` — `SignedEnvelope`, `AnySignedEnvelope`, decode pipeline. Middleware consumes `famp-envelope` decode directly.
- `.planning/phases/02-minimal-task-lifecycle/02-CONTEXT.md` — FSM state types and `TaskTransitionInput`. Phase 4 does not touch these; noted here because the runtime glue (unchanged) drives them.
- `.planning/phases/03-memorytransport-tofu-keyring-same-process-example/03-CONTEXT.md` — **the load-bearing prior context.** Specifically:
  - **D-A1/A2/A3** — `Principal` is the stable routing key; keyring is `HashMap<Principal, TrustedVerifyingKey>`; `FampSigVerifyLayer` looks up keys exactly the same way the runtime does.
  - **D-C1/C2/C3/C4/C5/C6** — the entire `Transport` trait shape. `HttpTransport` implements it with zero shape change.
  - **D-D1/D3/D5** — runtime glue home + envelope→FSM adapter + sender cross-check. Phase 4 consumes runtime glue unchanged.
  - **D-D6/D7** — adversarial injection pattern and CONF-07 canonical-divergence fixture. D-D1 (new) promotes the test file to a directory + shared harness.
  - **D-E1/E2/E3** — Phase 3 example structure. Phase 4 mirrors it with HTTP + two processes.
- `.planning/phases/03-memorytransport-tofu-keyring-same-process-example/03-04-PLAN.md` — personal_two_agents example + adversarial matrix. Phase 4 extends the same matrix.

### v0.6 + Phase 3 implementation precedents (code that Phase 4 calls, does not modify)
- `crates/famp-core/src/identity.rs` — `Principal::from_str`. Called from the axum path-param extractor and `--addr` flag parser.
- `crates/famp-envelope/src/` — `SignedEnvelope` decode. Called from inside `FampSigVerifyLayer`.
- `crates/famp-crypto/src/` — `verify_strict`. Called from inside `FampSigVerifyLayer`.
- `crates/famp-keyring/src/` — `Keyring::get`. Called from inside `FampSigVerifyLayer` (read-only, `Arc<Keyring>`).
- `crates/famp-transport/src/lib.rs` — `Transport` trait + `TransportMessage`. `HttpTransport` implements this unchanged.
- `crates/famp-transport/src/memory.rs` — `MemoryTransport` reference implementation. Mental model for `HttpTransport`'s inbox hub; also provides Phase 3 adversarial `send_raw_for_test` that the Phase 4 harness refactor preserves.
- `crates/famp/src/runtime/` — the full receive pipeline (decode + verify + cross-check + FSM step). **Unchanged by Phase 4.**
- `crates/famp/examples/personal_two_agents.rs` — Phase 3 example. Phase 4's `cross_machine_two_agents.rs` is structurally parallel.
- `crates/famp/tests/adversarial.rs` — Phase 3 adversarial tests. Promoted to a directory module in Phase 4.

### Technology stack references (project CLAUDE.md — validate live before planning)
- `axum 0.8.8` — HTTP server framework. Uses `tower` middleware stack for `FampSigVerifyLayer`.
- `tower` — `Layer` / `Service` abstractions.
- `tower-http` — `RequestBodyLimitLayer` for §18 enforcement.
- `hyper 1.x` — transitive through axum.
- `reqwest 0.13.2` — HTTP client, rustls backend only, no default features.
- `rustls 0.23.38` — TLS stack. No OpenSSL.
- `rustls-platform-verifier 0.5.x` — OS trust store integration.
- `rustls-pemfile ~2.0` — loading PEM certificates from disk.
- `rcgen 0.14.x` — self-signed cert generation for example/tests (dev-dep only).
- `tokio 1.51.1` — async runtime. Server uses `tokio::spawn`; `JoinHandle` held inside `HttpTransport` for graceful shutdown.
- `ed25519-dalek 2.2.0` — keypair for example binary (identity, not TLS).
- `thiserror 2.0.18` — narrow error enums (`MiddlewareError`, `HttpTransportError`).
- `serde_json 1.x` — error body JSON only; NOT used for on-wire envelope parsing (that is `famp-envelope` + `serde_jcs` from Phase 1).
- `url 2.x` — `Url` type for `addr_map`.
- `stateright` — NOT used in Phase 4. v0.14.

### Explicitly NOT referenced (deferred / out of scope)
- `.well-known` Agent Card distribution (TRANS-05) — v0.8 Identity & Cards.
- Cancellation-safe spawn-channel send path (TRANS-08) — v0.9 Causality & Replay Defense.
- Pluggable `TrustStore` trait, federation credential — v0.8+.
- HTTP/2, HTTP/3, QUIC, mTLS — not v0.7.
- `openssl` crate, `native-tls` — permanently banned in FAMP v1.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`famp_core::Principal` (`crates/famp-core/src/identity.rs`)** — parsed on the axum `:principal` path param and on `--addr`/`--peer` flag parse. One identity type everywhere.
- **`famp_envelope::SignedEnvelope` / `AnySignedEnvelope` / decode pipeline** — called from inside `FampSigVerifyLayer`. Phase 4 does not re-implement envelope parsing.
- **`famp_crypto::verify_strict`** — called from inside `FampSigVerifyLayer` AND from the Phase 3 runtime glue (double-check is intentional; see D-C2).
- **`famp_keyring::Keyring` + `TrustedVerifyingKey`** — injected into the sig-verify layer as `Arc<Keyring>`. Phase 4 adds no new keyring surface.
- **`famp_transport::Transport` trait + `TransportMessage`** — `HttpTransport` implements the trait unchanged. This is the whole point of Phase 3 D-C1.
- **`famp_transport::MemoryTransport::register` / inbox hub pattern** — `HttpTransport` mirrors it: `HashMap<Principal, mpsc::Sender<_>>` inboxes, handler pushes bytes, runtime loop on the other side of the channel calls `recv(&me).await`.
- **`crates/famp/src/runtime/`** — **unchanged**. Phase 4 reuses decode + verify + cross-check + FSM step wholesale.
- **`crates/famp/examples/personal_two_agents.rs`** — structural template for `cross_machine_two_agents.rs`. Same agent names, same keyring bootstrap (inline `with_peer`), same trace format.
- **`crates/famp/tests/adversarial.rs`** — Phase 3 tests. Promoted to directory module; existing test bodies become `memory.rs` inside the new `adversarial/` directory.
- **`crates/famp/tests/fixtures/conf-07-canonical-divergence.json`** — reused byte-identically by the HTTP adversarial row.
- **`crates/famp-transport-http/` (Phase 0 stub)** — already exists as a stub with a smoke test. Phase 4 fills in the body; does NOT create a new crate.

### Established Patterns
- **Phase-local narrow error enums** — `MiddlewareError`, `HttpTransportError` follow the v0.6 + Phase 1/2/3 pattern. Neither is collapsed into `ProtocolErrorKind`; mapping to protocol-level error happens at the runtime boundary.
- **Owned types at crate boundaries** — `TransportMessage` keeps its `Vec<u8>` ownership through the axum handler into the inbox channel. No lifetimes in public types.
- **"Narrow by absence"** — no `Option<AgentCard>`, no feature-gated `.well-known` discovery, no `TrustStore` trait stub. Personal v0.7 just does not reach for these.
- **Compile-time layering** — `famp-transport` stays ignorant of `famp-envelope`, `famp-keyring`, `famp-fsm`. `famp-transport-http` pulls in `famp-envelope` + `famp-crypto` + `famp-keyring` because the sig-verify middleware decodes — this is a deliberate layering step up, not a precedent for widening `famp-transport` itself.
- **Phase 0 stub pattern** — `crates/famp-transport-http/src/lib.rs` is the existing stub; Phase 4 fills it in rather than scaffolding from scratch.
- **`rust 1.87+` toolchain** — native AFIT for the `Transport` implementation, no `async-trait` macro.
- **No `openssl` anywhere** — CI gate added via `cargo tree -i openssl` check.

### Integration Points
- **`famp-transport-http` ↔ `crates/famp/` runtime glue:** raw-bytes in via the inbox channel, raw-bytes out via `HttpTransport::send`. Same surface the Phase 3 runtime already consumes.
- **`famp-transport-http` ↔ `famp-keyring`:** `Arc<Keyring>` read-only injection into `FampSigVerifyLayer::new`. No new keyring mutation path.
- **`famp-transport-http` ↔ `famp-envelope`:** sig-verify middleware decodes; no envelope struct changes in Phase 4.
- **`crates/famp/examples/cross_machine_two_agents.rs` ↔ `famp-transport-http` + `rcgen`:** the one place in the repo that generates self-signed certs at runtime.
- **`crates/famp/tests/adversarial/` shared harness ↔ both transports:** same case definitions and assertions run against both. Phase 3 `memory.rs` adapter and Phase 4 `http.rs` adapter are parallel.
- **`crates/famp/tests/cross_machine_happy_path.rs` (new) ↔ the example binary:** subprocess invocation with ephemeral ports + tempdir cert/key exchange. CI gate for CONF-04.
- **CI `cargo tree -i openssl` gate (new)** — fails the build if `openssl` appears transitively. Guards D-F4.

</code_context>

<specifics>
## Specific Ideas

- **"One listener, path-multiplexed by principal."** The axum server mounts `POST /famp/v0.5.1/inbox/:principal` once; path parameter IS the routing key. Operationally simple, mirrors the MemoryTransport inbox hub exactly, and lets a single `rustls` cert cover a process running multiple principals.
- **"Address config stays separate from the keyring."** Identity/trust and network location are different concerns. `--peer` is for pubkeys, `--addr` is for URLs. Do not overload the keyring with `Url`. If v0.8 adds Agent Cards, they can collapse the two at that time; Phase 4 does not pre-optimize for that.
- **"Self-signed cert + explicit `--trust-cert`. No SPKI pinning, no custom verifier."** The boring TLS path. Self-signed certs are trusted the normal way — added to the root store. Dev-only custom verifiers are a footgun and buy nothing for a two-process example.
- **"Body limit first, sig-verify second, handler last."** Tower layer ordering is the normative Phase 4 contract. Body limit rejects at 413 before any decoding runs. Sig-verify rejects before route dispatch (TRANS-09 SC#2, verifiable via the `handler-not-entered` sentinel assertion in `http.rs`).
- **"Middleware is a fast-reject gate, not a replacement for runtime glue."** The sig-verify layer does NOT run `cross_check_recipient` or `TaskFsm::step`. Runtime glue owns those. The handler pushes raw bytes back into the inbox channel, and the runtime loop re-runs the full pipeline. Double-decode is cheap and keeps layering honest. This was a deliberate discussion decision.
- **"Plain HTTP status + small typed JSON body. No signed FAMP ack on rejection."** Signing an error response drags server signing keys into the middleware — wrong complexity level for Phase 4. 400/401/413/404 + `{"error": "...", "detail": "..."}` is boring, testable, and future-proof.
- **"One generic adversarial harness, two transport adapters, six rows total."** Not a copy-paste. The three cases are defined once; the harness runs them against both transports from shared byte fixtures. If an HTTP row ever tests something subtly different from a MemoryTransport row, the harness is wrong.
- **"Reuse Phase 3's CONF-07 fixture byte-identically."** `conf-07-canonical-divergence.json` is the whole point of the canonicalization gate — if posting those exact bytes over HTTP does not surface the same error, either HTTP or the canonicalization stack is broken.
- **"Fixed roles, two terminals, no auto-orchestration."** The example is `--role alice` and `--role bob`. A single binary that spawns both peers would hide transport mistakes behind test harness convenience. Ben's specific ask during discussion.
- **"Symmetric HTTP topology."** Both alice and bob run `HttpTransport` as server + client, because the `request → commit → deliver → ack` cycle flows both ways. Both need `--cert`, `--key`, and each other's `--trust-cert`. The example documents this explicitly so nobody is surprised.
- **"`famp-transport-http` is library-only."** No binary in the transport-http crate. Example and integration tests live in `crates/famp/`. Same pattern as Phase 3 D-D1 / D-E1.
- **"No `openssl`, no `native-tls`, ever."** CI gate via `cargo tree -i openssl`. This is a project-wide commitment that Phase 4 concretizes by adding the check.

</specifics>

<deferred>
## Deferred Ideas

- **`.well-known` Agent Card distribution (TRANS-05)** — v0.8 Identity & Cards.
- **Cancellation-safe spawn-channel send path (TRANS-08)** — v0.9 Causality & Replay Defense.
- **Pluggable `TrustStore` trait + federation credential** — v0.8+.
- **Sibling `peers.toml` / file-based address map** — revisit if v0.8 Agent Cards need it. Phase 4 gets by with `--peer` + `--addr` flags only.
- **SPKI cert pinning / dev-only custom rustls verifier** — rejected for v0.7. Self-signed + explicit root works.
- **mTLS (client auth via TLS cert)** — FAMP trust lives at the envelope signature layer, not the TLS layer. TLS identity is host-level only in v0.7; mTLS is not on the roadmap.
- **HTTP/2 / HTTP/3 / QUIC** — HTTP/1.1 only in v0.7. HTTP/2 is an axum/hyper/reqwest flip when needed, but no phase owns it yet.
- **Dynamic inbox registration at runtime** — Phase 4 registers inboxes at binary startup. Live re-pin / dynamic principal provisioning is v0.8+.
- **Connection pooling tuning / keepalive / per-peer reqwest client** — one shared `reqwest::Client`, default pool. Revisit only when a concrete use case demands it.
- **Middleware that calls the full runtime pipeline** — rejected. Middleware is a fast-reject gate only. Runtime glue owns decode + verify + cross-check + FSM step as a single unit.
- **Signed FAMP ack as HTTP rejection response** — rejected. Middleware cannot reach signing keys without widening its remit; plain HTTP status + JSON body is the boring correct answer.
- **Single-binary auto-orchestrated `cross_machine_two_agents`** — rejected per discussion. Fixed `--role` flags are the only supported mode.
- **Separate `http_adversarial.rs` that duplicates Phase 3 case logic** — rejected. One generic harness, two adapters.
- **`test-util` feature flag on `famp-transport-http` mirroring Phase 3 D-D6** — rejected. Raw HTTP POSTs already provide the adversarial injection surface without widening the library's public boundary. This is a deliberate difference from Phase 3.
- **`stateright` model check over the HTTP middleware pipeline** — v0.14 Adversarial Conformance.
- **Conformance Level 2/3 badges** — v0.14.
- **`famp` CLI subcommands (`famp keygen`, `famp serve`, etc.)** — v0.8+ CLI milestone; v0.7 ships example binaries only.

</deferred>

---

*Phase: 04-minimal-http-transport-cross-machine-example*
*Context gathered: 2026-04-13*

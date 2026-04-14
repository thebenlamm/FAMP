---
phase: 04
plan: 03
subsystem: famp-transport-http
tags: [http, transport, tls, rustls, client, openssl-gate]
provides:
  - HttpTransport struct + native AFIT impl famp_transport::Transport
  - tls::{load_pem_cert, load_pem_key, build_server_config, build_client_config}
  - tls_server::{serve, serve_std_listener} wrapping axum_server::bind_rustls / from_tcp_rustls
  - CI no-openssl gate (D-F4) on the build job
requires:
  - famp_transport::Transport (Phase 3)
  - famp_transport::TransportMessage
  - rustls 0.23.38 + rustls-platform-verifier 0.5.3 + rustls-pemfile 2 + axum-server 0.8 + reqwest 0.13.2
  - error::HttpTransportError + server::InboxRegistry (Plans 04-01 / 04-02)
affects:
  - crates/famp-transport-http/src/lib.rs (modules + re-exports + silencer cleanup)
  - .github/workflows/ci.yml (build job gains openssl gate)
tech-stack:
  added:
    - rustls 0.23 ServerConfig + ClientConfig builders (now live consumers)
    - rustls_platform_verifier::Verifier::new_with_extra_roots (D-B5 full)
    - axum_server::tls_rustls::{RustlsConfig, bind_rustls, from_tcp_rustls}
    - reqwest::ClientBuilder::use_preconfigured_tls (rustls path)
  patterns:
    - "Idempotent default crypto provider install (rustls::crypto::aws_lc_rs::default_provider().install_default())"
    - "OS roots + explicit anchor via rustls_platform_verifier custom verifier (D-B5 full combination, not simplified)"
    - "Two-helper serve API: bind by SocketAddr (production) + bind by std::net::TcpListener (ephemeral-port subprocess tests)"
    - "Drop-on-abort for spawned server task: HttpTransport::Drop try_locks server_handle and aborts the JoinHandle"
key-files:
  created:
    - crates/famp-transport-http/src/tls.rs
    - crates/famp-transport-http/src/tls_server.rs
    - crates/famp-transport-http/src/transport.rs
  modified:
    - crates/famp-transport-http/src/lib.rs
    - .github/workflows/ci.yml
key-decisions:
  - "Crypto provider is aws-lc-rs, not ring (the plan said ring). The workspace dep graph already pulls aws-lc-rs via reqwest 0.13.2 → rustls-platform-verifier 0.6.2 → rustls; ring is not in the graph at all. Forcing ring would add a second crypto provider for zero benefit. Verified via `cargo tree -i aws-lc-rs` and `cargo tree -i ring`."
  - "Inbox URL constructed by string concat + Url::parse rather than Url::join, because Url::join's last-segment replacement semantics are surprising for non-trailing-slash bases. The join-versus-parse distinction is documented inline."
  - "clippy::significant_drop_tightening allowed on Transport::recv impl: tokio::mpsc::Receiver::recv requires &mut Receiver, so the outer Mutex guard MUST stay alive across the await. Tightening the lock would invalidate the borrow."
metrics:
  duration_min: 14
  tasks: 3
  files_created: 3
  files_modified: 2
  completed: 2026-04-13
---

# Phase 4 Plan 03: HttpTransport + rustls TLS helpers + CI no-openssl gate Summary

Wave 3 of Phase 4 closed out the client side of `famp-transport-http`: a
native-AFIT `HttpTransport` over `reqwest 0.13` + `rustls 0.23`, a `tls.rs`
helper module that loads PEM cert/key pairs and constructs both server and
client rustls configs (D-B5 full — OS root store **plus** an explicit
`--trust-cert` anchor via `rustls_platform_verifier::Verifier::new_with_extra_roots`),
a `tls_server.rs` wrapper around `axum_server::bind_rustls` /
`from_tcp_rustls` so Plan 04-04 has a concrete serve helper to call instead
of writing `todo!()`, and a CI gate on the build job that fails the workspace
if `openssl` or `native-tls` ever appears in the dep tree (D-F4 lock).

## Tasks Completed

| # | Task | Files | Commit |
|---|------|-------|--------|
| 1 | tls.rs PEM loaders + D-B5 client config + tls_server bind_rustls wrapper | crates/famp-transport-http/src/{tls.rs,tls_server.rs,lib.rs} | `55f522e` |
| 2 | HttpTransport struct + native AFIT impl Transport | crates/famp-transport-http/src/{transport.rs,lib.rs} | `56bd4dc` |
| 3 | CI no-openssl gate (D-F4) | .github/workflows/ci.yml | `af2d88b` |

## Verification Results

- `cargo check -p famp-transport-http` — green
- `cargo clippy -p famp-transport-http --all-targets -- -D warnings` — clean (0 warnings)
- `cargo nextest run -p famp-transport-http --all-targets` — 15 / 15 passing
  (1 status-mapping unit + 6 tls unit + 4 transport unit + 4 sentinel layering)
- `cargo nextest run --workspace` — 243 / 243 passing (zero regressions; +10 from prior 233)
- `cargo tree -i openssl --workspace` — no path
- `cargo tree -i native-tls --workspace` — no path
- Local CI gate body (`bash -c '... cargo tree -i openssl ...'`) — exits 0

## Acceptance Criteria

### Task 1 (tls.rs + tls_server.rs)
- File `crates/famp-transport-http/src/tls.rs` exists ✓
- Contains `fn load_pem_cert`, `fn load_pem_key`, `fn build_server_config`, `fn build_client_config` ✓
- Contains literal `Verifier::new_with_extra_roots` (D-B5 full — checker B-6) ✓
- Contains literal `with_custom_certificate_verifier` ✓
- Does NOT contain `native-tls` or `openssl` ✓
- File `crates/famp-transport-http/src/tls_server.rs` exists ✓
- Contains literal `axum_server::bind_rustls` AND literal `axum_server::from_tcp_rustls` ✓
- Contains literal `RustlsConfig::from_config` ✓
- Exposes `pub fn serve` AND `pub fn serve_std_listener` (both returning `JoinHandle`) ✓
- `cargo check -p famp-transport-http` succeeds ✓

### Task 2 (transport.rs)
- File `crates/famp-transport-http/src/transport.rs` exists ✓
- Contains literal `impl Transport for HttpTransport` ✓
- Contains literal `application/famp+json` ✓
- Contains literal `famp/v0.5.1/inbox/` ✓
- Contains literal `use_preconfigured_tls` ✓
- Contains literal `http1_only` ✓
- Contains literal `build_client_config` ✓
- Does NOT contain `async_trait` (D-F5) ✓
- Does NOT contain `default-tls` or `native-tls` ✓
- `cargo check -p famp-transport-http` succeeds ✓
- `cargo clippy -p famp-transport-http --all-targets -- -D warnings` exits 0 ✓

### Task 3 (ci.yml)
- `.github/workflows/ci.yml` contains literal `cargo tree -i openssl` ✓
- `.github/workflows/ci.yml` contains literal `cargo tree -i native-tls` ✓
- `.github/workflows/ci.yml` contains literal `D-F4` ✓
- The new step is placed inside the same job as the existing cargo build/check step (job `build`) ✓
- Local execution of the step's bash body exits 0 ✓

## Deviations from Plan

### [Rule 3 — Blocking issue] Crypto provider is aws-lc-rs, not ring

- **Found during:** Task 1, while writing the `install_default_provider` helper.
- **Issue:** Plan said `rustls::crypto::ring::default_provider().install_default()`. But `cargo tree -i ring -p famp-transport-http` returns nothing — the workspace has no `ring` dep at all. `cargo tree -i aws-lc-rs` shows the crypto provider that *is* in the graph: `aws-lc-rs 1.16.2` is pulled by `rustls 0.23.38` (with the `aws_lc_rs` feature enabled) via `reqwest 0.13.2 → hyper-rustls → rustls` AND via `rustls-platform-verifier 0.6.2 → rustls`. Calling `rustls::crypto::ring::default_provider()` would fail to compile (the `ring` cargo feature on rustls is not enabled in the workspace).
- **Fix:** Use `rustls::crypto::aws_lc_rs::default_provider().install_default()`. Wrapped in a private `install_default_provider()` helper called from both `build_server_config` and `build_client_config` so test-order does not matter. The plan's intent — "install rustls default provider explicitly at startup" — is preserved; the choice of *which* provider was wrong by accident, not by intent.
- **Files modified:** `crates/famp-transport-http/src/tls.rs`
- **Commit:** `55f522e`
- **Plan note alignment:** This matches Plan 04-01's notes-for-downstream observation: "aws-lc-rs is currently in the dep graph (pulled transitively by rustls-platform-verifier 0.6.2 via reqwest)."

### [Rule 1 — Bug] tls_server::serve_std_listener axum_server return type

- **Found during:** Task 1 cargo check.
- **Issue:** Plan provided `axum_server::from_tcp_rustls(listener, rustls_config).serve(...)` — but the actual `axum-server 0.8.0` API (verified by reading `~/.cargo/registry/src/.../axum-server-0.8.0/src/tls_rustls/mod.rs:60`) is `pub fn from_tcp_rustls(listener, config) -> io::Result<Server<...>>`. It returns a `Result`, not the server directly.
- **Fix:** Match the result inside the spawned task and propagate the `io::Error` through the `JoinHandle<io::Result<()>>` return type. Documented inline.
- **Files modified:** `crates/famp-transport-http/src/tls_server.rs`
- **Commit:** `55f522e`

### [Rule 1 — Bug] clippy::significant_drop_tightening on Transport::recv

- **Found during:** Task 2 clippy.
- **Issue:** Clippy's `significant_drop_tightening` (pedantic, but `-D warnings`) flagged `let mut guard = self.receivers.lock().await;` as "should be tightened" — but `tokio::mpsc::Receiver::recv` requires `&mut Receiver`, and `guard.get_mut(&who)` returns a borrow whose lifetime is tied to `guard`. Tightening the lock would force the borrow to drop before the await, making the whole shape uncompilable.
- **Fix:** Added `#[allow(clippy::significant_drop_tightening)]` to the `recv` method with a doc comment explaining why the guard intentionally outlives the await.
- **Files modified:** `crates/famp-transport-http/src/transport.rs`
- **Commit:** `56bd4dc`

### [Rule 1 — Bug] Plan-supplied `Url::join` for inbox URL had wrong semantics

- **Found during:** Task 2 implementation.
- **Issue:** Plan said `url.join(&format!("famp/v0.5.1/inbox/{}", msg.recipient))`. But `Url::join` follows RFC 3986: if the base URL is `https://host:8443` (no trailing slash), `join("famp/v0.5.1/inbox/...")` *replaces* the last path segment rather than appending. For the bare-host base URLs that 04-04 will pass, this happens to work — but it silently breaks if the user supplies `https://host:8443/api` as the base.
- **Fix:** Build the URL string by trimming any trailing `/` from the base and concatenating the inbox path, then `Url::parse` the result. Explicit semantics; no surprises if a future caller passes a base with a path component.
- **Files modified:** `crates/famp-transport-http/src/transport.rs`
- **Commit:** `56bd4dc`

### Auto-fixed clippy hygiene (no behavior change)

- **Found during:** Task 1 clippy.
- **Fixes:** (a) `load_pem_key` collapsed to `.ok_or(TlsError::NoPrivateKey)` instead of an `if let / else` (clippy::option_if_let_else style). (b) tls.rs test module annotated with `#![allow(clippy::unwrap_used, clippy::expect_used)]` to match `crates/famp-transport/src/memory.rs`'s test convention. (c) tls_server.rs `serve_std_listener` doc comment split into a short title + paragraph to satisfy `clippy::doc_markdown` first-paragraph length lint. (d) transport.rs module doc backticks added around `addr_map` to satisfy `clippy::doc_markdown` "missing backticks". (e) Test `add_peer_populates_addr_map` rewritten to chain the `.lock().await.get(...).cloned()` rather than holding the guard in a local — satisfies `significant_drop_tightening` for the test.
- **Files modified:** `crates/famp-transport-http/src/{tls.rs,tls_server.rs,transport.rs}`
- **Commits:** `55f522e`, `56bd4dc`

## Notes for Downstream Plans

- **Plan 04-04 example binary** can now construct an `HttpTransport` like:
  ```rust
  let transport = HttpTransport::new_client_only(Some(&trust_cert_path))?;
  transport.add_peer(bob, bob_url).await;
  transport.register(alice).await;
  let inboxes = transport.inboxes();
  let server_config = Arc::new(tls::build_server_config(certs, key)?);
  let router = build_router(keyring, inboxes);
  let handle = tls_server::serve_std_listener(std_listener, router, server_config);
  transport.attach_server(handle).await;
  ```
  The `serve_std_listener` path is what the subprocess test wants because it
  lets the example bind `127.0.0.1:0` and read `local_addr()` *before* spawning
  the server task.
- **Drop-on-abort:** `HttpTransport::Drop` uses `try_lock` on `server_handle` and
  aborts the `JoinHandle` if it can. This is best-effort — if a long-running
  task is holding the `server_handle` mutex at drop time (which should never
  happen in practice), the abort is silently skipped. The example binary
  should explicitly `transport.attach_server(handle)` only once and not race
  with drop.
- **Crypto provider drift:** if a future change pulls the `ring` feature on
  rustls (e.g. switching reqwest features), `rustls::crypto::aws_lc_rs::default_provider`
  will still resolve (both providers can coexist), but `install_default` will
  set whichever was called first. For consistency, keep `tls.rs` as the single
  caller of `install_default_provider()` and let the rest of the workspace
  use `CryptoProvider::get_default()`.
- **No example binary yet:** Plan 04-04 will exercise the full
  cross-machine HTTPS cycle. Until then, `HttpTransport::send` against a real
  axum server is only covered by the 4 sentinel tests in
  `tests/middleware_layering.rs` (which use `tower::ServiceExt::oneshot`
  in-process, not real reqwest). The `transport::tests::*` unit tests cover
  builder + addr-map + register paths but do NOT issue real HTTPS — that
  arrives in 04-04.
- **Url construction policy:** Future code that builds URLs from a base +
  path segment should follow the trim-and-concat pattern in `transport.rs`
  rather than `Url::join`, unless the caller is explicitly handling the
  RFC 3986 last-segment-replace semantics.

## Self-Check: PASSED

- crates/famp-transport-http/src/tls.rs — FOUND
- crates/famp-transport-http/src/tls_server.rs — FOUND
- crates/famp-transport-http/src/transport.rs — FOUND
- crates/famp-transport-http/src/lib.rs — FOUND (modified)
- .github/workflows/ci.yml — FOUND (modified, contains `cargo tree -i openssl`, `cargo tree -i native-tls`, `D-F4`)
- Commit 55f522e — FOUND
- Commit 56bd4dc — FOUND
- Commit af2d88b — FOUND

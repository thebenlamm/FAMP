---
phase: 02-daemon-inbox
plan: 02
subsystem: cli-daemon
tags: [axum, tokio, tls, signal, listen, inbox]

requires:
  - phase: 01-identity-cli-foundation
    provides: load_identity → IdentityLayout, CliError enum, resolve_famp_home
  - phase: 02-daemon-inbox-plan-01
    provides: famp_inbox::Inbox::open + append(&[u8]) with fsync-before-return

provides:
  - famp listen subcommand (clap variant + tokio runtime bootstrap)
  - listen::run / listen::run_on_listener public API (Plan 02-03 test surface)
  - Custom axum Router reusing FampSigVerifyLayer unmodified with
    inbox-append-then-200 handler
  - CliError::PortInUse, CliError::Inbox (#[from]), CliError::Tls (#[from])
  - SIGINT + SIGTERM graceful shutdown via tokio::signal::unix
  - Durable HTTP 200 receipt — fsync runs before the handler returns

affects: [02-03-shutdown-durability, 03-conversation-cli]

tech-stack:
  added:
    - "axum 0.8 (promoted from dev-dep to dep for the custom router)"
    - "tower 0.5 + tower-http 0.6 limit feature"
    - "tokio signal + net features"
  patterns:
    - "Middleware stack replicated byte-for-byte from famp-transport-http::server::build_router (outer RequestBodyLimitLayer(1 MiB) → map_request(Body::new) → inner FampSigVerifyLayer)"
    - "Sync cli::run dispatcher that boots tokio only inside the Listen arm (Init remains sync)"
    - "std::net::TcpListener for bind-fail-fast semantics, then set_nonblocking(true) before handing to axum-server"
    - "Self-keyring bootstrap: single entry agent:localhost/self → own verifying key (peer keys deferred to Phase 3)"
    - "Test-facing run_on_listener(home, listener, shutdown_signal) allowing ephemeral-port integration tests in Plan 02-03"

key-files:
  created:
    - crates/famp/src/cli/listen/mod.rs
    - crates/famp/src/cli/listen/router.rs
    - crates/famp/src/cli/listen/signal.rs
  modified:
    - crates/famp/Cargo.toml
    - crates/famp/src/cli/error.rs
    - crates/famp/src/cli/mod.rs
    - crates/famp/src/bin/famp.rs
    - crates/famp/examples/personal_two_agents.rs
    - crates/famp/examples/cross_machine_two_agents.rs
    - crates/famp/examples/_gen_fixture_certs.rs

key-decisions:
  - "200 OK (not 202 ACCEPTED) as the handler's success status — stricter than the upstream famp-transport-http handler because the 200 is a durability receipt (fsync happened before we return). Documented deviation from CONTEXT.md which only specified inbox semantics, not the HTTP status."
  - "Self-principal `agent:localhost/self` for Phase 2's single-entry keyring. Plan 02-03's integration tests will sign with this principal so the sig-verify middleware can resolve them. Peer keys land in Phase 3."
  - "Merged Task 1 + Task 2 into a single atomic commit. Splitting would leave an intermediate state where cli/mod.rs references listen::run but cli/listen/mod.rs is absent (non-compiling). The 'one commit per task' ideal loses to compile-atomicity here."
  - "set_nonblocking(true) applied in BOTH run and run_on_listener. axum-server 0.8 panics on blocking sockets (tokio-rs/tokio#7172); rather than document this as a caller contract for run_on_listener, we enforce it inside so Plan 02-03 tests can bind 127.0.0.1:0 the natural way."

patterns-established:
  - "Typed PortInUse mapping — std::io::ErrorKind::AddrInUse → CliError::PortInUse { addr } with no random-port fallback loop. Every future bind-to-port subcommand reuses this shape."
  - "Shutdown via tokio::select! between JoinHandle and shutdown_signal future; on signal we drop the JoinHandle (axum-server stops accepting) — in-flight handlers that have already fsynced return 200; handlers mid-fsync see client drop. Phase 2 accepts this per CONTEXT §Graceful Shutdown."
  - "Extension<Arc<AnySignedEnvelope>> extracted in the handler even when unused — compile-time proof that FampSigVerifyLayer actually ran. Removing the layer would cause runtime 500s on every request (loud failure)."

requirements-completed: [CLI-02, DAEMON-01, DAEMON-02, DAEMON-03, DAEMON-04]

duration: ~35min
completed: 2026-04-14
---

# Phase 2 Plan 2: famp listen Summary

**`famp listen` daemon wired end-to-end: clap subcommand → tokio runtime → pre-bound TcpListener → axum router (reusing FampSigVerifyLayer unmodified) → fsync-before-200 inbox-append handler → SIGINT/SIGTERM graceful shutdown.**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-04-14T21:50Z (approx)
- **Completed:** 2026-04-14T22:25Z
- **Tasks:** 2/2 (merged into one atomic commit — see Decisions)
- **Files created:** 3
- **Files modified:** 7

## Accomplishments

- `famp listen` binds its configured address, prints `listening on https://<addr>` to stderr, and blocks until SIGINT/SIGTERM → exit 0. Verified by manual smoke test.
- Second `famp listen` on the same port exits 1 with `another famp listen is already bound to 127.0.0.1:18443`. Verified by manual smoke test.
- Custom Router in `crates/famp/src/cli/listen/router.rs` reuses `FampSigVerifyLayer` and the `RequestBodyLimitLayer(1 MiB)` → `map_request(Body::new)` bridge byte-for-byte from `famp-transport-http::server`. Zero source changes to `famp-transport-http`.
- Handler: `inbox.append(&body).await` → `200 OK` on success, `500` with `eprintln!` diagnostic on `InboxError`. Append runs INSIDE the HTTP request lifecycle so the 200 is a durability receipt.
- `CliError` has 14 variants (11 Phase 1 + PortInUse, Inbox, Tls). `Inbox` and `Tls` use `#[from]` so `?` flows cleanly.
- Shutdown signal future (`signal::shutdown_signal`) awaits the first of `ctrl_c` / `SIGTERM` via `tokio::signal::unix`; non-unix falls back to ctrl_c only (Windows out of scope per CONTEXT).
- `cargo nextest run --workspace` → **293/293 passed, 1 skipped** (no regressions; 292 from Plan 02-01 + 1 new signal smoke test).
- `cargo clippy -p famp --all-targets -- -D warnings` → 0 warnings.
- `cargo tree -i openssl` guard intact (no new transitive OpenSSL pulls).

## Task Commits

1. **Tasks 1+2 merged — wire famp listen subcommand with inbox-append handler** — `f51b590` (feat)
   - CliError variants (PortInUse, Inbox, Tls), Commands::Listen(ListenArgs), sync dispatcher that boots tokio only inside the Listen arm.
   - `cli/listen/{mod,router,signal}.rs` created — full implementation.
   - axum/tower/tower-http promoted to non-dev deps.
   - bin/famp.rs + all three examples: silencer stanzas updated for new transitive deps.

2. **fix(02-02): set listener non-blocking before handing to axum-server** — `0dc56a7` (fix)
   - Auto-fixed via smoke test: axum-server 0.8 panics on blocking sockets (tokio-rs/tokio#7172). Added `listener.set_nonblocking(true)` in both `run` and `run_on_listener`.

## Files Created/Modified

- `crates/famp/src/cli/listen/mod.rs` — `ListenArgs`, `run`, `run_on_listener`. Identity load, config parse, self-keyring build, inbox open, TLS config, server spawn, shutdown select.
- `crates/famp/src/cli/listen/router.rs` — `build_listen_router`, `inbox_append_handler`. Reuses FampSigVerifyLayer unmodified.
- `crates/famp/src/cli/listen/signal.rs` — `shutdown_signal()` async fn + smoke test.
- `crates/famp/src/cli/error.rs` — 3 new variants.
- `crates/famp/src/cli/mod.rs` — `pub mod listen;`, `Commands::Listen`, tokio runtime bootstrap in `run` for the Listen arm only.
- `crates/famp/Cargo.toml` — famp-inbox path dep; tokio signal+net features; axum/tower/tower-http as non-dev deps.
- `crates/famp/src/bin/famp.rs` — silencers for `famp_inbox`, `tower`, `tower_http`; `axum` promoted from test-only to always-silenced (examples always reference it).
- `crates/famp/examples/{personal,cross_machine}_two_agents.rs`, `_gen_fixture_certs.rs` — silencer stanzas updated for new deps.

## Decisions Made

- **200 OK vs 202 ACCEPTED** — The upstream `famp-transport-http` handler returns 202 (dispatched to an in-memory mpsc). Our listen handler returns 200 because the semantics are stronger: the response is only sent AFTER fsync. Documented in `router.rs` module comment.
- **Self-principal is `agent:localhost/self`** — `famp_core::Principal` is not pubkey-derived (v0.7 design — principals are authority/name strings, keyring maps them to verifying keys). Phase 2 has no configured principal yet (CONTEXT deferred that to where the first subcommand reads it), so we hardcode a self-principal for the single keyring entry. Plan 02-03 tests will sign with this principal.
- **Tasks 1+2 merged into one commit** — See summary note above. The alternative (two commits) would leave an intermediate state with `cli::run` referencing a non-existent module.
- **`set_nonblocking(true)` in both entry points** — Enforced inside both `run` and `run_on_listener` rather than making it a caller contract. Simpler surface for Plan 02-03 tests.
- **`load_identity` returns `IdentityLayout` (paths only)** — the CONTEXT.md hint `(signing_key, tls_cert_pair, config)` was inaccurate; Phase 1 actually returns a path layout. Each file is read explicitly in `run_on_listener`, with errors mapped through `CliError::Io { path, source }`.

## Deviations from Plan

### Rule 1 — Bug: listener must be non-blocking before axum-server 0.8

- **Found during:** Post-Task-2 smoke test (`famp listen`).
- **Issue:** `std::net::TcpListener::bind` returns a blocking socket. `axum-server 0.8` panics in `tokio-rt-worker` with "Registering a blocking socket with the tokio runtime is unsupported" (tokio-rs/tokio#7172). The plan specified `TcpListener::bind` without flagging this caveat; the tls_server doc comment claims `from_tcp_rustls` sets it automatically, but that has changed in 0.8.
- **Fix:** Added `listener.set_nonblocking(true)` in both `run` (prod path) and `run_on_listener` (test path).
- **Files modified:** `crates/famp/src/cli/listen/mod.rs`.
- **Commit:** `0dc56a7`.

### Rule 3 — Blocking (lint): clippy pedantic fixes during Task implementation

- **Found during:** First `cargo clippy -p famp --all-targets -- -D warnings` after writing the listen module.
- **Issues fixed inline before the first commit:**
  1. `clippy::double_must_use` on `build_listen_router` (returning `Router` which is already `#[must_use]`) — removed the redundant `#[must_use]`.
  2. `clippy::manual_let_else` + `clippy::single_match_else` on `signal.rs`'s `match signal(SignalKind::terminate())` — rewrote as `let Ok(mut sigterm) = signal(...) else { ... };`.
  3. `clippy::too_long_first_doc_paragraph` on `run()` doc — split into title + paragraph.
  4. `clippy::doc_markdown` on `JoinHandle` — added backticks.
  5. `unused_crate_dependencies` on the three examples — added silencers for `famp_inbox`, `tower`, `tower_http`.
- **Fix:** All inline before commit `f51b590`.

### Note: no Task-2 unit test for the router

Plan action step 5 for Task 2 said "A unit test verifying `build_listen_router` returns a router with the expected route set is optional and low-value — skip it." Skipped as directed. Router behavior is tested end-to-end by Plan 02-03's integration tests.

---

**Total deviations:** 2 (1 Rule-1 bug found via smoke test, 1 Rule-3 lint batch).
**Impact on plan:** Neither changes public API or plan scope. The non-blocking fix is required for the daemon to function at all; without it, Plan 02-03's first test would panic.

## Issues Encountered

One surprise: axum-server 0.8 changed its socket handling compared to the tls_server helper's doc comment. Documented in the `run_on_listener` fix note.

## Verification Artifacts

- `cargo check -p famp` → clean
- `cargo clippy -p famp --all-targets -- -D warnings` → 0 warnings
- `cargo nextest run -p famp` → 40/40 passed (39 Phase 1 + 1 new `cli::listen::signal::tests::shutdown_signal_is_a_future`)
- `cargo nextest run --workspace` → **293/293 passed, 1 skipped**
- Manual smoke — first instance:
  - `FAMP_HOME=/tmp/famp-smoke-02-02 famp init` → OK
  - `FAMP_HOME=/tmp/famp-smoke-02-02 famp listen --listen 127.0.0.1:18443` → `listening on https://127.0.0.1:18443` to stderr, blocks.
  - `kill <pid>` → `shutdown signal received, exiting`, exit 0.
- Manual smoke — second instance while first is running:
  - `FAMP_HOME=/tmp/famp-smoke-02-02 famp listen --listen 127.0.0.1:18443` → `another famp listen is already bound to 127.0.0.1:18443`, exit 1.
- `grep -q 'PortInUse' crates/famp/src/cli/error.rs` → hit.
- `grep -q 'FampSigVerifyLayer' crates/famp/src/cli/listen/router.rs` → hit.
- `grep -q 'sync_data' crates/famp-inbox/src/append.rs` → hit (durability contract inherited from Plan 02-01).

## Threat Flags

None. The single new trust boundary (inbound HTTPS → daemon) is already enumerated in the plan's `<threat_model>` table. No new auth path, schema, or file-access pattern introduced beyond what the plan covered.

## Next Phase Readiness

- **Plan 02-03 (shutdown durability)** can consume `listen::run_on_listener(home, listener, shutdown_signal)` directly. Ephemeral-port tests bind `127.0.0.1:0` via `std::net::TcpListener`, read `local_addr()`, then call `run_on_listener` with an oneshot-driven shutdown future. Signature testing uses the self-principal `agent:localhost/self`.
- **Phase 3 (send, await, peer add)** will extend the self-keyring to a multi-entry keyring loaded from peers.toml. The self-entry will remain; peer entries will be added via `with_peer`. No refactor required in `listen` — it already takes `Arc<Keyring>`.

## Re-Verification Note (2026-04-14)

Per `02-VERIFICATION.md`, the requirement labeling in this plan's frontmatter was corrected without code changes:

- The `INBOX-03` claim was removed. `famp await` poll-with-timeout semantics belong to Phase 3 (now tracked in `03-01-PLAN.md` and later plans).
- The DAEMON-03 ↔ DAEMON-04 label mapping was confirmed as:
  - **DAEMON-03** = SIGINT/SIGTERM graceful shutdown → `tests/listen_shutdown.rs`
  - **DAEMON-04** = single-instance bind gate (port-in-use) → `tests/listen_bind_collision.rs`
- Both behaviors were always implemented correctly in this plan; only the narrative prose in the plan body had them swapped. Code is unchanged.

## Self-Check: PASSED

- `crates/famp/src/cli/listen/mod.rs` — FOUND
- `crates/famp/src/cli/listen/router.rs` — FOUND
- `crates/famp/src/cli/listen/signal.rs` — FOUND
- `crates/famp/src/cli/error.rs` contains `PortInUse` — FOUND
- `crates/famp/src/cli/error.rs` contains `Inbox(#[from]` — FOUND
- `crates/famp/src/cli/error.rs` contains `Tls(#[from]` — FOUND
- `crates/famp/src/cli/mod.rs` contains `Commands::Listen` — FOUND
- Commit `f51b590` — FOUND in `git log`
- Commit `0dc56a7` — FOUND in `git log`

---
*Phase: 02-daemon-inbox*
*Plan: 02*
*Completed: 2026-04-14*

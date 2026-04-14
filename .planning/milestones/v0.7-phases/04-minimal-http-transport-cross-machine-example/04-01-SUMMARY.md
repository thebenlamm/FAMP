---
phase: 04
plan: 01
subsystem: famp-transport-http
tags: [http, transport, infra, error-enums, peek-sender, layering]
provides:
  - famp-transport-http Cargo.toml dep set (axum/axum-server/tower/tower-http/reqwest/rustls/rustls-platform-verifier/rustls-pemfile/url/futures-util)
  - MiddlewareError + IntoResponse status mapping (D-C6 lock test)
  - HttpTransportError client-side error enum
  - famp_envelope::peek_sender (lifted from crates/famp/src/runtime/peek.rs)
requires:
  - famp-canonical::from_slice_strict
  - famp-core::Principal
  - famp-envelope::EnvelopeDecodeError
affects:
  - crates/famp/src/runtime/peek.rs (now thin wrapper over famp_envelope::peek_sender)
tech-stack:
  added:
    - axum 0.8.8
    - axum-server 0.8 (tls-rustls-no-provider)
    - tower 0.5
    - tower-http 0.6 (limit)
    - reqwest 0.13.2 (rustls-no-provider)
    - rustls 0.23.38
    - rustls-platform-verifier 0.5
    - rustls-pemfile 2
    - url 2
    - futures-util 0.3
  patterns:
    - "Phase-local thiserror enums (no leakage to famp-core)"
    - "Two-phase decode peek lifted into envelope crate to break crates/famp dep cycle for HTTP middleware"
key-files:
  created:
    - crates/famp-transport-http/src/error.rs
    - crates/famp-envelope/src/peek.rs
  modified:
    - crates/famp-transport-http/Cargo.toml
    - crates/famp-transport-http/src/lib.rs
    - crates/famp-envelope/src/lib.rs
    - crates/famp/src/runtime/peek.rs
key-decisions:
  - "reqwest 0.13.2 feature is `rustls-no-provider` (not `rustls-tls`/`rustls-tls-native-roots` from plan — those features do not exist in this version). Plan's intent (rustls + platform-verifier without auto-pulling a crypto provider) preserved."
metrics:
  duration_min: 6
  tasks: 3
  files_created: 2
  files_modified: 4
  completed: 2026-04-13
---

# Phase 4 Plan 01: famp-transport-http Skeleton + peek_sender Lift Summary

Filled in `famp-transport-http` with the full Phase 4 dependency set (including `axum-server` for TLS serving), introduced phase-local `MiddlewareError` and `HttpTransportError` enums with a load-bearing D-C6 status-code lock test, and lifted `peek_sender` from `crates/famp/src/runtime/peek.rs` into `famp-envelope` so the upcoming HTTP sig-verify middleware (Plan 04-02) can call the same canonical two-phase decode without depending on `crates/famp`.

## Tasks Completed

| # | Task | Files | Commit |
|---|------|-------|--------|
| 1 | Fill famp-transport-http Cargo.toml | crates/famp-transport-http/Cargo.toml | `f13bc9e` |
| 2 | error.rs + lib.rs (MiddlewareError + HttpTransportError + status lock test) | crates/famp-transport-http/src/{error.rs,lib.rs} | `336e156` |
| 3 | Lift peek_sender to famp-envelope + thin runtime wrapper | crates/famp-envelope/src/{peek.rs,lib.rs}, crates/famp/src/runtime/peek.rs | `f849be1` |

## Verification Results

- `cargo check -p famp-transport-http` — green
- `cargo nextest run -p famp-transport-http` — 1 / 1 passing (`middleware_error_status_mapping_is_load_bearing`)
- `cargo nextest run -p famp-envelope -p famp` — 87 / 87 passing (3 new peek tests + zero regressions across runtime call sites)
- `cargo clippy --workspace -- -D warnings` — clean
- `cargo tree -i openssl --workspace` — no path (D-F4 satisfied)
- `cargo tree -i native-tls --workspace` — no path

## Deviations from Plan

### [Rule 3 - Blocking issue] reqwest feature naming

- **Found during:** Task 1 cargo check
- **Issue:** Plan specified `features = ["rustls-tls", "rustls-tls-native-roots"]` for reqwest 0.13. Neither feature exists in `reqwest 0.13.2`; cargo refused to resolve. Available rustls-related features in 0.13.2 are `rustls`, `rustls-no-provider`, and the internal `__rustls`. The `rustls` feature pulls aws-lc-rs by default; `rustls-no-provider` pulls rustls + rustls-platform-verifier without forcing a crypto provider — matching the plan's stated intent ("we install the rustls default crypto provider explicitly at startup") and matching the `tls-rustls-no-provider` choice already made for `axum-server`.
- **Fix:** Replaced reqwest features with `["rustls-no-provider"]`. No openssl, no native-tls, no forced crypto-provider — Plan 04-03 Task 1 will install the provider at startup as already specified.
- **Files modified:** `crates/famp-transport-http/Cargo.toml`
- **Commit:** `f13bc9e`

### Auto-fixed silencer list in lib.rs

- **Found during:** Task 2 compile
- **Issue:** Plan's silencer block listed `reqwest as _;`, `url as _;`, `serde as _;`, `serde_json as _;`. But `error.rs` actively uses `reqwest::Error`, `url::ParseError`, and `serde::Serialize`, so emitting `use reqwest as _;` etc. would shadow real usage warnings. Conversely, `famp_envelope` was NOT in the silencer list but is also not yet used in this plan.
- **Fix:** Removed `reqwest`/`url`/`serde` from silencer block (they are real consumers via error.rs); added `famp_envelope as _;` and `serde_json as _;` to keep `unused_crate_dependencies` happy until later plans wire them.
- **Files modified:** `crates/famp-transport-http/src/lib.rs`
- **Commit:** `336e156`

## Notes for Downstream Plans

- `rustls-platform-verifier` direct dep is `0.5.3`, but reqwest 0.13.2 transitively pulls `rustls-platform-verifier 0.6.2`. Plan 04-03 Task 1 should call the 0.5.x API (`Verifier::new_with_extra_roots`) on the direct dep — both versions coexist in the graph today.
- aws-lc-rs is currently in the dep graph (pulled transitively by `rustls-platform-verifier 0.6.2` via reqwest). The plan's openssl/native-tls gates do not forbid aws-lc-rs presence; the active rustls crypto provider is still chosen explicitly at startup (Plan 04-03 Task 1). If a future decision wants aws-lc-rs out of the graph entirely, that requires upstream coordination with reqwest 0.13.x.
- `axum-server 0.8` resolved to `0.8.0` (current available patch). No version drift action needed.

## Self-Check: PASSED

- crates/famp-transport-http/Cargo.toml — FOUND
- crates/famp-transport-http/src/error.rs — FOUND
- crates/famp-transport-http/src/lib.rs — FOUND
- crates/famp-envelope/src/peek.rs — FOUND
- crates/famp/src/runtime/peek.rs — FOUND (thin wrapper, 13 lines)
- Commit f13bc9e — FOUND
- Commit 336e156 — FOUND
- Commit f849be1 — FOUND

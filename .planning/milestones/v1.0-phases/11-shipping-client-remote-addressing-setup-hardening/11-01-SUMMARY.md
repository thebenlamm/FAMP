---
phase: 11-shipping-client-remote-addressing-setup-hardening
plan: 01
subsystem: infra
tags: [thiserror, error-handling, observability, reqwest, gateway]

# Dependency graph
requires:
  - phase: 09-end-to-end-cross-host-delivery
    provides: famp-gateway egress drain-sign-POST loop (relay_one, run_egress) and HttpTransport
provides:
  - HttpTransportError::ReqwestFailed/InvalidUrl Display strings that interpolate the wrapped source error
  - RelayError::Transport holding the typed HttpTransportError via #[from] instead of a flattened String
  - egress drain-loop log rendering {e:?} (full #[source] chain) instead of {e} (Display)
affects: [11-07 (own-domain trust check builds on this egress.rs), any future cross-host debugging in Phase 11]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "thiserror Display interpolates {0}/field to expose #[source] text instead of a bare fixed-string prefix"
    - "operator-visible eprintln logs render {e:?} (Debug) not {e} (Display) so nested #[source] chains (incl. TLS leaf causes reqwest's own Display can omit) reach the log"

key-files:
  created: []
  modified:
    - crates/famp-transport-http/src/error.rs
    - crates/famp-gateway/src/egress.rs

key-decisions:
  - "InvalidUrl's Display was also fixed alongside ReqwestFailed (not explicitly required but flagged in read_first as the same drops-source pattern) -- both variants now interpolate {0}."
  - "Used url::Url parse failure (not a real reqwest::Error) to build the HttpTransportError in the famp-gateway RelayError test, avoiding a new reqwest dev-dependency in famp-gateway; famp-transport-http's own error.rs tests construct a real reqwest::Error via Client::get(bad_url).send().await synchronously (no network I/O)."

patterns-established:
  - "Error::source() chain must stay walkable end-to-end: transport-http Display carries the source, and the gateway relay error wraps it via #[from] rather than e.to_string(), and the drain-loop log renders Debug not Display."

requirements-completed: [OBS-01]

coverage:
  - id: D1
    description: "HttpTransportError::ReqwestFailed and InvalidUrl Display strings interpolate the wrapped source error text instead of a bare fixed prefix"
    requirement: "OBS-01"
    verification:
      - kind: unit
        ref: "crates/famp-transport-http/src/error.rs#reqwest_failed_display_contains_source_text"
        status: pass
      - kind: unit
        ref: "crates/famp-transport-http/src/error.rs#invalid_url_display_contains_source_text"
        status: pass
    human_judgment: false
  - id: D2
    description: "RelayError::Transport holds the typed HttpTransportError via #[from] (not a flattened String), and the gateway-visible drain-loop log names the underlying cause"
    requirement: "OBS-01"
    verification:
      - kind: unit
        ref: "crates/famp-gateway/src/egress.rs#relay_error_transport_display_names_underlying_cause"
        status: pass
      - kind: integration
        ref: "just ci (workspace build/test incl. e2e_cross_host_delivery)"
        status: pass
    human_judgment: false

duration: 33min
completed: 2026-07-28
status: complete
---

# Phase 11 Plan 01: Un-swallow the transport error chain Summary

**HttpTransportError Display strings now interpolate their wrapped source, and famp-gateway's RelayError::Transport carries the typed error (not a flattened String) so the egress drain-loop log renders the full #[source] chain via `{e:?}` instead of the opaque `{e}` Display.**

## Performance

- **Duration:** 33 min
- **Started:** 2026-07-28T10:19:57-04:00
- **Completed:** 2026-07-28T10:51:45-04:00
- **Tasks:** 2 completed
- **Files modified:** 2

## Accomplishments
- `HttpTransportError::ReqwestFailed` and `::InvalidUrl` Display strings now interpolate the wrapped error text (`"reqwest failure: {0}"` / `"invalid url: {0}"`), no longer masking TLS/connect/parse causes behind a fixed prefix.
- `famp-gateway`'s `RelayError::Transport` changed from `Transport(String)` (flattened via `e.to_string()`) to `Transport(#[from] HttpTransportError)`, keeping `Error::source()` walkable end-to-end from `HttpTransport::send` through to the drain loop.
- The `run_egress` drain-loop `eprintln!` now renders `{e:?}` (Debug, walks the full `#[source]` chain) instead of `{e}` (Display, which can still hide a TLS leaf cause reqwest's own Display omits) — this is the actual gateway-visible operator log line.
- Two new unit tests in `famp-transport-http` and one in `famp-gateway` regression-pin the source-preserving behavior.

## Task Commits

Each task was committed atomically:

1. **Task 1: Preserve the source in HttpTransportError Display (OBS-01)** - `2f81680` (fix)
2. **Task 2: Capture the full source chain at the egress relay log (OBS-01)** - `cf4f8e8` (fix)

**Plan metadata:** (this commit, following)

## Files Created/Modified
- `crates/famp-transport-http/src/error.rs` - `ReqwestFailed`/`InvalidUrl` Display now interpolate `{0}`; two new unit tests
- `crates/famp-gateway/src/egress.rs` - `RelayError::Transport` holds typed `HttpTransportError` via `#[from]`; `relay_one`'s `.map_err` propagates the typed error; drain-loop log uses `{e:?}`; one new unit test

## Decisions Made
- Fixed `InvalidUrl`'s Display alongside `ReqwestFailed` (same drops-source pattern flagged in the plan's `read_first` audit instruction, not just the explicitly named variant).
- Built the `famp-gateway` regression test's `HttpTransportError` from a `url::ParseError` (via `InvalidUrl`) rather than a real `reqwest::Error`, avoiding a new `reqwest` dev-dependency in `famp-gateway` — the `famp-transport-http` crate's own tests already cover the real-`reqwest::Error` case directly (`Client::get(bad_url).send().await` fails synchronously with no network I/O).

## Deviations from Plan

None - plan executed exactly as written. Both tasks matched the plan's `<action>` instructions precisely (Display interpolation, `#[from]` propagation, `{e:?}` drain-loop log, unit tests for both).

## Issues Encountered
- First `just lint` pass on Task 1 failed on `clippy::expect_used` inside the new test module — fixed by adding the same `#[allow(clippy::unwrap_used, clippy::expect_used)]` attribute already used by every sibling test module in this file (`middleware.rs`, `transport.rs`, `tls.rs` all follow this convention).
- First `just lint` pass on Task 2 failed on `clippy::items_after_statements` (pedantic) for a `use std::error::Error as _;` placed after other statements inside the test body — moved the `use` to the top of the function.
- A one-off `just ci` run hit a stale-`target/` incremental-cache error (`failed to create dependency graph ... No such file or directory`) unrelated to this plan's changes — matches the known project note that stale incremental artifacts intermittently break `cargo build` (see MEMORY.md `project_stale_target_http_happy_path`). Re-running `just ci` succeeded cleanly.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Plan 07 (wave 2) can now build the `from.authority() == own-domain` trust check on top of `egress.rs` with a source-preserving `RelayError::Transport` already in place.
- Every subsequent cross-host debugging session in Phase 11 will see the actual TLS/connect/status cause in the gateway's drain-loop log instead of the opaque `"reqwest failure"` string.
- No blockers.

---
*Phase: 11-shipping-client-remote-addressing-setup-hardening*
*Completed: 2026-07-28*

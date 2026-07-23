---
phase: 07-broker-liveness-fork-gateway-skeleton
plan: 01
subsystem: infra
tags: [rust, uds, broker-liveness, gateway, tokio, thiserror]

requires: []
provides:
  - "famp-gateway crate (lib + killable [[bin]]) as a registered workspace member"
  - "BusClient::connect_no_spawn — additive no-auto-spawn UDS connect constructor"
  - "ProxiedPrincipal::register — Design A's Register-with-gateway's-own-PID mechanism"
  - "GatewayRegistry — per-principal demux table enforcing GW-04 (no cross-talk)"
affects: [07-02, 07-03, 08-signed-cross-host-envelope-trust-bootstrap]

tech-stack:
  added: []
  patterns:
    - "Design A: back a remote principal with a plain Register carrying the gateway's own std::process::id() on a dedicated UDS connection — rides the broker's existing kill(pid,0) sweep unmodified"
    - "connect_no_spawn: fail-loud UDS connect variant (no spawn_broker_if_absent) for long-running service processes, contrasted with connect()'s CLI/MCP auto-spawn convenience"
    - "One ProxiedPrincipal (one UDS connection) per remote principal name — GatewayRegistry rejects a second back() for an already-backed name rather than silently sharing"

key-files:
  created:
    - crates/famp-gateway/Cargo.toml
    - crates/famp-gateway/src/lib.rs
    - crates/famp-gateway/src/error.rs
    - crates/famp-gateway/src/principal.rs
    - crates/famp-gateway/src/registry.rs
    - crates/famp-gateway/src/main.rs
  modified:
    - Cargo.toml
    - crates/famp/src/bus_client/mod.rs

key-decisions:
  - "GatewayError::Io/BrokerDidNotStart-family connect failures collapse to GatewayError::BrokerUnreachable — the must-have 'fails loud' truth, not a generic Io passthrough"
  - "famp-gateway depends on famp (path dep) to reuse BusClient::connect_no_spawn rather than rolling a minimal UDS client against famp-bus directly — the A3 default from 07-RESEARCH.md; dep tree was not unacceptably heavy"
  - "[[bin]] famp-gateway hand-rolls --socket/positional-name arg parsing (no clap dependency added) since the crate's dependency list is deliberately minimal (famp-bus, famp, tokio, thiserror only)"

requirements-completed: [LIVE-01, LIVE-02, GW-04]

coverage:
  - id: D1
    description: "famp-gateway crate scaffolded as a workspace member, compiling as a lib + killable [[bin]] famp-gateway"
    requirement: "GW-04"
    verification:
      - kind: unit
        ref: "cargo build -p famp-gateway && cargo build -p famp-gateway --bin famp-gateway"
        status: pass
    human_judgment: false
  - id: D2
    description: "ProxiedPrincipal::register sends Register { pid: std::process::id(), listen: false } over a connect_no_spawn connection — the gateway's own live PID, never the remote's"
    requirement: "LIVE-01"
    verification:
      - kind: unit
        ref: "grep -c 'std::process::id()' crates/famp-gateway/src/principal.rs (>=1) and grep -c 'listen: false' (>=1)"
        status: pass
    human_judgment: false
  - id: D3
    description: "connect_no_spawn omits spawn::spawn_broker_if_absent (fail loud), while connect() keeps auto-spawn unchanged — additive, no regression"
    requirement: "LIVE-02"
    verification:
      - kind: unit
        ref: "cargo test -p famp --lib bus_client (13/13 pass)"
        status: pass
    human_judgment: false
  - id: D4
    description: "GatewayRegistry demuxes strictly by principal name and rejects a duplicate back() call (GW-04 no cross-talk)"
    requirement: "GW-04"
    verification:
      - kind: unit
        ref: "code inspection: GatewayRegistry::back returns GatewayError::DuplicatePrincipal on a repeat name (no integration test in this plan; covered by plan 07-03)"
        status: unknown
    human_judgment: true
    rationale: "This plan is scaffold-only per its own scope (07-RESEARCH.md Wave 0 Gaps); the actual GW-04 SIGKILL/no-cross-talk integration test is explicitly deferred to plan 07-03 (tests/liveness.rs, tests/no_cross_talk.rs). No automated test exists yet to point at."
  - id: D5
    description: "Zero famp-bus source change (Design A honored)"
    verification:
      - kind: unit
        ref: "git diff --name-only -- crates/famp-bus/ | wc -l"
        status: pass
    human_judgment: false

duration: 45min
completed: 2026-07-23
status: complete
---

# Phase 07 Plan 01: Broker-Liveness Fork + Gateway Skeleton — Scaffold Summary

**Stood up the `famp-gateway` crate and Design A's core mechanism: `ProxiedPrincipal::register` backs each remote principal with a dedicated no-spawn UDS `Register` carrying the gateway's own `std::process::id()`, riding the broker's unmodified `kill(pid,0)` liveness sweep.**

## Performance

- **Duration:** ~45 min
- **Tasks:** 2/2 completed
- **Files modified:** 6 (1 modified in `crates/famp`, 5 created in `crates/famp-gateway`), plus root `Cargo.toml`

## Accomplishments
- `famp-gateway` crate registered as a workspace member, compiling as both a lib and a killable `[[bin]] famp-gateway` (LIVE-02's process-exit test needs a real OS process)
- `BusClient::connect_no_spawn` added additively in `crates/famp/src/bus_client/mod.rs` — identical Hello handshake to `connect()` minus `spawn::spawn_broker_if_absent`, so the gateway fails loud (`BrokerUnreachable`) instead of papering over an absent daemon
- `ProxiedPrincipal::register` implements the exact Design A wire pattern: `Hello{bind_as:None}` then `Register{name, pid: std::process::id(), cwd:None, listen:false}` — the gateway's own real PID, never the remote's
- `GatewayRegistry` demuxes strictly by principal name via `HashMap<String, ProxiedPrincipal>`, rejecting a duplicate `back()` call with `GatewayError::DuplicatePrincipal` instead of silently sharing a connection
- `[[bin]] famp-gateway` parses `--socket <path>` (default via `famp::bus_client::resolve_sock_path`) plus 1+ principal names, backs each, and parks on `ctrl_c` — holding the registry keeps every connection (and thus the reported PID) alive
- Zero `crates/famp-bus/` source change — Design A confirmed to ride the existing, unmodified `register()`/`tick()`/`kill(pid,0)` path

## Task Commits

1. **Task 1: Scaffold the famp-gateway crate (compiles empty)** - `a03d317` (feat)
2. **Task 2: Implement connect_no_spawn + ProxiedPrincipal + GatewayRegistry + parking bin** - `fa99693` (feat)

## Files Created/Modified
- `crates/famp-gateway/Cargo.toml` - workspace-member manifest; deps famp-bus, famp, tokio (rt/sync/macros/net/signal), thiserror; dev-dep assert_cmd; `[[bin]] famp-gateway`
- `crates/famp-gateway/src/lib.rs` - crate root, `#![forbid(unsafe_code)]`, re-exports `GatewayError`/`ProxiedPrincipal`/`GatewayRegistry`
- `crates/famp-gateway/src/error.rs` - `GatewayError` (thiserror): `Io`, `BrokerUnreachable`, `HelloFailed`, `RegisterFailed`, `UnexpectedReply`, `DuplicatePrincipal`
- `crates/famp-gateway/src/principal.rs` - `ProxiedPrincipal::register` (the Design A register-with-own-PID mechanism) + `map_bus_client_err` translation layer
- `crates/famp-gateway/src/registry.rs` - `GatewayRegistry::back`/`get`/`names` — the GW-04 demux point
- `crates/famp-gateway/src/main.rs` - `[[bin]] famp-gateway`: arg parsing, backs N principals, parks on `ctrl_c`
- `Cargo.toml` - added `"crates/famp-gateway"` workspace member, immediately after `"crates/famp-transport-http"`
- `crates/famp/src/bus_client/mod.rs` - additive `BusClient::connect_no_spawn(sock_path, bind_as)` constructor

## Decisions Made
- `GatewayError`'s connect-failure mapping collapses `BusClientError::Io`/`BrokerDidNotStart` to `GatewayError::BrokerUnreachable` (rather than exposing a raw Io passthrough) so the "fails loud on daemon absence" must-have truth is unambiguous at the gateway's own error-type boundary.
- Reused `famp::bus_client::BusClient` (path dependency on the `famp` umbrella crate) rather than rolling a minimal UDS client directly against `famp-bus::codec` — 07-RESEARCH.md's A3 assumption held; the dependency tree was not unacceptably heavy (crate compiles clean, `just lint` green workspace-wide).
- Hand-rolled `--socket`/positional-name CLI parsing in `main.rs` instead of adding a `clap` dependency, keeping `famp-gateway`'s dependency surface to exactly the four crates in Cargo.toml (per the phase's Package Legitimacy Audit: zero new external packages).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `too_long_first_doc_paragraph` clippy failure in registry.rs**
- **Found during:** Task 2 verification (`just lint`)
- **Issue:** The module-doc comment on `GatewayRegistry` exceeded clippy's first-doc-paragraph length limit, failing the `-D warnings` gate
- **Fix:** Split into a short summary line + a continuation paragraph
- **Files modified:** `crates/famp-gateway/src/registry.rs`
- **Verification:** `just lint` exits 0
- **Committed in:** `fa99693` (Task 2 commit)

**2. [Rule 3 - Blocking] `unused_crate_dependencies` warnings across lib/bin targets**
- **Found during:** Task 1 and Task 2 verification (`cargo build` / `just lint`)
- **Issue:** `famp`, `famp-bus`, `tokio`, and the dev-only `assert_cmd` are declared as dependencies but not every one is referenced by every compilation unit (lib target vs. bin target vs. test cfg are separate units) at each point in the two-task sequence
- **Fix:** Added scoped `use <crate> as _;` silencers (the same idiom already used by `famp-transport-http`), removing each silencer as the corresponding module started using the real dependency
- **Files modified:** `crates/famp-gateway/src/lib.rs`, `crates/famp-gateway/src/main.rs`
- **Verification:** `cargo build -p famp-gateway`, `cargo build -p famp-gateway --bin famp-gateway`, and `just lint` all exit 0
- **Committed in:** `a03d317`, `fa99693`

---

**Total deviations:** 2 auto-fixed (both Rule 3 — blocking clippy/build issues, not scope creep)
**Impact on plan:** No behavior change; both fixes were required for the plan's own stated `just lint` acceptance criterion to pass.

## Issues Encountered

**Pre-existing, unrelated `cargo test -p famp --lib` failures (5 tests, out of scope):** `cli::install::codex::tests::*` and `cli::uninstall::codex::tests::uninstall_after_install_removes_famp_table` fail with "resolved famp binary does not support `hook codex-stop`" — a `target/debug/famp` build-staleness issue from concurrent, unrelated session activity already present in the working tree at plan start (`crates/famp/src/cli/install/codex.rs`, `crates/famp/src/cli/hook/*`, none of which this plan touches or committed). The targeted acceptance-relevant subset, `cargo test -p famp --lib bus_client`, passes 13/13, and `just lint` is clean workspace-wide. Logged to `deferred-items.md`; not fixed (out of this plan's scope per the deviation-rules scope boundary).

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `famp-gateway` crate and `connect_no_spawn` are in place for Plan 07-02 (the pure-`Broker<E>` LIVE-01 test extending `crates/famp-bus/src/broker/handle/tests.rs`) and Plan 07-03 (the SIGKILL-based LIVE-02/GW-04 integration tests against the new `[[bin]] famp-gateway`).
- No blockers. `GatewayRegistry::back`'s `DuplicatePrincipal` rejection path and the `[[bin]]`'s parking behavior are implemented but not yet integration-tested under a real SIGKILL — that is Plan 07-03's explicit job, not a gap in this plan.

---
*Phase: 07-broker-liveness-fork-gateway-skeleton*
*Completed: 2026-07-23*

## Self-Check: PASSED

All 6 created/modified `crates/famp-gateway/` files and the SUMMARY.md itself confirmed present on disk; all 3 commit hashes (`a03d317`, `fa99693`, and this summary's own docs commit) confirmed in `git log`.

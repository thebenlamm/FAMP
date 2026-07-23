---
phase: 07-broker-liveness-fork-gateway-skeleton
plan: 03
subsystem: testing
tags: [rust, uds, broker-liveness, gateway, integration-test, assert_cmd, tokio]

requires:
  - phase: 07-01
    provides: "famp-gateway crate (lib + killable [[bin]]), connect_no_spawn, ProxiedPrincipal, GatewayRegistry"
provides:
  - "crates/famp-gateway/tests/liveness.rs — LIVE-02 real-process SIGKILL/reap test"
  - "crates/famp-gateway/tests/no_cross_talk.rs — GW-04 real-process isolation test"
  - "crates/famp-gateway/tests/common/child_guard.rs — ChildGuard test-util, copied into this crate"
affects: [08-signed-cross-host-envelope-trust-bootstrap]

tech-stack:
  added: []
  patterns:
    - "Cross-package Command::cargo_bin(\"famp\") from famp-gateway's tests relies on assert_cmd's shared-workspace-target-dir fallback, not CARGO_BIN_EXE_famp (Cargo does not propagate that env var across a package boundary) — an explicit `cargo build -p famp --bin famp` prebuild step makes this hermetic"
    - "GW-04 isolation proven via a D-10 bind_as proxy connection (BusClient::connect(sock, Some(\"bob\"))) as the sender, reusing the gateway's own live registration — no extra spawned process needed"
    - "Cross-correlating a tagged send without reading message bodies: envelope.id + body.event==famp.send.new_task makes both SendOk.task_id and the inspector's task_id row resolve to the same value (INSP-MSG-01 never exposes bodies)"

key-files:
  created:
    - crates/famp-gateway/tests/common/child_guard.rs
    - crates/famp-gateway/tests/liveness.rs
    - crates/famp-gateway/tests/no_cross_talk.rs
  modified:
    - crates/famp-gateway/Cargo.toml
    - crates/famp-gateway/src/lib.rs
    - crates/famp-gateway/src/main.rs
    - Cargo.lock

key-decisions:
  - "Added an explicit cargo build -p famp --bin famp prebuild step inside both test files (Rule 3 — blocking fix): Cargo only sets CARGO_BIN_EXE_<name> for bin targets in the SAME package as the test; famp-gateway's tests need famp's bin (a different package), so relying on assert_cmd's target_dir fallback alone would make the test pass/fail based on unrelated build-order luck rather than being hermetic."
  - "GW-04 sender is a D-10 bind_as proxy connection onto the gateway's own live 'bob' registration (BusClient::connect(sock, Some(\"bob\"))), not a third spawned process — bob sending to alice under ONE gateway process is exactly the GW-04 scenario, and avoids needing an extra registered identity."
  - "Correlation via envelope.id + body.event=='famp.send.new_task' rather than message body content, since famp inspect messages never exposes bodies (INSP-MSG-01) — this makes SendOk.task_id and the inspector row's task_id resolve identically."

requirements-completed: [LIVE-02, GW-04]

coverage:
  - id: D1
    description: "A real famp-gateway process backing 2 principals (alice, bob) keeps both live in famp inspect identities while it runs; SIGKILLing it reaps both within one broker sweep interval (~1s), leaving no orphan holders"
    requirement: "LIVE-02"
    verification:
      - kind: integration
        ref: "cargo test -p famp-gateway --test liveness live02_gateway_exit_reaps_all_principals -- --nocapture"
        status: pass
    human_judgment: false
  - id: D2
    description: "A single gateway process backing alice and bob never lets a message addressed to alice appear in bob's mailbox"
    requirement: "GW-04"
    verification:
      - kind: integration
        ref: "cargo test -p famp-gateway --test no_cross_talk gw04_no_cross_talk_between_proxied_principals -- --nocapture"
        status: pass
    human_judgment: false

duration: 70min
completed: 2026-07-23
status: complete
---

# Phase 07 Plan 03: Broker-Liveness Fork + Gateway Skeleton — LIVE-02/GW-04 Subprocess Proof Summary

**Real-process SIGKILL/reap and cross-talk-isolation subprocess tests (`crates/famp-gateway/tests/{liveness,no_cross_talk}.rs`) proving Design A's LIVE-02 and GW-04 guarantees against a genuine `famp-gateway` OS process, not a pure-`Broker<E>` unit double.**

## Performance

- **Duration:** ~70 min
- **Tasks:** 2/2 completed
- **Files modified:** 7 (3 created under `crates/famp-gateway/tests/`, 3 modified in `crates/famp-gateway/{Cargo.toml,src/lib.rs,src/main.rs}`, 1 lockfile)

## Accomplishments
- `crates/famp-gateway/tests/common/child_guard.rs` — ChildGuard RAII helper copied verbatim from `crates/famp/tests/common/child_guard.rs`
- `crates/famp-gateway/tests/liveness.rs` — `live02_gateway_exit_reaps_all_principals`: spawns a real broker + a real `famp-gateway` process backing `alice`+`bob`, polls `famp inspect identities --json` with a bounded deadline until BOTH are observed live (also proves LIVE-01 at the subprocess level), SIGKILLs the gateway, then polls until BOTH are reaped within the ~1s `TICK_INTERVAL` sweep — no orphan holders
- `crates/famp-gateway/tests/no_cross_talk.rs` — `gw04_no_cross_talk_between_proxied_principals`: ONE gateway process backs `alice`+`bob`; `bob` (via a D-10 `bind_as` proxy connection onto the gateway's own live registration) sends a uniquely-tagged message to `alice`; asserts the tag lands in alice's mailbox and never appears in bob's
- Both tests are ChildGuard-wrapped for every spawned child and poll-with-deadline throughout (never a fixed `sleep()`-then-assert, per 07-RESEARCH.md Pitfall 4)
- Both tests were falsification-checked (see Deviations) to confirm they would genuinely fail if the underlying guarantee were broken, not just trivially pass

## Task Commits

1. **Task 1: LIVE-02 — gateway exit reaps all proxied principals (SIGKILL + poll)** - `672eef0` (feat)
2. **Task 2: GW-04 — no cross-talk between two proxied principals** - `1d2d052` (feat)

## Files Created/Modified
- `crates/famp-gateway/tests/common/child_guard.rs` - ChildGuard RAII kill+wait-on-drop helper (copied verbatim)
- `crates/famp-gateway/tests/liveness.rs` - LIVE-02 subprocess test: broker+gateway spawn helpers, socket-readiness poll, live/reap poll-with-deadline helpers, the test itself
- `crates/famp-gateway/tests/no_cross_talk.rs` - GW-04 subprocess test: reuses the same spawn-helper shape (minimally duplicated per-file so each file's own `ChildGuard` grep-gate is self-contained), D-10 bind_as sender, tagged-envelope correlation
- `crates/famp-gateway/Cargo.toml` - added `tempfile`, `serde_json`, `uuid`, `famp-inspect-proto` dev-dependencies
- `crates/famp-gateway/src/lib.rs` - `#[cfg(test)] use <dep> as _;` silencers for the new dev-only dependencies
- `crates/famp-gateway/src/main.rs` - same silencer pattern for the `[[bin]]`'s own unittest compilation unit
- `Cargo.lock` - picks up the 4 new dev-dependency entries

## Decisions Made
- **Explicit `cargo build -p famp --bin famp` prebuild step** inside both test files, called at the top of each `#[test]`/`#[tokio::test]` fn. Empirically verified (2026-07-23) that Cargo does NOT propagate `CARGO_BIN_EXE_famp` — at compile time via `env!()` OR at runtime via `std::env::var` — to a *different* package's test binary; it is only set for bin targets in the SAME package as the compiling test. `crates/famp/tests/broker_proxy_semantics.rs`'s `Command::cargo_bin("famp")` "just works" today only because `-p famp` always builds famp's own `[[bin]]` as a side effect of testing that package. `famp-gateway`'s tests cross that package boundary, so without this prebuild step, `cargo test -p famp-gateway --test liveness` would panic with `CARGO_BIN_EXE_famp is unset` on a clean checkout with no prior `-p famp` build — a real, reproducible reliability gap, not a hypothetical one (confirmed by deleting `target/debug/famp` and reproducing the panic before adding the fix).
- **GW-04 sender is a D-10 `bind_as` proxy connection**, not a third spawned process: `BusClient::connect(sock, Some("bob".into()))` piggybacks on the gateway's own live `bob` registration — the same mechanism `famp send --as bob` uses. This is a more faithful GW-04 scenario (bob, backed by the SAME gateway process as alice, sends to alice) than introducing an unrelated third identity, and keeps the test simpler.
- **Tag-based correlation instead of body-content matching**: the envelope carries `{"id": <uuidv7>, "body": {"event": "famp.send.new_task"}}`, which both the broker's `SendOk.task_id` (`task_id_from`, `handle.rs:1157`) and the inspector's `MessageRow.task_id` (`EnvelopeView::task_id`, third resolution branch) resolve to identically — needed because `famp inspect messages` never exposes body content (INSP-MSG-01).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Gateway spawned before the broker's socket was bound, causing an immediate exit(1)**
- **Found during:** Task 1, first test run
- **Issue:** `BusClient::connect_no_spawn` (the gateway's own connect path, plan 07-01) makes exactly ONE connect attempt with no retry/backoff, unlike the CLI's `connect()`. Spawning `famp-gateway` immediately after `famp broker` (with zero readiness wait) raced the broker's socket bind; the gateway hit `NotFound`/`ConnectionRefused`, printed an error, and exited before registering anything — which looked identical to a genuine LIVE-01/LIVE-02 failure (both principals simply never appeared live).
- **Fix:** Added `wait_for_broker_socket()` — a bounded-deadline poll on a raw `UnixStream::connect` — before spawning the gateway in both test files.
- **Files modified:** `crates/famp-gateway/tests/liveness.rs`, `crates/famp-gateway/tests/no_cross_talk.rs`
- **Verification:** Both tests pass reliably; re-ran each 3x with no flakes.
- **Committed in:** `672eef0`, `1d2d052`

**2. [Rule 3 - Blocking] Cross-package `Command::cargo_bin("famp")` not hermetic**
- **Found during:** Task 1, investigating the CARGO_BIN_EXE_famp mechanism before relying on it
- **Issue:** See "Decisions Made" above — Cargo does not set `CARGO_BIN_EXE_famp` for `famp-gateway`'s test binaries at all (empirically confirmed with instrumented probe tests, then reverted). Without a fix, the test's correctness would depend on `target/debug/famp` already existing from an unrelated prior build.
- **Fix:** Added `ensure_famp_bin_built()` (invokes `cargo build --quiet -p famp --bin famp` via `std::process::Command`, using the `CARGO` env var Cargo does reliably set) at the top of each test.
- **Files modified:** `crates/famp-gateway/tests/liveness.rs`, `crates/famp-gateway/tests/no_cross_talk.rs`
- **Verification:** Deleted `target/debug/famp`, re-ran both tests standalone (`cargo test -p famp-gateway --test <name>`) — both passed without any other package having been built first.
- **Committed in:** `672eef0`, `1d2d052`

**3. [Rule 3 - Blocking] `unused_crate_dependencies` warnings for the 4 new dev-dependencies**
- **Found during:** Task 1, `just lint`
- **Issue:** `tempfile`, `serde_json`, `uuid`, `famp-inspect-proto` are only referenced by the integration test binaries, not by the lib (`src/lib.rs`) or bin (`src/main.rs`) unittest compilation units, tripping the workspace's `unused_crate_dependencies = "warn"` lint (elevated to `-D warnings` by `just lint`).
- **Fix:** Added `#[cfg(test)] use <dep> as _;` silencers in both `lib.rs` and `main.rs`, matching the existing project convention already used there for `assert_cmd`.
- **Files modified:** `crates/famp-gateway/src/lib.rs`, `crates/famp-gateway/src/main.rs`
- **Verification:** `cargo clippy -p famp-gateway --all-targets -- -D warnings` and full `just lint` both exit 0.
- **Committed in:** `672eef0`

---

**Total deviations:** 3 auto-fixed, all Rule 3 (blocking correctness/hermeticity issues surfaced by actually running the tests, not scope creep).
**Impact on plan:** No behavior change to the gateway/broker under test; all three fixes were required for the plan's own stated acceptance criteria (tests pass reliably, `just lint` clean) to hold on more than "it happened to work on this exact machine right now."

## Issues Encountered

None beyond the deviations above. The pre-existing `cli::install::codex`/`cli::uninstall::codex` test-staleness issue noted in 07-01-SUMMARY.md ("Issues Encountered") was independently observed to have resolved itself during this plan's `cargo nextest run -p famp-bus -p famp-gateway -p famp` wave-merge verification (560/560 passed, 4 skipped) — nextest's own fresh-build step regenerated `target/debug/famp`, eliminating the staleness. Not touched by this plan; noted here only because the same full-workspace nextest run incidentally re-verified it.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- LIVE-01, LIVE-02, and GW-04 are now proven at BOTH the pure-`Broker<E>` unit level (plan 07-02) and the real-OS-process subprocess level (this plan) — Design A is fully validated for the phase's success criteria.
- No blockers for Phase 8 (signed cross-host envelope trust bootstrap): `famp-gateway`'s skeleton (crate, `[[bin]]`, `GatewayRegistry`, `ProxiedPrincipal`) is proven to keep proxied principals live and isolated; Phase 8 can build the signing/verification and actual cross-host wire transport on top without revisiting this liveness mechanism.
- The `ensure_famp_bin_built()` / cross-package `cargo_bin` pattern discovered here is worth keeping in mind for any FUTURE `famp-gateway` (or other non-`famp`-package) test that needs to spawn the `famp` CLI binary — it is not automatic via `assert_cmd::Command::cargo_bin` alone across a package boundary.

---
*Phase: 07-broker-liveness-fork-gateway-skeleton*
*Completed: 2026-07-23*

## Self-Check: PASSED

All 3 created files (`crates/famp-gateway/tests/common/child_guard.rs`, `crates/famp-gateway/tests/liveness.rs`, `crates/famp-gateway/tests/no_cross_talk.rs`) and this SUMMARY.md confirmed present on disk; both commit hashes (`672eef0`, `1d2d052`) confirmed in `git log --oneline`.

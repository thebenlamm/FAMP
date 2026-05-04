---
phase: 01-famp-bus-library-and-audit-log
plan: 01
subsystem: protocol-library
tags: [rust, famp-bus, canonical-json, proptest, just]

requires:
  - phase: v0.8 substrate
    provides: famp-canonical, famp-core, famp-envelope, famp-inbox patterns
provides:
  - tokio-free `famp-bus` workspace crate scaffold
  - Bus protocol message/reply/error types
  - length-prefixed canonical-JSON frame codec
  - read-only mailbox and liveness test abstractions
  - TDD-01 green codec fuzz test plus TDD-02/03/04 compile-red broker scaffolds
affects: [phase-01-plan-02, phase-01-plan-03, phase-02-uds-wire]

tech-stack:
  added: [regex]
  patterns: [closed serde enums, exhaustive consumer stub, sync frame codec, sealed-style trait bundle]

key-files:
  created:
    - crates/famp-bus/Cargo.toml
    - crates/famp-bus/src/lib.rs
    - crates/famp-bus/src/proto.rs
    - crates/famp-bus/src/codec.rs
    - crates/famp-bus/src/error.rs
    - crates/famp-bus/src/mailbox.rs
    - crates/famp-bus/src/liveness.rs
    - crates/famp-bus/src/env.rs
    - crates/famp-bus/tests/codec_fuzz.rs
    - crates/famp-bus/tests/buserror_consumer_stub.rs
    - crates/famp-bus/tests/tdd02_drain_cursor_order.rs
    - crates/famp-bus/tests/tdd03_pid_reuse.rs
    - crates/famp-bus/tests/tdd04_eof_cleanup.rs
  modified:
    - Cargo.toml
    - Cargo.lock
    - Justfile

key-decisions:
  - "TDD-02/03/04 remain compile-red by design until Plan 01-02 adds the Broker actor."
  - "Bus-side drained envelopes remain serde_json::Value until Plan 01-03 introduces AnyBusEnvelope."
  - "The no-tokio gate now fails closed if cargo tree cannot run."

patterns-established:
  - "BUS-05 exhaustive enum consumers use an explicit ALL array plus deny(unreachable_patterns)."
  - "Mailbox cursors count each in-memory line as line.len() + 1 to match JSONL disk offsets."

requirements-completed: [BUS-01, BUS-02, BUS-03, BUS-04, BUS-05, BUS-06, BUS-10, TDD-01, TDD-02, TDD-03, TDD-04, CARRY-04]

duration: 23min
completed: 2026-04-27
---

# Phase 01 Plan 01: famp-bus Scaffold and Primitives Summary

**Tokio-free `famp-bus` substrate with canonical frame codec, protocol types, mailbox/liveness traits, and RED-first broker tests.**

## Performance

- **Duration:** 23 min
- **Started:** 2026-04-27T19:45:29Z
- **Completed:** 2026-04-27T20:08:01Z
- **Tasks:** 2
- **Files modified:** 17

## Accomplishments

- Added `famp-bus` as a workspace crate with narrow runtime dependencies and a `just check-no-tokio-in-bus` CI gate.
- Implemented `BusMessage`, `BusReply`, `Target`, `Delivered`, `SessionRow`, `BusErrorKind`, `ClientId`, and `AwaitFilter`.
- Implemented the 4-byte big-endian canonical-JSON codec with max-size, empty-frame, split-read, and partial-prefix coverage.
- Added `MailboxRead`, `InMemoryMailbox`, `LivenessProbe`, `FakeLiveness`, and `BrokerEnv`.
- Added TDD-01 as green proptests and TDD-02/03/04 as deliberate compile-red broker scaffolds for Plan 01-02.

## Task Commits

1. **Task 1: Crate scaffold + workspace registration + Justfile no-tokio gate** - `0a116f5` (feat)
2. **Task 2 RED: Add failing bus codec and broker red gates** - `c604f03` (test)
3. **Task 2 GREEN: Implement famp-bus primitives** - `235c752` (feat)

## Verification

- `cargo build -p famp-bus` - PASS
- `cargo nextest run -p famp-bus --lib --test codec_fuzz --test buserror_consumer_stub` - PASS, 17/17 tests
- `PATH="/Users/benlamm/.cargo/bin:$PATH" just check-no-tokio-in-bus` - PASS
- `cargo tree -p famp-bus --edges normal | grep -E '^\s*tokio v'` - PASS by no match
- `cargo clippy -p famp-bus --lib --no-deps -- -D warnings` - PASS

## Files Created/Modified

- `Cargo.toml` - registered `crates/famp-bus` and pinned workspace `regex`.
- `Cargo.lock` - locked regex runtime dependencies.
- `Justfile` - added and wired the no-tokio gate.
- `crates/famp-bus/src/*.rs` - new protocol, codec, error, mailbox, liveness, and env modules.
- `crates/famp-bus/tests/*.rs` - codec fuzz, exhaustive consumer, shared test env, and broker RED gates.

## Decisions Made

Followed the plan's compile-red convention for TDD-02/03/04 instead of introducing premature broker stubs. `MailboxRead` uses a sealed-style supertrait while allowing the external integration-test `TestEnv` builder required by the plan.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed no-tokio gate false success when cargo is missing**
- **Found during:** Task 2 verification
- **Issue:** `just check-no-tokio-in-bus` could print success when `cargo` was absent from PATH because the pipeline's grep returned no matches.
- **Fix:** Added an explicit `command -v cargo` guard and made `cargo tree` failure fail the recipe.
- **Files modified:** `Justfile`
- **Verification:** `PATH="/Users/benlamm/.cargo/bin:$PATH" just check-no-tokio-in-bus`
- **Committed in:** `235c752`

**2. [Rule 1 - Bug] Corrected overlong channel-name test vector**
- **Found during:** Task 2 lib tests
- **Issue:** The initial "overlong" channel test used a name exactly at the regex maximum, so it deserialized successfully.
- **Fix:** Increased the test string by one character so it exceeds `^#[a-z0-9][a-z0-9_-]{0,31}$`.
- **Files modified:** `crates/famp-bus/src/proto.rs`
- **Verification:** `cargo nextest run -p famp-bus --lib`
- **Committed in:** `235c752`

**3. [Rule 3 - Blocking] Reconstructed STATE.md after SDK truncation**
- **Found during:** State update
- **Issue:** `gsd-sdk query state.advance-plan` reduced `.planning/STATE.md` to frontmatter only.
- **Fix:** Reconstructed a concise valid state file with current position, decisions, metrics, and session fields.
- **Files modified:** `.planning/STATE.md`
- **Verification:** Manual readback of `.planning/STATE.md`
- **Committed in:** final metadata commit

**Total deviations:** 3 auto-fixed (2 Rule 1 bugs, 1 Rule 3 blocker).  
**Impact on plan:** No product scope expansion; fixes preserve planned verification and GSD continuity.

## Issues Encountered

- Full dependency clippy via `cargo clippy -p famp-bus --lib -- -D warnings` fails in pre-existing `crates/famp-envelope/src/version.rs` doc markdown before reaching `famp-bus`. Logged in `deferred-items.md`; `cargo clippy -p famp-bus --lib --no-deps -- -D warnings` passes for the new crate.
- `gsd-sdk query state.advance-plan` truncated `.planning/STATE.md`; reconstructed it before final metadata commit.

## Known Stubs

- `crates/famp-bus/tests/tdd02_drain_cursor_order.rs`, `tdd03_pid_reuse.rs`, and `tdd04_eof_cleanup.rs` intentionally reference missing `Broker`, `BrokerInput`, and `Out` types. This is the plan's RED-first compile-fail gate; Plan 01-02 makes them compile and pass.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 01-02 can add the pure broker actor on top of the committed protocol types, read traits, liveness fake, and compile-red tests. Plan 01-03 can replace `serde_json::Value` drain fields with the typed bus envelope decoder when `AnyBusEnvelope` lands.

## Self-Check: PASSED

- Summary file exists.
- Key created files exist.
- Task commits `0a116f5`, `c604f03`, and `235c752` exist in git history.

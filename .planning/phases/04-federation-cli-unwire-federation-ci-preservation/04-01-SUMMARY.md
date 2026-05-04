---
phase: 04-federation-cli-unwire-federation-ci-preservation
plan: 01
subsystem: testing
tags: [rust, nextest, federation, http-transport, keyring]

requires:
  - phase: 03-claude-code-integration-polish
    provides: v0.9 local-bus positioning before federation CLI unwire
provides:
  - library-API HTTPS e2e coverage for famp-transport-http
  - unsigned-envelope middleware short-circuit sentinel
affects: [federation-cli-unwire, v1.0-federation-preservation, just-ci]

tech-stack:
  added: []
  patterns:
    - same-process HTTPS integration test using build_router and tls_server
    - black-box mpsc try_recv sentinel for handler-not-entered proof

key-files:
  created:
    - crates/famp/tests/e2e_two_daemons_adversarial.rs
    - .planning/phases/04-federation-cli-unwire-federation-ci-preservation/deferred-items.md
  modified:
    - crates/famp/tests/e2e_two_daemons.rs

key-decisions:
  - "Copied the http_happy_path.rs library-API body into e2e_two_daemons.rs, changing only the Phase 4 doc comment and test function name."
  - "Kept the adversarial sentinel independent of famp::runtime because the runtime module is removed later in Phase 4."

patterns-established:
  - "Federation plumb-line tests consume famp-transport-http directly instead of spawning the famp CLI."
  - "Unsigned-envelope rejection is asserted by HTTP non-success plus an empty inbox receiver."

requirements-completed: [FED-03]

duration: 9min
completed: 2026-05-04
---

# Phase 04 Plan 01: Federation E2E Preservation Summary

**Library-API federation HTTPS coverage now exercises request -> commit -> deliver -> ack directly, with a sibling unsigned-envelope middleware sentinel.**

## Performance

- **Duration:** 9 min
- **Started:** 2026-05-04T00:41:22Z
- **Completed:** 2026-05-04T00:50:21Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Replaced the 9-line `e2e_two_daemons.rs` skeleton with a direct `famp-transport-http` library-API HTTPS test.
- Added `e2e_two_daemons_adversarial.rs`, proving an unsigned envelope is rejected before the inbox handler queues a message.
- Preserved the exact happy-path conversation shape from `tests/http_happy_path.rs`: request, commit, deliver, ack.

## Task Commits

Each task was committed atomically:

1. **Task 1: Refactor e2e_two_daemons.rs to library-API happy path** - `c4cf1c2` (test)
2. **Task 2: Add adversarial sentinel proving handler-not-entered on unsigned envelope** - `3c9c01e` (test)

## Files Created/Modified

- `crates/famp/tests/e2e_two_daemons.rs` - Library-API HTTPS happy path copied from `tests/http_happy_path.rs`; source lines 16-162 were preserved except the test function rename, and the header was replaced with the Phase 4 plumb-line note.
- `crates/famp/tests/e2e_two_daemons_adversarial.rs` - Plain-HTTP unsigned-envelope sentinel using `build_router`, `InboxRegistry`, and `mpsc::Receiver::try_recv`.
- `.planning/phases/04-federation-cli-unwire-federation-ci-preservation/deferred-items.md` - Out-of-scope `just ci` clippy findings logged for follow-up.

## Verification

- PASS: `cargo nextest run -p famp -E 'test(=e2e_two_daemons_happy_path)' --no-fail-fast` -> 1 passed after rerun outside the sandbox for loopback listener binding.
- PASS: `cargo nextest run -p famp -E 'test(=e2e_two_daemons_rejects_unsigned)' --no-fail-fast` -> 1 passed.
- PASS: `cargo nextest run -p famp -E 'test(/e2e_two_daemons/)' --no-fail-fast` -> 2 passed.
- PASS: `cargo tree --workspace -i famp-keyring` lists `famp` and `famp-transport-http` as reverse consumers.
- PASS: `cargo tree --workspace -i famp-transport-http` lists `famp` as a reverse consumer.
- FAIL (out of scope): `just ci` reaches clippy and fails on pre-existing `clippy::option_if_let_else` findings in `crates/famp/src/cli/install/claude_code.rs:200` and `crates/famp/src/cli/uninstall/claude_code.rs:212`.

## Decisions Made

- Used the existing `http_happy_path.rs` body rather than introducing a new harness, matching the plan's "verbatim template" requirement.
- Used a plain HTTP adversarial rig because the middleware rejection behavior is independent of TLS and the existing adversarial test suite uses the same pattern.
- Did not import or project through `famp::runtime::RuntimeError` in the sentinel because Phase 4 later deletes the runtime module.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Adjusted Keyring insertion API in adversarial sentinel**
- **Found during:** Task 2
- **Issue:** The plan snippet used `Keyring::insert`, but the actual API exposes `with_peer`.
- **Fix:** Built the sentinel keyring with `Keyring::new().with_peer(...).unwrap()`.
- **Files modified:** `crates/famp/tests/e2e_two_daemons_adversarial.rs`
- **Verification:** `cargo nextest run -p famp -E 'test(=e2e_two_daemons_rejects_unsigned)' --no-fail-fast` passed.
- **Committed in:** `3c9c01e`

---

**Total deviations:** 1 auto-fixed (Rule 3).
**Impact on plan:** No scope expansion; the fix was required for the sentinel to compile against the current keyring API.

## Issues Encountered

- The first happy-path test run failed in the sandbox with `Operation not permitted` while binding local TCP listeners. The same command passed outside the sandbox.
- `just ci` is not green at this point because of unrelated clippy findings in install/uninstall Claude Code code. These were logged to `deferred-items.md` and left untouched per plan scope.

## User Setup Required

None - no external service configuration required.

## Known Stubs

None.

## Next Phase Readiness

Plan 04-02 can move/freeze federation CLI tests with the library-API plumb-line already committed. FED-04 / TEST-06 still need a green `just ci`; the current blocker is unrelated clippy in pre-existing files outside Plan 04-01 scope.

## Self-Check: PASSED

- FOUND: `crates/famp/tests/e2e_two_daemons.rs`
- FOUND: `crates/famp/tests/e2e_two_daemons_adversarial.rs`
- FOUND: `.planning/phases/04-federation-cli-unwire-federation-ci-preservation/04-01-SUMMARY.md`
- FOUND: task commit `c4cf1c2`
- FOUND: task commit `3c9c01e`

---
*Phase: 04-federation-cli-unwire-federation-ci-preservation*
*Completed: 2026-05-04*

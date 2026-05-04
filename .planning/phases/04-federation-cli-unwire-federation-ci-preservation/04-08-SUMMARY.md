---
phase: 04-federation-cli-unwire-federation-ci-preservation
plan: 08
subsystem: cli
tags: [rust, clap, cargo-tree, federation-cleanup]

requires:
  - phase: 04-06
    provides: v0.8.1-federation-preserved tag at debed78f1b55df44fb2ca18687c5794147226a40
provides:
  - Hard deletion of federation CLI verbs and TLS-form send path
  - Self-contained info command with inlined PeerCard and load_identity
  - FED/MIGRATE/TEST-06 REQUIREMENTS and ROADMAP closeout
affects: [famp-cli, federation-v1, local-bus]

tech-stack:
  added: []
  patterns:
    - Test-only federation library preservation via dev-dependencies
    - Runtime helper logic localized to active tests after CLI runtime deletion

key-files:
  created:
    - crates/famp/tests/cli_help_invariant.rs
  modified:
    - crates/famp/src/cli/mod.rs
    - crates/famp/src/cli/info.rs
    - crates/famp/src/cli/send/mod.rs
    - crates/famp/Cargo.toml
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md

key-decisions:
  - "Risk #1 resolved by inlining PeerCard and load_identity into info.rs rather than keeping setup/init stubs."
  - "Runtime/signal helpers used by live local-bus tests were moved to surviving utility/test-local modules instead of preserving deleted runtime/listen modules."
  - "FED-06 verification uses cargo tree --workspace -i for dev-dependency reverse edges; production no-dev checks show no famp consumer."

patterns-established:
  - "Deleted federation CLI verbs fail through clap unknown-subcommand behavior; no soft-deprecation stubs remain."
  - "Federation crates remain workspace members and dev-dependencies for e2e preservation only."

requirements-completed: [FED-01, FED-05, FED-06, MIGRATE-01, MIGRATE-02, MIGRATE-03, MIGRATE-04]

duration: 23min
completed: 2026-05-04
---

# Phase 04 Plan 08: Federation CLI Deletion Summary

**Federation CLI verbs hard-deleted while preserving the HTTP federation test line as dev-only coverage.**

## Performance

- **Duration:** 23 min
- **Started:** 2026-05-04T01:41:15Z
- **Completed:** 2026-05-04T02:04:12Z
- **Tasks:** 3
- **Files modified:** 47

## Accomplishments

- Removed `famp init`, `famp setup`, `famp listen`, `famp peer`, and TLS-form `famp send` without compatibility stubs.
- Kept `famp info` by making `info.rs` self-contained with local `PeerCard` and private `load_identity`.
- Moved `famp-keyring` and `famp-transport-http` out of `famp` production dependencies while preserving e2e coverage through dev-dependencies.
- Closed FED/MIGRATE/TEST-06 rows in REQUIREMENTS and marked Phase 4 complete in ROADMAP.

## Task Commits

1. **Deletion sweep + test gate:** `1935bef` (`feat!(04): remove federation CLI surface (init, setup, listen, peer, TLS-form send)`)
2. **Closeout docs + summary:** included in the closeout commit containing this file (`docs(04): close FED + MIGRATE + TEST-06 requirements`)

## Files Deleted

Exact deleted line counts from `git show --numstat`:

| Path | Deleted lines |
| --- | ---: |
| `crates/famp/src/cli/init/atomic.rs` | 79 |
| `crates/famp/src/cli/init/mod.rs` | 296 |
| `crates/famp/src/cli/init/tls.rs` | 136 |
| `crates/famp/src/cli/listen/auto_commit.rs` | 167 |
| `crates/famp/src/cli/listen/mod.rs` | 277 |
| `crates/famp/src/cli/listen/router.rs` | 106 |
| `crates/famp/src/cli/listen/signal.rs` | 38 |
| `crates/famp/src/cli/peer/add.rs` | 75 |
| `crates/famp/src/cli/peer/import.rs` | 52 |
| `crates/famp/src/cli/peer/mod.rs` | 57 |
| `crates/famp/src/cli/send/client.rs` | 323 |
| `crates/famp/src/cli/send/fsm_glue.rs` | 98 |
| `crates/famp/src/cli/setup.rs` | 311 |
| `crates/famp/src/runtime/adapter.rs` | 72 |
| `crates/famp/src/runtime/error.rs` | 49 |
| `crates/famp/src/runtime/loop_fn.rs` | 77 |
| `crates/famp/src/runtime/mod.rs` | 12 |
| `crates/famp/src/runtime/peek.rs` | 13 |
| `crates/famp/examples/_gen_fixture_certs.rs` | 71 |

## Verification Evidence

- `cargo build -p famp` passed.
- `cargo nextest run -p famp -E 'test(=famp_help_omits_deleted_federation_verbs)'`: 1 passed.
- `cargo nextest run -p famp -E 'test(/e2e_two_daemons/)'`: 2 passed.
- `cargo nextest run -p famp --no-fail-fast`: 187 passed, 1 skipped.
- `just ci`: passed; final line was `local CI-parity checks passed`.
- Clap exits: `init/setup/listen/peer --help` returned 2; `send/register/info --help` returned 0.
- `cargo tree -i openssl`: package not found, so OpenSSL is absent.

## FED-06 Cargo Tree Evidence

Production graph:

```text
$ cargo tree --workspace --edges no-dev -i famp-transport-http
famp-transport-http v0.1.0 (.../crates/famp-transport-http)

$ cargo tree --workspace --edges no-dev -i famp-keyring
famp-keyring v0.1.0 (.../crates/famp-keyring)
└── famp-transport-http v0.1.0 (.../crates/famp-transport-http)
```

Dev graph:

```text
$ cargo tree --workspace -i famp-transport-http
famp-transport-http v0.1.0 (.../crates/famp-transport-http)
[dev-dependencies]
└── famp v0.1.0 (.../crates/famp)

$ cargo tree --workspace -i famp-keyring
famp-keyring v0.1.0 (.../crates/famp-keyring)
└── famp-transport-http v0.1.0 (.../crates/famp-transport-http)
    [dev-dependencies]
    └── famp v0.1.0 (.../crates/famp)
[dev-dependencies]
└── famp v0.1.0 (.../crates/famp)
```

Interpretation: no production `famp` umbrella consumer remains; `famp` reaches the federation crates only as dev-dependencies for active e2e preservation.

## D-07 Tag Invariant

`git log v0.8.1-federation-preserved..HEAD --oneline` after this plan shows:

```text
docs(04): close FED + MIGRATE + TEST-06 requirements
1935bef feat!(04): remove federation CLI surface (init, setup, listen, peer, TLS-form send)
26ff039 docs(04-06): record pre-tag gate resolution
```

The post-tag log does not contain the Plan 04-01 e2e refactor. The existing `26ff039` follow-up summary commit was present before this plan and records the pre-tag gate resolution.

## Requirement Closure

- FED-01..06: 6/6 checked.
- MIGRATE-01..04: 4/4 checked.
- TEST-06: checked.
- CARRY-01 was already checked in Plan 04-07.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Relocated shared shutdown signal helper**
- **Found during:** Task 3
- **Issue:** `broker` and `register` still referenced `cli::listen::signal::shutdown_signal` after the `listen` module deletion.
- **Fix:** Moved the signal future into `cli::util` and updated callers.
- **Files modified:** `crates/famp/src/cli/util.rs`, `crates/famp/src/cli/broker/mod.rs`, `crates/famp/src/cli/register.rs`
- **Verification:** `cargo build -p famp`, `just ci`
- **Committed in:** `1935bef`

**2. [Rule 3 - Blocking] Rehomed active runtime test helpers**
- **Found during:** Task 3
- **Issue:** Active tests and examples still imported deleted `famp::runtime` helpers.
- **Fix:** Localized the runtime glue into `tests/common/cycle_driver.rs`, rewired adversarial/runtime unit tests, and simplified examples to use the shared driver.
- **Files modified:** `crates/famp/tests/common/cycle_driver.rs`, `crates/famp/tests/runtime_unit.rs`, `crates/famp/tests/adversarial/*`, `crates/famp/examples/personal_two_agents.rs`, `crates/famp/examples/cross_machine_two_agents.rs`
- **Verification:** `cargo nextest run -p famp --no-fail-fast`, `just ci`
- **Committed in:** `1935bef`

**3. [Rule 3 - Blocking] Removed remaining active `famp init` test setup**
- **Found during:** Task 3
- **Issue:** MCP tests still shelled `famp init`, which correctly fails after deletion.
- **Fix:** Updated MCP harnesses to rely on broker registration and isolated bus sockets.
- **Files modified:** `crates/famp/tests/mcp_malformed_input.rs`, `crates/famp/tests/common/mcp_harness.rs`
- **Verification:** `cargo nextest run -p famp --no-fail-fast`, `just ci`
- **Committed in:** `1935bef`

**Total deviations:** 3 auto-fixed (Rule 3). **Impact:** All were direct fallout from the planned hard deletion; no federation CLI stubs were retained.

## Known Stubs

None.

## Issues Encountered

None remaining. The plan exposed stale active test harness references to deleted federation-era setup, and those were updated to the local-bus path.

## User Setup Required

None.

## Next Phase Readiness

Phase 4 is closed. `v0.9.0` can be cut from a commit after this summary/closeout commit if desired: CI is green, OpenSSL is absent, and the preservation tag remains at `debed78f1b55df44fb2ca18687c5794147226a40`.

## Self-Check: PASSED

- Summary file exists.
- Code deletion commit exists: `1935bef`.
- REQUIREMENTS and ROADMAP closeout rows are staged with this summary.

---
*Phase: 04-federation-cli-unwire-federation-ci-preservation*
*Completed: 2026-05-04*

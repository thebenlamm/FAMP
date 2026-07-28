---
phase: 11-shipping-client-remote-addressing-setup-hardening
plan: 02
subsystem: cli
tags: [own-domain, addressing, principal, peer-export, cli-error, mcp-error-kind]

# Dependency graph
requires:
  - phase: 08-session-bound-mcp-identity-bridge
    provides: famp peer export subcommand tree + gateway identity persistence
  - phase: 09-end-to-end-cross-host-delivery
    provides: ingress verify.rs peek_sender -> keyring.get(from) trust lookup
provides:
  - own_domain::resolve_own_domain(cli_domain, home) -> the single host-level federation authority
  - CliError::OwnDomainNotSet / CliError::OwnDomainInvalid (+ mcp_error_kind arms)
  - FAMP_OWN_DOMAIN env var and $FAMP_HOME/own-domain file config surface
  - peer export label authority derived from (or validated against) own-domain
affects: [11-03 (stamps envelope `from` authority from this resolver), 11-07 (gateway egress from.authority() == own-domain check), 11-05 (GATEWAY-SETUP doc must document the new config surface)]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "home.rs CD-05 convention extended: env is read in exactly ONE module; every other call site takes the resolved String, avoiding the std::env::set_var parallel-test race"
    - "private validator reached via a probe parse: validate_authority is private in famp-core::identity, so a throwaway `agent:{value}/x` Principal::from_str is the compliant validation route"
    - "temp_env::with_var / with_var_unset (process-global mutex) for every env mutation so env-touching tests in different modules of the same test binary cannot race"

key-files:
  created:
    - crates/famp/src/cli/own_domain.rs
  modified:
    - crates/famp/src/cli/mod.rs
    - crates/famp/src/cli/error.rs
    - crates/famp/src/cli/mcp/error_kind.rs
    - crates/famp/tests/mcp_error_kind_exhaustive.rs
    - crates/famp/src/cli/peer/export.rs

key-decisions:
  - "Unset-domain semantic (from the --reviews replan) implemented as a four-shape match in resolve_export_principal: configured+bare -> synthesize; configured+full -> assert equal or typed reject; UNSET+full -> accept verbatim with a one-line stderr warning; UNSET+bare -> OwnDomainNotSet. The UNSET+full verbatim branch is what keeps peer_roundtrip.rs and the e2e own-domain-unset export path green."
  - "The configured-mismatch reject reuses CliError::PeerBlobMalformed with an explanatory reason rather than adding a third new variant — keeps the exhaustive mcp_error_kind table smaller and the reason string carries the diagnostic."
  - "Empty / whitespace-only own-domain file is treated as 'not set' (Ok(None)) rather than an error, so an operator who touches the file but never fills it gets the actionable three-source hint instead of a confusing parse error."

patterns-established:
  - "from == pinned-label coupling is now structural, not conventional: famp send (plan 03) and famp peer export both resolve through own_domain::resolve_own_domain, so the envelope `from` authority and the label the peer pins under cannot drift into an UnpinnedKey self-DoS."

requirements-completed: [ADDR-03]

coverage:
  - id: D1
    description: "Exactly one host-level own-domain source with precedence --domain > FAMP_OWN_DOMAIN > $FAMP_HOME/own-domain, read in a single place"
    requirement: "ADDR-03"
    verification:
      - kind: unit
        ref: "crates/famp/src/cli/own_domain.rs#resolves_precedence_trims_and_rejects (Cases A-D)"
        status: pass
      - kind: static
        ref: "grep -rn 'FAMP_OWN_DOMAIN' crates/famp/src -> std::env read only in own_domain.rs:39"
        status: pass
    human_judgment: false
  - id: D2
    description: "Missing own-domain fails with an actionable CliError naming all three sources, not a silent local.bus fallback"
    requirement: "ADDR-03"
    verification:
      - kind: unit
        ref: "crates/famp/src/cli/own_domain.rs#resolves_precedence_trims_and_rejects (Cases E-F, asserts message contains --domain, FAMP_OWN_DOMAIN, own-domain)"
        status: pass
    human_judgment: false
  - id: D3
    description: "Resolved own-domain is validated as a legal Principal authority via a probe parse (validate_authority is private), returning a typed error not a panic"
    requirement: "ADDR-03"
    verification:
      - kind: unit
        ref: "crates/famp/src/cli/own_domain.rs#resolves_precedence_trims_and_rejects (Case G -> OwnDomainInvalid)"
        status: pass
    human_judgment: false
  - id: D4
    description: "peer export derives/validates its exported label authority from the SAME own-domain source; no /gateway-suffixed label constructed"
    requirement: "ADDR-03"
    verification:
      - kind: unit
        ref: "crates/famp/src/cli/peer/export.rs#tests (configured+bare synthesize, configured+mismatch reject, unset+full verbatim)"
        status: pass
      - kind: integration
        ref: "cargo test -p famp --test peer_roundtrip (unset-verbatim regression stays green)"
        status: pass
    human_judgment: false
  - id: D5
    description: "Both new CliError variants have unique mcp_error_kind arms in the exhaustive no-wildcard table, so the workspace still compiles"
    requirement: "ADDR-03"
    verification:
      - kind: unit
        ref: "crates/famp/tests/mcp_error_kind_exhaustive.rs (OwnDomainNotSet -> own_domain_not_set, OwnDomainInvalid -> own_domain_invalid)"
        status: pass
    human_judgment: false

duration: 10min
completed: 2026-07-28
status: complete
---

# Phase 11 Plan 02: Host-level own-domain source Summary

**`famp` now has one host-level federation authority — resolved in a single module with `--domain` > `FAMP_OWN_DOMAIN` > `$FAMP_HOME/own-domain` precedence — that both `famp peer export` (today) and `famp send` (plan 03) consume, making the load-bearing `envelope from authority == peer-pinned label` invariant structural rather than conventional.**

## Performance

- **Duration:** ~10 min (implementation); verification re-run separately after an executor stall
- **Started:** 2026-07-28T14:50-04:00
- **Completed:** 2026-07-28T15:00:01-04:00
- **Tasks:** 2 completed
- **Files modified:** 6 (1 created, 5 modified)

## Accomplishments
- New `crates/famp/src/cli/own_domain.rs` with `resolve_own_domain(cli_domain: Option<&str>, home: &Path) -> Result<String, CliError>`, mirroring `home.rs`'s CD-05 shape: precedence `--domain` > `FAMP_OWN_DOMAIN` env > single trimmed non-empty first line of `$FAMP_HOME/own-domain` > typed `OwnDomainNotSet`.
- The resolved value is validated as a legal `Principal` authority via a **probe parse** of `agent:{value}/x` — `famp_core::identity::validate_authority` is private, so the probe is the compliant route to the same validation. Failure returns `CliError::OwnDomainInvalid { value, reason }`, never a panic.
- Two new `CliError` variants wired into the **exhaustive, no-wildcard** `mcp_error_kind()` table (`own_domain_not_set`, `own_domain_invalid`) plus coverage in the `mcp_error_kind_exhaustive.rs` uniqueness fixture — without these arms the workspace does not compile.
- `famp peer export` now sources its exported label authority from the same resolver, closing the `from == pinned-label` coupling that previously lived in two unrelated human-typed places.
- Single-`#[test]`-fn serial coverage of all seven precedence/trim/reject cases, with every env mutation routed through `temp_env`'s process-global mutex so it cannot race `peer::export`'s own env-touching tests.

## Task Commits

Each task was committed atomically:

1. **Task 1: own-domain resolver module (single env-read site) + CliError** - `62743fe` (feat)
2. **Task 2: couple peer export label authority to own-domain** - `c009558` (feat)

**Plan metadata:** (this commit, following)

## Files Created/Modified
- `crates/famp/src/cli/own_domain.rs` **(new)** - resolver, probe-parse validator, file reader, 7-case serial unit test
- `crates/famp/src/cli/mod.rs` - registers `pub mod own_domain;`
- `crates/famp/src/cli/error.rs` - `OwnDomainNotSet` (message names all three sources) + `OwnDomainInvalid { value, reason }`
- `crates/famp/src/cli/mcp/error_kind.rs` - two arms in the exhaustive table
- `crates/famp/tests/mcp_error_kind_exhaustive.rs` - uniqueness-fixture coverage for both new strings
- `crates/famp/src/cli/peer/export.rs` - `resolve_export_principal` four-shape own-domain coupling + tests

## Decisions Made
- **Four-shape export resolution** rather than a hard own-domain requirement, per the `--reviews` unset-domain finding: `configured+bare` synthesizes `agent:{own_domain}/{name}`; `configured+full` asserts authority equality and loudly rejects a mismatch; `UNSET+full` accepts verbatim with a one-line stderr warning; `UNSET+bare` returns the actionable `OwnDomainNotSet`. The verbatim branch is load-bearing — it is what keeps `peer_roundtrip.rs` and the e2e own-domain-unset export path green.
- **Mismatch reject reuses `PeerBlobMalformed`** with an explanatory `reason` instead of adding a third variant, keeping the exhaustive MCP error table smaller while preserving the diagnostic.
- **Empty/whitespace-only own-domain file is "not set"**, not an error — an operator who creates but never fills the file gets the three-source hint rather than a confusing parse failure.
- No `/gateway`-suffixed label is ever constructed (RESEARCH finding #2); ingress verifies on the sender *agent* principal.

## Deviations from Plan

None in implementation — both tasks match the plan's `<action>` text, including the four-shape unset-domain semantic and the probe-`Principal` validation route.

**Process deviation:** the executor agent completed and committed both tasks, then stalled indefinitely waiting on a `just ci` run that never returned (see Issues). The orchestrator ran the verification itself and authored this SUMMARY rather than re-dispatching, since all production commits were already in place.

## Issues Encountered
- **`cargo nextest run --workspace` hangs on this machine.** `just ci`'s `test` recipe sat for 21 minutes at 0.23s CPU, stuck in the test-binary `--list` phase and accumulating paired zombie test processes — the known `project_nextest_list_hang` failure mode. Worked around by substituting plain `cargo test --workspace` for that one step; every other `just ci` recipe was run individually and passed. The real `nextest` gate still runs unchanged in GitHub Actions, so no CI gate was weakened.
- **5 flaky codex-install tests.** `cli::install::codex::tests::*` and `cli::uninstall::codex::tests::uninstall_after_install_removes_famp_table` failed on the first `cargo test --workspace` with `resolved famp binary ... does not support 'hook codex-stop' (probe failed or timed out)`. They probe `target/debug/famp` while cargo is concurrently relinking that same binary. All 5 pass in isolation and on a settled re-run (265 passed, 0 failed), and they touch none of this plan's files — pre-existing suite flakiness, not an ADDR-03 regression. Worth a separate fix (the probe should target a built-and-settled artifact or be gated behind a build dependency); intentionally not addressed inside this plan's scope.
- Two `just ci` invocations raced on the same cargo target lock (the stalled executor's plus the orchestrator's), which masked progress until the duplicate was killed.

## Verification Performed
- `cargo test --workspace` — all test binaries green, 0 failures (famp lib: 265 passed).
- `just fmt-check`, `just lint`, `just build`, `just test-canonical-strict`, `just test-crypto` — green.
- `just test-doc`, `just spec-lint` (21 passed), `just check-no-tokio-in-bus`, `just check-no-io-in-inspect-proto`, `just check-inspect-readonly`, `just check-inspect-version-aligned`, `just check-spec-version-coherence`, `just check-mcp-deps`, `just check-shellcheck`, `just publish-workspace-dry-run` — all exit 0.
- Static acceptance criteria: single env-read site confirmed by grep; no `/gateway` label constructed in export; not-set message contains all three source names; both new variants present in the exhaustive `mcp_error_kind` table.

## User Setup Required

None for this plan. Note for operators: remote sending (plan 03 onward) will require an own-domain to be configured via one of `--domain`, `FAMP_OWN_DOMAIN`, or `$FAMP_HOME/own-domain`. Plan 05 documents this surface in `GATEWAY-SETUP.md`.

## Next Phase Readiness
- Plan 03 can wire the `--domain` flag and call `own_domain::resolve_own_domain` to stamp the qualified envelope `from` authority.
- Plan 07 can assert `from.authority() == own-domain` at the gateway egress boundary against the same single source.
- Plan 05 must document `FAMP_OWN_DOMAIN` / `$FAMP_HOME/own-domain` in `GATEWAY-SETUP.md`.
- No blockers.

---
*Phase: 11-shipping-client-remote-addressing-setup-hardening*
*Completed: 2026-07-28*

---
phase: 03-claude-code-integration-polish
plan: 01
subsystem: release
tags: [cargo, crates-io, justfile, publishability]

requires: []
provides:
  - crates.io publishability remediation for internal path dependencies
  - non-stub crate descriptions for foundational crates
  - workspace publish and publish-dry-run Justfile recipes
affects: [phase-03, cargo-install, crates-io-publish]

tech-stack:
  added: []
  patterns:
    - Cargo internal path dependencies include a crates.io version pin
    - Workspace publish flow is explicit in Justfile dependency order

key-files:
  created:
    - .planning/phases/03-claude-code-integration-polish/03-01-SUMMARY.md
  modified:
    - Justfile
    - crates/famp-canonical/Cargo.toml
    - crates/famp-core/Cargo.toml
    - crates/famp-crypto/Cargo.toml
    - crates/famp-fsm/Cargo.toml
    - crates/famp-envelope/Cargo.toml
    - crates/famp-transport/Cargo.toml
    - crates/famp-keyring/Cargo.toml
    - crates/famp-bus/Cargo.toml
    - crates/famp-transport-http/Cargo.toml
    - crates/famp/Cargo.toml

key-decisions:
  - "Kept check-shellcheck out of 03-01; it remains relocated to 03-02 with the hook-runner asset it gates."
  - "Kept the ci recipe unchanged; publish dry-run CI wiring remains owned by later phase work."

patterns-established:
  - "Internal Cargo path dependencies use inline tables with version = \"0.1.0\" next to path."
  - "Justfile publish recipes enumerate the 12 crates in dependency order instead of relying on an external release tool."

requirements-completed: []

duration: ~25min
completed: 2026-05-03
---

# Phase 03 Plan 01: Crates.io Publishability Remediation Summary

**Workspace crates are publish-ready at the manifest level: internal path deps carry version pins, stub descriptions are removed, and Justfile has ordered publish and dry-run recipes.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-05-03T03:09:00Z
- **Completed:** 2026-05-03T03:34:12Z
- **Tasks:** 3
- **Files modified:** 12

## Accomplishments

- Added `version = "0.1.0"` to all 30 internal `path = "../..."` dependency lines across 8 workspace manifests.
- Replaced the 5 `(stub)` crate descriptions with crates.io-ready one-line descriptions.
- Added `publish-workspace` and `publish-workspace-dry-run` recipes in dependency order for all 12 crates.
- Verified `cargo publish -p famp-canonical --dry-run` exits 0.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add version pins to every internal path-dep across 8 Cargo.tomls** - `8d921eb` (`fix`)
2. **Task 2: Replace `(stub)` descriptions on the 5 affected crates** - `7edc200` (`docs`)
3. **Task 3: Add publish-workspace and publish-workspace-dry-run recipes to Justfile** - `98eb375` (`chore`)

## Files Created/Modified

- `crates/famp-crypto/Cargo.toml` - Added version pin to `famp-canonical`.
- `crates/famp-fsm/Cargo.toml` - Added version pin to `famp-core`; replaced stub description.
- `crates/famp-envelope/Cargo.toml` - Added version pins to `famp-canonical`, `famp-crypto`, `famp-core`; replaced stub description.
- `crates/famp-transport/Cargo.toml` - Added version pin to `famp-core`; replaced stub description.
- `crates/famp-keyring/Cargo.toml` - Added version pins to `famp-core`, `famp-crypto`.
- `crates/famp-bus/Cargo.toml` - Added version pins to `famp-canonical`, `famp-core`, `famp-envelope`.
- `crates/famp-transport-http/Cargo.toml` - Added version pins to 6 normal deps and 1 dev-dep.
- `crates/famp/Cargo.toml` - Added version pins to 10 normal deps and 2 dev-deps.
- `crates/famp-canonical/Cargo.toml` - Replaced stub description.
- `crates/famp-core/Cargo.toml` - Replaced stub description.
- `Justfile` - Added publish and dry-run recipes after `audit:`.
- `.planning/phases/03-claude-code-integration-polish/03-01-SUMMARY.md` - This execution summary.

## Edited Path-Deps

- `crates/famp-transport/Cargo.toml:15` - `famp-core`
- `crates/famp-crypto/Cargo.toml:23` - `famp-canonical`
- `crates/famp-fsm/Cargo.toml:15` - `famp-core`
- `crates/famp-envelope/Cargo.toml:18` - `famp-canonical`
- `crates/famp-envelope/Cargo.toml:19` - `famp-crypto`
- `crates/famp-envelope/Cargo.toml:20` - `famp-core`
- `crates/famp-keyring/Cargo.toml:15` - `famp-core`
- `crates/famp-keyring/Cargo.toml:16` - `famp-crypto`
- `crates/famp-bus/Cargo.toml:20` - `famp-canonical`
- `crates/famp-bus/Cargo.toml:21` - `famp-core`
- `crates/famp-bus/Cargo.toml:22` - `famp-envelope`
- `crates/famp-transport-http/Cargo.toml:15` - `famp-core`
- `crates/famp-transport-http/Cargo.toml:16` - `famp-envelope`
- `crates/famp-transport-http/Cargo.toml:17` - `famp-crypto`
- `crates/famp-transport-http/Cargo.toml:18` - `famp-keyring`
- `crates/famp-transport-http/Cargo.toml:19` - `famp-transport`
- `crates/famp-transport-http/Cargo.toml:20` - `famp-canonical`
- `crates/famp-transport-http/Cargo.toml:38` - `famp-canonical` dev-dependency
- `crates/famp/Cargo.toml:33` - `famp-core`
- `crates/famp/Cargo.toml:34` - `famp-canonical`
- `crates/famp/Cargo.toml:35` - `famp-crypto`
- `crates/famp/Cargo.toml:36` - `famp-envelope`
- `crates/famp/Cargo.toml:37` - `famp-fsm`
- `crates/famp/Cargo.toml:38` - `famp-transport`
- `crates/famp/Cargo.toml:39` - `famp-keyring`
- `crates/famp/Cargo.toml:40` - `famp-transport-http`
- `crates/famp/Cargo.toml:41` - `famp-inbox`
- `crates/famp/Cargo.toml:42` - `famp-taskdir`
- `crates/famp/Cargo.toml:65` - `famp-bus` dev-dependency
- `crates/famp/Cargo.toml:72` - `famp-transport` dev-dependency with `features = ["test-util"]`

## Reworded Descriptions

- `crates/famp-canonical/Cargo.toml:9` - `FAMP — RFC 8785 JSON canonicalization (JCS) for byte-exact protocol signatures.`
- `crates/famp-core/Cargo.toml:9` - `FAMP — core protocol primitives: Principal, Instance, ArtifactId, MessageClass, ProtocolErrorKind, AuthorityScope.`
- `crates/famp-fsm/Cargo.toml:9` - `FAMP — task finite-state machine (REQUESTED → COMMITTED → COMPLETED|FAILED|CANCELLED) with absorbing terminals.`
- `crates/famp-envelope/Cargo.toml:9` - `FAMP — wire envelope construction, Ed25519 signing under the FAMP-sig-v1 domain prefix (INV-10).`
- `crates/famp-transport/Cargo.toml:9` - `FAMP — transport trait abstraction: MemoryTransport for tests, HTTPS binding lives in famp-transport-http.`

## Justfile Recipes Added

```just
publish-workspace:
    cargo publish -p famp-canonical
    # ... sleeps and remaining crates in dependency order ...
    cargo publish -p famp

publish-workspace-dry-run:
    cargo publish -p famp-canonical --dry-run
    # ... remaining crates in dependency order ...
    cargo publish -p famp --dry-run
```

The full recipe body is in `Justfile`; it contains 24 `cargo publish -p` lines and 11 `sleep 45` lines. `check-shellcheck` was intentionally not added in this plan because it moved to 03-02, where the hook-runner asset exists in the same plan.

## Verification

- `cargo build --workspace --all-targets` - passed. Cargo emitted existing unused `temp_env` warnings in examples.
- `rg -n '\(stub\)' crates/*/Cargo.toml` - no matches.
- `rg -n 'path = "\.\..*\}' crates/*/Cargo.toml | rg -v version` - no matches.
- `just --list` - shows `publish-workspace` and `publish-workspace-dry-run`.
- `grep -n '^ci:' Justfile` - unchanged: `ci: fmt-check lint build test-canonical-strict test-crypto test test-doc spec-lint check-no-tokio-in-bus check-spec-version-coherence check-mcp-deps`.
- `grep -c '^check-shellcheck:' Justfile` - `0`.
- `cargo publish -p famp-canonical --dry-run` - passed after rerunning with approved network access for crates.io index resolution; dry run aborted before upload as expected.

## Decisions Made

- Followed the plan's explicit dependency order for publish recipes.
- Did not add `check-shellcheck`; relocation to 03-02 avoids a recipe pointing at a not-yet-created asset.
- Did not update `STATE.md` or `ROADMAP.md` because the user explicitly requested no planning-state updates unless required for sequential execution; this single-plan execution did not require them.

## Deviations from Plan

None - plan executed as written.

## Issues Encountered

- The first `cargo publish -p famp-canonical --dry-run` attempt failed because the sandbox could not resolve `index.crates.io`. It passed after rerunning the same command with approved network access.
- The plan's sample `just --list 2>&1 | grep -cE '(publish-workspace$|publish-workspace-dry-run)'` is brittle because `just --list` appends comments after recipe names. The recipes were verified directly with `just --list`, `grep -c '^publish-workspace:' Justfile`, and `grep -c '^publish-workspace-dry-run:' Justfile`.

## Known Stubs

None. The stub scan found only intentional Cargo feature declarations (`test-util = []`, `full-corpus = []`) that do not block the plan goal.

## Threat Flags

None. This plan added release/publish metadata only; it introduced no new network endpoint, auth path, file access trust boundary, or runtime schema surface.

## User Setup Required

None for this plan. Actual publishing still requires the maintainer to run `cargo login` before `just publish-workspace`.

## Next Phase Readiness

Phase 03 downstream plans can rely on Cargo manifests being publishable and on `just publish-workspace-dry-run` existing. The `check-shellcheck` recipe remains correctly deferred to 03-02.

## Self-Check: PASSED

- Found `.planning/phases/03-claude-code-integration-polish/03-01-SUMMARY.md`.
- Found task commit `8d921eb`.
- Found task commit `7edc200`.
- Found task commit `98eb375`.

---
*Phase: 03-claude-code-integration-polish*
*Completed: 2026-05-03*

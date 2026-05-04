---
phase: 04-federation-cli-unwire-federation-ci-preservation
verified: 2026-05-04T02:25:50Z
status: passed
score: 12/12 must-haves verified
overrides_applied: 0
---

# Phase 4: Federation CLI Unwire Verification Report

**Phase Goal:** Federation CLI unwire, federation internals preservation, v0.8.1 escape-hatch tag, and v0.9 local-first bus documentation/requirements closeout.
**Verified:** 2026-05-04T02:25:50Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | FED-01: removed federation CLI verbs are gone from user-facing CLI | VERIFIED | `target/debug/famp init/setup/listen/peer --help` each exited 2 with "unrecognized subcommand"; top-level `famp --help` lists no `init`, `setup`, `listen`, or `peer`; `crates/famp/src/cli/init`, `listen`, `peer`, `setup.rs`, `runtime`, `send/client.rs`, and `send/fsm_glue.rs` do not exist. |
| 2 | FED-02: `famp-keyring` and `famp-transport-http` are relabeled as v1.0 federation internals and preserved | VERIFIED | Root `Cargo.toml` has exactly two `# v1.0 federation internals` comments above `crates/famp-keyring` and `crates/famp-transport-http`; both crates remain workspace members and were exercised by `just ci`. |
| 3 | FED-03: `e2e_two_daemons` targets library API directly | VERIFIED | `crates/famp/tests/e2e_two_daemons.rs` is 167 lines, imports `famp_transport_http::{build_router, tls, tls_server, HttpTransport}`, calls `build_router`, `tls_server::serve_std_listener`, and `cycle_driver`, and has no `cargo_bin` / `assert_cmd` subprocess usage. |
| 4 | FED-04: federation e2e preservation tests run green in CI | VERIFIED | `cargo nextest run -p famp -E 'test(/e2e_two_daemons/)' --no-fail-fast` passed 2/2; full `just ci` passed and included the workspace test suite with both e2e tests green. |
| 5 | FED-05: escape-hatch tag exists at the intended pre-deletion commit | VERIFIED | `git rev-parse v0.8.1-federation-preserved` returned `debed78f1b55df44fb2ca18687c5794147226a40`; `git tag --points-at debed78...` returned `v0.8.1-federation-preserved`; post-tag log contains only review/doc/deletion closeout commits, not the Plan 04-01 e2e refactor. |
| 6 | FED-06: federation crates are not production CLI dependencies and are preserved only through dev/e2e coverage | VERIFIED | `cargo tree --workspace --edges no-dev -i famp-transport-http` shows only the crate itself; `cargo tree --workspace --edges no-dev -i famp-keyring` shows only keyring via transport-http, with no `famp` consumer. Full dev graph shows `famp` as `[dev-dependencies]` consumer for both. |
| 7 | MIGRATE-01: migration doc has CLI mapping table | VERIFIED | `docs/MIGRATION-v0.8-to-v0.9.md` starts with a table mapping `famp init`, `setup`, `listen`, `peer add`, `peer import`, TLS-form `send`, and MCP env usage to v0.9 equivalents/removals. |
| 8 | MIGRATE-02: migration doc includes `.mcp.json` cleanup | VERIFIED | Migration doc has `.mcp.json cleanup` section instructing removal of `FAMP_HOME` / `FAMP_LOCAL_ROOT` env keys and `args: ["mcp"]`; review fix corrected the scope to manual project cleanup plus user-scope `install-claude-code`. |
| 9 | MIGRATE-03: README, CLAUDE, and MILESTONES use local-first/v1.0 staged framing | VERIFIED | `README.md` opens with "FAMP today is local-first" and v1.0 federation framing; `CLAUDE.md` has the same staged framing; `.planning/MILESTONES.md` v0.9 section says local-first today and federation at v1.0. |
| 10 | MIGRATE-04: prep-sprint `famp-local` is archived/frozen and superseded | VERIFIED | `scripts/famp-local` no longer exists; `docs/history/v0.9-prep-sprint/famp-local/famp-local` exists with 1316 lines; archive README marks it frozen and points bug fixes to the live `famp` binary / `famp-local hook`. |
| 11 | TEST-06: conformance gates still run in CI | VERIFIED | `Justfile` `ci` includes `test-canonical-strict` and `test-crypto`; full `just ci` passed, including `famp-canonical` RFC 8785 tests, `famp-crypto` RFC 8032 / §7.1c tests, doc tests, spec lint, and package checks. |
| 12 | CARRY-01: listen-subprocess nextest pin remains closed | VERIFIED | `.config/nextest.toml` has `listen-subprocess = { max-threads = 4 }`; `.planning/REQUIREMENTS.md` CARRY-01 row is checked and references `ebd0854`; traceability table marks CARRY-01 complete. |

**Score:** 12/12 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/famp/tests/e2e_two_daemons.rs` | Library-API HTTPS happy path | VERIFIED | 167 lines; direct transport-http imports; no CLI subprocess; targeted nextest passed. |
| `crates/famp/tests/e2e_two_daemons_adversarial.rs` | Unsigned-envelope middleware sentinel | VERIFIED | 122 lines; uses `build_router`, `InboxRegistry`, `AtomicBool`, `try_recv`; no `famp::runtime`; targeted nextest passed. |
| `crates/famp/tests/_deferred_v1/README.md` | Frozen federation test archive explainer | VERIFIED | Explains dormant v0.9 status, v1.0 reactivation trigger, e2e preservation tests, migration doc, and tag. |
| `docs/history/v0.9-prep-sprint/famp-local/famp-local` | Archived prep-sprint script | VERIFIED | Exists at archived path, 1316 lines; original `scripts/famp-local` path absent. |
| `docs/history/v0.9-prep-sprint/famp-local/README.md` | Frozen archive marker | VERIFIED | Marks script frozen and superseded by live `famp` / `famp-local hook`. |
| `docs/MIGRATION-v0.8-to-v0.9.md` | Migration guide | VERIFIED | 101 lines; table-first; includes CLI mapping, `.mcp.json` cleanup, `~/.famp/` cleanup, tag, deferred tests, workspace internals, and archive path. |
| `README.md`, `CLAUDE.md`, `.planning/MILESTONES.md`, `ARCHITECTURE.md` | v0.9 local-first/v1.0 federation framing | VERIFIED | Required staged-framing strings and migration/tag pointers present. |
| `Cargo.toml` | Federation internals workspace relabel | VERIFIED | Two `# v1.0 federation internals` comments immediately above preserved crate members. |
| `.planning/REQUIREMENTS.md` | Requirement closeout | VERIFIED | FED-01..06, MIGRATE-01..04, TEST-06, CARRY-01 all checked and traceability rows complete. |
| `.config/nextest.toml` | CARRY-01 listen-subprocess pin | VERIFIED | `listen-subprocess = { max-threads = 4 }` present. |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `e2e_two_daemons.rs` | `famp-transport-http` library API | direct imports/calls | VERIFIED | Imports `build_router`, `tls`, `tls_server`, `HttpTransport`; calls two routers and two TLS listeners. |
| `e2e_two_daemons_adversarial.rs` | middleware short-circuit proof | HTTP non-success + empty inbox receiver | VERIFIED | Uses `build_router`, `InboxRegistry`, `try_recv`; test `e2e_two_daemons_rejects_unsigned` passed. |
| `famp --help` | absence of deleted CLI verbs | clap command table | VERIFIED | Top-level help omits removed verbs; direct deleted-verb help calls exit 2. |
| `famp send/register/info` | surviving CLI surface | clap help | VERIFIED | `target/debug/famp send --help`, `register --help`, and `info --help` exit 0. `info` retained intentionally per 04-08 and 04-REVIEW-FIX CR-01 disposition. |
| `famp-transport-http` / `famp-keyring` | e2e test target only | cargo tree production/dev split | VERIFIED | No `famp` consumer under `--edges no-dev`; `famp` appears only under `[dev-dependencies]` in the full reverse tree. |
| `Justfile ci` | conformance gates | recipe dependencies | VERIFIED | `ci: fmt-check lint build test-canonical-strict test-crypto test test-doc spec-lint check-no-tokio-in-bus check-spec-version-coherence check-mcp-deps check-shellcheck publish-workspace-dry-run`; full `just ci` passed. |
| `docs/MIGRATION-v0.8-to-v0.9.md` | tag/deferred/archive references | doc links | VERIFIED | Contains `v0.8.1-federation-preserved`, `_deferred_v1`, and `docs/history/v0.9-prep-sprint/famp-local/`. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|---|---|---|---|---|
| `e2e_two_daemons.rs` | `trace_alice`, `trace_bob` | `cycle_driver::drive_alice` / `drive_bob` over two live `HttpTransport` instances and TLS listeners | Yes | VERIFIED |
| `e2e_two_daemons_adversarial.rs` | `inbox_rx` / `sentinel` | `InboxRegistry` mpsc receiver after POSTing unsigned JSON to live axum router | Yes; expected no message on rejection | VERIFIED |
| `crates/famp/src/cli/mcp/tools/peers.rs` | `online` | `BusMessage::Sessions` -> `BusReply::SessionsOk { rows }` | Yes | VERIFIED |
| Documentation artifacts | N/A | Static docs | N/A | NOT_APPLICABLE |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Top-level help omits deleted verbs | `cargo run --bin famp -- --help` | Exit 0; listed retained commands only | PASS |
| Deleted verb: init | `target/debug/famp init --help` | Exit 2, unrecognized subcommand | PASS |
| Deleted verb: setup | `target/debug/famp setup --help` | Exit 2, unrecognized subcommand | PASS |
| Deleted verb: listen | `target/debug/famp listen --help` | Exit 2, unrecognized subcommand | PASS |
| Deleted verb: peer | `target/debug/famp peer --help` | Exit 2, unrecognized subcommand | PASS |
| Bus send help retained | `target/debug/famp send --help` | Exit 0 | PASS |
| Register help retained | `target/debug/famp register --help` | Exit 0 | PASS |
| Info help intentionally retained | `target/debug/famp info --help` | Exit 0 | PASS |
| Help invariant test | `cargo nextest run -p famp -E 'test(=famp_help_omits_deleted_federation_verbs)'` | 1 passed | PASS |
| Federation preservation tests | `cargo nextest run -p famp -E 'test(/e2e_two_daemons/)' --no-fail-fast` | 2 passed | PASS |
| OpenSSL absent | `cargo tree -i openssl` | Package ID did not match any packages | PASS |
| CI parity | `just ci` | Final line: `local CI-parity checks passed` | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| FED-01 | 04-02, 04-08 | Top-level CLI removals | SATISFIED | Deleted commands absent from help and source directories removed; deferred tests moved. |
| FED-02 | 04-06 | Federation crates relabeled v1.0 internals and preserved | SATISFIED | Root `Cargo.toml` comments and workspace members verified; CI passes. |
| FED-03 | 04-01 | `e2e_two_daemons` library-API refactor | SATISFIED | Direct `famp-transport-http` usage; targeted e2e tests pass. |
| FED-04 | 04-01 | Federation e2e green in CI | SATISFIED | Targeted e2e tests and full `just ci` pass. |
| FED-05 | 04-06, 04-08 | Escape-hatch tag | SATISFIED | Tag points at `debed78f1b55df44fb2ca18687c5794147226a40`. |
| FED-06 | 04-08 | Federation crates consumed only by e2e/dev surface | SATISFIED | Cargo tree production/dev split verified. |
| MIGRATE-01 | 04-04, 04-08 | CLI mapping table | SATISFIED | Migration doc table verified. |
| MIGRATE-02 | 04-04, review-fix | `.mcp.json` cleanup instructions | SATISFIED | Manual project cleanup and user-scope install guidance verified. |
| MIGRATE-03 | 04-05, 04-08 | README/CLAUDE/MILESTONES local-first/v1.0 framing | SATISFIED | Staged framing present in all required docs. |
| MIGRATE-04 | 04-03, 04-04, 04-08 | Archived `famp-local` marked frozen/superseded | SATISFIED | Archive exists; old path absent; docs point to frozen archive. |
| TEST-06 | 04-01, 04-08 | Conformance gates unchanged in CI | SATISFIED | Justfile recipe and full `just ci` verify canonical/crypto gates. |
| CARRY-01 | 04-07 | listen-subprocess nextest pin | SATISFIED | `.config/nextest.toml` pin and requirements closeout verified. |

All declared Phase 04 requirement IDs in plan frontmatter are accounted for: FED-01..06, MIGRATE-01..04, TEST-06, CARRY-01. No additional Phase 04 IDs found in `.planning/REQUIREMENTS.md` outside this set.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---:|---|---|---|
| `README.md` | 286 | Manual MCP config section says `config.toml` is "created by `famp init`" | INFO | Stale sentence in an advanced/manual section; does not defeat Phase 04 must-haves because migration docs and primary Quick Start correctly route users to v0.9 local-first paths. Recommend cleanup in the next docs pass. |

### Human Verification Required

None. The phase goal is verifiable through static code inspection, git/tag checks, CLI help behavior, cargo-tree inspection, targeted tests, and full CI.

### Gaps Summary

No blocking gaps found. The phase goal is achieved: federation CLI verbs are unwired, federation internals remain preserved and exercised, the escape-hatch tag is present at the intended pre-deletion commit, migration/local-first documentation is in place, and all declared requirement IDs are closed with passing CI.

---

_Verified: 2026-05-04T02:25:50Z_
_Verifier: the agent (gsd-verifier)_

---
phase: 01-broker-diagnosis-identity-inspection
verified: 2026-05-10T19:03:13Z
status: passed
score: 5/5 must-haves verified
overrides_applied: 0
---

# Phase 1: Broker Diagnosis & Identity Inspection Verification Report

**Phase Goal:** Operator runs `famp inspect broker` and `famp inspect identities` against the v0.9 broker and gets the conversation state needed to retire the orphan-listener incident class, including dead-broker diagnosis. All three inspector crates ship under workspace dependency-version discipline.
**Verified:** 2026-05-10T19:03:13Z
**Status:** passed
**Re-verification:** No - initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `famp inspect broker` diagnoses running and dead brokers with required output and exit codes. | VERIFIED | `broker.rs` probes via `raw_connect_probe`, maps HEALTHY plus DOWN_CLEAN/STALE_SOCKET/ORPHAN_HOLDER/PERMISSION_DENIED, prints stdout, and returns exit 1 for down states (`crates/famp/src/cli/inspect/broker.rs:54`). Integration tests cover healthy, down-clean, stale socket, orphan holder, JSON, and stdout/stderr behavior. |
| 2 | `famp inspect identities` lists registered identities and mailbox metadata without double-print counters. | VERIFIED | Proto `IdentityRow` has required fields and no double/surfaced counters (`crates/famp-inspect-proto/src/lib.rs:65`). Server populates rows from `BrokerStateView` plus mailbox metadata (`crates/famp-inspect-server/src/lib.rs:86`). CLI renders table/JSON (`crates/famp/src/cli/inspect/identities.rs:27`). Tests cover rows, schema, no Debug format, last-activity, and last-received metadata. |
| 3 | Broker accepts `BusMessage::Inspect` on the existing UDS path and dispatch is read-only by construction. | VERIFIED | `BusMessage::Inspect` exists in the bus protocol; broker actor emits `Out::InspectRequest` after the existing Hello gate (`crates/famp-bus/src/broker/handle.rs:78`), executor dispatches with `broker.view()` (`crates/famp/src/cli/broker/mod.rs:343`), server `dispatch` takes `&BrokerStateView` (`crates/famp-inspect-server/src/lib.rs:56`). `just check-inspect-readonly` passed. |
| 4 | `famp inspect` CLI surface has `broker` and `identities`, `--json`, fixed human output, and identities dead-broker fast-fail. | VERIFIED | Inspect subcommands are only broker/identities (`crates/famp/src/cli/inspect/mod.rs`). Broker and identities args expose `--json`; identities prints the required stderr fast-fail on non-healthy probe (`crates/famp/src/cli/inspect/identities.rs:31`) and renders fixed headers (`crates/famp/src/cli/inspect/identities.rs:55`). Integration tests pass. |
| 5 | Inspector crates ship with dependency-version discipline and no forbidden deps. | VERIFIED | Crates exist with workspace versions. `famp-inspect-proto` deps are serde/serde_json/uuid only; `famp-inspect-client` has no clap dependency; `famp-inspect-server` uses the same `famp-canonical`, `famp-envelope`, and `famp-fsm` v0.1.0 versions. `just check-no-io-in-inspect-proto`, `just check-inspect-readonly`, and `just check-inspect-version-aligned` passed. |

**Score:** 5/5 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|---|---|---|---|
| `crates/famp-inspect-proto/src/lib.rs` | Type-only inspect request/reply crate | VERIFIED | Exports `InspectKind`, broker/identities/tasks/messages request/reply types, `IdentityRow`, and forbidden-field tests. |
| `crates/famp-inspect-client/src/lib.rs` | UDS client, raw probe, peer PID diagnosis, no clap | VERIFIED | Implements `raw_connect_probe`, `call`, `connect_and_call`, `peer_pid`, `PidSource`, and `ProbeOutcome`; `cargo tree` grep found no clap. |
| `crates/famp-inspect-server/src/lib.rs` | Read-only dispatch handlers | VERIFIED | Dispatches broker/identities from immutable `BrokerStateView` and caller-supplied `BrokerCtx`; no tokio dependency. |
| `crates/famp-bus/src/proto.rs` | Existing bus protocol carries inspect frames | VERIFIED | `BusMessage::Inspect { kind }` and `BusReply::InspectOk { payload }` present. |
| `crates/famp-bus/src/broker/handle.rs` | Broker actor inspect sentinel and activity updates | VERIFIED | Emits `Out::InspectRequest`; updates `last_activity` on authenticated non-register frames (`crates/famp-bus/src/broker/handle.rs:50`). |
| `crates/famp/src/cli/broker/mod.rs` | Executor-side dispatch and mailbox metadata | VERIFIED | Builds `BrokerCtx`, pre-reads mailbox metadata, parses RFC3339 `ts` into `last_received_at_unix_seconds` (`crates/famp/src/cli/broker/mod.rs:391`). |
| `crates/famp/src/cli/inspect/broker.rs` | Broker CLI rendering | VERIFIED | Implements human and JSON rendering for HEALTHY and all four down states. |
| `crates/famp/src/cli/inspect/identities.rs` | Identities CLI rendering | VERIFIED | Implements JSON, fixed-width table headers, and dead-broker fast-fail. |
| `crates/famp/tests/inspect_broker.rs` | Broker CLI integration coverage | VERIFIED | 8 targeted tests passed. |
| `crates/famp/tests/inspect_identities.rs` | Identity CLI integration coverage | VERIFIED | 6 targeted tests passed, including review-regression coverage. |
| `Justfile` | Inspector invariant gates wired into CI | VERIFIED | `ci` includes all three inspect gates; each gate passed. |

### Key Link Verification

| From | To | Via | Status | Details |
|---|---|---|---|---|
| `famp inspect broker` | `famp-inspect-client` | `raw_connect_probe`, `call`, `peer_pid` | VERIFIED | CLI branches over `ProbeOutcome`, calls broker inspect RPC only when healthy, and asks `peer_pid` for orphan-holder evidence. |
| `famp inspect identities` | broker inspect RPC | `raw_connect_probe` then `call(InspectKind::Identities)` | VERIFIED | Non-healthy probe exits 1 with stderr; healthy path decodes `InspectIdentitiesReply`. |
| broker actor | executor-side inspect dispatch | `Out::InspectRequest` | VERIFIED | Actor remains pure; executor builds context and sends `BusReply::InspectOk`. |
| executor metadata | server identity rows | `read_mailbox_meta_for` -> `BrokerCtx.mailbox_metadata` | VERIFIED | Unread/total/last-sender/last-received values flow into `IdentityRow`. |
| `Justfile ci` | inspect invariant gates | ci prerequisites | VERIFIED | `ci` depends on `check-no-io-in-inspect-proto`, `check-inspect-readonly`, and `check-inspect-version-aligned`. |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|---|---|---|---|---|
| `inspect/broker.rs` | `BrokerStateRender` | `raw_connect_probe` plus `InspectBrokerReply` from broker RPC | Yes | VERIFIED |
| `inspect/identities.rs` | `InspectIdentitiesReply.rows` | `call(InspectKind::Identities)` response | Yes | VERIFIED |
| `famp-inspect-server` | `IdentityRow` | `BrokerStateView.clients` and `BrokerCtx.mailbox_metadata` | Yes | VERIFIED |
| broker executor | `MailboxMeta` | `famp_inbox::read::read_all/read_from` and RFC3339 `ts` parse | Yes | VERIFIED |
| broker state | `last_activity` | `touch_activity` on authenticated frames | Yes | VERIFIED |

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|---|---|---|---|
| Inspector invariant: proto has no I/O deps | `just check-no-io-in-inspect-proto` | Passed | PASS |
| Inspector invariant: server read-only gate | `just check-inspect-readonly` | Passed | PASS |
| Inspector invariant: dependency versions aligned | `just check-inspect-version-aligned` | Passed | PASS |
| Client crate has no clap dependency | `cargo tree -p famp-inspect-client --edges normal \| grep ... clap` | No matches | PASS |
| Inspector crate unit tests | `cargo test -p famp-inspect-proto -p famp-inspect-server -p famp-inspect-client` | 13 passed | PASS |
| Phase CLI integration tests | `cargo test -p famp --test inspect_broker --test inspect_identities` | 14 passed | PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|---|---|---|---|---|
| INSP-BROKER-01 | 01-04 | Healthy broker output | SATISFIED | CLI renders HEALTHY with pid/socket/started_at/build and test `inspect_broker_healthy_exit_0` passed. |
| INSP-BROKER-02 | 01-04 | Dead-broker states and evidence | SATISFIED | Down-clean, stale-socket, orphan-holder, permission-denied branches present; tests cover first three observable states. |
| INSP-BROKER-03 | 01-04 | Orphan-holder PID evidence | SATISFIED | `orphan_holder` always includes `holder_pid` optional plus `pid_source`; human output prints unknown when absent. |
| INSP-BROKER-04 | 01-04 | Exit 0 only healthy; down diagnosis stdout | SATISFIED | `broker.rs` returns `CliError::Exit(1)` for all non-healthy states; stdout/stderr test passed. |
| INSP-IDENT-01 | 01-04 | Identity registration metadata | SATISFIED | Rows include name/listen/cwd/registered/last_activity; tests verify rendered rows and JSON keys. |
| INSP-IDENT-02 | 01-04 | Mailbox metadata | SATISFIED | Executor reads unread/total/last_sender/last_received; regression test verifies timestamp is populated after messages. |
| INSP-IDENT-03 | 01-01, 01-04 | No double-print counters | SATISFIED | Proto schema test and integration JSON key assertions reject forbidden fields. |
| INSP-RPC-01 | 01-03 | Inspect namespace on existing UDS | SATISFIED | `BusMessage::Inspect` rides existing bus socket; no separate inspector listener added. |
| INSP-RPC-02 | 01-02, 01-04 | Read-only handlers plus gate | SATISFIED | `dispatch(&BrokerStateView, ...)` and `just check-inspect-readonly` passed. |
| INSP-CRATE-01 | 01-01, 01-04 | Proto crate no I/O deps | SATISFIED | Manifest is type-only; `just check-no-io-in-inspect-proto` passed. |
| INSP-CRATE-02 | 01-02 | Client crate no clap | SATISFIED | Manifest has no clap; cargo-tree grep found no clap dependency. |
| INSP-CRATE-03 | 01-02, 01-04 | Server crate mounted and version-aligned | SATISFIED | Executor calls server dispatch; version-aligned gate passed for canonical/envelope/fsm. |
| INSP-CLI-01 | 01-03 | `famp inspect` subcommands | SATISFIED | `InspectSubcommand` includes broker and identities only for Phase 1. |
| INSP-CLI-02 | 01-04 | `--json` stable shape | SATISFIED | Broker and identities JSON tests passed. |
| INSP-CLI-03 | 01-04 | Fixed-width human tables | SATISFIED | Identities table has explicit headers and no Debug markers; broker human line matches required single-line shape. |
| INSP-CLI-04 | 01-04 | Non-broker inspect dead-broker error | SATISFIED | Identities fast-fail test passed with stderr and empty stdout. |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|---|---|---|---|---|
| None | - | - | - | No blocking stub, placeholder, hollow data source, or unwired artifact found for Phase 1 scope. Phase 2 tasks/messages are intentionally absent from CLI. |

### Human Verification Required

None.

### Gaps Summary

No gaps found. The two code-review blockers were fixed: `last_activity` is refreshed for authenticated operations and covered by regression assertions, and mailbox `last_received_at_unix_seconds` now parses real RFC3339 envelope timestamps and is covered by identities integration tests.

---

_Verified: 2026-05-10T19:03:13Z_
_Verifier: the agent (gsd-verifier)_

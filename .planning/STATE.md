---
gsd_state_version: 1.0
milestone: v0.10
milestone_name: Inspector & Observability
status: Awaiting next milestone
stopped_at: Phase 03 UAT passed — v0.10 milestone complete, ready for /gsd-complete-milestone
last_updated: "2026-05-11T13:23:45.946Z"
last_activity: 2026-05-11 — Milestone v0.10 completed and archived
progress:
  total_phases: 9
  completed_phases: 3
  total_plans: 10
  completed_plans: 10
  percent: 100
---

# STATE: FAMP — v0.10 Inspector & Observability (complete)

**Last Updated:** 2026-05-11 — v0.10 milestone complete. All 3 phases passed UAT. Phase 3 UAT: 5/5 tests passed (no-starvation load test ratio 0.95, all 8 inspect_broker tests pass, MAX_CONCURRENT_INSPECT_REQUESTS=1 fast-shed visible, migration guide complete, orphan-holder incident-class label present). Phase 2 completed 2026-05-10 (INSP-TASK-01..04, INSP-MSG-01..03, INSP-RPC-03, INSP-RPC-04 all validated). Phase 1 completed 2026-05-10 (16 reqs). 26/26 v1 requirements mapped and delivered.

## Project Reference

See: .planning/PROJECT.md — v0.10 Inspector & Observability is **COMPLETE** (shipped 2026-05-11). All three phases done: read-only `famp inspect` surface (broker, identities, tasks, messages) on the v0.9 broker UDS; GAP-03-01 closed; no-starvation under saturated direct-RPC pressure; operator migration guide shipped. Next milestone: v1.0 Federation Profile (trigger-gated: Gate A = Ben's sustained symmetric cross-machine use; Gate B = 2nd implementer interop).

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later. v0.10 makes that substrate's runtime state legible to the operator running it.

**Current focus:** v0.10 archived — ready for next milestone planning

## Current Position

Phase: Milestone v0.10 complete
Plan: —
Status: Awaiting next milestone
Last activity: 2026-05-11 — Milestone v0.10 completed and archived

## v0.10 Phase Map

- **Phase 1: Broker Diagnosis & Identity Inspection** (16 reqs) — INSP-BROKER-01..04, INSP-IDENT-01..03, INSP-RPC-01, INSP-RPC-02, INSP-CRATE-01..03, INSP-CLI-01..04. `famp.inspect.*` namespace on the existing UDS via new `BusMessage::Inspect` enum variant; all three inspector crates ship (`-proto` no-I/O, `-client` no-clap, `-server` version-aligned with broker); `famp inspect broker` end-to-end (connect-handshake-based dead-broker diagnosis: HEALTHY / DOWN_CLEAN / STALE_SOCKET / ORPHAN_HOLDER / PERMISSION_DENIED, no PID file because v0.9 uses bind()-exclusion); `famp inspect identities` end-to-end (in-memory BrokerState read only); `--json` + fixed-width tables on both subcommands; `just check-inspect-readonly` workspace dep-graph gate; `just check-no-io-in-inspect-proto`. Closes the orphan-listener incident class in one merge. **No budget or cancel handlers needed in Phase 1 — both Phase 1 commands are pure in-memory reads or client-side network probes; budget/cancel land in Phase 2 with the I/O-bound handlers that actually exercise them.**
- **Phase 2: Task FSM & Message Visibility** (9 reqs) — INSP-TASK-01..04, INSP-MSG-01..03, INSP-RPC-03 (500ms budget enforces at the tokio wrapper for I/O handlers), INSP-RPC-04 (cancellable handlers, 1000-concurrent-cancel test against the real `inspect tasks` and `inspect messages` paths). The taskdir + mailbox file walks are the I/O surface; budget and cancel finally have something real to enforce against.
- **Phase 3: Load Verification & Integration Hardening** (1 req owned + cross-phase E2E) — INSP-RPC-05 no-starvation load test owns this phase; Phase 1's INSP-BROKER-02..04 + INSP-CLI-04 are re-exercised under integration-grade orphan-listener scenario; `docs/MIGRATION-v0.9-to-v0.10.md` ships.

## Architectural Invariants Locked at Roadmap Time

1. **Read-only discipline (INSP-RPC-02)** — every `famp.inspect.*` handler is read-only, enforced at compile time by `&BrokerState` (not `&mut`) handler signatures, AND at build time by a workspace dep-graph gate (`just check-inspect-readonly`) that fails CI if `famp-inspect-server` transitively imports any mailbox-write, taskdir-write, or broker `&mut self` mutation surface. **Replaces the originally-drafted runtime property test on broker state hashes**, which Matt + Zed flagged as ceremony for a compile-time invariant. No mutation surface in v0.10. `famp doctor` (mutation) is gated to v0.10.x only after the read-only view tells us *which* mutations we actually keep reaching for.
2. **Crate dependency-version alignment (INSP-CRATE-03)** — `famp-inspect-server` shares `famp-canonical`, `famp-envelope`, `famp-fsm` versions exactly with the broker. Version skew would re-introduce the failure mode the inspector exists to expose (inspector decoding envelopes with a different canonicalizer than the broker that wrote them — unacceptable for a byte-exactness protocol). Separate `famp-inspect` binary was rejected for this reason; it is a subcommand of `famp`.
3. **Dead-broker workability (INSP-BROKER-02 + INSP-CLI-04)** — `famp inspect broker` is the one command that must produce a useful diagnosis when the broker is dead. v0.9 uses bind()-exclusion (no PID file), so detection is **connect-handshake-based**: DOWN_CLEAN (no socket file) / STALE_SOCKET (file exists, ECONNREFUSED) / ORPHAN_HOLDER (connect succeeds, Hello rejected) / PERMISSION_DENIED (EACCES). Replaces the originally-drafted STALE_PID / pid-file states. Every other `famp inspect` subcommand exits 1 with `"error: broker not running at <socket-path>"` on stderr when the broker is dead. The orphan-listener incident class from v0.9 is the named target.
4. **No double-print counter (INSP-IDENT-03 + Out of Scope)** — broker-side counter for the wake-up-notification + inbox-fetch double-billing failure mode was rejected as wrong instrument. Right surface is per-message token attribution at the model boundary, or a static audit of the `famp_await` notification payload — both are separate investigations from the inspector.
5. **Wire shape (INSP-RPC-01)** — `famp.inspect.*` rides the existing UDS via a new `BusMessage::Inspect { kind, ... }` enum variant in `famp-bus`. Single dispatch path in `Broker::handle()` gains one new arm. No second socket. `InspectKind` sub-enum carries the four operations (broker, identities — Phase 1; tasks, messages — Phase 2). `famp-bus` stays tokio-free; budget enforcement lives at the tokio wrapper layer (`crates/famp/src/cli/broker/`), only for I/O-bound handlers (none in Phase 1).

## Carry-Forward from v0.9

- v0.9 broker (`famp-bus`, `~/.famp/bus.sock`, posix_spawn+setsid lifecycle, bind()-IS-the-lock single-broker exclusion) is the substrate v0.10 mounts on. No broker-side rewrites planned.
- 8-tool stable MCP surface (`famp_register`, `famp_send`, `famp_inbox`, `famp_await`, `famp_peers`, `famp_join`, `famp_leave`, `famp_whoami`) carried forward unchanged. v0.10 does **not** add MCP tools; the inspector consumer is a CLI subcommand, not an MCP tool. (Future MCP exposure of inspector data is gated on usage signals reaching for it.)
- `just check-no-tokio-in-bus` permanent CI gate is the precedent for v0.10's `just check-no-io-in-inspect-proto` recipe (parallel discipline at the proto crate boundary).
- `FAMP_SPEC_VERSION = "0.5.2"` unchanged; v0.10 does not require a spec amendment.

## Open Items Inherited (not v0.10-blocking, just persistent)

- **Architect counsel parked from v0.9 Phase 03** (`famp send` audit_log wrapper at `crates/famp/src/cli/send/mod.rs::build_envelope_value`). Three options on the table; lean is option 1. Question parked for next architect session — does not block v0.10 Phase 1.
- **8 pre-existing TLS-loopback timeouts** documented in v0.9 audit `tech_debt`. Triage as separate hygiene task. Not v0.10's surface.
- **WR-06 env-var test races** waived under nextest. Not v0.10's surface.

## Decisions

- [Roadmap]: Three-phase structure recut after matt-essentialist + zed-velocity-engineer review (2026-05-10): Phase 1 closes orphan-listener incident class end-to-end (broker + identities, RPC + CLI both); Phase 2 ships the I/O-bound enrichment (tasks + messages) and is where budget+cancel finally have something to enforce against; Phase 3 unchanged. **Rejected the original cut** (Phase 1 = RPC foundation with stub handlers; Phase 2 = all CLI) as yak-shaving — Phase 1's success criteria around budget+cancel were testing synthetic test-only handlers, not real work. The v0.10 user-visible win is closing the orphan-listener incident class; the recut ships that in one merge.
- [Roadmap]: Phase numbering reset to Phase 1 per FAMP convention (v0.7/v0.8/v0.9 each reset; v0.10 follows). Confirmed with user at roadmap open.
- [Roadmap]: Read-only discipline (INSP-RPC-02) and crate version alignment (INSP-CRATE-03) treated as architectural invariants, not feature requirements — locked at roadmap time so plan-phase cannot soften them.
- [Phase ?]: Kind-tagged inspector reply enums — Locks D-02 wire shape for task/message replies before broker I/O and CLI rendering depend on it.
- [Phase ?]: Pre-read snapshots in BrokerCtx — Keeps famp-inspect-server sync/tokio-free while allowing Plan 02 to populate TaskSnapshot and MessageSnapshot inside the broker executor.
- [Phase ?]: Canonical fixture for A1 proof — Uses Phase 1 vector_0 canonical.hex rather than pretty envelope.json so canonicalize_roundtrip proves byte-for-byte JCS reproducibility.
- [Phase 02 Plan 02]: Set block_on_async max_blocking_threads to 1024 for 1000 concurrent inspect calls.
- [Phase 02 Plan 02]: Capture cursor offsets before spawn_blocking because Broker is not Send.
- [Phase 02 Plan 02]: Return budget_exceeded as an InspectOk payload to preserve the BusReply codec.

## Issues / Blockers

- **GAP-03-01: CLOSED 2026-05-11** — `03-03-PLAN.md` shipped non-blocking bounded inspect dispatch (MAX_CONCURRENT_INSPECT_REQUESTS=1, Semaphore fast-shed) and saturated direct-RPC load test; observed ratio 0.82–1.01 (≥0.80 threshold). Prior 0.17 was paced-CLI evidence; now backed by saturated direct `InspectKind::Tasks` RPC pressure.
- v1.0-track items (Gate A: Ben symmetric cross-machine; Gate B: 2nd implementer) are independent of v0.10 — v0.10 ships on its own track regardless.

## Deferred Items

Items acknowledged and deferred at v0.9 milestone close on 2026-05-04 (per `gsd-sdk query audit-open`); carried forward into v0.10 unchanged unless v0.10 work pulls one in:

| Category | Item | Status |
|----------|------|--------|
| quick_task | 260414-cme-remove-obsolete-wave2-impl-feature-gate- | missing |
| quick_task | 260414-ecp-wire-unsupportedversion-error-on-envelop | missing |
| quick_task | 260414-esi-seal-famp-field-visibility-and-cover-adv | missing |
| quick_task | 260414-f4i-docs-pr-famp-crypto-rustdoc-readme-overv | missing |
| quick_task | 260414-fjo-pr-4-architectural-cleanup-drop-signer-v | missing |
| quick_task | 260414-g32-pr-4-1-fix-weakkey-docstring-drop-dead-v | missing |
| quick_task | 260420-viu-fail-open-on-invaliduuid-in-inbox-list-f | missing |
| quick_task | 260424-7z5-fix-famp-send-new-task-body-loss-scope-i | missing |
| quick_task | 260425-cic-bump-rustls-webpki-2026-0104 | missing |
| quick_task | 260425-gst-fix-famp-fsm-commit-receipt-error-suppre | missing |
| quick_task | 260425-ho8-fix-lost-update-race-in-await-commit-rec | missing |
| quick_task | 260425-kbx-harden-await-commit-receipt-red-test-tig | missing |
| quick_task | 260425-lg7-tighten-try-update-closure-err-docstring | missing |
| quick_task | 260425-lny-fix-b2-class-bug-at-send-mod-rs-514-surf | missing |
| quick_task | 260425-m0f-write-scripts-redeploy-listeners-sh-safe | missing |
| quick_task | 260425-of2-t1-2-tighten-mcp-body-schema-docstring | missing |
| quick_task | 260425-pc7-add-more-coming-flag-to-new-task-envelop | missing |
| quick_task | 260425-re1-t2-2-readme-redeploy-verification-spot-c | missing |
| quick_task | 260425-rz6-fix-clierror-envelope-masking-fsm-transi | missing |
| quick_task | 260425-sl0-t3-x-file-three-backlog-items-999-3-999- | missing |
| quick_task | 260425-so2-absorb-format-drift-in-send-mod-rs-after | missing |
| quick_task | 260425-tey-absorb-rz6-adversarial-review-findings-d | missing |
| quick_task | 260426-q1q-fix-famp-local-wire-first-call-mesh-size | missing |
| quick_task | 260426-s2j-add-famp-local-identity-of-subcommand-an | missing |
| quick_task | 260426-stp-align-bash-validate-identity-name-with-r | missing |
| quick_task | 260426-u2t-t5-spec-amendment-v0-5-1-to-v0-5-2-audit | missing |
| quick_task | 260427-k7v-add-clear-subcommand-to-scripts-famp-loc | missing |
| quick_task | 260427-kna-add-famp-local-doctor-subcommand-and-fam | missing |
| quick_task | 260427-l2t-fix-doctor-walk-up-to-read-input-dir-mcp | missing |
| quick_task | 260427-lb8-fix-adversarial-review-findings-doctor-i | missing |
| seed | SEED-001-serde-jcs-conformance-gate | dormant (Gate B) |
| seed | SEED-002-harness-adapter-push-notifications | dormant (gate assignment deferred — re-read seed when surfaced) |
| uat_gap | 02 (02-HUMAN-UAT.md, 0 pending scenarios) | unknown |

**Notes:** All 30 quick_tasks are orphan slugs (drift residue from federation-era + v0.9 prep-sprint work; no completion artifacts but no active obligations). SEED-001 (vector-pack interop) is unambiguously **Gate B** (2nd implementer commits to interop). SEED-002 (push-notification harness) is gate-assignment-deferred. UAT gap header status drift only — 0 pending scenarios.

## Quick Tasks Completed

| # | Description | Date | Commit | Directory |
|---|-------------|------|--------|-----------|
| 260504-ubf | Cleanup late-join debug findings: delete stale v0.8 cursor artifacts, add RegisterOk.peers snapshot doc note | 2026-05-05 | a55be0d | [260504-ubf-clean-up-late-join-debug-findings-delete](./quick/260504-ubf-clean-up-late-join-debug-findings-delete/) |
| 260506-c65 | Wire famp-await.sh into famp install-claude-code / uninstall-claude-code distribution | 2026-05-06 | 54fcb47 | [260506-c65-wire-famp-await-into-install-claude-code](./quick/260506-c65-wire-famp-await-into-install-claude-code/) |
| 260506-s1t | Add smoke-test for Quick Start install path (just smoke-test + CI job) | 2026-05-06 | 53eec99 | [260506-s1t-add-smoke-test-quick-start-install](./quick/260506-s1t-add-smoke-test-quick-start-install/) |
| 260506-cc9 | Trim README Quick Start fence from 19→12 lines (CC-09) + D-11 cargo install path | 2026-05-06 | 120f040 | [260506-cc9-trim-readme-quick-start-fence-cc09](./quick/260506-cc9-trim-readme-quick-start-fence-cc09/) |
| 260507-fcs | fix-channel-send-hash-principal-bug | complete | Fix #-prefixed peer name corrupting channel mailbox |
| 260507-k9x | Fix broker await broadcast race: replace find_map with Vec broadcast, D-04 AppendMailbox ordering, proxy liveness gate, 4 regression tests | 2026-05-07 | 77d045b | [260507-k9x-fix-broker-await-broadcast-race-conditio](./quick/260507-k9x-fix-broker-await-broadcast-race-conditio/) |
| 260507-sv8 | Fix task_id zeros bug and wire causality into build_envelope_value | 2026-05-08 | a9c1451 | [260507-sv8-fix-task-id-zeros-bug-and-wire-causality](./quick/260507-sv8-fix-task-id-zeros-bug-and-wire-causality/) |
| 260508-ib4 | Add woken bool to SendOk so famp_send callers can tell if recipient was live at delivery time | 2026-05-08 | c699859 | [260508-ib4-add-woken-bool-to-sendok-so-famp-send-ca](./quick/260508-ib4-add-woken-bool-to-sendok-so-famp-send-ca/) |
| 260509-kcf | Propagate v1.0 trigger unweld decision into project docs | 2026-05-09 | ba66ee4 | [260509-kcf-propagate-v1-0-trigger-unweld-decision-i](./quick/260509-kcf-propagate-v1-0-trigger-unweld-decision-i/) |

## Performance Metrics

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| Phase 01 P01 (v0.9) | 23min | 2 tasks | 17 files |
| Phase 01 P02 (v0.9) | 15min | 2 tasks | 15 files |
| Phase 01 P03 (v0.9) | atomic | 1 task | 28 files |
| Phase 04 P05 (v0.9) | 8min | 1 tasks | 6 files |
| Phase 02 P02 | 20min | 2 tasks | 3 files |
| Phase 03 P01 | 30 min | 2 tasks | 2 files |
| Phase 03 P02 | 15 min | 2 tasks | 2 files |

## Session

**Last session:** 2026-05-11T12:35:00Z
**Stopped At:** Phase 03 UAT passed — v0.10 milestone complete, ready for /gsd-complete-milestone
**Resume File:** None

## Operator Next Steps

- Start the next milestone with /gsd-new-milestone

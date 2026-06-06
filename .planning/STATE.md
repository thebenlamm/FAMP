---
gsd_state_version: 1.0
milestone: v0.11
milestone_name: refresh. DOC-01/02/03 verified live against the installed binary
status: completed
stopped_at: Phase 6 context gathered
last_updated: "2026-06-06T21:00:40.952Z"
last_activity: 2026-06-06 -- Phase 06 marked complete
progress:
  total_phases: 16
  completed_phases: 6
  total_plans: 21
  completed_plans: 21
  percent: 38
---

# STATE: FAMP — v0.11 Broker Daemon & Cross-Tool Bootstrap

**Last Updated:** 2026-06-03 — v0.11 roadmap created. 3 phases (4–6), 15/15 requirements mapped. Phase 4 begins.

## Project Reference

See: .planning/PROJECT.md — v0.10 Inspector & Observability is **COMPLETE** (shipped 2026-05-11). v0.11 Broker Daemon & Cross-Tool Bootstrap is now active.

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later. v0.11 makes that substrate *reliably reachable* — a service-managed daemon restores broker presence so any local client, sandboxed or not, always finds a broker.

**Current focus:** Phase 06 — onboarding-cross-platform-docs

## Current Position

Phase: 06 — COMPLETE
Plan: 1 of 3
Status: Phase 06 complete
Last activity: 2026-06-06 -- Phase 06 marked complete

```
[Phase 4 ░░░░░░░░░░] [Phase 5 ░░░░░░░░░░] [Phase 6 ░░░░░░░░░░]
  0%                                                          100%
```

## v0.11 Phase Map

- **Phase 4: Broker Lifecycle & Bootstrap Diagnostics** (3 reqs: BLC-01, BLC-02, BOOT-01). `famp broker --no-idle-exit` flag disabling the 300s idle self-terminate (hard prerequisite for the daemon — a service-managed broker must never self-terminate on idle); regression guard confirming default idle-exit behavior is unchanged; actionable EPERM-on-bind error in `spawn.rs:92` replacing the swallowed `let _ =` with a message that names the sandbox constraint and the remedy. Changes land in `crates/famp/src/cli/broker/mod.rs` and `crates/famp/src/bus_client/spawn.rs`. Run `just install` before closing any PR that changes the spawn-error surface.
- **Phase 5: Daemon Service Management & Version Safety** (9 reqs: DAEMON-01..06, BOOT-02, VER-01, VER-02). `famp daemon install/uninstall/status/restart` cross-platform service lifecycle (launchd LaunchAgent on macOS, systemd `--user` unit on Linux); sandbox-detect refusal at install time; version handshake at connect so a long-lived daemon and a freshly-upgraded client fail loud on skew; `famp -V` / banner / handshake reconciled to a single source of truth. **DAEMON-02 guardian plist review gate is blocking: do not load the service until the literal plist XML has guardian sign-off.** Socket activation and spawn-lock explicitly deferred.
- **Phase 6: Onboarding & Cross-Platform Docs** (3 reqs: DOC-01, DOC-02, DOC-03). README one-command quickstart (`famp daemon install` once → both Claude Code and Codex connect forever); zero-setup bridge line (`famp broker --no-idle-exit` in an unsandboxed terminal); explicit cross-platform support boundary section naming what the installer covers (macOS launchd, Linux systemd `--user`) and what it does not (minimal distros, containers, WSL, headless without linger). Docs land after Phase 5 so they describe commands that exist and behave as written.

## v0.10 Phase Map (complete)

- **Phase 1: Broker Diagnosis & Identity Inspection** (16 reqs) — INSP-BROKER-01..04, INSP-IDENT-01..03, INSP-RPC-01, INSP-RPC-02, INSP-CRATE-01..03, INSP-CLI-01..04. `famp.inspect.*` namespace on the existing UDS via new `BusMessage::Inspect` enum variant; all three inspector crates ship (`-proto` no-I/O, `-client` no-clap, `-server` version-aligned with broker); `famp inspect broker` end-to-end (connect-handshake-based dead-broker diagnosis: HEALTHY / DOWN_CLEAN / STALE_SOCKET / ORPHAN_HOLDER / PERMISSION_DENIED, no PID file because v0.9 uses bind()-exclusion); `famp inspect identities` end-to-end (in-memory BrokerState read only); `--json` + fixed-width tables on both subcommands; `just check-inspect-readonly` workspace dep-graph gate; `just check-no-io-in-inspect-proto`. Closes the orphan-listener incident class in one merge. **No budget or cancel handlers needed in Phase 1 — both Phase 1 commands are pure in-memory reads or client-side network probes; budget/cancel land in Phase 2 with the I/O-bound handlers that actually exercise them.**
- **Phase 2: Task FSM & Message Visibility** (9 reqs) — INSP-TASK-01..04, INSP-MSG-01..03, INSP-RPC-03 (500ms budget enforces at the tokio wrapper for I/O handlers), INSP-RPC-04 (cancellable handlers, 1000-concurrent-cancel test against the real `inspect tasks` and `inspect messages` paths). The taskdir + mailbox file walks are the I/O surface; budget and cancel finally have something real to enforce against.
- **Phase 3: Load Verification & Integration Hardening** (1 req owned + cross-phase E2E) — INSP-RPC-05 no-starvation load test owns this phase; Phase 1's INSP-BROKER-02..04 + INSP-CLI-04 are re-exercised under integration-grade orphan-listener scenario; `docs/MIGRATION-v0.9-to-v0.10.md` ships.

## Architectural Invariants Locked at Roadmap Time

1. **Read-only discipline (INSP-RPC-02)** — every `famp.inspect.*` handler is read-only, enforced at compile time by `&BrokerState` (not `&mut`) handler signatures, AND at build time by a workspace dep-graph gate (`just check-inspect-readonly`) that fails CI if `famp-inspect-server` transitively imports any mailbox-write, taskdir-write, or broker `&mut self` mutation surface. **Replaces the originally-drafted runtime property test on broker state hashes**, which Matt + Zed flagged as ceremony for a compile-time invariant. No mutation surface in v0.10. `famp doctor` (mutation) is gated to v0.10.x only after the read-only view tells us *which* mutations we actually keep reaching for.
2. **Crate dependency-version alignment (INSP-CRATE-03)** — `famp-inspect-server` shares `famp-canonical`, `famp-envelope`, `famp-fsm` versions exactly with the broker. Version skew would re-introduce the failure mode the inspector exists to expose (inspector decoding envelopes with a different canonicalizer than the broker that wrote them — unacceptable for a byte-exactness protocol). Separate `famp-inspect` binary was rejected for this reason; it is a subcommand of `famp`.
3. **Dead-broker workability (INSP-BROKER-02 + INSP-CLI-04)** — `famp inspect broker` is the one command that must produce a useful diagnosis when the broker is dead. v0.9 uses bind()-exclusion (no PID file), so detection is **connect-handshake-based**: DOWN_CLEAN (no socket file) / STALE_SOCKET (file exists, ECONNREFUSED) / ORPHAN_HOLDER (connect succeeds, Hello rejected) / PERMISSION_DENIED (EACCES). Replaces the originally-drafted STALE_PID / pid-file states. Every other `famp inspect` subcommand exits 1 with `"error: broker not running at <socket-path>"` on stderr when the broker is dead. The orphan-listener incident class from v0.9 is the named target.
4. **No double-print counter (INSP-IDENT-03 + Out of Scope)** — broker-side counter for the wake-up-notification + inbox-fetch double-billing failure mode was rejected as wrong instrument. Right surface is per-message token attribution at the model boundary, or a static audit of the `famp_await` notification payload — both are separate investigations from the inspector.
5. **Wire shape (INSP-RPC-01)** — `famp.inspect.*` rides the existing UDS via a new `BusMessage::Inspect { kind, ... }` enum variant in `famp-bus`. Single dispatch path in `Broker::handle()` gains one new arm. No second socket. `InspectKind` sub-enum carries the four operations (broker, identities — Phase 1; tasks, messages — Phase 2). `famp-bus` stays tokio-free; budget enforcement lives at the tokio wrapper layer (`crates/famp/src/cli/broker/`), only for I/O-bound handlers (none in Phase 1).

## v0.11 Architectural Invariants

1. **Primitive crates stay untouched** — `famp-bus`, `famp-canonical`, `famp-crypto`, `famp-core`, `famp-envelope`, `famp-fsm` are transport-neutral protocol primitives. All v0.11 changes are CLI-layer (`crates/famp/src/cli/`, `crates/famp/src/bus_client/spawn.rs`).
2. **`just install` required when deployed surface changes** — the installed `~/.cargo/bin/famp` is what every agent session reads; `target/release/famp` is not the deployment target. Run `just install` before closing any PR that touches the spawn-error path (Phase 4) or the daemon subcommand (Phase 5).
3. **Pre-commit hook stays fmt-check only** — the hook must not be expanded without explicit signoff. New CI gates (e.g. a plist-shape check) need separate signoff before going into pre-commit.
4. **Guardian plist review is a blocking pre-load gate** — DAEMON-02 requires guardian to approve the literal plist XML before the service is first loaded. This is not an advisory review; the service must not be loaded until sign-off is received.
5. **Socket activation + spawn-lock stay deferred** — do not create phases or plan items for launchd/systemd socket activation or for the `bind_exclusive` stale-branch spawn-lock. Both are explicitly out of v0.11 scope.

## Carry-Forward from v0.10

- v0.9 broker (`famp-bus`, `~/.famp/bus.sock`, posix_spawn+setsid lifecycle, bind()-IS-the-lock single-broker exclusion) is the substrate v0.11 builds on. No broker-side rewrites planned.
- 8-tool stable MCP surface (`famp_register`, `famp_send`, `famp_inbox`, `famp_await`, `famp_peers`, `famp_join`, `famp_leave`, `famp_whoami`) carried forward unchanged. v0.11 does **not** add MCP tools.
- `FAMP_SPEC_VERSION = "0.5.2"` unchanged; v0.11 does not require a spec amendment.
- `just check-no-tokio-in-bus` and `just check-inspect-readonly` permanent CI gates remain intact.
- The broker-unreachable connect/spawn-stage disambiguation (commits `4da30a3`/`ebbf1d3`) is the direct ancestor of BOOT-01's EPERM handling. Phase 4 extends, not replaces, that work.

## Open Items Inherited (not v0.11-blocking, just persistent)

- **Architect counsel parked from v0.9 Phase 03** (`famp send` audit_log wrapper at `crates/famp/src/cli/send/mod.rs::build_envelope_value`). Three options on the table; lean is option 1. Question parked for next architect session.
- **8 pre-existing TLS-loopback timeouts** documented in v0.9 audit `tech_debt`. Triage as separate hygiene task.
- **WR-06 env-var test races** waived under nextest.

## Decisions

- [Roadmap v0.11]: Three-phase structure (Phases 4–6) derived from natural delivery boundaries: Phase 4 lands the `--no-idle-exit` prerequisite + EPERM diagnostics before any daemon work begins; Phase 5 delivers the full daemon lifecycle + version safety once the flag exists; Phase 6 lands docs after the commands exist so docs describe real behavior. VER-01/VER-02 placed in Phase 5 (not a separate phase) because they are most valuable once the daemon keeps a broker alive, and two reqs are too thin for their own phase at standard granularity.
- [Roadmap v0.11]: Phase numbering continues from v0.10 (4/5/6) rather than resetting to 1. Reason: v0.10 phase dirs `01/02/03` are still present under `.planning/phases/`; resetting would collide. New phase dirs: `04-broker-lifecycle-bootstrap/`, `05-daemon-service-version/`, `06-onboarding-docs/`.
- [Roadmap v0.10]: Three-phase structure recut after matt-essentialist + zed-velocity-engineer review (2026-05-10): Phase 1 closes orphan-listener incident class end-to-end (broker + identities, RPC + CLI both); Phase 2 ships the I/O-bound enrichment (tasks + messages) and is where budget+cancel finally have something to enforce against; Phase 3 unchanged. **Rejected the original cut** (Phase 1 = RPC foundation with stub handlers; Phase 2 = all CLI) as yak-shaving — Phase 1's success criteria around budget+cancel were testing synthetic test-only handlers, not real work. The v0.10 user-visible win is closing the orphan-listener incident class; the recut ships that in one merge.
- [Roadmap v0.10]: Read-only discipline (INSP-RPC-02) and crate version alignment (INSP-CRATE-03) treated as architectural invariants, not feature requirements — locked at roadmap time so plan-phase cannot soften them.
- [05-01]: classify_hello_reply() extracted as pure fn from async connect() to enable unit tests without a live broker socket; BusClientError::ProtocolMismatch split from HelloFailed so bus_proto mismatch is distinctly typed and names `famp daemon restart` in its Display.
- [05-01]: eprintln! used for HelloOk connect log (not tracing::info!) to avoid adding a new Cargo dep (plan verification constraint). Consistent with wait_for_disconnect usage in the same file.
- [05-02]: Tasks 1+2 implemented together in a single commit: DaemonError and generate_plist both live in install.rs; splitting into separate commits would have required a partial install.rs that doesn't compile until Task 2. sample_fixture_matches_generate_plist test added for byte-exact guardian artifact invariant via include_str!.
- [05-04]: check_not_sandboxed creates bus_dir before probe — preflight_bind_probe returns Ok on ENOENT (not EPERM/EACCES), so a missing bus_dir would silently pass the probe even in a sandbox. create_dir_all before the probe ensures a real permission answer (BOOT-02 correctness).
- [05-04]: integration test cleanup-before-assert — launchctl bootout/uninstall runs before any assert!/expect! call so a panic cannot leave a persistent LaunchAgent on the machine.

## Issues / Blockers

- None at roadmap time. DAEMON-02 guardian plist review is a known external dependency, not a current blocker — it becomes blocking when Phase 5 is ready to load the service for the first time.

## Deferred Items

Items acknowledged and deferred at v0.9 milestone close on 2026-05-04 (per `gsd-sdk query audit-open`); carried forward into v0.11 unchanged unless v0.11 work pulls one in:

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
| 260512-jdv | Fix famp inspect identities UNREAD always equals TOTAL: source cursor from disk, delete dead BrokerState.cursors | 2026-05-12 | 765171f | [260512-jdv-fix-famp-inspect-identities-unread-total](./quick/260512-jdv-fix-famp-inspect-identities-unread-total/) |
| 260515-iyi | Add famp_channel_log MCP tool for channel history | 2026-05-15 | cb7ee11 | [260515-iyi-add-famp-channel-log-mcp-tool-for-channe](./quick/260515-iyi-add-famp-channel-log-mcp-tool-for-channe/) |
| 260515-kqx | Implement Option 3 batch AwaitOk delivery to fix burst message loss | 2026-05-15 | 146ca9f | [260515-kqx-implement-option-3-batch-awaitok-deliver](./quick/260515-kqx-implement-option-3-batch-awaitok-deliver/) |
| 260515-s3h | Fix stop-hook blind spot: famp await CLI emits wrapper JSON with mailbox info so hook generates channel-aware wake messages | 2026-05-15 | ae13c43 | [260515-s3h-fix-stop-hook-blind-spot-famp-await-cli-](./quick/260515-s3h-fix-stop-hook-blind-spot-famp-await-cli-/) |
| 260527-uhb | Harden FAMP install Stop-hook dedup matcher to catch wrapped command forms (e.g. `bash <path>`) | 2026-05-28 | 5f3673c | [260527-uhb-harden-famp-install-stop-hook-dedup-matc](./quick/260527-uhb-harden-famp-install-stop-hook-dedup-matc/) |
| 260530-wj6 | Disambiguate `famp register` "broker unreachable" into stage-aware connect vs spawn (fork/setsid) errors with errno | 2026-05-31 | 4da30a3 | [260530-wj6-disambiguate-famp-register-broker-unreac](./quick/260530-wj6-disambiguate-famp-register-broker-unreac/) |
| 260531-k2p | Disambiguate MCP `ensure_bus` broker-unreachable (the surface that bit Codex) into stage-aware connect vs spawn errors; all 4 callers forward detail, JSON-RPC -32108 unchanged | 2026-05-31 | ebbf1d3 | [260531-k2p-disambiguate-mcp-ensure-bus-broker-unreac](./quick/260531-k2p-disambiguate-mcp-ensure-bus-broker-unreac/) |

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
| Phase 05 P01 | 35 min | 2 tasks | 18 files |
| Phase 05-daemon-service-management-version-safety P05 | 45min | 2 tasks | 4 files |
| Phase 05-daemon-service-management-version-safety P04 | 35min | 2 tasks | 4 files |

## Session

**Last session:** 2026-06-04T23:33:02.655Z
**Stopped At:** Phase 6 context gathered
**Resume File:** .planning/phases/06-onboarding-cross-platform-docs/06-CONTEXT.md

## Operator Next Steps

- Plan Phase 4 with /gsd:plan-phase 4

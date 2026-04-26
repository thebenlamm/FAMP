---
gsd_state_version: 1.0
milestone: v0.8
milestone_name: Usable from Claude Code
status: All v0.8 phases shipped
last_updated: "2026-04-15T20:12:44.035Z"
last_activity: 2026-04-15
---

# STATE: FAMP — v0.8 Usable from Claude Code

**Last Updated:** 2026-04-15 (v0.8 complete; 4/4 phases, 13/13 plans, 355/355 tests)

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-14 with v0.8 Current Milestone section)

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

**Current focus:** v0.8 COMPLETE — Ready for milestone audit

## Current Position

Phase: 04 (mcp-server-e2e) — COMPLETE
Plan: 3/3 complete
Status: All v0.8 phases shipped
Last activity: 2026-04-25 - Completed quick task 260425-sl0: Filed three Tier-3 backlog items (999.3 heartbeat, 999.4 user_attention, 999.5 spec-by-path deferral to v1.0)

```
v0.8 Progress: [████████████████████] 100% (4/4 phases)
```

## Last Shipped

- **v0.7 Personal Runtime** (2026-04-14) — 4/4 phases, 15/15 plans, 32/32 requirements, 253/253 tests green. `famp-envelope`, `famp-fsm`, `famp-transport` + `MemoryTransport`, `famp-keyring` (TOFU), `famp-transport-http` (axum + rustls + reqwest), two finish-line examples, 3×2 adversarial matrix. `cargo tree -i openssl` empty.

## Accumulated Context

### Key Decisions Logged (carried forward)

- **Language: Rust** — compiler-checked INV-5 via exhaustive enum `match`
- **Personal Profile before Federation Profile** (adopted 2026-04-12) — v0.6 + v0.7 are the solo-dev finish line; v0.8+ stacks federation semantics on top without substrate churn
- **v0.5.1 spec fork is authority** — all implementation bytes hash against `FAMP-v0.5.1-spec.md`
- **SEED-001 RESOLVED 2026-04-13:** keep `serde_jcs 0.2.0` — 12/12 RFC 8785 conformance gate green; fallback plan on disk as insurance. Evidence in `.planning/SEED-001.md`.
- **`verify_strict`-only public surface** — raw `verify` unreachable from `famp-crypto` public API
- **Domain separation prefix prepended internally** — callers never assemble signing input by hand; `canonicalize_for_signature` is the only sanctioned path
- **Narrow, phase-appropriate error enums** — not one god enum (repeated pattern in Plans 01-01 D-16 and 02-01)
- **15-category flat `ProtocolErrorKind` + exhaustive consumer stub under `#![deny(unreachable_patterns)]`** — new error categories become compile errors in downstream crates
- **`AuthorityScope` hand-written 5×5 `satisfies()` truth table, no `Ord` derive** — authority is a ladder, not a total order
- **v0.7 TOFU keyring stays local-file** — `HashMap<Principal, VerifyingKey>`, principal = raw Ed25519 pubkey, loaded from file or `--peer` CLI flag. Agent Cards defer to v0.9.
- **v0.7 adversarial matrix = 3 cases × 2 transports, not 18** — CONF-05/06/07 own the three cases; Phase 4 extended the same matrix to HTTP without new CONF-0x requirements.
- **ENV-09 and ENV-12 are intentionally narrowed for v0.7** — ENV-09 ships with no capability-snapshot binding; ENV-12 ships cancel-only. Wider forms defer to Federation Profile.
- **D-B5 + D-D1 resolved (2026-04-13):** `TerminalStatus` lifted to `famp-core` alongside `MessageClass`; `famp-fsm` depends only on `famp-core`, never on `famp-envelope`.
- **relation field dropped from TaskTransitionInput (2026-04-13):** D-B3 resolved — no v0.7 legal arrow needs relation inspection.
- **FSM step() and accessors are const fn** — All transition arms operate on Copy enums.
- **v0.8 phase shape:** 4 phases derived from the requirement dependency graph — Phase 1 (identity + CLI scaffold), Phase 2 (daemon + inbox), Phase 3 (conversation CLI + task records), Phase 4 (MCP + E2E). Dependency chain: v0.7 → P1 → P2 → P3 → P4.
- **Plan 02-01 (2026-04-14):** `famp-inbox` takes raw `&[u8]`, not a typed `SignedEnvelope`, to preserve byte-exactness (P3) and keep the crate decoupled from `famp-envelope`. Append path is fsync-sealed via `tokio::fs::File::sync_data` under `Arc<Mutex<File>>`; 16-task concurrent test locks the serialization contract; `read_all` tail-tolerance swallows the final truncated line but surfaces mid-file corruption as a hard `CorruptLine { line_no }` error. Covers INBOX-01/02/04/05.
- **Plan 02-02 (2026-04-14):** `famp listen` wired end-to-end. Custom axum Router reuses `FampSigVerifyLayer` from `famp-transport-http` unmodified (byte-for-byte middleware stack), but owns its own handler that calls `inbox.append(&body).await` and returns **200 OK** (stricter than upstream's 202 — the 200 is a durability receipt because fsync ran before return). CliError gains `PortInUse { addr }`, `Inbox(#[from])`, `Tls(#[from])`. SIGINT/SIGTERM graceful shutdown via `tokio::signal::unix`. Self-keyring bootstrapped with single entry `agent:localhost/self` → own vk (peer keys defer to Phase 3). `set_nonblocking(true)` enforced in both `run` and `run_on_listener` because axum-server 0.8 panics on blocking sockets (tokio-rs/tokio#7172). Test-facing `run_on_listener(home, listener, shutdown_signal)` is Plan 02-03's ephemeral-port entry point. 293/293 workspace tests green. Covers CLI-02, DAEMON-01/02/03/04, INBOX-03.
- **Plan 02-03 (2026-04-14):** Five integration test binaries lock all Phase 2 ROADMAP success criteria at the OS-process boundary. `listen_smoke` (in-process run_on_listener, POST → 200 → inbox line), `listen_durability` (subprocess + SIGKILL-after-200 proves fsync-before-200), `listen_bind_collision` (two daemons same port → PortInUse), `listen_shutdown` (SIGINT → exit 0 within 5s via `/bin/kill -INT`), `listen_truncated_tail` (hand-crafted mid-write-crash inbox exercises `read_all` tail tolerance). Shared harness in `tests/common/listen_harness.rs` provides spawn/sign/POST/read + `ChildGuard` RAII cleanup. Smoke is in-process; the three contract tests are subprocesses because the OS-process boundary is load-bearing. Beacon-parse over bind-and-drop for durability eliminates the port-race window. Self-principal `agent:localhost/self` with `from == to` matches Plan 02-02's single-entry keyring. Found + fixed a stderr-drainer bug during shutdown-test debugging: the reader thread now drains stderr to EOF after finding the beacon so the daemon's mid-shutdown eprintln! never blocks on a full pipe. SIGINT-race fix: sync on TCP accept + 150ms settle so tokio's ctrl_c() handler is installed before the signal fires. 298/298 workspace tests green (293 baseline + 5 new). Covers DAEMON-05, INBOX-02/04/05 at the integration layer.

### Open TODOs

- None carried.

### Known Blockers

- **None.**

### Quick Tasks Completed

| # | Description | Date | Commit | Status | Directory |
|---|-------------|------|--------|--------|-----------|
| 260414-cme | Remove obsolete wave2_impl feature gate from famp-canonical | 2026-04-14 | a77cfe1 |  | [260414-cme-remove-obsolete-wave2-impl-feature-gate-](./quick/260414-cme-remove-obsolete-wave2-impl-feature-gate-/) |
| 260414-ecp | Wire UnsupportedVersion error on envelope decode (PR #2) | 2026-04-14 | 8d14341 |  | [260414-ecp-wire-unsupportedversion-error-on-envelop](./quick/260414-ecp-wire-unsupportedversion-error-on-envelop/) |
| 260414-esi | Seal famp field visibility + cover adversarial gaps (PR #2.1) | 2026-04-14 | 2e9cf92, bf4c70a |  | [260414-esi-seal-famp-field-visibility-and-cover-adv](./quick/260414-esi-seal-famp-field-visibility-and-cover-adv/) |
| 260414-f4i | famp-crypto rustdoc + README "How FAMP Signs a Message" + CONTRIBUTING.md (PR #3) | 2026-04-14 | c0c5311, 243fc19, 1b432c5 |  | [260414-f4i-docs-pr-famp-crypto-rustdoc-readme-overv](./quick/260414-f4i-docs-pr-famp-crypto-rustdoc-readme-overv/) |
| 260414-fjo | PR #4 architectural cleanup: drop Signer/Verifier traits, remove 5 stub crates, add famp umbrella re-exports | 2026-04-14 | 9e5426f, 08c442a, e8ecf9f |  | [260414-fjo-pr-4-architectural-cleanup-drop-signer-v](./quick/260414-fjo-pr-4-architectural-cleanup-drop-signer-v/) |
| 260414-g32 | PR #4.1 adversarial review followups: reword WeakKey doc, delete dead InvalidSigningInput variant, add is_weak() gate to CONTRIBUTING "Do Not Touch" list | 2026-04-14 | 278cb83 |  | [260414-g32-pr-4-1-fix-weakkey-docstring-drop-dead-v](./quick/260414-g32-pr-4-1-fix-weakkey-docstring-drop-dead-v/) |
| 260420-viu | Fail-open on InvalidUuid in inbox list filter (follow-up to 974cc4b) | 2026-04-21 | 42327a1 |  | [260420-viu-fail-open-on-invaliduuid-in-inbox-list-f](./quick/260420-viu-fail-open-on-invaliduuid-in-inbox-list-f/) |
| 260424-7z5 | Fix famp_send new_task body loss — scope instructions lift | 2026-04-24 | 526ac2c, fddc24d, 9f94d0c |  | [260424-7z5-fix-famp-send-new-task-body-loss-scope-i](./quick/260424-7z5-fix-famp-send-new-task-body-loss-scope-i/) |
| 260425-gst | Fix await commit-receipt FSM error suppression (bug B2) | 2026-04-25 | a31c1c0, c69b4e9 |  | [260425-gst-fix-famp-fsm-commit-receipt-error-suppre](./quick/260425-gst-fix-famp-fsm-commit-receipt-error-suppre/) |
| 260425-ho8 | Close lost-update race in await commit-receipt via try_update; drop gag dep | 2026-04-25 | 6c35460, 1f66f4d, 65e5bb2 | Verified | [260425-ho8-fix-lost-update-race-in-await-commit-rec](./quick/260425-ho8-fix-lost-update-race-in-await-commit-rec/) |
| 260425-kbx | Harden await commit-receipt RED test (sentinel discriminator) + tighten try_update rustdoc | 2026-04-25 | 004ea87, 36d6b72 |  | [260425-kbx-harden-await-commit-receipt-red-test-tig](./quick/260425-kbx-harden-await-commit-receipt-red-test-tig/) |
| 260425-lg7 | Tighten try_update closure-Err guarantee bullet (round-3 honesty fix) | 2026-04-25 | cf29196 |  | [260425-lg7-tighten-try-update-closure-err-docstring](./quick/260425-lg7-tighten-try-update-closure-err-docstring/) |
| 260425-lny | Fix B2-class FSM error suppression at send/mod.rs:514 | 2026-04-25 | 22eacd3, 238e397 |  | [260425-lny-fix-b2-class-bug-at-send-mod-rs-514-surf](./quick/260425-lny-fix-b2-class-bug-at-send-mod-rs-514-surf/) |
| 260425-m0f | Write scripts/redeploy-listeners.sh — safe rebuild + restart of all FAMP listeners (T1.3) | 2026-04-25 | af4c8e9, c018ed1 | Verified | [260425-m0f-write-scripts-redeploy-listeners-sh-safe](./quick/260425-m0f-write-scripts-redeploy-listeners-sh-safe/) |
| 260425-of2 | Tighten MCP body schema docstring on famp_send (T1.2 / Fix B3) | 2026-04-25 | ccdb636, 1c6d4c5 | Verified | [260425-of2-t1-2-tighten-mcp-body-schema-docstring](./quick/260425-of2-t1-2-tighten-mcp-body-schema-docstring/) |
| 260425-pc7 | Add scope.more_coming flag on new_task envelopes — sender signals "more briefing follows" (T2.1 / Gap G4) | 2026-04-25 | 0c00ade, 937c34a, 2f71fda, 2a386ba, 756208d, 70009d8 | Verified | [260425-pc7-add-more-coming-flag-to-new-task-envelop](./quick/260425-pc7-add-more-coming-flag-to-new-task-envelop/) |
| 260425-re1 | README "Verifying a redeploy succeeded" subsection (T2.2 spot-check follow-up) | 2026-04-25 | 5f78651 | Verified | [260425-re1-t2-2-readme-redeploy-verification-spot-c](./quick/260425-re1-t2-2-readme-redeploy-verification-spot-c/) |
| 260425-rz6 | Fix CliError::Envelope masking FSM transition errors — add FsmTransition + InvalidTaskState variants (adversarial review bonus) | 2026-04-25 | e749af7, 7932389, 33747bc | Verified | [260425-rz6-fix-clierror-envelope-masking-fsm-transi](./quick/260425-rz6-fix-clierror-envelope-masking-fsm-transi/) |
| 260425-sl0 | File three Tier-3 backlog items as Phase 999.3/4/5 (heartbeat, user_attention, spec-by-path deferral to v1.0) — T3.x | 2026-04-25 | 6bce7e2 | Verified | [260425-sl0-t3-x-file-three-backlog-items-999-3-999-](./quick/260425-sl0-t3-x-file-three-backlog-items-999-3-999-/) |

## Session Continuity

### Recent Activity

- **2026-04-25:** Completed quick task 260425-sl0: Filed three Tier-3 backlog items (T3.x) into ROADMAP.md as Phase 999.3 (`heartbeat` envelope class — G3 work-in-progress visibility), 999.4 (`user_attention` envelope class — G5 human-in-loop primitive), and 999.5 (spec-by-path tracking — G2, **deferred to v1.0 federation gateway, NOT promoted independently**). Each entry carries Goal/Context/Plans matching the existing 999.1/999.2 template and references the resume doc evidence trail. Filings only — no code, no tests. Commit `6bce7e2`.
- **2026-04-25:** Completed quick task 260425-rz6: Fix `CliError::Envelope` masking FSM transition errors (adversarial review bonus). Three error categories were being shoehorned into the `Envelope` variant in `crates/famp/src/cli/send/fsm_glue.rs` (parse_state failure, advance_committed FSM step, advance_terminal FSM step) — top-line stderr said `"envelope encode/sign failed"` even when nothing failed to encode/sign, and MCP `famp_error_kind` returned `"envelope_error"` for FSM rejections (real API mismatch for MCP consumers). Added two new variants: `FsmTransition(#[from] famp_fsm::TaskFsmError)` → `"fsm_transition_illegal"` and `InvalidTaskState { value }` → `"invalid_task_state"`. Drops the `.map_err(|e| CliError::Envelope(Box::new(e)))` boilerplate at the three sites; legitimate Envelope mappings in `send/mod.rs:415,418,481,482` left untouched. Workspace tests green; clippy clean. CLI-only — no daemon redeploy required. Commits `e749af7` (RED) + `7932389` (GREEN) + `33747bc` (exhaustive coverage).
- **2026-04-25:** Completed quick task 260425-re1: README "Verifying a redeploy succeeded" subsection (T2.2). Spot-check found 2/3 of resume-doc checklist already covered by 260425-m0f's README addition (daemon.pid path ✅, script link ✅), but the "how to verify a redeploy succeeded" item was missing. Added 6-line subsection pointing operators at four independent signals: script exit code + final "all N agent(s) cycled cleanly" line, per-agent summary table, fresh `listening on https://...` beacon in daemon.log via `tail -1`, and binary-timestamp check via `ls -l ~/.cargo/bin/famp`. README-only; no code touched. Commit `5f78651`.
- **2026-04-25:** Completed quick task 260425-of2: Tighten MCP `body` schema docstring on `famp_send` (T1.2 / Fix B3). Replaced generic `"Message content"` description at `crates/famp/src/cli/mcp/server.rs:47` with explicit guidance that `body` is REQUIRED on `new_task` (title is summary only) and is the reply text on `deliver`/`terminal`. Added regression test `mcp_famp_send_body_description_flags_required_for_new_task` in `tests/mcp_stdio_tool_calls.rs` asserting positive substring `"REQUIRED for new_task"` AND negative substring `"Message content"` (catches accidental reverts). No new dev-deps. Workspace tests green; clippy clean. Commits `ccdb636` (RED) + `1c6d4c5` (GREEN).
- **2026-04-25:** Completed quick task 260425-m0f: Write scripts/redeploy-listeners.sh — T1.3 safe daemon redeploy. Dirty-tree + in-flight-task guards, SIGTERM/SIGKILL cycling, daemon.log append, per-agent beacon verification, four flag modes (--dry-run, --force, --no-rebuild, interactive). shellcheck clean. README "Redeploying after daemon code changes" section added. 397/397 tests green, clippy clean. Commits af4c8e9 + c018ed1.
- **2026-04-25:** Completed quick task 260425-lny: Fix B2-class FSM error suppression at send/mod.rs:514. Replaced `let _ = advance_terminal(...)` inside `tasks.update(...)` with `tasks.try_update(...)` + explicit `match` over `TryUpdateError` variants, mirroring `await_cmd/mod.rs` post-ho8 verbatim. Sentinel-discriminator TDD: RED test proves spurious write; GREEN fix proves no write on closure Err. Stash-pop sanity confirmed. 397/397 workspace tests green. Commits `22eacd3` (RED test) + `238e397` (GREEN fix).
- **2026-04-25:** Completed quick task 260425-gst: Fix await commit-receipt FSM error suppression (bug B2). Two `let _ =` swallowing errors from `advance_committed()` and `tasks.update()` replaced with explicit `match` + `eprintln!`. TDD: mtime-based test proves no spurious disk writes on FSM error. 391/391 workspace tests green. Commits `a31c1c0` (RED test) + `c69b4e9` (GREEN fix).
- **2026-04-15:** **Plan 04-02 shipped — MCP stdio server + 4 tools.** Hand-rolled Content-Length-framed JSON-RPC server, exhaustive `CliError::mcp_error_kind()` (28 variants, no wildcard), `famp_send`/`famp_await`/`famp_inbox`/`famp_peers` tools, 4/4 subprocess integration tests pass. Commits `f2fb5ff` (Task 1) + `7005886` (Task 2). 353/354 workspace tests (1 pre-existing `send_new_task` failure). MCP-01..06 complete.
- **2026-04-14:** **Plan 02-03 shipped — Phase 2 complete.** 5 integration test binaries lock DAEMON-01/02/03/04/05 + INBOX-02/04/05 at the OS-process boundary. Shared harness at `tests/common/listen_harness.rs` (spawn/sign/POST/read + ChildGuard). 298/298 workspace tests green. Commits `82776b9` (test: harness) + `4d14f0f` (test: 5 integration tests + stderr-drainer fix + SIGINT-race fix).
- **2026-04-14:** **Plan 02-02 shipped** — `famp listen` daemon wired end-to-end. Custom Router reusing `FampSigVerifyLayer` + inbox-append handler returning 200 (durability receipt); SIGINT/SIGTERM shutdown; `PortInUse` mapping for `AddrInUse`. 293/293 workspace tests. Commits `f51b590` (feat) + `0dc56a7` (fix: non-blocking listener). CLI-02, DAEMON-01/02/03/04, INBOX-03 complete.
- **2026-04-14:** **Plan 02-01 shipped** — `famp-inbox` library crate with durable append (fsync-before-return) + tail-tolerant read. 8/8 crate tests, 292/292 workspace tests, `cargo tree -i openssl` empty. Commits `b7ca9bb` (feat) + `071b781` (test). INBOX-01/02/04/05 complete.
- **2026-04-14:** **v0.8 roadmap created.** 4 phases, 37 requirements, 100% coverage. Phase 1 (Identity & CLI Foundation) queued for `/gsd:plan-phase 1`.
- **2026-04-14:** **v0.7 Personal Runtime shipped.** 4/4 phases, 15/15 plans, 32/32 requirements, 253/253 tests. Archived to `.planning/milestones/v0.7-*.md`.
- **2026-04-14:** Completed quick task 260414-g32: PR #4.1 adversarial review followups. `just ci` green. 261/261 workspace tests.
- **2026-04-14:** Completed quick task 260414-fjo: PR #4 architectural cleanup. Drop Signer/Verifier traits, remove 5 stub crates, add famp umbrella re-exports. `just ci` green. 261/261 workspace tests.

---
*2026-04-14 — v0.8 roadmap defined. 4 phases: (1) Identity & CLI Foundation — CLI-01/07, IDENT-01..06; (2) Daemon & Inbox — CLI-02, DAEMON-01..05, INBOX-01..05; (3) Conversation CLI — CLI-03..06, CONV-01..05; (4) MCP Server & Same-Laptop E2E — MCP-01..06, E2E-01..03. 37/37 requirements mapped. Next: `/gsd:plan-phase 1`.*
| 2026-04-21 | fast | Disable nightly cron on nightly-full-corpus workflow | ✅ |

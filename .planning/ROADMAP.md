# Roadmap: FAMP

**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later.

## Milestones

- ✅ **v0.5.1 Spec Fork** — Phases 0–1 (shipped 2026-04-13). Interop contract locked; FAMP-v0.5.1-spec.md authoritative. See [milestones/v0.5.1-ROADMAP.md](milestones/v0.5.1-ROADMAP.md).
- ✅ **v0.6 Foundation Crates** — Phases 1–3 (shipped 2026-04-13). Substrate shipped: `famp-canonical`, `famp-crypto`, `famp-core`. 25/25 requirements satisfied, 112/112 tests green. See [milestones/v0.6-ROADMAP.md](milestones/v0.6-ROADMAP.md).
- ✅ **v0.7 Personal Runtime** — Phases 1–4 (shipped 2026-04-14). Minimal usable library on two transports. 4/4 phases, 15/15 plans, 253/253 tests green.
- ✅ **v0.8 Usable from Claude Code** — Phases 1–4 + v0.8.x bridge (shipped 2026-04-26). CLI + daemon + inbox + MCP server + session-bound identity (`famp_register`/`famp_whoami`, `FAMP_LOCAL_ROOT`-only). 5/5 phases, 18/18 plans, 39/39 requirements (37 + 2 bridge), 419/419 tests green. See [milestones/v0.8-ROADMAP.md](milestones/v0.8-ROADMAP.md) · [milestones/v0.8-MILESTONE-AUDIT.md](milestones/v0.8-MILESTONE-AUDIT.md).
- ✅ **v0.9 Local-First Bus** — Phases 1–4 + close-fix Phase 5 (shipped 2026-05-04). UDS-backed broker replacing the per-identity TLS listener mesh; zero crypto on the local path; IRC-style channels; durable per-name mailboxes; 8-tool stable MCP surface. 5/5 phases, 35 plans, **85/85 requirements**, audit `passed`. Federation internals (`famp-transport-http`, `famp-keyring`) preserved in CI via library-API `e2e_two_daemons`; escape-hatch tag `v0.8.1-federation-preserved`. See [milestones/v0.9-ROADMAP.md](milestones/v0.9-ROADMAP.md) · [milestones/v0.9-REQUIREMENTS.md](milestones/v0.9-REQUIREMENTS.md) · [milestones/v0.9-MILESTONE-AUDIT.md](milestones/v0.9-MILESTONE-AUDIT.md).
- ✅ **v0.10 Inspector & Observability** — Phases 1–3 (shipped 2026-05-11). Read-only inspector RPC on the v0.9 broker UDS + `famp inspect` CLI subcommand. Closes the conversation-state opacity gap that produced three recurring v0.9 incidents (orphan socket-holder vs stale PID file, task FSM invisibility, stale-mailbox relays). 26/26 requirements, audit `passed`. See [milestones/v0.10-ROADMAP.md](milestones/v0.10-ROADMAP.md) · [milestones/v0.10-REQUIREMENTS.md](milestones/v0.10-REQUIREMENTS.md) · [milestones/v0.10-MILESTONE-AUDIT.md](milestones/v0.10-MILESTONE-AUDIT.md).
- ✅ **v0.11 Broker Daemon & Cross-Tool Bootstrap** — Phases 4–6 (shipped 2026-06-06). Service-managed daemon (`famp daemon install`) restores the broker-presence guarantee that `56b2293` (correctly) removed; EPERM sandbox diagnostics + daemon install/status/uninstall/restart lifecycle + version-skew detection + daemon-first cross-platform README. 15/15 requirements, audit waived (Phase 6 human-verify E2E). See [milestones/v0.11-ROADMAP.md](milestones/v0.11-ROADMAP.md) · [milestones/v0.11-REQUIREMENTS.md](milestones/v0.11-REQUIREMENTS.md).
- 📋 **v1.0 Federation Profile — Gateway Core** — Phases 7–10 (roadmap created 2026-07-23; not yet started). Gate A fired (Ben's sustained cross-machine use); this milestone closes it: an agent on one of Ben's machines exchanges a signed FAMP envelope with an agent on a second machine he controls, bidirectionally and reliably, over a network he fully controls (direct or a VPN he already runs — no public relay, no cross-person trust). Resolves the broker-liveness fork (same-host `kill(pid,0)` reaping a naively-proxied remote principal), ships `famp-gateway` (Layer 2) wrapping the preserved `famp-transport-http` + `famp-keyring`, signed cross-host envelopes (INV-10 + forward-compat fields), two-machine TOFU key bootstrap, and reactivates the ~27 deferred federation tests. Tags `v1.0.0` on completion. Gate B (conformance vector pack, 2nd implementer) stays event-driven and out of this milestone's scope. See [REQUIREMENTS.md](REQUIREMENTS.md) (supersedes the 2026-06-08 mesh-VPN Gate A draft below in "Future Milestone Sketch").

## Phases

- [x] **Phase 1: Broker Diagnosis & Identity Inspection** — completed 2026-05-10 — `famp.inspect.*` namespace mounted on broker UDS, all three crates (`-proto`, `-client`, `-server`) shipped, `famp inspect broker` and `famp inspect identities` end-to-end (RPC + CLI). Closes the orphan-listener incident class in one merge.
- [x] **Phase 2: Task FSM & Message Visibility** — completed 2026-05-10 — `famp inspect tasks` and `famp inspect messages` end-to-end (RPC + CLI). I/O-bound handlers (taskdir + mailbox file walks) with 500 ms latency budget (INSP-RPC-03) and cancellable-handler discipline (INSP-RPC-04, 1000-concurrent-cancel test passing).
- [x] **Phase 3: Load Verification & Integration Hardening** — load test proving inspect-call traffic cannot starve bus message throughput (INSP-RPC-05); end-to-end orphan-listener scenario re-exercises Phase 1's `inspect broker` under integration conditions; doc + migration notes. **Complete 2026-05-11** (GAP-03-01 resolved; saturated direct-RPC ratio 0.82-1.01 vs prior 0.17).
- [x] **Phase 4: Broker Lifecycle & Bootstrap Diagnostics** — completed 2026-06-04 — `famp broker --no-idle-exit` flag (hard prerequisite for daemon — a daemon-managed broker must not self-terminate), its regression guard, and actionable EPERM-on-bind error surfacing the sandbox-constraint explanation.
- [x] **Phase 5: Daemon Service Management & Version Safety** — completed 2026-06-04 — `famp daemon install/uninstall/status/restart` cross-platform service lifecycle (launchd macOS, systemd `--user` Linux) plus version handshake at connect and `famp -V` banner reconciliation. 9/9 requirements; DAEMON-06 Linux behavioral deferred to a Linux host (05-HUMAN-UAT.md).
- [x] **Phase 6: Onboarding & Cross-Platform Docs** — completed 2026-06-06 — README rewritten daemon-first: `famp daemon install` quickstart (Claude Code + Codex), `famp broker --no-idle-exit` no-install bridge, dedicated `## Platform support` boundary, five reconciled downstream sections, v0.9→v0.11 refresh. DOC-01/02/03 verified live against the installed binary (human-verify E2E: fresh-clone Claude+Codex delivery + daemon lifecycle). Accuracy gate caught a stale-binary idempotency failure (fixed via `just install`) and one status exit-code drift (corrected).
- [ ] **Phase 7: Broker-Liveness Fork + Gateway Skeleton** - Resolve the same-host `kill(pid,0)` liveness fork (Design A local-proxy) and stand up the `famp-gateway` crate skeleton backing remote principals on the local bus.
- [ ] **Phase 8: Signed Cross-Host Envelope + Trust Bootstrap** - Ed25519-signed, forward-compatible cross-host envelope format plus two-machine TOFU key export/import.
- [ ] **Phase 9: End-to-End Cross-Host Delivery** - Full bidirectional `request → commit → deliver → ack` task cycle across two machines through the gateway.
- [ ] **Phase 10: Test Reactivation + Setup Docs** - Deferred federation tests triaged and green, a live two-process E2E in `just ci`, and a two-machine setup guide.

## Phase Details

### Phase 1: Broker Diagnosis & Identity Inspection

**Goal:** Operator runs `famp inspect broker` and `famp inspect identities` against the v0.9 broker and gets the conversation state needed to retire the orphan-listener incident class — including the load-bearing dead-broker diagnosis (`famp inspect broker` is the one command that must work even when the broker is dead). All three inspector crates (`famp-inspect-proto`, `famp-inspect-client`, `famp-inspect-server`) ship in this phase under workspace dependency-version discipline.
**Depends on:** v0.9 broker (`famp-bus`, `~/.famp/bus.sock`)
**Requirements:** INSP-BROKER-01..04, INSP-IDENT-01..03, INSP-RPC-01, INSP-RPC-02, INSP-CRATE-01, INSP-CRATE-02, INSP-CRATE-03, INSP-CLI-01, INSP-CLI-02, INSP-CLI-03, INSP-CLI-04 (16 requirements)
**Success Criteria** (what must be TRUE):

  1. `famp inspect broker` against a running broker prints `state: HEALTHY` plus pid, socket path, started-at, and build version on a single human-readable line; against a non-running broker it prints exactly one of `DOWN_CLEAN | STALE_SOCKET | ORPHAN_HOLDER | PERMISSION_DENIED` plus the evidence row used to decide. Detection is connect-handshake-based (no PID file): `DOWN_CLEAN` = no socket file; `STALE_SOCKET` = socket file present but `connect()` returns `ECONNREFUSED`; `ORPHAN_HOLDER` = `connect()` succeeds but the listener rejects FAMP's `Hello` (wrong `bus_proto` or non-bus reply); `PERMISSION_DENIED` = `EACCES`. Exit 0 only on `HEALTHY`; exit 1 with diagnosis on stdout for the four down-states.
  2. `famp inspect identities` lists every registered session identity with name, listen-mode, registered-at, last-activity, cwd, and mailbox unread/total/last-sender/last-received-at — and contains zero "double-print" / "received vs surfaced" counters (deferred per Out of Scope; the surface deliberately does not attempt the wrong instrument).
  3. The broker accepts `BusMessage::Inspect { kind, ... }` frames on the existing UDS socket (no separate inspector socket); the dispatch path is read-only by construction — handler signatures take `&BrokerState` (not `&mut`), and `just check-inspect-readonly` fails CI if `famp-inspect-server` transitively imports any mailbox-write, taskdir-write, or broker `&mut self` mutation surface.
  4. Every `famp inspect <subcommand>` accepts `--json` emitting a stable documented JSON shape; default output is fixed-width column-aligned with explicit headers (no Rust `Debug` format); when the broker is not running, `famp inspect identities` exits 1 with stderr `"error: broker not running at <socket-path>"` while `famp inspect broker` continues to work against the dead broker per success criterion 1.
  5. `just check-no-io-in-inspect-proto` (parallel to `check-no-tokio-in-bus`) fails compilation if `famp-inspect-proto` acquires a tokio / axum / reqwest / clap dependency; `cargo tree -p famp-inspect-client` contains no `clap` dependency (linkable by future SPA / `famp doctor` consumers); `cargo tree` shows `famp-inspect-server` linked to the same `famp-canonical`, `famp-envelope`, `famp-fsm` versions as the broker (no Cargo-resolved version skew).

**Plans:** 4 plans
Plans:

- [x] 01-01-PLAN.md — Wave 0: Proto types + state extensions + Wave-0 test scaffolds (famp-inspect-proto crate, BusMessage::Inspect variant, BrokerState::new with started_at, Register cwd/listen extension)
- [x] 01-02-PLAN.md — Wave 1: famp-inspect-server (tokio-free, &BrokerStateView handlers) + famp-inspect-client (UDS, no clap, peer_pid)
- [x] 01-03-PLAN.md — Wave 2: Broker dispatch arm (BusMessage::Inspect → famp-inspect-server) + CLI subcommand scaffolding
- [x] 01-04-PLAN.md — Wave 3: CLI rendering (HEALTHY + 4 down-states + table) + integration tests + 3 just check-* recipes wired into ci:

### Phase 2: Task FSM & Message Visibility

**Goal:** Operator runs `famp inspect tasks` and `famp inspect messages` and gets the FSM and envelope-metadata visibility that v0.9's task-FSM-invisibility and stale-mailbox-relay incidents asked for. This is the phase where the I/O-bound handlers land — taskdir file walks for tasks, mailbox file reads for messages — so it's also the phase where the 500 ms latency budget (INSP-RPC-03) and cancellable-handler discipline (INSP-RPC-04) gain real handlers to enforce against (Phase 1's pure in-memory handlers had nothing to budget or cancel; the budget would have been theater).
**Depends on:** Phase 1
**Requirements:** INSP-TASK-01..04, INSP-MSG-01..03, INSP-RPC-03, INSP-RPC-04 (9 requirements)
**Success Criteria** (what must be TRUE):

  1. `famp inspect tasks` groups by task_id with FSM state + envelope count + last-transition-age, surfaces `task_id == 0` rows in a top-level `--orphans` bucket above the per-task groups, supports `--id <task_id>` for the envelope chain summary and `--id <task_id> --full` whose output piped through `jq` reproduces the canonical JCS bytes that fed each envelope's signature input.
  2. `famp inspect messages --to <name>` returns envelope metadata only — sender, recipient, task_id, MessageClass, FSM state, timestamp, body byte length, body sha256 prefix (first 12 hex chars) — never message bodies; `--tail N` limits to the most-recent N envelopes (default 50).
  3. An I/O-bound inspect handler exceeding the 500 ms default latency budget is dropped at the tokio wrapper layer with a `BudgetExceeded` reply; concurrent bus message delivery on the same socket is unaffected (no queue stall). The budget enforces in `crates/famp/src/cli/broker/` (the tokio wrapper), not inside `famp-bus` (which stays tokio-free per the existing CI gate).
  4. A test issues 1000 concurrent `famp inspect tasks` and `famp inspect messages` calls and cancels them mid-flight; all 1000 close cleanly with no leaked file descriptors, mailbox locks, or in-flight allocations. Verified via `lsof` snapshot before/after the test plus an explicit allocation tracker.

**Plans:** 3 plans
Plans:

- [x] 02-01-PLAN.md — Wave 1: Proto enum reply types + famp-inspect-server TaskSnapshot/MessageSnapshot + sync handlers (D-01/D-02 wire commitment)
- [x] 02-02-PLAN.md — Wave 2: Broker executor spawn_blocking + timeout(500ms) wrapper + lazy taskdir/mailbox pre-read + max_blocking_threads(1024)
- [x] 02-03-PLAN.md — Wave 3: famp inspect tasks/messages CLI + integration tests + 1000-cancel test + nextest.toml serialization

### Phase 3: Load Verification & Integration Hardening

**Goal:** Prove under integration-grade conditions that (a) inspect-call pressure does not starve bus message throughput and (b) the dead-broker diagnosis path actually disambiguates the orphan-socket-holder failure class that produced the v0.9 incident, then ship the docs.
**Depends on:** Phase 2
**Requirements:** INSP-RPC-05 (1 requirement; load test owns this. Phase 1's INSP-BROKER-02..04 + INSP-CLI-04 are re-exercised under E2E integration conditions but ownership stays in Phase 1.)
**Success Criteria** (what must be TRUE):

  1. A sustained load test runs concurrent `famp.inspect.*` calls at saturating rate alongside live bus message traffic; bus message throughput under inspect pressure stays within an explicit, committed percentage of unloaded throughput (target threshold set during plan-phase). No starvation.
  2. An end-to-end orphan-listener scenario test reproduces the v0.9 incident class (a non-FAMP process holds `~/.famp/bus.sock`); `famp inspect broker` correctly reports state `ORPHAN_HOLDER` with the holder PID in the evidence row, exit code 1, diagnosis on stdout — verifying INSP-BROKER-02/03/04 + INSP-CLI-04 ride the full integration path, not just unit tests.
  3. `docs/MIGRATION-v0.9-to-v0.10.md` (or the v0.10 release-notes section of the README) names the new `famp inspect` surface, the four down-state values from `famp inspect broker`, the `--json` shape commitment, and explicitly calls out the read-only discipline + the deferred items (no `--body`, no doctor, no SPA, no double-print counter).

**Plans:** 3 plans
Plans:

- [x] 03-01-PLAN.md — Wave 1: INSP-RPC-05 load test (`inspect_load_test.rs`) + nextest.toml `inspect-subprocess` filter extension
- [x] 03-02-PLAN.md — Wave 1 (parallel): v0.9-incident-class label on existing orphan E2E test + `docs/MIGRATION-v0.9-to-v0.10.md` migration doc
- [x] 03-03-PLAN.md — Wave 2 (gap closure): saturated direct inspect RPC no-starvation proof for GAP-03-01

### Phase 4: Broker Lifecycle & Bootstrap Diagnostics

**Goal:** Users can run a long-lived broker that never self-terminates on idle, and sandboxed clients (Codex) receive an actionable error explaining the constraint and the remedy rather than a generic "broker unreachable" failure.
**Depends on:** v0.10 broker (the running `famp-bus` / `~/.famp/bus.sock`)
**Requirements:** BLC-01, BLC-02, BOOT-01
**Success Criteria** (what must be TRUE):

  1. A broker started with `famp broker --no-idle-exit` and zero connected clients is still alive — verified by a test using tokio time-pause or equivalent — after the 300-second idle window elapses. The flag appears in `--help` output with a one-line description.
  2. A broker started without the flag (the existing default) still self-terminates after 300 seconds of idle; the existing BROKER-04/04b idle-exit tests pass byte-for-byte with no behavior change, confirming the `56b2293` orphan-leak fix is intact.
  3. When `spawn.rs:92`'s `bind()` call returns EPERM (sandboxed shell, as in Codex's seatbelt), the client surfaces a message distinguishing the sandbox cause from other spawn failures — naming the cause ("can't create a broker inside a sandbox") and the remedy ("run `famp daemon install` from a normal shell") — instead of swallowing the errno via `let _ =`. A test injecting or simulating EPERM-on-bind asserts the actionable message and confirms EPERM is distinguished from non-EPERM spawn failures. This directly extends the connect/spawn-stage disambiguation shipped in commits `4da30a3`/`ebbf1d3`.

**Plans:** 3/3 complete
Plans:

- [x] 04-01-PLAN.md — `famp broker --no-idle-exit` flag + no-idle-exit regression coverage
- [x] 04-02-PLAN.md — SandboxEperm parent-side bind probe + CLI/MCP actionable diagnostics
- [x] 04-03-PLAN.md — Deployed binary refresh + full suite/deployed help verification

**Constraint notes:** Changes land in `crates/famp/src/cli/broker/mod.rs` (BLC-01/02) and `crates/famp/src/bus_client/spawn.rs` (BOOT-01); protocol-primitive crates (`famp-bus`, `famp-canonical`, `famp-crypto`) stay untouched. Run `just install` before closing a PR that changes the spawn-error surface (the installed `~/.cargo/bin/famp` is what agent sessions read). Pre-commit hook remains fmt-check only.

### Phase 5: Daemon Service Management & Version Safety

**Goal:** Users run `famp daemon install` once from a normal (unsandboxed) shell and FAMP is permanently reachable from any client — sandboxed or not — without per-session broker babysitting; version-skew between a long-lived daemon and an upgraded client is caught loudly at connect rather than silently misrouted.
**Depends on:** Phase 4 (requires `--no-idle-exit` before writing a service that launches the broker with that flag)
**Requirements:** DAEMON-01, DAEMON-02, DAEMON-03, DAEMON-04, DAEMON-05, DAEMON-06, BOOT-02, VER-01, VER-02
**Success Criteria** (what must be TRUE):

  1. After `famp daemon install` on macOS, `famp inspect broker` reports `HEALTHY`; running `install` a second time leaves exactly one service registration and one running broker (idempotent). The installed LaunchAgent plist matches the guardian-approved shape exactly — `RunAtLoad=true`, `KeepAlive=true` (unconditional), `ProcessType=Background`, `StandardOutPath`/`StandardErrorPath` → `~/.famp/broker.log`, no `EnvironmentVariables` key, `ProgramArguments` invokes `~/.cargo/bin/famp broker --no-idle-exit` — and the literal plist XML is reviewed and approved by guardian before the service is first loaded. This external review gate is a blocking requirement for this phase: do not load the service until the plist has guardian sign-off.
  2. `famp daemon status` prints the broker PID and socket path when running (exit 0), prints "installed but broker not running" when the service is registered but the broker is not alive (exit non-zero), and prints "not installed" when the service has never been installed (exit non-zero); all three states produce distinct output and distinct exit codes.
  3. `famp daemon uninstall` unloads and removes the service file; `launchctl`/`systemctl --user` listings show no orphaned registration afterward; running `uninstall` again exits 0 with no error (idempotent). `famp daemon restart` picks up a replaced on-disk binary — verifiable by running the new binary's version after restart and confirming it differs from the binary version before restart.
  4. `famp daemon install` refuses to run when invoked inside a sandbox (the same EPERM-on-bind condition detected in Phase 4), exiting non-zero with guidance rather than writing a service that can never bind. On Linux, when systemd `--user` or `loginctl enable-linger` is unavailable, install exits non-zero with a message pointing to the documented manual fallback (`famp broker --no-idle-exit`) rather than producing a silent half-install.
  5. On connect, client and broker exchange a protocol/build version; a client whose version is incompatible with the running daemon receives a loud actionable error and exits non-zero rather than proceeding silently. Compatible versions connect normally. `famp -V`, the help banner, and the handshake version all agree on the same value — the pre-existing `0.1.0`-crate-vs-`0.5.x`-banner discrepancy is resolved to a single source of truth.

**Plans:** 5 plans
Plans:

- [x] 05-01-PLAN.md — VER-02 version unification (0.11.0) + VER-01 client proto-mismatch enforcement
- [x] 05-02-PLAN.md — `famp daemon` scaffold + plist generation (DAEMON-02 shape) + reviewable fixture
- [x] 05-03-PLAN.md — guardian plist sign-off gate (BLOCKING, pre-load, autonomous: false)
- [x] 05-04-PLAN.md — install/uninstall load + BOOT-02 sandbox refusal + Linux systemd/linger (DAEMON-01/04/06)
- [x] 05-05-PLAN.md — three-state status (DAEMON-03, D-09 linger) + restart binary-pickup (DAEMON-05)

**Constraint notes:** `famp daemon` subcommand lands in `crates/famp/src/cli/`; it is CLI-layer and does not touch protocol-primitive crates. Run `just install` after any plist-shape or daemon-subcommand change (installed binary is the deployment target, not `target/release/famp`). Socket activation (launchd/systemd holds the socket and starts broker on first connect) is explicitly deferred — deferred because fd-inheritance is not implemented; the unconditional-KeepAlive plist is the correct interim shape. Spawn-lock for the `bind_exclusive` stale-branch unlink-race is also deferred to its own track (the daemon dissolves the race for daemon users).

### Phase 6: Onboarding & Cross-Platform Docs

**Goal:** A developer who has never used FAMP can do one of two things: install the daemon once and have both Claude Code and Codex connect automatically forever, or skip install and run a single terminal command for an immediate zero-setup bridge — and in either case the README tells them exactly which platforms are covered and which are not.
**Depends on:** Phase 5 (docs must describe commands that exist and behave as documented; DOC-02's bridge line depends on BLC-01 landing in Phase 4)
**Requirements:** DOC-01, DOC-02, DOC-03
**Success Criteria** (what must be TRUE):

  1. The README contains a quickstart section where `famp daemon install` is the one command, and a fresh-clone walkthrough on macOS — install the daemon, register from a Claude Code window, register from a Codex window — completes without broker-babysitting. The quickstart is accurate against actual `famp daemon install` behavior shipped in Phase 5.
  2. The README documents the zero-setup bridge: run `famp broker --no-idle-exit` in one unsandboxed terminal; any sandboxed or normal client then connects to that broker. The instructions are accurate against the `--no-idle-exit` behavior shipped in Phase 4.
  3. The README contains an explicit cross-platform support section naming what the installer covers (macOS launchd, Linux systemd `--user`) and what it does not (minimal distros without systemd, containers, WSL, headless without `loginctl enable-linger`), pointing unsupported configurations to the `famp broker --no-idle-exit` manual fallback. No "works for both Claude and Codex" claim overruns what the Phase 5 installer actually delivers.

**Plans:** 3 plans
Plans:
**Wave 1**

- [x] 06-01-PLAN.md — Wave 1: three-tier getting-started block (daemon-first quickstart + no-install bridge + `## Platform support` boundary) [DOC-01/02/03]

**Wave 2** *(blocked on Wave 1 completion)*

- [x] 06-02-PLAN.md — Wave 2 (depends_on 01): five D-04 reconciliation edits (CLI table, Onboarding path, When-NOT-to-Use reword, Upgrading, Troubleshooting) + v0.9→v0.11 version refresh

**Wave 3** *(blocked on Wave 2 completion)*

- [x] 06-03-PLAN.md — Wave 3 (depends_on 01,02): D-07 accuracy-against-binary gate — auto grep verify + BLOCKING human-verify (live launchctl lifecycle + fresh-clone Claude+Codex E2E)

### Phase 7: Broker-Liveness Fork + Gateway Skeleton

**Goal:** The same-host `kill(pid,0)` liveness fork is resolved — a gateway-proxied remote principal stays live for as long as the gateway process is alive and reaps cleanly when it exits — and the `famp-gateway` crate skeleton exists to back concurrent remote principals on the local UDS bus. This is the spine every later v1.0 phase depends on: Design A (local-proxy — the gateway backs each remote principal with a `bind_as` connection reporting the gateway's own live local PID, zero `famp-bus` change) is the recommended resolution; Design B (heartbeat/lease) is the fallback if Design A proves infeasible during implementation.
**Depends on:** v0.11 broker daemon (current runtime, `famp-bus` UDS at `~/.famp/bus.sock`); reuses `famp-transport-http` + `famp-keyring` (v0.8-preserved, tag `v0.8.1-federation-preserved`) without rebuilding them.
**Requirements:** LIVE-01, LIVE-02, GW-04 (3 requirements)
**Success Criteria** (what must be TRUE):

  1. A gateway-proxied remote principal shows as live (not reaped) in `famp inspect identities` / `famp inspect broker` for as long as the `famp-gateway` process backing it is running — including across the broker's normal same-host `kill(pid,0)` liveness sweep, which today reaps any principal not backed by a genuinely live local PID (LIVE-01).
  2. When the `famp-gateway` process exits, every principal it was proxying is reaped cleanly within one liveness-sweep interval — `famp inspect identities` / `famp inspect broker` show no orphan holders left behind (LIVE-02).
  3. A single `famp-gateway` process backs two or more remote principals concurrently; a message addressed to one proxied principal is never delivered into, or visible in, another proxied principal's mailbox (GW-04).

**Plans:** 3 plans
**Wave 1**

- [x] 07-01-PLAN.md — famp-gateway crate scaffold + connect-with-own-PID mechanism (ProxiedPrincipal/GatewayRegistry/bin) + no-spawn BusClient constructor (LIVE-01, LIVE-02, GW-04)
- [ ] 07-02-PLAN.md — pure-broker LIVE-01 test: N clients sharing one PID survive the sweep and reap together (LIVE-01)

**Wave 2** *(blocked on Wave 1 completion)*

- [ ] 07-03-PLAN.md — subprocess integration tests: gateway-exit reaping + no cross-talk (LIVE-02, GW-04)

### Phase 8: Signed Cross-Host Envelope + Trust Bootstrap

**Goal:** Every envelope that crosses the gateway boundary between two machines is Ed25519-signed under INV-10 and carries the forward-compatible fields v1.1/v2.0 need without a wire break, and two machines Ben controls establish mutual key trust via out-of-band export/import with TOFU pinning — with no implicit trust for unpinned keys. Reuses `famp-crypto` (Ed25519, `FAMP-sig-v1\0` domain prefix) and `famp-canonical` (JCS) as-is; this phase extends `famp-envelope` and wires the preserved `famp-keyring` into the gateway's cross-host path.
**Depends on:** Phase 7 (the gateway skeleton and resolved liveness model must exist before the wire format and trust bootstrap run over it)
**Requirements:** WIRE-01, WIRE-02, TRUST-01, TRUST-02 (4 requirements)
**Success Criteria** (what must be TRUE):

  1. Every envelope crossing between two machines is Ed25519-signed under the `FAMP-sig-v1\0` domain prefix; an unsigned or signature-invalid envelope is rejected at the receiving gateway before it touches the local bus (WIRE-01).
  2. The cross-host envelope schema carries sender/receiver domain + key_id, a nonce, and an expiry, with capability/approval fields omitted when empty, and round-trips through canonical-JSON encode/decode byte-exact (WIRE-02).
  3. A user runs a peer-export command on machine A, moves the output out-of-band (e.g. copy/paste, Signal), and runs a peer-import command on machine B (and the reverse, B→A); after both imports, each gateway trusts the other's signing key via TOFU pin — no manual key material is exchanged over FAMP itself (TRUST-01).
  4. A cross-host envelope signed by a key that was never exported/imported/pinned is rejected by the receiving gateway with no state created and no implicit trust granted (TRUST-02).

**Plans:** TBD

### Phase 9: End-to-End Cross-Host Delivery

**Goal:** A user on machine A addresses an agent on machine B by name/principal and a full bidirectional task exchange completes correctly through the gateway, with the task FSM advancing on both sides — proving the liveness fix (Phase 7) and the signed wire format + trust bootstrap (Phase 8) compose into the actual product promise of this milestone.
**Depends on:** Phase 7, Phase 8
**Requirements:** GW-01, GW-02, GW-03 (3 requirements)
**Success Criteria** (what must be TRUE):

  1. A user registers an agent on machine A, addresses an agent on machine B by name/principal, and the message is delivered into B's local bus mailbox (GW-01).
  2. The agent on machine B replies within the same task/conversation, and the reply is delivered back into A's local bus mailbox (GW-02).
  3. A full `request → commit → deliver → ack` task cycle completes across the two machines, with the task FSM advancing correctly on both sides to a terminal state — observable via `famp inspect tasks` on each machine (GW-03).

**Plans:** TBD

### Phase 10: Test Reactivation + Setup Docs

**Goal:** The deferred federation test suite is triaged and green in CI, a live two-process end-to-end test proves the full signed cross-host cycle on every `just ci` run (not just manually), and a new user can follow a written setup guide to stand up the gateway between two machines himself — closing the milestone with a durable regression net and a repeatable onboarding path.
**Depends on:** Phase 9 (tests and docs describe behavior that must already exist and work)
**Requirements:** TEST-01, TEST-02, DOC-04 (3 requirements)
**Success Criteria** (what must be TRUE):

  1. Every test in `crates/famp/tests/_deferred_v1/` (~27 parked tests) has been triaged: still-valid tests are reactivated and run green in CI, and each removed/obsolete test has documented rationale for its removal (TEST-01).
  2. A live two-process end-to-end test exercises the full signed cross-host task cycle (two gateway-backed processes, real signed envelopes over the wire) and runs as part of `just ci` on every commit — not gated behind a manual or `#[ignore]`'d path (TEST-02).
  3. A setup guide documents standing up the gateway on two machines — bind address, out-of-band key exchange (peer export/import from Phase 8), and connect/verify — and a developer following it unassisted successfully reaches a working cross-host connection (DOC-04).

**Plans:** TBD

<details>
<summary>✅ v0.5.1 Spec Fork (Phases 0–1) — SHIPPED 2026-04-13</summary>

- [x] Phase 0: Toolchain & Workspace Scaffold — completed 2026-04-13
- [x] Phase 1: Spec Fork (FAMP-v0.5.1) — completed 2026-04-13

Archive: [milestones/v0.5.1-ROADMAP.md](milestones/v0.5.1-ROADMAP.md)

</details>

<details>
<summary>✅ v0.6 Foundation Crates (Phases 1–3) — SHIPPED 2026-04-13</summary>

- [x] Phase 1: Canonical JSON Foundations (3/3 plans) — completed 2026-04-13 — SEED-001 resolved, RFC 8785 gate 12/12 green
- [x] Phase 2: Crypto Foundations (4/4 plans) — completed 2026-04-13 — Ed25519 `verify_strict`, §7.1c worked example byte-exact, NIST KATs green
- [x] Phase 3: Core Types & Invariants (2/2 plans) — completed 2026-04-13 — Principal/Instance, UUIDv7 IDs, ArtifactId, 15-category ProtocolErrorKind, AuthorityScope, INV-1..11

Archive: [milestones/v0.6-ROADMAP.md](milestones/v0.6-ROADMAP.md) · Phases: [milestones/v0.6-phases/](milestones/v0.6-phases/)

</details>

<details>
<summary>✅ v0.7 Personal Runtime (Phases 1–4) — SHIPPED 2026-04-14</summary>

- [x] Phase 1: Minimal Signed Envelope (3/3 plans) — completed 2026-04-13 — INV-10 mandatory-signature enforcement, 5 shipped message classes
- [x] Phase 2: Minimal Task Lifecycle (3/3 plans) — completed 2026-04-13 — 5-state TaskFsm, proptest transition legality, compiler-checked terminals
- [x] Phase 3: MemoryTransport + TOFU Keyring + Same-Process Example (4/4 plans) — completed 2026-04-13 — `personal_two_agents` example, 3 adversarial cases on MemoryTransport
- [x] Phase 4: Minimal HTTP Transport + Cross-Machine Example (5/5 plans) — completed 2026-04-14 — axum + rustls + reqwest, `cross_machine_two_agents` example, 3 adversarial cases × 2 transports

Archive: [milestones/v0.7-ROADMAP.md](milestones/v0.7-ROADMAP.md) · Audit: [milestones/v0.7-MILESTONE-AUDIT.md](milestones/v0.7-MILESTONE-AUDIT.md) · Requirements: [milestones/v0.7-REQUIREMENTS.md](milestones/v0.7-REQUIREMENTS.md)

</details>

<details>
<summary>✅ v0.8 Usable from Claude Code (Phases 1–4 + v0.8.x bridge) — SHIPPED 2026-04-26</summary>

- [x] Phase 1: Identity & CLI Foundation (3/3 plans) — completed 2026-04-14 — `famp init`, persistent Ed25519 + TLS, FAMP_HOME override
- [x] Phase 2: Daemon & Inbox (3/3 plans) — completed 2026-04-14 — `famp listen`, durable JSONL inbox with fsync, graceful shutdown
- [x] Phase 3: Conversation CLI (4/4 plans) — completed 2026-04-14 — `famp send/await/inbox/peer add`, task records, TLS TOFU
- [x] Phase 4: MCP Server & Same-Laptop E2E (3/3 plans) — completed 2026-04-15 — `famp mcp` stdio server, E2E-01 automated test, E2E-02 smoke test PASSED
- [x] v0.8.x bridge: Session-bound MCP identity (5/5 plans) — completed 2026-04-26 — `famp_register`/`famp_whoami`, `FAMP_LOCAL_ROOT`-only, pre-registration `not_registered` gating, B-strict variant, two-MCP-server E2E, `await_cmd` FSM advance fix

Archive: [milestones/v0.8-ROADMAP.md](milestones/v0.8-ROADMAP.md) · Requirements: [milestones/v0.8-REQUIREMENTS.md](milestones/v0.8-REQUIREMENTS.md) · Audit: [milestones/v0.8-MILESTONE-AUDIT.md](milestones/v0.8-MILESTONE-AUDIT.md) · Phases: [milestones/v0.8-phases/](milestones/v0.8-phases/)

</details>

<details>
<summary>✅ v0.9 Local-First Bus (Phases 1–4 + close-fix Phase 5) — SHIPPED 2026-05-04</summary>

- [x] Phase 1: `famp-bus` library + audit-log MessageClass (3/3 plans) — completed 2026-04-28 — pure state machine, codec, types, four TDD gates, proptest coverage, atomic v0.5.2 constant bump
- [x] Phase 2: UDS wire + CLI + MV-MCP rewire + hook subcommand (14/14 plans) — completed 2026-04-30 — `famp broker`, top-level CLI surface, MCP rewired to bus (8 tools), `famp-local hook add`, integration tests
- [x] Phase 3: Claude Code integration polish (6/6 plans) — completed 2026-05-03 — `famp install-claude-code`, slash commands, 12-line / 30-second README acceptance gate, Codex parity
- [x] Phase 4: Federation CLI unwire + federation-CI preservation (8/8 plans) — completed 2026-05-04 — top-level CLI removals, `e2e_two_daemons` library-API refactor, `v0.8.1-federation-preserved` tag, migration doc
- [x] Phase 5: Milestone close — CC-07 fix + HOOK-04b path parity + Phase 3 verification backfill (4/4 plans) — completed 2026-05-04 — closes gaps from v0.9-MILESTONE-AUDIT.md (CC-07 BROKEN→satisfied via `famp_peers` projection; HOOK-04b PARTIAL→fully wired via `FAMP_LOCAL_ROOT` parameterization; retroactive `03-VERIFICATION.md`; REQUIREMENTS sweep)

Archive: [milestones/v0.9-ROADMAP.md](milestones/v0.9-ROADMAP.md) · Requirements: [milestones/v0.9-REQUIREMENTS.md](milestones/v0.9-REQUIREMENTS.md) · Audit: [milestones/v0.9-MILESTONE-AUDIT.md](milestones/v0.9-MILESTONE-AUDIT.md)

</details>

<details>
<summary>v0.8 Phase Details (archived)</summary>

See [milestones/v0.8-phases/](milestones/v0.8-phases/) for full plan and summary files.

</details>

<details>
<summary>v0.7 Phase Details (archived)</summary>

### Phase 1: Minimal Signed Envelope

**Goal:** `famp-envelope` encodes, decodes, and signature-verifies every message class the Personal Runtime actually emits, and rejects anything else at the type level.
**Depends on:** v0.6 substrate (`famp-canonical`, `famp-crypto`, `famp-core`)
**Requirements:** ENV-01, ENV-02, ENV-03, ENV-06, ENV-07, ENV-09 (narrowed), ENV-10, ENV-12 (cancel-only), ENV-14, ENV-15 (10 requirements)
**Plans:** 3/3 plans complete

- [x] 01-01-PLAN.md — Crate scaffold + primitive types (class/scope/version/timestamp) + error skeleton + §7.1c vector 0 fixtures on disk
- [x] 01-02-PLAN.md — Sealed BodySchema trait + five shipped body types with ENV-09 and ENV-12 narrowings enforced at the type level
- [x] 01-03-PLAN.md — Type-state UnsignedEnvelope/SignedEnvelope + decode pipeline + AnySignedEnvelope dispatch + vector 0 byte-exact regression + full adversarial + proptest suite

### Phase 2: Minimal Task Lifecycle

**Goal:** The 5-state task FSM (`REQUESTED → COMMITTED → {COMPLETED | FAILED | CANCELLED}`) is compiler-checked and every illegal transition is unreachable, not merely rejected at runtime.
**Depends on:** Phase 1
**Requirements:** FSM-02 (narrowed), FSM-03, FSM-04, FSM-05, FSM-08 (5 requirements)
**Plans:** 3/3 plans complete

- [x] 02-01-PLAN.md — Lift MessageClass + TerminalStatus into famp-core (layering prerequisite for famp-fsm)
- [x] 02-02-PLAN.md — famp-fsm TaskState/TaskFsm engine + deterministic fixture tests (FSM-02, FSM-04, FSM-05)
- [x] 02-03-PLAN.md — FSM-03 consumer stub + FSM-08 proptest Cartesian legality matrix

### Phase 3: MemoryTransport + TOFU Keyring + Same-Process Example

**Goal:** A single developer runs `request → commit → deliver → ack` end-to-end in one binary, signatures verified against a local-file TOFU keyring, and the three adversarial cases fail closed on `MemoryTransport`.
**Depends on:** Phase 2
**Requirements:** TRANS-01, TRANS-02, KEY-01, KEY-02, KEY-03, EX-01, CONF-03, CONF-05, CONF-06, CONF-07 (10 requirements)
**Plans:** 4 plans

- [x] 03-01-PLAN.md — famp-transport: Transport trait + MemoryTransport + test-util feature (TRANS-01, TRANS-02)
- [x] 03-02-PLAN.md — famp-keyring: Keyring + file format + TOFU + --peer flag + round-trip fixture (KEY-01, KEY-02, KEY-03)
- [x] 03-03-PLAN.md — Runtime glue in crates/famp/src/runtime/: RuntimeError + peek_sender + canonical pre-check + recipient cross-check + envelope→FSM adapter
- [x] 03-04-PLAN.md — personal_two_agents example + subprocess test + CONF-05/06/07 adversarial tests + REQUIREMENTS.md KEY-01 D-A1 fix (EX-01, CONF-03, CONF-05, CONF-06, CONF-07)

### Phase 4: Minimal HTTP Transport + Cross-Machine Example

**Goal:** The same signed cycle runs across two processes over HTTPS, bootstrapped from the same TOFU keyring, and the Phase 3 adversarial matrix is extended to `HttpTransport` — no new conformance categories are introduced.
**Depends on:** Phase 3
**Requirements:** TRANS-03, TRANS-04, TRANS-06, TRANS-07, TRANS-09, EX-02, CONF-04 (7 requirements)
**Plans:** 5 plans

- [x] 04-01-PLAN.md — famp-transport-http skeleton: deps + error enums (MiddlewareError, HttpTransportError) + lift peek_sender into famp-envelope
- [x] 04-02-PLAN.md — Server side: build_router + FampSigVerifyLayer (two-phase decode) + RequestBodyLimitLayer + sentinel layering tests (TRANS-04, TRANS-07, TRANS-09 partial)
- [x] 04-03-PLAN.md — Client side: HttpTransport (native AFIT) + tls.rs PEM/rustls helpers + CI no-openssl gate (TRANS-03, TRANS-06)
- [x] 04-04-PLAN.md — cross_machine_two_agents example + fixture certs + subprocess CONF-04 test + same-process safety net (EX-02, CONF-04)
- [x] 04-05-PLAN.md — Promote tests/adversarial.rs to directory module + HTTP adapter + 3 sentinel-checked HTTP rows reusing CONF-07 fixture byte-identically (TRANS-09 complete; CONF-05/06/07 HTTP rows)

</details>

## Future Milestone Sketch (v1.0 Federation Profile)

**Trigger (re-framed 2026-05-09 to two independent ship gates per [`docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`](../docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md)):**

- **Gate A (Gateway):** Ben sustains symmetric cross-machine FAMP use (laptop ↔ home dev server, two equal agents) for ~2 weeks → unlocks `famp-gateway`, reactivates `crates/famp/tests/_deferred_v1/`, tags `v1.0.0`. Activated by Ben's own use case.
- **Gate B (Conformance):** A 2nd implementer commits to interop and exercises the wire format against their own code lineage → unlocks the conformance vector pack at whatever release tag is current. The "Sofer or named equivalent" framing survives only as Gate B's activation condition.

The original 4-week clock has been retired; both gates are event-driven. Conformance vector pack (drafted as `WRAP-V0-5-1-PLAN.md`) ships with Gate B.

**Note (2026-07-23):** Gate A fired; the committed v1.0 Federation Profile — Gateway Core roadmap (Phases 7–10 above) supersedes the rough sketch below for the gateway/liveness/wire/trust/test scope. The mesh-VPN reachability framing referenced in some historical notes below was itself superseded by the "own two machines, direct or VPN Ben already runs, no public relay" scope in `.planning/REQUIREMENTS.md` (defined 2026-07-23). The rough ordering below for post-Gateway-Core federation semantics (Cards, negotiation, delegation, provenance, extensions, conformance) remains a sketch, not committed.

Rough ordering inside v1.0+ (not committed):

- **v1.0 Identity & Cards** — Agent Card format, federation credential, capability declaration, and pluggable trust store, `.well-known` distribution (TRANS-05), SPEC-04..06. Also introduces `famp-gateway` bridging the v0.9 local bus to remote FAMP-over-HTTPS (Gate A) — now committed as Phases 7–10 above, narrowed to own-two-machines scope (no Agent Cards / `.well-known` distribution in this milestone; those remain deferred per REQUIREMENTS.md v2 Requirements).
- **v1.1 Causality & Replay Defense** — freshness windows, bounded replay cache, idempotency-key scoping, supersession, cancellation-safe send path (TRANS-08), SPEC-07/08
- **v1.2 Negotiation & Commitment** — propose/counter-propose, round limits, capability snapshot binding, conversation FSM
- **v1.3 Delegation** — assist / subtask / transfer forms, transfer timeout, delegation ceiling
- **v1.4 Provenance** — graph, canonicalization, redaction, signed terminal reports
- **v1.5 Extensions** — critical/non-critical registry, INV-9 fail-closed
- **v1.6 Adversarial Conformance + Level 2/3 Badges** — full CONF matrix, stateright model checking, conformance-badge automation

## Progress Table

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Canonical JSON Foundations | v0.6 | 3/3 | Complete | 2026-04-13 |
| 2. Crypto Foundations | v0.6 | 3/3 | Complete | 2026-04-13 |
| 3. Core Types & Invariants | v0.6 | 2/2 | Complete | 2026-04-13 |
| 1. Minimal Signed Envelope | v0.7 | 3/3 | Complete | 2026-04-13 |
| 2. Minimal Task Lifecycle | v0.7 | 3/3 | Complete | 2026-04-13 |
| 3. MemoryTransport + TOFU Keyring | v0.7 | 4/4 | Complete | 2026-04-13 |
| 4. Minimal HTTP Transport | v0.7 | 5/5 | Complete | 2026-04-14 |
| 1. Identity & CLI Foundation | v0.8 | 3/3 | Complete | 2026-04-14 |
| 2. Daemon & Inbox | v0.8 | 3/3 | Complete | 2026-04-14 |
| 3. Conversation CLI | v0.8 | 4/4 | Complete | 2026-04-14 |
| 4. MCP Server & Same-Laptop E2E | v0.8 | 3/3 | Complete | 2026-04-15 |
| 1. `famp-bus` library + audit-log MessageClass | v0.9 | 3/3 | Complete | 2026-04-28 |
| 2. UDS wire + CLI + MV-MCP rewire + hook subcommand | v0.9 | 14/14 | Complete | 2026-04-30 |
| 3. Claude Code integration polish | v0.9 | 6/6 | Complete | 2026-05-03 |
| 4. Federation CLI unwire + federation-CI preservation | v0.9 | 8/8 | Complete | 2026-05-04 |
| 5. v0.9 Milestone Close — CC-07 + HOOK-04b + verification backfill | v0.9 | 5/5 | Complete   | 2026-06-04 |
| 1. Broker Diagnosis & Identity Inspection | v0.10 | 4/4 | Complete | 2026-05-10 |
| 2. Task FSM & Message Visibility | v0.10 | 3/3 | Complete | 2026-05-10 |
| 3. Load Verification & Integration Hardening | v0.10 | 3/3 | Complete | 2026-05-11 |
| 4. Broker Lifecycle & Bootstrap Diagnostics | v0.11 | 3/3 | Complete | 2026-06-04 |
| 5. Daemon Service Management & Version Safety | v0.11 | 5/5 | Complete | 2026-06-04 |
| 6. Onboarding & Cross-Platform Docs | v0.11 | 3/3 | Complete | 2026-06-06 |
| 7. Broker-Liveness Fork + Gateway Skeleton | v1.0 | 1/3 | In Progress|  |
| 8. Signed Cross-Host Envelope + Trust Bootstrap | v1.0 | 0/TBD | Not started | - |
| 9. End-to-End Cross-Host Delivery | v1.0 | 0/TBD | Not started | - |
| 10. Test Reactivation + Setup Docs | v1.0 | 0/TBD | Not started | - |

## Backlog

### Phase 999.1: `famp await` crash safety — cursor advance vs flush ordering (BACKLOG)

**Goal:** [Captured for future planning]
**Requirements:** TBD
**Plans:** 0 plans

**Context:** Surfaced 2026-04-24 while wiring a Claude Code Stop hook that blocks on `famp await --timeout 23h`. Open question: if the `famp await` process is SIGKILL'd (or its parent dies) after the inbox cursor has advanced but before stdout is flushed/consumed by the caller, is the entry lost? Verification test: run `famp await` in a subshell, SIGKILL immediately after a peer sends, then check whether `famp inbox list` still shows the entry. If lost, cursor should only advance after successful flush/ack. Low urgency (single-consumer listeners rarely crash mid-flush) but a real correctness concern for the protocol layer.

Plans:

- [ ] TBD (promote with /gsd:review-backlog when ready)

### Phase 999.2: Multi-listener lock semantics — concurrent `famp await` consumers (BACKLOG)

**Goal:** [Captured for future planning]
**Requirements:** TBD
**Plans:** 0 plans

**Context:** Surfaced 2026-04-24 during adversarial review of the Stop hook listener. If two processes (e.g., two Claude Code windows sharing the same cwd + `.famp-listen` sentinel, or just two shells) both call `famp await` against the same `FAMP_HOME`, what happens? Expected: serialize cleanly via `inbox.lock` so exactly one consumer gets each new entry; the other blocks and awaits the next. Feared: cursor race where both processes read the same entry (duplicate delivery) or one deadlocks. Test plan: spawn two concurrent `famp await` processes against the same FAMP_HOME, have a peer send one envelope, verify exactly one consumer receives it and the other continues blocking. Low near-term priority (single-listener is the current usage pattern) but important before encouraging multi-listener workflows.

Plans:

- [ ] TBD (promote with /gsd:review-backlog when ready)

### Phase 999.3: `heartbeat` envelope class — work-in-progress visibility (BACKLOG)

**Goal:** Define and ship a low-bandwidth `heartbeat` envelope class so a long-running worker can periodically signal "still alive, working on `<one-liner>`" without the originator having to poll. Eliminates the failure mode where 8–15 minute silent gaps in a multi-agent task look indistinguishable from a crashed daemon.

**Requirements:** TBD
**Plans:** 0 plans

**Context:** Surfaced 2026-04-25 during the first 3-agent pressure test. Symptom: agent-a starved 21 minutes watching agent-b silently work on a pressure-tested artifact, then the operator intervened thinking it was stuck. Today there is no protocol-level signal between "actively working" and "crashed mid-task." Proposal: new envelope class `heartbeat` carrying `{ task_id, working_on: <≤120 char string>, ts }`; sender emits at most every N minutes (default 5) or on demand from a hypothetical `famp_status` MCP tool; receiver-side, the originator's `famp_await` surfaces "agent-b heartbeat at HH:MM, working on: ..." rather than rendering silence as suspicious. Sized as substrate work because it touches `famp-envelope` (new MessageClass) and `famp-fsm` (heartbeat is non-state-advancing — does not consume a slot in the 5-state FSM, but the inbox surface treats it like a deliver).

Plans:

- [ ] TBD (promote with /gsd:review-backlog when ready)

### Phase 999.4: `user_attention` envelope class — human-in-loop primitive (BACKLOG)

**Goal:** Define and ship a `user_attention` envelope class so a worker can explicitly mark a task as "blocked pending human input" — distinct from `REQUESTED`, `COMMITTED`, or any of the three terminal states. The inbox surface and orchestrator must render this as a first-class human-action signal, not just another deliver.

**Requirements:** TBD
**Plans:** 0 plans

**Context:** Surfaced 2026-04-25 during the same 3-agent pressure test. Symptom: agent-c (a reviewer-role agent on call) said "this needs the operator" during round-2 escalation; agent-b had no FAMP-native primitive to forward the blocked-on-human state to agent-a (the orchestrator) in a way that would surface differently from a normal reply. Workaround used: a prose-tagged deliver, indistinguishable from any other reply. Proposal: new envelope class `user_attention` carrying `{ task_id, reason: <markdown blob explaining what input is needed>, suggested_actions?: Vec<string> }`; receiver-side, `famp_inbox list` and `famp_await` MUST flag these distinctly (e.g., a separate column or icon). Open design question: does this advance the FSM (new state `BLOCKED_HUMAN`?) or is it a non-state-advancing signal layered on COMMITTED? Likely the latter — keeps the 5-state FSM intact and matches the heartbeat (999.3) pattern.

Plans:

- [ ] TBD (promote with /gsd:review-backlog when ready)

### Phase 999.5: Spec-by-path tracking — `~/Workspace/...` paths in messages (BACKLOG, deferred to v1.0)

**Goal:** Track the spec-by-path gap explicitly so it isn't forgotten before v1.0. The gap is already covered structurally by the v1.0 federation gateway design — this entry exists so there is a discoverable link from the pressure-test findings to the federation work, and so v1.0 planning explicitly verifies the gap is closed.

**Requirements:** TBD
**Plans:** 0 plans

**Context:** Surfaced 2026-04-25 during the first 3-agent pressure test. Symptom: agent-b sent absolute filesystem paths (e.g. `~/Workspace/FAMP/...`, `~/Workspace/<other-project>/...`) inside envelope bodies because the protocol has no native way to address a spec/artifact by content-id or by federation-resolvable URL. Today this works only because all three agents are co-resident on the same Mac with the same `$HOME`. The moment any agent runs cross-host, every such reference is dead. v0.9 (local-first bus, in design at `docs/superpowers/specs/2026-04-17-local-first-bus-design.md`) does NOT address this — it's a same-host design. v1.0's federation gateway is the right home for content-addressable refs (or signed-URL refs) because that's the layer where cross-host trust + transport already exists. **Action for v1.0 planning:** when scoping the federation gateway, include an explicit requirement that an envelope can carry a portable artifact reference (sha256-id or signed URL) and the receiver can dereference it without trusting the sender's filesystem. **Status (2026-07-23):** not picked up by the v1.0 Gateway Core roadmap (Phases 7–10) — those phases carry direct filesystem-independent principal addressing (name/principal, not path) but no portable content-addressable artifact reference. Remains open for v1.1+.

Plans:

- [ ] TBD — to be folded into v1.0 federation gateway scope, NOT promoted independently. (Surface during /gsd:new-milestone for v1.0.)

### Phase 999.7: Broker inspect ingress prioritization (BACKLOG)

**Goal:** Prevent saturated inspect RPC traffic from monopolizing the broker's shared ingress queue before the inspect semaphore is reached.
**Requirements:** TBD
**Plans:** 0 plans

**Context:** Captured 2026-05-13 after adversarial review of the `inspect_load_does_not_starve_bus_messages` flake fix. The current mitigation bounds inspect filesystem dispatch and removes unbounded shed-path reply tasks, but all client frames still share `broker_rx`: inspect `Hello`, `Inspect`, and disconnect frames can fill or monopolize ingress before `Out::InspectRequest` reaches the semaphore. Future planning should evaluate splitting inspect ingress from ordinary bus ingress, classifying inspect frames before the shared broker actor queue, or giving ordinary bus traffic priority/budgeted draining so live `Send`/`Inbox` traffic cannot be delayed by saturated inspect connection churn.

Plans:

- [ ] TBD (promote with /gsd:review-backlog when ready)

### Phase 999.8: Audit-log FSM state handling for inspect tasks (BACKLOG)

**Goal:** Teach `famp inspect tasks` and message metadata to derive stable FSM states from v0.9/v0.10 `audit_log` send envelopes, not only canonical `request|commit|deliver|control` classes.
**Requirements:** TBD
**Plans:** 0 plans

**Context:** Captured 2026-05-13 during adversarial review of inspect load-test hardening. `derive_fsm_state` currently maps canonical envelope classes explicitly, but current local bus sends are encoded as `class: "audit_log"` with `body.event` and `body.details.mode`. Mailbox-only task rows and message metadata can therefore surface `UNKNOWN` for valid local bus task traffic. Future work should add explicit audit-log send-mode handling, e.g. `famp.send.new_task` / `mode: new_task` -> `REQUESTED`, `mode: deliver` -> `COMMITTED`, terminal deliver modes -> `COMPLETED|FAILED|CANCELLED`, with focused unit tests for task rows and message rows.

Plans:

- [ ] TBD (promote with /gsd:review-backlog when ready)

---
*Roadmap updated: 2026-06-03 — v0.11 Broker Daemon & Cross-Tool Bootstrap roadmap created. Three phases (4–6) covering 15/15 requirements: Phase 4 (BLC-01, BLC-02, BOOT-01 — broker lifecycle flag + sandbox diagnostics), Phase 5 (DAEMON-01..06, BOOT-02, VER-01, VER-02 — daemon service lifecycle + version safety), Phase 6 (DOC-01..03 — onboarding docs + cross-platform boundary). Phase 5 guardian plist-review gate is a blocking pre-load requirement. Phase dirs: `.planning/phases/04-*`, `05-*`, `06-*`. Prior milestone: v0.10 Inspector & Observability shipped 2026-05-11 (3/3 phases, 10/10 plans, 26/26 requirements). v0.10 Inspector & Observability recut after matt-essentialist + zed-velocity-engineer review. Three-phase structure: Phase 1 (Broker Diagnosis & Identity Inspection — closes orphan-listener incident class end-to-end, 16 reqs), Phase 2 (Task FSM & Message Visibility — I/O-bound handlers + the budget/cancel reqs that finally have something real to enforce against, 9 reqs), Phase 3 (Load Verification & Integration Hardening, 1 req). 26/26 v1 requirements mapped. Original cut (RPC-foundation-with-stub-handlers in Phase 1, all CLI in Phase 2) rejected as yak-shaving — Phase 1 success criteria around budget+cancel were testing synthetic test-only handlers, not the inspector's real work surface. INSP-RPC-02 reworded from runtime property test to compile-time `&BrokerState` signature + workspace dep-graph gate (`just check-inspect-readonly`). Phase numbering reset to Phase 1 per FAMP convention (v0.7/v0.8/v0.9 each reset; v0.10 follows). Independent of v1.0 federation gates (Gate A: Ben symmetric cross-machine; Gate B: 2nd implementer interop) which were unwelded 2026-05-09 per `docs/superpowers/specs/2026-05-09-v1-trigger-unweld-design.md`. v0.9 Local-First Bus shipped 2026-05-04; v0.8 shipped 2026-04-26; v0.7 shipped 2026-04-14; v0.6 + v0.5.1 shipped 2026-04-13.*

*Roadmap updated: 2026-07-23 — v1.0 Federation Profile — Gateway Core roadmap created. Four phases (7–10), continuing sequential numbering from v0.11's Phase 6 (not reset), covering 13/13 v1 requirements from the 2026-07-23 REQUIREMENTS.md (supersedes the 2026-06-08 mesh-VPN Gate A draft). Foundation-first ordering, each phase gating the next: Phase 7 (LIVE-01, LIVE-02, GW-04 — resolves the broker-liveness fork with the Design-A local-proxy recommendation and stands up the `famp-gateway` skeleton; the spine every later phase depends on), Phase 8 (WIRE-01, WIRE-02, TRUST-01, TRUST-02 — signed cross-host envelope + two-machine TOFU key bootstrap, reusing `famp-crypto`/`famp-canonical`/`famp-keyring` without rebuilding them), Phase 9 (GW-01, GW-02, GW-03 — full bidirectional request→commit→deliver→ack cycle across two machines, proving Phases 7+8 compose), Phase 10 (TEST-01, TEST-02, DOC-04 — reactivates the ~27 parked `crates/famp/tests/_deferred_v1/` tests, lands a live two-process E2E in `just ci`, ships the two-machine setup guide). Scope is deliberately narrow: own-two-machines only (direct or Ben-controlled VPN), no public relay, no cross-person trust, no signed directory, no capability/approval plane — all deferred to v1.1/v2.0 per REQUIREMENTS.md v2 Requirements. Gate B (conformance vector pack) stays independent and out of this milestone. Phase dirs: `.planning/phases/07-*` through `10-*` (to be created at plan-phase time).*

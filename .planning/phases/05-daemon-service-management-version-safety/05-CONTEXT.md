# Phase 5: Daemon Service Management & Version Safety - Context

**Gathered:** 2026-06-04
**Status:** Ready for planning

<domain>
## Phase Boundary

Deliver `famp daemon install/uninstall/status/restart` as a cross-platform
service lifecycle (macOS launchd LaunchAgent, Linux systemd `--user` unit) so a
user installs once from a normal shell and FAMP is permanently reachable from
any client — sandboxed or not — without per-session broker babysitting. PLUS a
connect-time version handshake so version skew between a long-lived daemon and
an upgraded client is caught loudly at connect rather than silently misrouted.

Requirements (9): DAEMON-01..06, BOOT-02, VER-01, VER-02.

**Out of scope (locked by ROADMAP):** socket activation (launchd/systemd holds
the socket, fd inheritance) — deferred; the unconditional-`KeepAlive` plist is
the correct interim shape. Spawn-lock for the `bind_exclusive` stale-branch
unlink-race — deferred to its own track (the daemon dissolves the race for
daemon users). Touching protocol-primitive crates — `famp daemon` is CLI-layer
only.

</domain>

<decisions>
## Implementation Decisions

Discussed via a 2-agent panel (matt-essentialist + magnus-fixer) at the user's
request. Both agents converged with **no disagreement** on all three areas.

### A. Version compatibility policy (VER-01)
- **D-01:** Refuse the connection **only** on a `bus_proto` integer mismatch
  (real wire/codec break). A build-version difference with equal `bus_proto`
  is **logged once at handshake, never refused.**
- **D-02:** The refusal error **MUST name the fix the user actually runs:
  `famp daemon restart`** — not "upgrade"/"reinstall". A "version mismatch"
  message that omits `daemon restart` is as useless as no handshake.
- **D-03:** Client build logged at connect; daemon build surfaced via
  `famp daemon status` (intent-preserving — skew stays visible/diagnosable
  when a suspicious user investigates, never a wall).
  *Relaxed 2026-06-04 (plan-check): 'both at connect' would require a
  famp-bus/proto.rs edit (primitive crate, out of phase scope) or a per-connect
  Inspect round-trip (hot-path tax). Intent (skew visible/diagnosable, never a
  wall) preserved via `famp daemon status` using the existing
  `InspectBrokerReply.build_version`. Decision B, matt-essentialist counsel.*
- **Rationale / highest-regret guard:** exact-build-match was named the single
  highest-regret mistake. Because `KeepAlive=true` keeps the daemon long-lived,
  a NEW client meets the OLD daemon on **every** connect after `cargo install`
  until restart — build-strictness would turn every routine upgrade into a wall
  of errors over a wire-identical binary, training users to ignore the signal
  exactly when a real wire break occurs. The client is the party that refuses
  (daemon is long-lived).

### B. Version source of truth (VER-02)
- **D-04:** `bus_proto` (the existing `BUS_PROTO_VERSION: u32` in
  `crates/famp-bus/src/proto.rs:14`, already on the wire in `Hello`/`HelloOk`)
  is the **handshake authority**. VER-01 adds the *enforcement* — `bus_proto`
  is currently sent but never checked/refused. `bus_proto` stays **= 1** in
  this phase (no wire change).
- **D-05:** `BUS_PROTO_VERSION` gets a doc comment: **bump only when the wire
  frame changes, never automatically, never wired to `CARGO_PKG_VERSION`.**
  Without this, the first "tidy-up" couples it to the crate major and rearms
  the every-upgrade-refuses footgun.
- **D-06:** Unify the **human-facing display version** so `famp -V`, the help
  banner, and the build version reported in the handshake all agree on one
  honest number. Current state: `famp -V` → `0.1.0` (workspace default, never
  bumped); banner hardcodes `"FAMP v0.5.1 reference CLI"` (`crates/famp/src/cli/mod.rs:33`).
- **D-07:** The unified display version is **`0.11.0`** (milestone-aligned —
  `-V` should track the thing that gets `git tag`ged). Banner becomes e.g.
  `"FAMP 0.11.0 (spec v0.5.2)"`. This is distinct from `FAMP_SPEC_VERSION`
  (`"0.5.2"`, the federation wire-conformance constant) and from `bus_proto`
  (local-bus wire authority) — three separate axes, do not conflate.

### C. Linux persistence UX (DAEMON-06)
- **D-08:** **Detect-and-instruct.** If `loginctl enable-linger` is not enabled,
  install still succeeds (unit written + started), prints the exact
  `loginctl enable-linger <user>` command, and explains the one consequence
  (broker dies on logout until you run it). **Do NOT run `enable-linger` for
  the user** — it is a per-user system-policy change (processes persist with no
  active session) that some shared/hardened hosts forbid; silently escalating
  it gets the tool banned on managed machines and makes Linux behave
  asymmetrically from macOS (whose locked LaunchAgent does not self-escalate).
- **D-09:** `famp daemon status` **must report linger state**, not just
  unit-active state — otherwise the user runs the instructed command and
  nothing confirms it took; the failure (broker gone after next logout)
  surfaces hours later with no breadcrumb. Status is where the loop closes.
- **Note:** the systemd-ABSENT path is already locked by DAEMON-06's acceptance
  (exit non-zero, point to manual `famp broker --no-idle-exit` fallback). D-08
  is only the systemd-present-but-linger-off UX.

### Claude's Discretion (researcher/planner owns these)
- Exact `launchctl` invocation for `daemon restart` binary-pickup
  (`kickstart -k` vs `bootout`+`bootstrap`) — implementation detail; the
  user-facing guarantee ("running broker is the new binary after restart") is
  locked by DAEMON-05.
- Wire placement of the version exchange — extend the existing `Hello`/`HelloOk`
  frame (natural; `bus_proto` already lives there) vs a new frame. Researcher's
  call.
- BOOT-02 sandbox-refusal: reuse the existing Phase 4 EPERM-on-bind probe
  (`SandboxEperm` in `crates/famp/src/bus_client/spawn.rs`).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase scope & requirements
- `.planning/ROADMAP.md` § "Phase 5: Daemon Service Management & Version Safety"
  — goal, 5 success criteria, constraint notes (deferrals).
- `.planning/REQUIREMENTS.md` — DAEMON-01..06, BOOT-02, VER-01, VER-02 with
  acceptance criteria (lines ~21–36, mapping table ~62–71).

### BLOCKING pre-load gate (do not skip)
- **DAEMON-02 guardian plist review is a BLOCKING requirement.** The literal
  macOS plist XML MUST be reviewed and approved by **guardian** (the
  `mac-guardian` skill) against its review checklist **before the service is
  first loaded** (`launchctl load`). The planner MUST sequence guardian sign-off
  ahead of any `load`/first-run step. Locked plist shape: `RunAtLoad=true`,
  `KeepAlive=true` (unconditional), `ProcessType=Background`,
  `StandardOutPath`/`StandardErrorPath` → `~/.famp/broker.log`, **no**
  `EnvironmentVariables` key, `ProgramArguments` = `~/.cargo/bin/famp broker
  --no-idle-exit`.

### Deployment / convention
- `CLAUDE.md` § Conventions — run `just install` after any plist-shape or
  `famp daemon` subcommand change; the installed `~/.cargo/bin/famp` is the
  deployment target, not `target/release/famp`.

No external ADRs/specs beyond the above — the v1-trigger-unweld spec and the
FAMP-v0.5.x spec govern federation, not this local-daemon phase.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/famp-bus/src/proto.rs:14` — `BUS_PROTO_VERSION: u32 = 1` already
  exists and is already on the wire (`Hello { bus_proto }` / `HelloOk { bus_proto }`).
  VER-01 = add the **enforcement** (mismatch → loud refuse); `HelloErr { kind, .. }`
  reply variant already exists to carry it. VER-02's "separate protocol constant"
  is therefore already the de-facto architecture, not new machinery.
- `crates/famp-inspect-proto/src/lib.rs:41` — `InspectBrokerReply.build_version`
  (`pub build_version: String`, doc: "CARGO_PKG_VERSION of the answering broker
  process") already carries the **daemon build** on the wire. `famp daemon
  status` performs the `Inspect{Broker}` round-trip and surfaces it — this is the
  D-03 daemon-build surface (Decision B). `famp-inspect-proto` is NOT a forbidden
  primitive crate, so no primitive-crate edit is needed.
- `crates/famp/src/bus_client/spawn.rs` — `SandboxEperm` EPERM-on-bind probe
  from Phase 4; BOOT-02 (`daemon install` sandbox refusal) reuses it.
- `crates/famp/src/bus_client/mod.rs:104` — client sends `bus_proto:
  BUS_PROTO_VERSION` in its `Hello`; the refusal check lands on this path.
- `--no-idle-exit` broker flag (Phase 4) — the service's `ProgramArguments`
  launches the broker with it.

### Established Patterns
- `crates/famp/src/cli/mod.rs:33` — `#[command(name = "famp", version, about =
  "FAMP v0.5.1 reference CLI")]`. The banner string is hardcoded and separate
  from clap's `version` (which reads `CARGO_PKG_VERSION` = `0.1.0`). D-06/D-07
  reconcile both to `0.11.0`.
- Workspace version: `Cargo.toml:24` `version = "0.1.0"` under
  `[workspace.package]`; crate inherits via `version.workspace = true`. The
  bump to `0.11.0` happens here.
- `famp daemon` subcommand lands in `crates/famp/src/cli/` — CLI-layer; does
  NOT touch protocol-primitive crates (`famp-bus`, `famp-canonical`,
  `famp-crypto`).

### Integration Points
- Stage-aware spawn/connect error surface (Phase 4, commits `4da30a3`/`ebbf1d3`)
  — VER-01's skew error should surface consistently across both client surfaces
  (CLI `famp register` and MCP `famp_register`), matching that pattern.

### Planning note (from Matt)
- **Before bumping `-V` 0.1.0 → 0.11.0, grep for anything asserting on the
  current version string** (status skills, tests, `famp-doctor`) so the honest
  fix doesn't silently break a hardcoded `0.1` or `0.5` assertion.

</code_context>

<specifics>
## Specific Ideas

- User delegated the gray-area discussion to a named 2-agent panel (Matt =
  matt-essentialist, "Manus"/Magnus = magnus-fixer). Both produced decisive,
  converging recommendations; user accepted all three and chose `0.11.0` for the
  display version.

</specifics>

<deferred>
## Deferred Ideas

- **Two-brokers-bind-same-socket steal race (raised by magnus-fixer).** Nothing
  in Phase 5 stops a user/stale-session/test from running `famp broker` by hand
  while the daemon's broker is live, or two installs racing — a second binder
  either `EADDRINUSE`-crashes (loud, fine) or unlink-and-rebinds (silently
  steals `~/.famp/bus.sock`, orphans clients on the old broker → exactly the
  silent misrouting VER-01 exists to prevent). **NOT added to Phase 5 scope:**
  ROADMAP already defers the `bind_exclusive` spawn-lock to its own track, and
  the daemon dissolves the race for daemon users (the connect-first short-circuit
  in `spawn.rs:50` means a daemon-owned live broker is never re-spawned).
  Candidate for the dedicated spawn-lock track: flock'd pidfile / `O_EXCL`
  lockfile bind, refuse to steal a live socket, handshake version-log doubles as
  a "which broker am I talking to" identity check.

None of the above expands this phase — recorded so the spawn-lock track has the
full failure analysis when promoted.

</deferred>

---

*Phase: 5-Daemon Service Management & Version Safety*
*Context gathered: 2026-06-04*

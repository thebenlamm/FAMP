# Requirements: FAMP — v0.11 Broker Daemon & Cross-Tool Bootstrap

**Defined:** 2026-06-03
**Core Value:** A byte-exact, signature-verifiable FAMP substrate a single developer can use today, and two independent parties can interop against later. v0.11 makes that substrate *reliably reachable* — a broker is always present for any local client, sandboxed or not.

**Milestone goal:** Restore the guaranteed broker-presence that commit `56b2293` (correctly) removed, the principled way — a service-managed daemon — so a fresh clone of FAMP works for **both** Claude Code and Codex with no per-user broker babysitting.

**Root cause (verified):** Client bootstrap is `spawn_broker_if_absent` (`crates/famp/src/bus_client/spawn.rs:44`): connect-first, else fork a child that `bind()`s `~/.famp/bus.sock`. Codex runs in a seatbelt sandbox that blocks `bind()` (EPERM, os error 1); the forked child inherits the sandbox and fails, swallowed by `let _ =` at `spawn.rs:92`, surfacing only as "broker unreachable". This was masked until `56b2293` (2026-05-12) made brokers self-terminate when idle — before that, leaked brokers were always present, so the sandboxed client always found one to connect to.

## v1 Requirements

Requirements for the v0.11 release. Each maps to a roadmap phase below.

### Broker Lifecycle — `famp broker --no-idle-exit`

- [x] **BLC-01**: `famp broker --no-idle-exit` runs the broker with the 300s idle self-terminate (Arm 4, `crates/famp/src/cli/broker/mod.rs`) disabled — a broker started with the flag and zero connected clients is still alive after the idle window elapses. Acceptance: a test (tokio time-pause or equivalent) advances past `IDLE_TIMEOUT` with the flag set and the broker has not exited.
- [x] **BLC-02**: `famp broker` *without* the flag retains the current 300s idle-exit behavior unchanged (regression guard for the `56b2293` orphan-leak fix). Acceptance: existing BROKER-04/04b idle-exit tests still pass; default-path behavior is byte-for-byte the prior behavior.

### Bootstrap & Sandbox Diagnostics

- [ ] **BOOT-01**: When broker spawn fails because `bind()` returns EPERM (sandboxed shell), the client surfaces an actionable error naming the cause and the remedy — e.g. "can't create a broker inside a sandbox; run `famp daemon install` from a normal shell" — instead of the generic "broker unreachable". `spawn.rs:92` no longer swallows the EPERM. Acceptance: a test injecting/simulating EPERM-on-bind yields the actionable message and distinguishes EPERM from other spawn failures; extends the connect/spawn-stage disambiguation in commits `4da30a3`/`ebbf1d3`.
- [ ] **BOOT-02**: `famp daemon install` refuses to run when invoked inside a sandbox (the same condition that would make the broker's `bind()` fail), exiting non-zero with guidance, rather than writing a service that can never bind. Acceptance: the sandbox-detected path exits 1 with an explanation; the non-sandboxed path proceeds.

### Daemon Service Management — `famp daemon …`

- [ ] **DAEMON-01**: `famp daemon install` writes and loads a platform service that keeps exactly one broker running — a user-level launchd LaunchAgent on macOS, a systemd `--user` unit on Linux — and is idempotent (re-running does not create duplicates or error). Acceptance: after install, `famp inspect broker` reports `HEALTHY`; re-running install leaves exactly one service and one broker.
- [ ] **DAEMON-02**: The generated macOS plist matches the guardian-reviewed shape exactly: `RunAtLoad=true`, `KeepAlive=true` (unconditional), `ProcessType=Background`, `StandardOutPath`/`StandardErrorPath` → `~/.famp/broker.log`, **no** `EnvironmentVariables` key, and `ProgramArguments` invoking the broker with `--no-idle-exit`. Acceptance: the generated plist contains exactly these keys/values, carries no secrets, and is approved by guardian against its review checklist before first load.
- [ ] **DAEMON-03**: `famp daemon status` reports three distinguishable states — not-installed, installed-but-broker-down, running — including the broker pid and socket path when running; exits 0 when running and non-zero otherwise. Acceptance: each of the three states produces its distinct output and exit code.
- [ ] **DAEMON-04**: `famp daemon uninstall` unloads and removes the service file, leaving no orphaned service registration; idempotent. Acceptance: after uninstall the service is absent from `launchctl`/`systemctl --user` listings; re-running uninstall is a no-op success (exit 0).
- [ ] **DAEMON-05**: `famp daemon restart` reloads the service so a replaced on-disk binary (e.g. after `cargo install`/`brew upgrade`) is picked up. Acceptance: after replacing the binary and running restart, the running broker is the new binary (verifiable via version handshake / `famp daemon status`).
- [ ] **DAEMON-06**: On Linux, when systemd `--user` (or `loginctl enable-linger`) is unavailable, `famp daemon install` fails with a clear message pointing to the documented manual fallback rather than producing a silent half-install. Acceptance: systemd-present path installs and enables the unit; systemd-absent path exits non-zero with actionable guidance.

### Version Safety

- [ ] **VER-01**: Client and broker exchange a protocol/build version at connect; a client whose protocol version is incompatible with the running (long-lived) daemon receives a loud, actionable version-skew error and refuses to proceed silently. Acceptance: a simulated skew yields the error and a non-zero exit; matching versions connect normally.
- [ ] **VER-02**: `famp -V` reports a version consistent with the protocol/banner — the current `0.1.0` crate-version vs `0.5.x` banner discrepancy is reconciled to a single source of truth so version reporting is trustworthy for skew diagnosis. Acceptance: `famp -V`, the help banner, and the handshake version agree.

### Onboarding & Documentation

- [ ] **DOC-01**: The README contains a one-command quickstart — `famp daemon install` once, then both Claude Code and Codex connect with no further broker setup. Acceptance: the quickstart is present; a fresh-clone walkthrough on macOS succeeds end-to-end (install → register from Claude → register from Codex).
- [ ] **DOC-02**: The README documents a zero-setup bridge usable without install — run `famp broker --no-idle-exit` in one unsandboxed terminal — for users who cannot or will not install a service. Acceptance: the bridge instructions are present and accurate against actual behavior.
- [ ] **DOC-03**: The README names the cross-platform support boundary explicitly — macOS launchd + Linux systemd `--user` supported by the installer; minimal distros / containers / WSL / headless-without-linger called out as not covered by the installer, with the manual `famp broker --no-idle-exit` fallback. Acceptance: the limitation section exists and matches actual installer behavior (no "works for both" claim that overruns what the installer delivers).

## Future Requirements (deferred)

- **Socket activation** — launchd/systemd hold the listening socket and start the broker on first connect (zero idle residency, race fully closed). Deferred: needs fd-inheritance the broker binary doesn't support yet. The unconditional-KeepAlive daemon is the interim shape.
- **Spawn-lock for `bind_exclusive`'s stale-socket branch** — closes the cold-start unlink-race where two cold clients both unlink+bind and produce competing brokers. Deferred to its own track: the daemon dissolves this race for daemon users; the lock is independent hardening for the auto-spawn path that Claude Code still uses.
- **Windows service** — not in the "works for both Claude and Codex" scope today.

## Out of Scope (permanent for v0.11)

- **Reverting `56b2293`** — the orphan-broker leak it fixed was real (82 orphans / 4 days). Broker mortality stays; the daemon supplies presence without leaking.
- **System-level / root daemon** — user-level service only; no `sudo`, no `/Library/LaunchDaemons`, no system systemd unit.
- **Network exposure** — the broker stays UDS-only and local-trust; the daemon does not open a network listener. (Federation transport is the gated v1.0 milestone.)

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| BLC-01 | Phase 4 | Complete |
| BLC-02 | Phase 4 | Complete |
| BOOT-01 | Phase 4 | Pending |
| BOOT-02 | Phase 5 | Pending |
| DAEMON-01 | Phase 5 | Pending |
| DAEMON-02 | Phase 5 | Pending |
| DAEMON-03 | Phase 5 | Pending |
| DAEMON-04 | Phase 5 | Pending |
| DAEMON-05 | Phase 5 | Pending |
| DAEMON-06 | Phase 5 | Pending |
| VER-01 | Phase 5 | Pending |
| VER-02 | Phase 5 | Pending |
| DOC-01 | Phase 6 | Pending |
| DOC-02 | Phase 6 | Pending |
| DOC-03 | Phase 6 | Pending |

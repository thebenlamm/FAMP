# Phase 5: Daemon Service Management & Version Safety — Research

**Researched:** 2026-06-04
**Domain:** macOS launchd LaunchAgent, Linux systemd --user, Rust CLI service lifecycle, bus protocol version handshake
**Confidence:** HIGH (macOS/broker — verified against live system + source); MEDIUM (Linux systemd — verified via ArchWiki + Ubuntu man pages, not runnable on macOS host)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**A. Version compatibility policy (VER-01)**
- D-01: Refuse the connection **only** on a `bus_proto` integer mismatch. Build-version difference with equal `bus_proto` is **logged once at handshake, never refused.**
- D-02: The refusal error **MUST name `famp daemon restart`** — not "upgrade"/"reinstall".
- D-03: The handshake logs both versions (daemon build / client build) at connect so a non-fatal skew is visible.

**B. Version source of truth (VER-02)**
- D-04: `BUS_PROTO_VERSION: u32 = 1` in `crates/famp-bus/src/proto.rs:14` is the handshake authority.
- D-05: `BUS_PROTO_VERSION` gets a doc comment: bump only when the wire frame changes, never automatically, never wired to `CARGO_PKG_VERSION`.
- D-06: Unify human-facing display version so `famp -V`, help banner, and handshake build version agree.
- D-07: Unified display version is **`0.11.0`**.

**C. Linux persistence UX (DAEMON-06)**
- D-08: Detect-and-instruct linger. Install succeeds, prints exact `loginctl enable-linger <user>` command, explains consequence. Do NOT run it for the user.
- D-09: `famp daemon status` MUST report linger state, not just unit-active state.

**Out of scope (locked):** Socket activation (launchd/systemd holds the socket), spawn-lock for `bind_exclusive` stale-branch race.

### Claude's Discretion

- Exact `launchctl` invocation for `daemon restart` binary pickup (`kickstart -k` vs `bootout`+`bootstrap`).
- Wire placement of the version exchange — extend existing `Hello`/`HelloOk` frame vs new frame.
- BOOT-02 sandbox-refusal: reuse existing Phase 4 `SandboxEperm` probe.

### Deferred Ideas (OUT OF SCOPE)

- Two-brokers-bind-same-socket steal race / `bind_exclusive` spawn-lock.
- Socket activation (fd-inheritance not implemented).
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DAEMON-01 | `famp daemon install` writes and loads a platform service, idempotent | launchd bootstrap + bootout guard; systemd enable --now |
| DAEMON-02 | Generated macOS plist matches guardian-reviewed shape exactly | Plist XML verified against launchd.plist(5) man page |
| DAEMON-03 | `famp daemon status` reports three distinct states | File-existence + famp inspect broker approach |
| DAEMON-04 | `famp daemon uninstall` idempotent, no orphan registration | bootout + tolerate-absent-error pattern |
| DAEMON-05 | `famp daemon restart` picks up replaced on-disk binary | `launchctl kickstart -k` verified |
| DAEMON-06 | Linux: systemd-absent path exits non-zero; systemd-present with detect-and-instruct linger | loginctl show-user --property=Linger approach |
| BOOT-02 | `famp daemon install` refuses inside sandbox | Reuse `SandboxEperm` probe from `spawn.rs` |
| VER-01 | Client refuses on bus_proto mismatch, logs skew | Broker already rejects w/ HelloErr{BrokerProtoMismatch}; client translates to D-02 message naming `famp daemon restart` |
| VER-02 | `famp -V`, banner, handshake version agree | Workspace Cargo.toml:24 bump to 0.11.0 |
</phase_requirements>

---

## Summary

Phase 5 delivers `famp daemon install/uninstall/status/restart` as a cross-platform service lifecycle subcommand, plus connect-time version handshake enforcement and display-version unification. The implementation is CLI-layer only (`crates/famp/src/cli/`) — no primitive-crate changes.

The macOS path is straightforward: a single plist file at `~/Library/LaunchAgents/com.famp.broker.plist`, loaded via `launchctl bootstrap gui/$UID`, restarted via `launchctl kickstart -k gui/$UID/com.famp.broker`. The Linux path is identical in shape (`~/.config/systemd/user/famp-broker.service`, `systemctl --user enable --now`) with an additional detect-and-instruct layer for `loginctl enable-linger`.

The version handshake is more surgical than it appears: **the broker already rejects `bus_proto` mismatches** (verified in `crates/famp-bus/src/broker/handle.rs:185`) via `HelloErr { BrokerProtoMismatch }`. The client today lumps all `HelloErr` variants together into `HelloFailed` (verified in `crates/famp/src/bus_client/mod.rs:110`). VER-01 requires only distinguishing `BrokerProtoMismatch` from other `HelloErr` kinds and returning a new error whose Display names `famp daemon restart` — no broker change, no new wire frame, no primitive-crate edit. D-03's build-skew log is achievable using the broker's existing `BrokerCtx.build_version` returned by `famp inspect broker` — no `HelloOk` wire extension needed.

**Primary recommendation:** Implement as four waves: (1) version bump + banner fix, (2) macOS plist + launchctl subcommands, (3) VER-01 client-side enforcement + D-03 log, (4) Linux systemd path + BOOT-02 sandbox refusal.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Service lifecycle (install/uninstall/restart) | CLI layer (`crates/famp/src/cli/daemon/`) | OS platform (launchd/systemd) | Writes files and invokes OS APIs; CLI orchestrates |
| Daemon status detection | CLI layer + famp inspect broker | OS (launchctl/systemctl) | OS answers "registered?"; inspect broker answers "running + healthy?" |
| Bus proto version enforcement | CLI layer (`bus_client/mod.rs`) | Broker (`famp-bus` — already enforces) | Client is the refusing party (D-01); broker already rejects mismatch |
| Display version unification | Workspace `Cargo.toml` | `crates/famp/src/cli/mod.rs` banner | Single workspace.package.version drives CARGO_PKG_VERSION and clap version |
| Sandbox detection at install time | CLI layer | Phase 4 `spawn.rs:SandboxEperm` | Reuse existing probe — no new detection logic |

---

## Standard Stack

### Core (no new dependencies needed for this phase)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `std::process::Command` | stdlib | `launchctl`/`systemctl` invocation | Sync shell-out; correct for install-time one-shots |
| `std::fs` | stdlib | Plist/unit file write, directory creation | Sufficient for file write + chmod |
| `famp-bus` | workspace | `BUS_PROTO_VERSION`, `BusReply::HelloOk` | Already in use; no new dep |
| `thiserror` | workspace | `DaemonError` enum | Already in use in all CLI modules |

### Supporting (already workspace dependencies)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `nix` | workspace | `unistd::getuid()` for `gui/$UID` target construction | macOS-only arm; cfg-gated |
| `serde_json` | workspace | Structured status output (`--json`) | If status adds JSON flag |

**No new Cargo dependencies are required for this phase.** [VERIFIED: reading Cargo.toml and phase scope]

---

## Architecture Patterns

### System Architecture Diagram

```
famp daemon install
        │
        ├── [macOS] ──► write ~/Library/LaunchAgents/com.famp.broker.plist
        │                       │ (absolute paths, no tilde)
        │               launchctl bootstrap gui/$UID <plist-path>
        │               (guards: check plist exists before bootstrap;
        │                tolerate "already loaded" error for idempotency)
        │
        └── [Linux] ──► write ~/.config/systemd/user/famp-broker.service
                        systemctl --user daemon-reload
                        systemctl --user enable --now famp-broker.service
                        detect linger via loginctl show-user $USER --property=Linger
                        if Linger=no → print instruction, do NOT run enable-linger

famp daemon status
        │
        ├── platform check: plist/unit file exists? → "not-installed" if absent (exit 1)
        ├── OS liveness: launchctl print gui/$UID/com.famp.broker → registered? (exit code)
        ├── broker liveness: famp inspect broker logic (connect-handshake) → running + pid
        └── [Linux] loginctl show-user $USER --property=Linger → linger state in output

famp daemon restart
        │
        └── [macOS] launchctl kickstart -k gui/$UID/com.famp.broker
            [Linux] systemctl --user restart famp-broker.service

famp daemon uninstall
        │
        ├── [macOS] launchctl bootout gui/$UID <plist-path> (tolerate "not loaded" error)
        │           rm -f ~/Library/LaunchAgents/com.famp.broker.plist
        └── [Linux] systemctl --user disable --now famp-broker.service (tolerate not-found)
                    rm -f ~/.config/systemd/user/famp-broker.service

BusClient::connect (VER-01 enforcement)
        │
        Hello { bus_proto: BUS_PROTO_VERSION, client: "famp-cli/0.11.0", .. }
        │
        ├── HelloOk { bus_proto } ──► protos match (broker only emits HelloOk on match)
        │       → connected. D-03: log "client=0.11.0; daemon proto=1" at INFO level.
        │
        └── HelloErr { BrokerProtoMismatch, .. }
                → THIS is the real skew path (old long-lived daemon, new client)
                → translate broker's refusal into D-02 actionable error:
                  "protocol mismatch (broker proto=Y, client proto=X);
                   run `famp daemon restart` to pick up the new binary"
                → BusClientError::ProtocolMismatch (distinct from NotRegistered)
```

### Recommended Project Structure

```
crates/famp/src/cli/
├── daemon/              # NEW — famp daemon subcommand
│   ├── mod.rs           # DaemonArgs, DaemonCommands dispatch, run()
│   ├── install.rs       # install logic: sandbox check, plist/unit write, load
│   ├── uninstall.rs     # uninstall logic: bootout/disable, file removal
│   ├── status.rs        # status logic: file check + launchctl/inspect broker
│   └── restart.rs       # restart logic: kickstart -k / systemctl restart
├── mod.rs               # add Commands::Daemon(daemon::DaemonArgs)
└── broker/mod.rs        # BrokerArgs socket default already correct
```

### Pattern 1: Idempotent launchctl Install/Uninstall

The `launchctl bootstrap` command errors (exit 37 / "service already registered") if the service is already loaded. The `launchctl bootout` command errors if the service is not loaded. Both must be handled for idempotency.

```rust
// Source: verified against launchctl(1) man page + live macOS system
// Install: guard with plist file check first
fn install_macos(plist_path: &Path, uid: u32) -> Result<(), DaemonError> {
    if plist_path.exists() {
        // Already installed — try bootstrap, tolerate "already registered"
    }
    write_plist(plist_path)?;
    let status = Command::new("launchctl")
        .args(["bootstrap", &format!("gui/{uid}"), plist_path.to_str().unwrap()])
        .status()?;
    if !status.success() {
        let code = status.code().unwrap_or(-1);
        if code == 37 {
            return Ok(()); // already loaded — idempotent
        }
        return Err(DaemonError::LaunchctlFailed(code));
    }
    Ok(())
}

// Uninstall: tolerate bootout failure when not loaded
fn uninstall_macos(plist_path: &Path, uid: u32) -> Result<(), DaemonError> {
    let _ = Command::new("launchctl")
        .args(["bootout", &format!("gui/{uid}"), plist_path.to_str().unwrap()])
        .status(); // tolerate failure — service may not be loaded
    if plist_path.exists() {
        std::fs::remove_file(plist_path)?;
    }
    Ok(())
}
```

### Pattern 2: DAEMON-03 Status Detection (three-state)

Do NOT parse `launchctl print` free-text — its man page says "NOT API in any sense at all. Do NOT rely on the structure or information emitted." [VERIFIED: launchd.plist(5) man page]

Use two independent probes:
1. Plist file existence → "not installed"
2. `launchctl print gui/$UID/<label>` exit code (0 = registered, 113 = not found) → "installed but broker not running" vs "registered"
3. Reuse `famp inspect broker` connect-handshake logic for actual liveness + PID + socket

```rust
// Source: verified exit codes on macOS Sequoia (launchctl 7.0.0)
// exit 0   = service registered (may or may not be running)
// exit 113 = "Could not find service" = not registered
fn launchctl_is_registered(label: &str, uid: u32) -> bool {
    Command::new("launchctl")
        .args(["print", &format!("gui/{uid}/{label}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
```

The three states map as:
- `plist_path.exists() == false` → **not-installed** (exit 1)
- `launchctl_is_registered() == true` but `inspect_broker()` → not HEALTHY → **installed-but-down** (exit 2)
- `launchctl_is_registered() == true` AND `inspect_broker()` → HEALTHY → **running** (print pid + socket, exit 0)

### Pattern 3: Binary-Pickup Restart (DAEMON-05)

`launchctl kickstart -k gui/$UID/<label>` kills the running instance and immediately starts a new one from the on-disk binary. This is the correct invocation for "pick up a replaced binary." [VERIFIED: launchctl(1) man page, `-k` flag]

`bootout`+`bootstrap` is the correct pattern only when the **plist content** has changed (the bootout/bootstrap pair re-reads the plist). For binary-only upgrades, `kickstart -k` is faster and cleaner.

```bash
# Restart (binary pickup):
launchctl kickstart -k gui/$UID/com.famp.broker

# Only use bootout+bootstrap when plist shape changes:
launchctl bootout gui/$UID ~/Library/LaunchAgents/com.famp.broker.plist
launchctl bootstrap gui/$UID ~/Library/LaunchAgents/com.famp.broker.plist
```

### Pattern 4: VER-01 Client-Side Enforcement

**Critical design insight (verified from source):** The broker (`handle.rs:185`) ALREADY rejects `bus_proto != BUS_PROTO_VERSION` with `HelloErr { BrokerProtoMismatch }`. `HelloOk` is ONLY emitted when protos match, and it always carries `bus_proto: BUS_PROTO_VERSION` (the broker's own constant). Therefore:

- `HelloOk.bus_proto` will always equal the client's `BUS_PROTO_VERSION` when a HelloOk arrives — a mismatch check on HelloOk is dead code against any conformant broker.
- The **real skew path** is `HelloErr { BrokerProtoMismatch }` — this is what fires when a new client (proto 2) hits an old long-lived daemon (proto 1).
- VER-01's job is to **translate** that `HelloErr` into the D-02-compliant actionable message. Currently `mod.rs:110` lumps all `HelloErr` variants together into `HelloFailed` — it does not distinguish `BrokerProtoMismatch` from `NotRegistered`.

```rust
// Source: crates/famp/src/bus_client/mod.rs:109 — current code
// VER-01 change: distinguish BrokerProtoMismatch from other HelloErr variants
match client.send_recv(hello).await? {
    BusReply::HelloOk { .. } => {
        // D-03: log client build version at INFO level.
        // Daemon proto is known (= BUS_PROTO_VERSION, since we got HelloOk).
        // Daemon BUILD version: available from `famp daemon status`/`inspect broker`.
        tracing::info!(
            client_build = env!("CARGO_PKG_VERSION"),
            bus_proto = BUS_PROTO_VERSION,
            "connected to broker"
        );
        Ok(client)
    }
    BusReply::HelloErr { kind: BusErrorKind::BrokerProtoMismatch, message } => {
        // THIS is the real skew path (D-02: MUST name famp daemon restart).
        // The broker's message already includes "expected bus_proto=Y" — extract it.
        Err(BusClientError::ProtocolMismatch { broker_message: message })
    }
    BusReply::HelloErr { kind, message } | BusReply::Err { kind, message } => {
        // Other failures (NotRegistered for bind_as proxy, etc.)
        Err(BusClientError::HelloFailed { kind, message })
    }
    other => Err(BusClientError::UnexpectedReply(format!("{other:?}"))),
}
```

New error variant (D-02: MUST name `famp daemon restart`):
```rust
#[error(
    "bus protocol mismatch ({broker_message});      run `famp daemon restart` to pick up the new binary"
)]
ProtocolMismatch { broker_message: String },
```

The `proto_mismatch_names_restart` test MUST drive the `HelloErr { BrokerProtoMismatch }` path — not a mismatched HelloOk — to cover the real failure mode.

**D-03 build-skew log:** The daemon's build version is available from `famp inspect broker` (`BrokerCtx.build_version`, already in `InspectBrokerReply.build_version`). No `HelloOk` wire extension needed — no primitive-crate edit. The skew log at connect can emit the client build; daemon build surfaces via `famp daemon status` (which calls inspect broker). [ASSUMED A3 — D-03 says "logs both versions at connect"; if strictly at-connect, an extra Inspect round-trip after HelloOk would be needed]

### Pattern 5: Linux systemd --user Unit

```ini
# ~/.config/systemd/user/famp-broker.service
# Source: ArchWiki systemd/User, Ubuntu loginctl man page
[Unit]
Description=FAMP Local Bus Broker
After=default.target

[Service]
ExecStart=/home/<user>/.cargo/bin/famp broker --no-idle-exit
Restart=always
StandardOutput=append:/home/<user>/.famp/broker.log
StandardError=append:/home/<user>/.famp/broker.log

[Install]
WantedBy=default.target
```

Paths must be absolute (no `~` expansion) — resolved from `$HOME` at install time. [VERIFIED: same constraint as macOS launchd]

```bash
# Install:
mkdir -p ~/.config/systemd/user/
# write unit file
systemctl --user daemon-reload
systemctl --user enable --now famp-broker.service

# Check linger:
loginctl show-user "$USER" --property=Linger
# Output: "Linger=yes" or "Linger=no"

# Instruct (D-08 — print but do NOT run):
echo "Run: loginctl enable-linger $USER"

# Status (D-09 — report linger state):
loginctl show-user "$USER" --property=Linger  # include in daemon status output
```

### Anti-Patterns to Avoid

- **Parsing `launchctl print` output**: explicitly marked "NOT API" in launchctl(1). Use exit codes + file existence instead.
- **Using tilde (~) in plist/unit file content**: launchd does NOT expand `~`. Generated files must contain `/Users/<username>/...` (resolved from `$HOME` at install time). [VERIFIED: all real LaunchAgent plists on this machine use absolute paths]
- **Using `launchctl load`/`unload`**: deprecated in favor of `bootstrap`/`bootout` + `enable`/`disable`. [VERIFIED: launchctl(1) man page]
- **Silently running `loginctl enable-linger`**: forbidden by D-08.
- **Bumping `BUS_PROTO_VERSION` with the version number**: D-05 explicitly prohibits this. Doc comment must say "bump only when wire frame changes."
- **Coupling `BUS_PROTO_VERSION` to `CARGO_PKG_VERSION`**: three axes stay separate (spec version = FAMP_SPEC_VERSION, display/milestone = CARGO_PKG_VERSION, local bus wire = BUS_PROTO_VERSION).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Broker liveness check | Custom PID-file probe | `famp inspect broker` connect-handshake | Already implemented (Phase 1); PID file was explicitly rejected as architecture (bind()-IS-the-lock) |
| Service registration state | Parse launchctl print output | File-existence + launchctl exit code | launchctl print is "NOT API" per man page |
| OS detection | Complex heuristics | `#[cfg(target_os = "macos")]` / `#[cfg(target_os = "linux")]` | Compile-time; correct for the two supported platforms |
| Binary path resolution | Walk PATH | `std::env::current_exe()` at install time → absolute path | Gives the exact installed binary path |

**Key insight:** The hardest problem (broker liveness) is already solved by `famp inspect broker`. DAEMON-03's three states reduce to file-existence + launchctl-exit-code + inspect-broker.

---

## Common Pitfalls

### Pitfall 1: Tilde in Plist/Unit File
**What goes wrong:** Plist written with `~/.cargo/bin/famp`; launchd cannot execute it.
**Why it happens:** `~` is shell shorthand; launchd does not invoke a shell to expand it.
**How to avoid:** At install time, resolve `$HOME` in Rust (`std::env::var("HOME")`) and write `/Users/<username>/...` in the plist. Add to guardian review checklist.
**Warning signs:** Service loads but broker never starts; `launchctl print` shows a nonzero last-exit-status.

### Pitfall 2: launchctl bootstrap "Already Loaded" Error
**What goes wrong:** Re-running `famp daemon install` exits non-zero because `bootstrap` returns exit 37 "service already registered."
**Why it happens:** `bootstrap` is not idempotent by default.
**How to avoid:** Check plist file existence first; tolerate exit 37 from bootstrap. Document idempotency in acceptance test.
**Warning signs:** Second `famp daemon install` call fails with exit code 37.

### Pitfall 3: launchctl bootout "Not Loaded" Error
**What goes wrong:** `famp daemon uninstall` on a not-loaded service exits non-zero.
**Why it happens:** `bootout` errors when the service is not currently registered.
**How to avoid:** Ignore bootout failure; always remove the plist file afterward. Run `uninstall` twice in acceptance test.
**Warning signs:** `famp daemon uninstall` fails on a clean system.

### Pitfall 4: Version Bump Breaks Hardcoded Assertions
**What goes wrong:** Bumping `Cargo.toml:24` `version = "0.1.0"` → `"0.11.0"` breaks a test asserting on the literal string.
**Why it happens:** Some test or CI script hard-codes the current version string.
**How to avoid:** Before bumping, grep for version assertions (findings below — none assert "0.1.0" literally in tests, only 0.5.2 SPEC_VERSION). Safe to bump.
**Warning signs:** CI red on string equality test after bump.

### Pitfall 5: VER-01 Error Missing `famp daemon restart`
**What goes wrong:** Error message says "protocol version mismatch" without naming the fix.
**Why it happens:** Easy to omit; D-02 is explicit.
**How to avoid:** `BusClientError::ProtocolMismatch::fmt` MUST include `famp daemon restart` literally. Test asserts on the string.
**Warning signs:** Error message contains "mismatch" but not "restart".

### Pitfall 6: Linux Unit File with Tilde Paths
**What goes wrong:** Same as Pitfall 1 for systemd — `ExecStart=~/.cargo/bin/famp` fails.
**Why it happens:** systemd does not expand `~` in ExecStart.
**How to avoid:** Resolve absolute path from `$HOME` before writing unit file.
**Warning signs:** `systemctl --user status famp-broker` shows "No such file or directory".

---

## Code Examples

### macOS Plist XML (Guardian-Review Shape)

```xml
<!-- Source: verified against launchd.plist(5) man page + real LaunchAgent examples on this machine -->
<!-- IMPORTANT: All paths must be absolute (no ~). Resolve $HOME at install time. -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.famp.broker</string>
    <key>ProgramArguments</key>
    <array>
        <string>/Users/USERNAME/.cargo/bin/famp</string>
        <string>broker</string>
        <string>--no-idle-exit</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>ProcessType</key>
    <string>Background</string>
    <key>StandardOutPath</key>
    <string>/Users/USERNAME/.famp/broker.log</string>
    <key>StandardErrorPath</key>
    <string>/Users/USERNAME/.famp/broker.log</string>
</dict>
</plist>
```

**Plist shape checklist (for guardian review gate):**
- `Label` = `com.famp.broker`
- `ProgramArguments` = `[absolute_famp_binary, "broker", "--no-idle-exit"]`
- `RunAtLoad` = `true`
- `KeepAlive` = `true` (unconditional boolean, not a dict)
- `ProcessType` = `"Background"`
- `StandardOutPath` and `StandardErrorPath` = same absolute log path
- **NO** `EnvironmentVariables` key
- **NO** `UserName` / `GroupName` (user-level agent runs as current user automatically)
- All paths are absolute (no `~`)

### launchctl Invocations

```bash
# Install (idempotent):
launchctl bootstrap gui/$UID ~/Library/LaunchAgents/com.famp.broker.plist
# Tolerate exit 37 = already loaded

# Check registered (DAEMON-03 probe):
launchctl print gui/$UID/com.famp.broker >/dev/null 2>&1
# exit 0 = registered; exit 113 = not found

# Restart — binary pickup (DAEMON-05):
launchctl kickstart -k gui/$UID/com.famp.broker

# Uninstall (idempotent):
launchctl bootout gui/$UID ~/Library/LaunchAgents/com.famp.broker.plist
# Tolerate failure; remove plist file regardless
```

### systemd --user Invocations (Linux)

```bash
# Source: ArchWiki systemd/User, Ubuntu loginctl(1) man page
# Install:
mkdir -p ~/.config/systemd/user/
# write unit file with absolute paths
systemctl --user daemon-reload
systemctl --user enable --now famp-broker.service

# Check linger state (D-09 status):
loginctl show-user "$USER" --property=Linger
# Output: Linger=yes or Linger=no

# Restart — binary pickup:
systemctl --user restart famp-broker.service

# Uninstall (idempotent):
systemctl --user disable --now famp-broker.service 2>/dev/null || true
rm -f ~/.config/systemd/user/famp-broker.service
systemctl --user daemon-reload

# Detect systemd absent:
command -v systemctl >/dev/null 2>&1 || { echo "systemd not available"; exit 1; }
```

### BOOT-02: Sandbox Refusal at Install Time

```rust
// Source: crates/famp/src/bus_client/spawn.rs:112 — preflight_bind_probe
// BOOT-02 reuses the existing SandboxEperm probe:
use crate::bus_client::spawn::{preflight_bind_probe, SpawnError};

fn check_not_sandboxed(bus_dir: &Path) -> Result<(), DaemonError> {
    match preflight_bind_probe(bus_dir) {
        Err(SpawnError::SandboxEperm) => Err(DaemonError::SandboxedShell),
        Err(e) => Err(DaemonError::Io(e.into())),
        Ok(()) => Ok(()),
    }
}
// Note: preflight_bind_probe is currently private to spawn.rs; it will need
// pub(crate) visibility or extraction to a shared location.
```

### Version Bump — Files to Change

```
Cargo.toml:24   version = "0.11.0"   (was "0.1.0")
crates/famp/src/cli/mod.rs:33   about = "FAMP 0.11.0 (spec v0.5.2)"   (was "FAMP v0.5.1 reference CLI")
```

**Version string assertion audit (pre-bump grep findings):**
- No test asserts on `"0.1.0"` or `"famp 0.1.0"` literally. [VERIFIED: grep across crates/]
- `inspect_broker.rs:247` asserts `build_version.as_str().is_some()` — non-specific, safe.
- `famp-inspect-server/src/lib.rs:562` asserts `!value["build_version"].as_str().unwrap().is_empty()` — non-specific, safe.
- `FAMP_SPEC_VERSION = "0.5.2"` assertions in `famp-envelope` tests — NOT affected by workspace version bump.
- `tg-codex-famp-connection-report-2026-05-31.md` shows `famp 0.1.0` in a field report — doc file only, no assertion.
- `crates/famp/src/cli/mcp/server.rs:30` `SERVER_VERSION = env!("CARGO_PKG_VERSION")` — will update automatically.
- `crates/famp/src/bus_client/mod.rs:105` `client: format!("famp-cli/{}", env!("CARGO_PKG_VERSION"))` — will update automatically to `"famp-cli/0.11.0"`.
- README.md line 11 mentions `0.1.0` in prose — should be updated manually in Phase 6.

---

## VER-01 Design Specifics

### What the Broker Already Does (VERIFIED from source)

```
crates/famp-bus/src/broker/handle.rs:185:
    if bus_proto != BUS_PROTO_VERSION {
        return vec![Out::Reply(client, BusReply::HelloErr {
            kind: BusErrorKind::BrokerProtoMismatch,
            message: "client bus_proto=X is not supported by this broker; expected bus_proto=Y",
        })];
    }
```

The broker returns `HelloErr { BrokerProtoMismatch }` on mismatch. `HelloOk { bus_proto: BUS_PROTO_VERSION }` always echoes the broker's own constant (not the client's). [VERIFIED: handle.rs:239,263]

### What the Client Must Do (VER-01 addition)

**Premise correction from research (trust source over brief):** The CONTEXT.md summary "client sends bus_proto but never checks" is imprecise. More accurately: the broker already refuses (handle.rs:185), and client already receives that refusal as `HelloErr { BrokerProtoMismatch }` — it just doesn't handle it distinctly. The client's job is to translate the broker's `HelloErr` into the D-02-compliant message, not to add a redundant check on HelloOk.

Current state at `bus_client/mod.rs:109-114`:
```rust
BusReply::HelloOk { .. } => Ok(client),
BusReply::HelloErr { kind, message } | BusReply::Err { kind, message } => {
    Err(BusClientError::HelloFailed { kind, message })  // lumps all HelloErr together
}
```

Required change — see Pattern 4 above for the full code. Summary:
1. Match `HelloErr { kind: BusErrorKind::BrokerProtoMismatch, message }` as a separate arm
2. Return `BusClientError::ProtocolMismatch { broker_message: message }` whose Display MUST include `famp daemon restart`
3. Keep the existing `HelloErr` arm for non-mismatch failures (NotRegistered, etc.)
4. Add an `INFO` log in the `HelloOk` arm with the client build version

The broker's error message already contains "expected bus_proto=Y" — relay it verbatim plus the D-02 instruction.

### D-03 Build-Skew Log: Recommended Approach

Rather than adding `build_version` to `HelloOk` (which would edit `famp-bus` — a primitive crate, against invariant #1 of STATE.md), the D-03 log should surface via `famp daemon status` output and a one-time `tracing::info!` in the connect path:

```rust
// In daemon status output: call famp inspect broker to get daemon build_version
// In connect path: log client build vs "use famp daemon status to see daemon build"
tracing::info!(
    client_build = env!("CARGO_PKG_VERSION"),
    "connected to broker; use `famp daemon status` to verify daemon build version"
);
```

If D-03 strictly requires both versions at connect time, an additional `BusMessage::Inspect { kind: InspectKind::Broker }` round-trip immediately after `Hello`/`HelloOk` would fetch daemon build version — but this doubles connect latency and adds complexity. Recommended: surface skew in status, log client-side build at connect. Flag as `[ASSUMED]` A3 below.

---

## Runtime State Inventory

This phase is not a rename/refactor, so a full runtime state inventory is not required. However, the version bump from `0.1.0` → `0.11.0` affects one runtime artifact:

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — bus proto stays at 1; no stored version strings | None |
| Live service config | None currently (Phase 5 creates the LaunchAgent for the first time) | None |
| OS-registered state | No existing `com.famp.broker` LaunchAgent (verified: `ls ~/Library/LaunchAgents/` shows no famp entry) | None |
| Secrets/env vars | None | None |
| Build artifacts | `~/.cargo/bin/famp` at `0.1.0` — will be updated by `just install` after the bump | `just install` after bump |

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `preflight_bind_probe` in `spawn.rs` is extractable to `pub(crate)` without touching primitive crates | BOOT-02 / Code Examples | May need to be re-implemented in daemon/install.rs if the function can't be shared — low risk, same logic |
| A2 | `loginctl show-user $USER --property=Linger` outputs `Linger=yes` or `Linger=no` on all systemd-enabled Linux distros | Linux path | Could vary; test on Ubuntu 22.04+ and Fedora 38+ before shipping Linux path |
| A3 | D-03 build-skew log is satisfied by `famp daemon status` output rather than requiring an inline log at every `connect()` | VER-01 / D-03 | If user requirement is "log at connect," an extra inspect round-trip is needed — adds ~1ms latency but is architecturally clean |
| A4 | `launchctl bootout` exit code behavior (tolerate failure on uninstall) is stable across macOS versions | Uninstall pattern | Tested on macOS 15 (Sequoia); behavior may differ on macOS 12–14 |

---

## Open Questions

1. **D-03 at-connect vs in-status**
   - What we know: D-03 says "logs both versions (daemon build / client build) at connect"
   - What's unclear: "at connect" could mean inline log during connect() or available in status output
   - Recommendation: Implement as inline `tracing::info!` at connect with client build only; add daemon build to `famp daemon status` output. If insufficient, escalate to user before adding inspect round-trip.

2. **`preflight_bind_probe` visibility for BOOT-02**
   - What we know: Function is `fn preflight_bind_probe` (private) in `spawn.rs`
   - What's unclear: Whether the install path calls it directly or gets its own copy
   - Recommendation: Change to `pub(crate)` and call from `daemon/install.rs`.

3. **Linux unit `StandardOutput=append:` availability**
   - What we know: `append:` prefix requires systemd ≥ 240 (released 2018)
   - What's unclear: Whether very old distros (RHEL 7) need `>> file` wrapper instead
   - Recommendation: Use `append:` and document systemd ≥ 240 as minimum. Flag in DOC-03 (Phase 6).

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `launchctl` | macOS daemon install/uninstall/status/restart | ✓ (macOS only) | 7.0.0 (Bootstrapper) | N/A — macOS-only code path |
| `systemctl` | Linux daemon install/uninstall/status/restart | ✗ (not on macOS host) | — | N/A — Linux-only code path |
| `loginctl` | Linux linger detection (D-08/D-09) | ✗ (not on macOS host) | — | N/A — Linux-only code path |
| `~/.cargo/bin/famp` | ProgramArguments in plist | ✓ (already installed) | 0.1.0 → 0.11.0 after bump | — |
| `just` | `just install` after changes | ✓ | present | `cargo install --path crates/famp --force` |

**Missing dependencies with no fallback:** None blocking macOS development. Linux path requires a Linux host for integration testing.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (not nextest — see learned-rules about nextest hanging) |
| Config file | nextest.toml exists but `cargo test --lib / --test` is the safe runner |
| Quick run command | `cargo test -p famp --lib 2>&1` |
| Full suite command | `cargo test --workspace 2>&1` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DAEMON-01 | install is idempotent; single service + single broker after 2x install | integration (subprocess) | `cargo test -p famp --test daemon_lifecycle` | ❌ Wave 0 |
| DAEMON-02 | generated plist XML matches locked shape exactly | unit (string assertion) | `cargo test -p famp --lib -- daemon::install::tests::plist_shape_matches_locked` | ❌ Wave 0 |
| DAEMON-03 | three-state status: not-installed / installed-down / running | unit (mock states) | `cargo test -p famp --lib -- daemon::status::tests` | ❌ Wave 0 |
| DAEMON-04 | uninstall idempotent; 2x uninstall exits 0 | integration (subprocess) | `cargo test -p famp --test daemon_lifecycle` | ❌ Wave 0 |
| DAEMON-05 | restart picks up new binary | manual/platform-gated | `cargo test -p famp --test daemon_restart_binary_pickup` | ❌ Wave 0 |
| DAEMON-06 | Linux: systemd-absent exits non-zero; linger detect-and-instruct | manual/platform-gated (Linux CI) | platform-gated; unit test the linger-detection path | ❌ Wave 0 |
| BOOT-02 | daemon install refuses in sandbox (EPERM) | unit (simulate EPERM) | `cargo test -p famp --lib -- daemon::install::tests::refuses_in_sandbox` | ❌ Wave 0 |
| VER-01 | proto mismatch → loud error naming `famp daemon restart` | unit | `cargo test -p famp --lib -- bus_client::tests::proto_mismatch_names_restart` | ❌ Wave 0 |
| VER-01 | matching proto connects normally | unit | `cargo test -p famp --lib -- bus_client::tests::matching_proto_connects` | ❌ Wave 0 |
| VER-02 | `famp -V`, banner, handshake client string agree | unit | `cargo test -p famp --lib -- cli::tests::version_strings_unified` | ❌ Wave 0 |

### Notes on Platform-Gated Tests

- DAEMON-01, DAEMON-04, DAEMON-05: require actual `launchctl` — must run on macOS, not in a container. Mark `#[cfg(target_os = "macos")]` and gate behind a feature flag or env var (`FAMP_RUN_LAUNCHCTL_TESTS=1`) so CI passes on Linux.
- DAEMON-06 Linux path: unit-test the linger-detection string parsing; integration test requires Linux CI runner.
- DAEMON-02 plist shape: fully unit-testable — generate plist in-process, assert XML keys. No launchctl needed.

### Sampling Rate

- **Per task commit:** `cargo test -p famp --lib 2>&1`
- **Per wave merge:** `cargo test --workspace 2>&1`
- **Phase gate:** Full suite green + `just ci` before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `crates/famp/tests/daemon_lifecycle.rs` — covers DAEMON-01, DAEMON-04 (launchctl integration, macOS-gated)
- [ ] `crates/famp/tests/daemon_restart_binary_pickup.rs` — covers DAEMON-05 (macOS-gated)
- [ ] `crates/famp/src/cli/daemon/` — all unit test modules (DAEMON-02, DAEMON-03, BOOT-02)
- [ ] `crates/famp/src/bus_client/mod.rs` — add `proto_mismatch_names_restart` + `matching_proto_connects` tests (VER-01)
- [ ] `crates/famp/src/cli/mod.rs` — add `version_strings_unified` test (VER-02)

---

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — |
| V3 Session Management | no | — |
| V4 Access Control | partial | User-level service only (no root, no sudo); plist installs to user's own `~/Library/LaunchAgents/` |
| V5 Input Validation | yes | Plist path construction must use safe path joining (no injection); `std::path::Path` composition, not string concatenation |
| V6 Cryptography | no | — |

### Known Threat Patterns for this Stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Plist path injection (malicious $HOME) | Tampering | Validate $HOME is an absolute path before writing plist |
| Service writes secrets to log (EnvironmentVariables) | Information Disclosure | Lock shape has NO EnvironmentVariables key; guardian review enforces this |
| Daemon runs as root | Elevation of Privilege | User-level LaunchAgent only; no `UserName` key in plist; locked by REQUIREMENTS.md |
| Proto version forced downgrade | Tampering | BUS_PROTO_VERSION enforced symmetrically: broker rejects old clients, client (after VER-01) rejects old brokers |

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `launchctl load/unload` | `launchctl bootstrap/bootout` + `enable/disable` | macOS 10.10 Yosemite (2014) | load/unload still work but deprecated; bootstrap/bootout is the API |
| `~/Library/LaunchAgents/` plist name convention `com.domain.app` | same (unchanged) | — | Label must match plist filename (e.g., `com.famp.broker.plist`) |
| famp display version `0.1.0` | `0.11.0` (this phase) | Phase 5 | Milestone-aligned; resolves crate-vs-banner discrepancy |

---

## Sources

### Primary (HIGH confidence)
- `crates/famp-bus/src/broker/handle.rs:185` — verified broker hello() rejects on bus_proto mismatch; returns `BUS_PROTO_VERSION` in HelloOk
- `crates/famp/src/bus_client/mod.rs:109` — verified client discards HelloOk.bus_proto (current state)
- `crates/famp/src/bus_client/spawn.rs:112` — verified `preflight_bind_probe` EPERM detection shape
- `launchctl(1)` man page — verified `bootstrap`, `bootout`, `kickstart -k`, `print` exit codes; "NOT API" warning on print output
- `launchd.plist(5)` man page — verified all plist keys: `RunAtLoad`, `KeepAlive`, `ProcessType Background`, `StandardOutPath`, `StandardErrorPath`, `ProgramArguments`; tilde-not-expanded behavior confirmed from real plist examples on this machine
- `~/Library/LaunchAgents/com.benlamm.corporate-brain-mcp.plist` — real KeepAlive=true LaunchAgent example with absolute paths

### Secondary (MEDIUM confidence)
- [ArchWiki systemd/User](https://wiki.archlinux.org/title/Systemd/User) — `~/.config/systemd/user/` location, `systemctl --user enable --now`, `loginctl enable-linger` pattern
- [Ubuntu loginctl(1) man page](https://manpages.ubuntu.com/manpages/jammy/man1/loginctl.1.html) — `loginctl show-user $USER --property=Linger` syntax

### Tertiary (LOW confidence)
- Training knowledge: systemd `append:` prefix requires ≥ 240 (not verified against a live Linux system in this session)

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new deps; all patterns sourced from live system or verified source code
- Architecture (macOS): HIGH — verified against live launchctl, real plist examples, and source code
- Architecture (Linux): MEDIUM — verified against authoritative docs, not runnable on macOS host
- VER-01 design: HIGH — verified broker behavior from source, client behavior from source
- Version bump safety: HIGH — grepped all test files, confirmed no "0.1.0" string assertions

**Research date:** 2026-06-04
**Valid until:** 90 days (launchd API is stable across macOS versions; systemd --user is stable)

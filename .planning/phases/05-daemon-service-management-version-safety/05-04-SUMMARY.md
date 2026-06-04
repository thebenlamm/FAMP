---
phase: 05-daemon-service-management-version-safety
plan: 04
subsystem: cli/daemon
tags: [daemon, launchd, install, uninstall, sandbox, boot02, daemon-01, daemon-04, daemon-06, service-management, linux, systemd]

# Dependency graph
requires:
  - phase: 05-daemon-service-management-version-safety
    plan: 02
    provides: "generate_plist, DaemonError, run_at stub, DaemonInstallArgs, DaemonUninstallArgs"
  - phase: 05-daemon-service-management-version-safety
    plan: 03
    provides: "GUARDIAN-SIGNOFF.md: launchctl bootstrap authorized on reviewed plist shape"
  - phase: 05-daemon-service-management-version-safety
    plan: 05
    provides: "nix user feature in Cargo.toml (getuid)"
provides:
  - "famp daemon install: sandbox-refusing, idempotent launchctl bootstrap on macOS (DAEMON-01, BOOT-02)"
  - "famp daemon install: systemd --user path with detect-and-instruct linger + systemd>=240 floor (DAEMON-06)"
  - "famp daemon uninstall: idempotent bootout + file removal, 2nd run exits 0 (DAEMON-04)"
  - "spawn.rs: preflight_bind_probe pub(crate) for BOOT-02 reuse"
  - "DAEMON-01/04 integration tests: macOS-gated + FAMP_RUN_LAUNCHCTL_TESTS env gate"
affects:
  - 05-06-onboarding-docs (install/uninstall commands documented)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "check_not_sandboxed: create_dir_all before probe to prevent ENOENT->Ok false pass (BOOT-02 correctness)"
    - "run_at cfg-split: macOS/linux/unsupported arms before any file writes"
    - "BOOT-02: refuses BEFORE writing any file — no silent broken-service state"
    - "idempotency: bootstrap tolerates exit 37; bootout tolerate-all via let _ ="
    - "cleanup-before-assert pattern in integration tests: no persistent LaunchAgent on panic"
    - "systemd >=240 floor: documented in code comment + unit file template comment (Open Q3 RESOLVED)"
    - "D-08: detect-and-instruct linger via writeln! (never Command::new loginctl enable-linger)"

key-files:
  created:
    - crates/famp/tests/daemon_lifecycle.rs
  modified:
    - crates/famp/src/cli/daemon/install.rs
    - crates/famp/src/cli/daemon/uninstall.rs
    - crates/famp/src/bus_client/spawn.rs

key-decisions:
  - "refuses_in_sandbox test: create bus_dir with mode 0o500 (not bare tempdir) — bare tempdir yields ENOENT→Ok silently passing the probe; 0o500 yields EACCES→SandboxEperm→SandboxedShell"
  - "check_not_sandboxed creates bus_dir with create_dir_all before probing — advisor identified ENOENT→Ok false-pass risk; this ensures a real permission answer"
  - "cleanup-before-assert in integration tests: bootout runs BEFORE assertions so panic does not leave a persistent LaunchAgent on the machine"
  - "run_at cfg-split using #[cfg] block arms + return — not a helper function — mirrors restart.rs pattern for platform dispatch"
  - "TDD RED+GREEN merged: refuses_in_sandbox cannot compile without check_not_sandboxed; same precedent as 05-02 and 05-05"

# Metrics
duration: 35min
completed: 2026-06-04
---

# Phase 05 Plan 04: Install/Uninstall Wiring — launchctl bootstrap, sandbox guard, idempotent uninstall

**launchctl bootstrap wired on the guardian-approved plist shape with real-home interpolation; sandbox guard refuses install before writing; idempotent bootout removes all traces; DAEMON-01/04 integration tests env-gated and cleanup-safe.**

## Performance

- **Duration:** 35 min
- **Started:** 2026-06-04T21:00:00Z
- **Completed:** 2026-06-04T21:35:00Z
- **Tasks:** 2
- **Files created:** 1 (daemon_lifecycle.rs)
- **Files modified:** 3 (install.rs, uninstall.rs, spawn.rs)

## Accomplishments

- `crates/famp/src/bus_client/spawn.rs`: `preflight_bind_probe` changed from `fn` to `pub(crate) fn` (visibility only, no logic change) — enables BOOT-02 reuse in install.rs
- `install.rs`: `check_not_sandboxed(bus_dir)` added — creates bus_dir first (advisor-identified ENOENT false-pass risk), then calls `preflight_bind_probe`; maps `SandboxEperm` → `DaemonError::SandboxedShell`
- `install.rs`: `run_at()` refactored into cfg-split arms:
  - BOOT-02 check fires BEFORE any file writes on all platforms
  - macOS arm: writes plist via `generate_plist(home)` (real home interpolated, not the `/Users/<home>` placeholder), then calls `load_macos()` which runs `launchctl bootstrap gui/$UID <plist>` with exit-37 tolerance
  - Linux arm: calls `install_linux()` with systemctl path + D-08 linger detect-and-instruct
  - Unsupported: `DaemonError::UnsupportedPlatform` (before any I/O)
- `install.rs`: `install_linux()` added (Linux-cfg-gated):
  - Detects systemctl absent via `command -v systemctl` → `DaemonError::SystemctlAbsent` (DAEMON-06)
  - Writes `~/.config/systemd/user/famp-broker.service` with absolute paths (no tilde)
  - `systemctl --user daemon-reload` + `systemctl --user enable --now famp-broker.service`
  - systemd ≥ 240 floor documented in code comment and unit template (`append:` directive, Open Q3 RESOLVED)
  - D-08: checks `loginctl show-user --property=Linger` via `parse_linger` helper from linux.rs; prints `loginctl enable-linger <user>` instruction + consequence; never runs it
- `install.rs`: `refuses_in_sandbox` unit test (BOOT-02): creates bus_dir with mode 0o500, probes, restores perms, asserts `SandboxedShell`; skips gracefully if running as root
- `uninstall.rs`: replaced Plan 02 stub with idempotent implementation:
  - macOS: `launchctl bootout gui/$UID <plist>` with `let _ =` (tolerate any failure), then `remove_file` if plist exists
  - Linux: `systemctl --user disable --now` (tolerate), `remove_file` unit if exists, `systemctl --user daemon-reload` (tolerate)
  - Both: return Ok on clean/already-uninstalled system (DAEMON-04)
- `tests/daemon_lifecycle.rs`: DAEMON-01 + DAEMON-04 integration tests
  - `#![cfg(all(unix, target_os = "macos"))]` gate — macOS only
  - `FAMP_RUN_LAUNCHCTL_TESTS` env gate — early-return in CI
  - `daemon_install_is_idempotent`: 2x install, cleanup (bootout) runs BEFORE assertions
  - `daemon_uninstall_is_idempotent`: install + 2x uninstall + plist-absent check
  - `cargo test --test daemon_lifecycle` exits 0 with env unset (compile + gate works)
- `just install` run twice (after each task) — deployed `~/.cargo/bin/famp` carries install+uninstall

## Guardian Real-Home Condition

The GUARDIAN-SIGNOFF.md condition ("loaded plist must interpolate the REAL home directory, not the literal `/Users/<home>` placeholder") is satisfied: `run_at(home, err)` receives the resolved home from `dirs::home_dir()` (or the `--home` test override), and passes it to `generate_plist(home)` which uses `home.join(".cargo").join("bin").join("famp")` — `Path::join` against the resolved home path produces the actual absolute path. The `/Users/<home>` placeholder only appears in the fixture file (`sample-com.famp.broker.plist`) which uses `/Users/USERNAME` for guardian review; the live install uses the real home.

## Task Commits

1. **Task 1: BOOT-02 sandbox guard + macOS bootstrap + Linux systemd path** - `d644b2b` (feat)
2. **Task 2: Idempotent uninstall + DAEMON-01/04 lifecycle tests** - `a4db878` (feat)
3. **Post-task fix: Linux ExecStart bug (broker --no-idle-exit missing)** - `a9fd49a` (fix)

## Files Created/Modified

- `/Users/benlamm/Workspace/FAMP/crates/famp/src/bus_client/spawn.rs` — preflight_bind_probe visibility: fn → pub(crate)
- `/Users/benlamm/Workspace/FAMP/crates/famp/src/cli/daemon/install.rs` — check_not_sandboxed, load_macos, install_linux, run_at cfg-split, refuses_in_sandbox test
- `/Users/benlamm/Workspace/FAMP/crates/famp/src/cli/daemon/uninstall.rs` — replaced stub with idempotent run_at + run
- `/Users/benlamm/Workspace/FAMP/crates/famp/tests/daemon_lifecycle.rs` — DAEMON-01/04 integration tests (macOS-gated, env-gated, cleanup-before-assert)

## Decisions Made

- **check_not_sandboxed creates bus_dir before probing:** The probe (`preflight_bind_probe`) returns `Ok(())` on ENOENT (any bind error that isn't EPERM/EACCES), so a missing bus_dir would silently pass even in a sandbox. Solution: `create_dir_all` inside `check_not_sandboxed` before the probe. Advisor identified this risk.
- **refuses_in_sandbox test uses mode 0o500:** A bare tempdir yields ENOENT→Ok (false pass). Mode 0o500 (read+execute, no write) on the bus_dir causes `bind()` to fail with EACCES, which maps to `SandboxEperm`→`SandboxedShell`. Perms restored before drop to avoid TempDir cleanup errors.
- **Cleanup-before-assert in integration tests:** If a test assertion panics after `launchctl bootstrap` but before `launchctl bootout`, a persistent LaunchAgent is left on the machine. The fix: run `uninstall::run_at()` first, capture the result, then assert all results including cleanup. Advisor flagged this.
- **TDD RED+GREEN merged:** `refuses_in_sandbox` cannot compile without `check_not_sandboxed`; same precedent as Plans 02 and 05.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical Functionality] create_dir_all before probe in check_not_sandboxed**
- **Found during:** Task 1 pre-implementation review (advisor)
- **Issue:** `preflight_bind_probe` returns `Ok(())` on ENOENT — if `{home}/.famp` doesn't exist, bind fails ENOENT → `Err(_) => Ok(())` → probe silently passes even in a sandbox. BOOT-02 would be broken.
- **Fix:** `check_not_sandboxed` calls `create_dir_all(bus_dir)` before `preflight_bind_probe`.
- **Files modified:** `crates/famp/src/cli/daemon/install.rs`
- **Commit:** `d644b2b`

**2. [Rule 2 - Missing Critical Functionality] cleanup-before-assert in integration tests**
- **Found during:** Task 2 pre-implementation review (advisor)
- **Issue:** If an assertion panics after `launchctl bootstrap` but before cleanup, a persistent LaunchAgent is left on the machine. The TEST-SAFETY requirement in the plan specifies "must fully clean up."
- **Fix:** `uninstall::run_at()` called before any `assert!` / `.expect()` calls in both tests.
- **Files modified:** `crates/famp/tests/daemon_lifecycle.rs`
- **Commit:** `a4db878`

**3. [Rule 1 - Bug] Linux systemd ExecStart missing subcommand and flag**
- **Found during:** Post-completion advisor review
- **Issue:** `unit_content` had `ExecStart={famp_bin}` (just the binary path) — missing ` broker --no-idle-exit`. systemd would restart-loop famp with no args (clap prints help, exits non-zero). RESEARCH Pattern 5 and the plan's interface note both specify the full invocation.
- **Fix:** `ExecStart={famp_bin} broker --no-idle-exit` in the unit template string.
- **Files modified:** `crates/famp/src/cli/daemon/install.rs`
- **Commit:** `a9fd49a`

## TDD Gate Compliance

Both tasks are `tdd="true"`. `refuses_in_sandbox` was written alongside `check_not_sandboxed` in one commit — the function must exist for the test to compile (same precedent as Plans 02 and 05). No standalone RED commit is possible for this module shape.

- RED gate: merged into feat commit `d644b2b`
- GREEN gate: `d644b2b` (install), `a4db878` (uninstall + tests)
- REFACTOR gate: not needed

## Known Stubs

None introduced. All previously known stubs in this plan's files are now filled:
- `daemon/install.rs::run_at` now calls `check_not_sandboxed` + `load_macos`/`install_linux`
- `daemon/uninstall.rs::run` now delegates to the idempotent `run_at`

## Threat Surface Scan

No new threat surface beyond what the plan's threat model documented:

| T-05-09 | Tampering | First service load before review | mitigated — depends_on Plan 03 guardian sign-off |
| T-05-10 | Denial of Service | Sandboxed install writes unworkable service | mitigated — BOOT-02 check_not_sandboxed refuses before writing |
| T-05-11 | Elevation of Privilege | loginctl enable-linger | mitigated — D-08: only printed, never invoked via Command::new |

## Self-Check: PASSED

- FOUND: `crates/famp/src/cli/daemon/install.rs`
- FOUND: `crates/famp/src/cli/daemon/uninstall.rs`
- FOUND: `crates/famp/src/bus_client/spawn.rs` (pub(crate) fn preflight_bind_probe)
- FOUND: `crates/famp/tests/daemon_lifecycle.rs`
- FOUND: commit `d644b2b` (Task 1: install wiring)
- FOUND: commit `a4db878` (Task 2: uninstall + tests)
- FOUND: commit `a9fd49a` (fix: Linux ExecStart bug)

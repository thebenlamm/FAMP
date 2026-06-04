---
phase: 5
slug: daemon-service-management-version-safety
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-06-04
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Derived from `05-RESEARCH.md` § Validation Architecture.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (NOT nextest — nextest stalls in the `--list` phase, see project memory `cargo nextest -p famp hangs`) |
| **Config file** | none required — `cargo test --lib / --test` is the safe runner |
| **Quick run command** | `cargo test -p famp --lib 2>&1` |
| **Full suite command** | `cargo test --workspace 2>&1` |
| **Estimated runtime** | ~30–60 seconds (lib); workspace longer |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p famp --lib 2>&1`
- **After every plan wave:** Run `cargo test --workspace 2>&1`
- **Before `/gsd-verify-work`:** Full suite green + `just ci`
- **Max feedback latency:** ~60 seconds

---

## Per-Task Verification Map

| Requirement | Behavior | Test Type | Automated Command | File Exists | Status |
|-------------|----------|-----------|-------------------|-------------|--------|
| DAEMON-01 | install idempotent — single service + single broker after 2× install | integration (subprocess, macOS-gated) | `cargo test -p famp --test daemon_lifecycle` | ❌ W0 | ⬜ pending |
| DAEMON-02 | generated plist XML matches locked shape exactly | unit (string assertion) | `cargo test -p famp --lib -- daemon::install::tests::plist_shape_matches_locked` | ❌ W0 | ⬜ pending |
| DAEMON-03 | three-state status: not-installed / installed-but-down / running (distinct output + exit codes) | unit (mock states) | `cargo test -p famp --lib -- daemon::status::tests` | ❌ W0 | ⬜ pending |
| DAEMON-04 | uninstall idempotent — 2× uninstall exits 0, no orphan registration | integration (subprocess, macOS-gated) | `cargo test -p famp --test daemon_lifecycle` | ❌ W0 | ⬜ pending |
| DAEMON-05 | restart picks up replaced on-disk binary | manual / platform-gated | `cargo test -p famp --test daemon_restart_binary_pickup` | ❌ W0 | ⬜ pending |
| DAEMON-06 | Linux: systemd-absent exits non-zero; linger detect-and-instruct (no auto-escalate) | unit (linger-string parse) + Linux-CI integration | `cargo test -p famp --lib -- daemon::linux::tests` | ❌ W0 | ⬜ pending |
| BOOT-02 | daemon install refuses in sandbox (EPERM-on-bind) | unit (simulate EPERM) | `cargo test -p famp --lib -- daemon::install::tests::refuses_in_sandbox` | ❌ W0 | ⬜ pending |
| VER-01 | proto mismatch → loud error naming `famp daemon restart` | unit | `cargo test -p famp --lib -- bus_client::tests::proto_mismatch_names_restart` | ❌ W0 | ⬜ pending |
| VER-01 | matching proto connects normally | unit | `cargo test -p famp --lib -- bus_client::tests::matching_proto_connects` | ❌ W0 | ⬜ pending |
| VER-02 | `famp -V`, banner, handshake build string all agree on `0.11.0` | unit | `cargo test -p famp --lib -- cli::tests::version_strings_unified` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/famp/tests/daemon_lifecycle.rs` — DAEMON-01, DAEMON-04 (launchctl integration, `#[cfg(target_os = "macos")]`, gated behind `FAMP_RUN_LAUNCHCTL_TESTS=1`)
- [ ] `crates/famp/tests/daemon_restart_binary_pickup.rs` — DAEMON-05 (macOS-gated)
- [ ] `crates/famp/src/cli/daemon/` — unit test modules for DAEMON-02 (plist shape), DAEMON-03 (status states), BOOT-02 (sandbox refusal)
- [ ] `crates/famp/src/cli/daemon/linux.rs` (or equiv) — linger-detection string-parse unit test (DAEMON-06)
- [ ] `crates/famp/src/bus_client/mod.rs` — `proto_mismatch_names_restart` + `matching_proto_connects` tests (VER-01)
- [ ] `crates/famp/src/cli/mod.rs` — `version_strings_unified` test (VER-02)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Real launchd install → `famp inspect broker` reports HEALTHY; 2nd install leaves exactly one registration | DAEMON-01 | Requires live `launchctl bootstrap gui/$UID` — not runnable in CI container | On macOS: `famp daemon install` twice; `launchctl print gui/$UID/<label>` shows one entry; `famp inspect broker` → HEALTHY |
| Restart picks up a freshly `cargo install`ed binary (new `-V` after restart ≠ old) | DAEMON-05 | Requires replacing the on-disk binary and a live `kickstart -k` | On macOS: note `famp -V`; rebuild+`just install` with bumped version; `famp daemon restart`; confirm running broker reports new version |
| **guardian plist sign-off (BLOCKING)** | DAEMON-02 | External human/skill review gate — the literal plist XML must pass the `mac-guardian` checklist before first `launchctl bootstrap` | Generate plist, pass XML to `mac-guardian` skill, obtain approval BEFORE any load step |
| Linux systemd `--user` install + linger detect-and-instruct end-to-end | DAEMON-06 | Requires a Linux host with systemd `--user` session | On Linux: `famp daemon install`; verify unit started; with linger off, confirm exact `loginctl enable-linger <user>` instruction printed and NOT auto-run; `famp daemon status` reports linger state |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 60s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-06-04

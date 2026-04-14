---
phase: 1
slug: identity-cli-foundation
status: planned
nyquist_compliant: true
wave_0_complete: true
created: 2026-04-14
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo-nextest 0.9.x (unit + integration) |
| **Config file** | `Cargo.toml` workspace + `crates/famp/tests/` |
| **Quick run command** | `cargo nextest run -p famp` |
| **Full suite command** | `cargo nextest run --workspace` |
| **Estimated runtime** | ~30–60 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo nextest run -p famp`
- **After every plan wave:** Run `cargo nextest run --workspace`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 60 seconds

---

## Per-Task Verification Map

*Populated by planner — one row per task with its automated command and requirement mapping.*

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-01-T1 | 01 | 1 | IDENT-01 (prep) | T-1-01, T-1-02 | Typed CliError with no key bytes; write_secret() creates 0600 atomically | unit | `cargo nextest run -p famp perms::tests` | ❌ new | ⬜ |
| 01-01-T2 | 01 | 1 | CLI-07, IDENT-03, IDENT-04 | T-1-03 | FAMP_HOME absolute-only; Config deny_unknown_fields; empty peers.toml → empty Peers | unit | `cargo nextest run -p famp cli::home cli::config` | ❌ new | ⬜ |
| 01-02-T1 | 02 | 2 | IDENT-02 | — | rcgen PEMs loadable by famp_transport_http::tls::build_server_config (cross-phase gate) | integration (unit-inline) | `cargo nextest run -p famp cli::init::tls` | ❌ new | ⬜ |
| 01-02-T1b | 02 | 2 | CLI-01 (--force) | T-1-02 | atomic_replace() stages + rename with best-effort rollback | unit | `cargo nextest run -p famp cli::init::atomic` | ❌ new | ⬜ |
| 01-02-T2 | 02 | 2 | CLI-01, IDENT-01, IDENT-02, IDENT-03, IDENT-04, IDENT-06 | T-1-01, T-1-02 | init writes 6 files with exact modes; D-15 stdout/stderr; bin rewrite | integration (end-to-end bin) | `cargo nextest run -p famp cli::init && cargo build -p famp` | ❌ new | ⬜ |
| 01-03-T1 | 03 | 3 | IDENT-05, IDENT-06 | T-1-01 | load_identity returns first missing file; compile_fail doc-test locks no-Display on FampSigningKey | unit + doc-test | `cargo nextest run -p famp cli::init::load_identity_tests && cargo test -p famp-crypto --doc` | ❌ new | ⬜ |
| 01-03-T2a | 03 | 3 | CLI-01, IDENT-01, IDENT-02 | T-1-02 | happy path: 6 files, exact modes, D-15 output, TLS conformance gate | integration | `cargo nextest run -p famp --test init_happy_path` | ❌ new | ⬜ |
| 01-03-T2b | 03 | 3 | CLI-01 (--force) | T-1-02 | --force atomically replaces; new keys differ from old | integration | `cargo nextest run -p famp --test init_force` | ❌ new | ⬜ |
| 01-03-T2c | 03 | 3 | CLI-01 | T-1-02 | AlreadyInitialized lists existing files; stale content untouched | integration | `cargo nextest run -p famp --test init_refuses` | ❌ new | ⬜ |
| 01-03-T2d | 03 | 3 | IDENT-05 | — | IdentityIncomplete on missing file; HomeNotAbsolute on relative path | integration | `cargo nextest run -p famp --test init_identity_incomplete` | ❌ new | ⬜ |
| 01-03-T2e | 03 | 3 | IDENT-06 | T-1-01 | 8-byte seed window scan over captured stdout+stderr (D-17 mech #3) | integration | `cargo nextest run -p famp --test init_no_leak` | ❌ new | ⬜ |
| 01-03-T2f | 03 | 3 | CLI-07 | T-1-03 | FAMP_HOME env var override (single serial test in its own binary) | integration (serial) | `cargo nextest run -p famp --test init_home_env` | ❌ new | ⬜ |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*Derived from RESEARCH.md Wave 0 gap list — planner populates with exact paths.*

- [x] `crates/famp/tests/` integration test files — 6 files shipped in Plan 03 Task 2: init_happy_path, init_force, init_refuses, init_identity_incomplete, init_no_leak, init_home_env
- [x] Shared test helpers — none needed; each test uses `tempfile::TempDir` + `famp::cli::init::run_at(&home, force, &mut out, &mut err)` directly (CD-05 Rust-API route)
- [x] Serial test harness — `init_home_env.rs` is the only env-var-sensitive test and lives alone in its own integration-test binary (nextest runs each test binary in its own process → serial by construction, no `serial_test` crate required)

### Additional Wave 0 scaffolding (added during planning)
- [x] Plan 01 establishes type scaffolding (CliError, IdentityLayout, Config, Peers, perms helpers) BEFORE Plan 02 implements init — Plan 03 tests build against these exact public signatures
- [x] Plan 03 Task 1 adds `cli::init::load_identity` before integration tests need it
- [x] Plan 03 Task 1 adds `compile_fail` doc-test on `famp_crypto::FampSigningKey` — locks D-17 mechanism #1

*Existing substrate (`famp-crypto`, `famp-keyring`, `famp-transport-http`) provides base crates; Phase 1 installs `famp` binary crate test scaffolding.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| _(none expected — all phase behaviors automatable)_ | | | |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending

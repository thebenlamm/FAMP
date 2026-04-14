---
phase: 01-identity-cli-foundation
milestone: v0.8
verified: 2026-04-14T20:02:00Z
status: passed
score: 8/8
overrides_applied: 0
re_verification: null
gaps: []
deferred: []
human_verification: []
---

# Phase 01 — Identity & CLI Foundation — Verification Report

**Phase Goal:** A developer can run `famp init` on a fresh laptop and get a fully wired persistent identity — Ed25519 keypair, self-signed TLS cert, config, and peer list — ready to be used by every subsequent subcommand.

**Verdict:** PASS

## Goal Achievement — Success Criteria (ROADMAP)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `famp init` on fresh machine produces `key.ed25519` (0600, 32 bytes), `pub.ed25519`, `tls.cert.pem`, `tls.key.pem`, `config.toml` with `listen_addr = "127.0.0.1:8443"`, empty `peers.toml`, exits 0 | VERIFIED | Live smoke: exit 0; ls/stat shows all six files with modes `600 key.ed25519`, `600 tls.key.pem`, `644` on public files; `config.toml` = `listen_addr = "127.0.0.1:8443"` (31 bytes); `peers.toml` = 0 bytes. Locked by `tests/init_happy_path.rs`. |
| 2 | Second `famp init` without `--force` exits non-zero with human-readable error, no silent overwrite | VERIFIED | `cli::init::run_at` in `crates/famp/src/cli/init/mod.rs:44` returns `CliError::AlreadyInitialized { existing_files }` when `force=false` and any layout entry exists. Locked by `tests/init_refuses.rs` (stale bytes unchanged). |
| 3 | `FAMP_HOME` override creates identity at override path; every subcommand reads same override | VERIFIED | `resolve_famp_home` in `crates/famp/src/cli/home.rs` reads env first, rejects relative paths (`HomeNotAbsolute`). Locked by `tests/init_home_env.rs` + `cli::home::tests::resolves_and_rejects`. Subcommand dispatch routes through `cli::run` → `init::run` which reuses `resolve_famp_home`. |
| 4 | Any subcommand against missing/incomplete `FAMP_HOME` produces typed error naming the absent/malformed file, exits non-zero | VERIFIED (Phase-1 slice) | `pub fn load_identity(home: &Path) -> Result<IdentityLayout, CliError>` at `crates/famp/src/cli/init/mod.rs:224` walks `IdentityLayout::entries()` and returns `CliError::IdentityIncomplete { missing }` on first gap. Locked by `tests/init_identity_incomplete.rs` + `cli::init::load_identity_tests` (3 unit tests). Note: this is the reader path Phase 2 subcommands will consume; Phase 1 has only `init` as a subcommand, so the reader is exercised via the loader, not via a separate subcommand. |
| 5 | Private key bytes never appear in stdout, stderr, logs, or CLI error messages | VERIFIED | Three-layer defense: (a) `CliError` variants in `crates/famp/src/cli/error.rs` carry only `PathBuf` + `#[source]` — no `[u8]`/`FampSigningKey` fields (structural audit); (b) `compile_fail` doc-test on `famp_crypto::FampSigningKey` at `crates/famp-crypto/src/keys.rs:48` forbids `Display` impl (passed under `cargo test -p famp-crypto --doc`); (c) `tests/init_no_leak.rs` does 8-byte sliding-window scan of captured stdout+stderr against on-disk seed, zero matches. `Debug` returns `FampSigningKey(<redacted>)` (`keys.rs:123`). |

## Requirements Coverage

| REQ | Status | Evidence (file:line) |
|-----|--------|---------------------|
| CLI-01 (`famp init` creates identity, refuses without `--force`) | SATISFIED | `crates/famp/src/cli/init/mod.rs:44` (`run_at`), `tests/init_happy_path.rs`, `tests/init_force.rs`, `tests/init_refuses.rs` |
| CLI-07 (`FAMP_HOME` env override) | SATISFIED | `crates/famp/src/cli/home.rs` `resolve_famp_home`, `tests/init_home_env.rs` |
| IDENT-01 (raw 32-byte key files, 0600 on secret) | SATISFIED | `crates/famp/src/cli/perms.rs` (`write_secret` `O_EXCL` + `mode(0o600)`); live smoke `stat` shows `600 key.ed25519` / `600 tls.key.pem`; `init_happy_path` asserts byte length 32 + mode 0600 |
| IDENT-02 (self-signed TLS via `rcgen`, `tls.cert.pem` + `tls.key.pem`) | SATISFIED | `crates/famp/src/cli/init/tls.rs:16` `generate_tls` using `CertificateParams::new(...).self_signed(&KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256))`; cross-phase gate test `generated_pems_load_via_transport_http_loader` round-trips through `famp_transport_http::tls::build_server_config` |
| IDENT-03 (`config.toml`; Phase-1 narrowing: `listen_addr` only) | SATISFIED (narrowed) | `crates/famp/src/cli/config.rs` `Config { listen_addr }` with `serde(deny_unknown_fields)`; byte-exact test `config_default_serializes_one_field`; matches CONTEXT D-12 |
| IDENT-04 (`peers.toml`; Phase-1 narrowing: empty placeholder) | SATISFIED (narrowed) | Empty `PeerEntry {}` placeholder; zero-byte `peers.toml` written by `init`; test `peers_empty_file_loads_empty`; matches CONTEXT D-14 |
| IDENT-05 (startup load + fail-closed typed error) | SATISFIED (Phase-1 slice) | `load_identity` at `mod.rs:224` returns `IdentityIncomplete { missing }` or `HomeNotAbsolute`; 3 unit tests + integration test. Permission-enforcement path deferred per plan to Phase 2 readers. |
| IDENT-06 (no key material in logs/stdout/MCP) | SATISFIED | Structural `CliError` audit (no `[u8]` variants), `compile_fail` doc-test on `FampSigningKey`, `Debug` redaction test (`keys.rs:234`), `init_no_leak` sliding-window scan |

No orphaned requirements: REQUIREMENTS.md maps exactly CLI-01, CLI-07, IDENT-01..06 to Phase 1 — all eight appear in plan frontmatter and all eight are satisfied.

## Required Artifacts

| Artifact | Status | Details |
|----------|--------|---------|
| `crates/famp/src/cli/mod.rs` | VERIFIED | `Cli`/`Commands`/`InitArgs`, real `run` dispatcher at line 37 |
| `crates/famp/src/cli/error.rs` | VERIFIED | 11-variant `CliError`, zero key material |
| `crates/famp/src/cli/home.rs` | VERIFIED | `resolve_famp_home` with env + absolute-path checks |
| `crates/famp/src/cli/paths.rs` | VERIFIED | `IdentityLayout` + 6 filename constants |
| `crates/famp/src/cli/perms.rs` | VERIFIED | `write_secret`/`write_public` with `O_EXCL` + explicit mode |
| `crates/famp/src/cli/config.rs` | VERIFIED | `Config`, `Peers`, `PeerEntry`, `deny_unknown_fields` |
| `crates/famp/src/cli/init/mod.rs` | VERIFIED | `run`, `run_at`, `materialize_identity`, `load_identity` |
| `crates/famp/src/cli/init/tls.rs` | VERIFIED | `generate_tls` via rcgen `CertificateParams::new(...).self_signed(&KeyPair::generate_for(...))` |
| `crates/famp/src/cli/init/atomic.rs` | VERIFIED | `atomic_replace` — sibling TempDir + two-step rename + rollback |
| `crates/famp/src/bin/famp.rs` | VERIFIED | Rewritten from 8-line placeholder; real `Cli::parse` → `famp::cli::run` dispatch |
| `crates/famp-crypto/src/keys.rs` | VERIFIED | `compile_fail` doc-test at line 48 forbids `Display` on `FampSigningKey` |
| `crates/famp/tests/init_happy_path.rs` | VERIFIED | Two test fns — files/modes/D-15 stdout+stderr + transport-http loader round-trip |
| `crates/famp/tests/init_force.rs` | VERIFIED | `--force` atomic replace, fresh pubkey |
| `crates/famp/tests/init_refuses.rs` | VERIFIED | Non-empty home + no force → `AlreadyInitialized` |
| `crates/famp/tests/init_identity_incomplete.rs` | VERIFIED | Missing file → `IdentityIncomplete` |
| `crates/famp/tests/init_no_leak.rs` | VERIFIED | 8-byte sliding-window leak scan |
| `crates/famp/tests/init_home_env.rs` | VERIFIED | `FAMP_HOME` env override routing |

## Key Link Verification

| From | To | Status | Details |
|------|----|--------|---------|
| `cli::run` dispatcher → `init::run_at` | WIRED | `cli/mod.rs:37` matches `Commands::Init(args)` and calls `init::run(args)` → `run_at(&home, args.force, &mut io::stdout().lock(), &mut io::stderr().lock())` |
| `bin/famp.rs` → `famp::cli::run` | WIRED | Body: `Cli::parse()` → `famp::cli::run(cli)` → `eprintln!` + `exit(1)` on `Err`. Forbids unsafe (`#![forbid(unsafe_code)]`). |
| `init::materialize_identity` → `perms::write_secret` (0600 `O_EXCL`) | WIRED | Secret key + TLS key go through `write_secret`; public key + cert + config + peers go through `write_public`. Live `stat` confirms `600` on `key.ed25519` and `tls.key.pem` only. |
| `init::tls::generate_tls` → `famp_transport_http::tls::{load_pem_cert, load_pem_key, build_server_config}` | WIRED | Cross-phase conformance gate test `generated_pems_load_via_transport_http_loader` round-trips PEMs through the existing v0.7 loader — no divergent cert format. |
| `resolve_famp_home` → `std::env::var("FAMP_HOME")` + absolute-path check | WIRED | `home.rs` reads env, rejects relative, rejects no-parent; fallback to `$HOME/.famp`. |
| `FampSigningKey` Display attempt | BLOCKED (by design) | `compile_fail` doc-test: `format!("{}", sk)` must fail to compile; doc-test harness confirms this at `cargo test -p famp-crypto --doc`. |

## Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Workspace test suite | `cargo nextest run --workspace` | **284 passed, 1 skipped** (v0.7 baseline was 253; Phase 1 adds 31 net) | PASS |
| Clippy workspace (deny-warnings) | `cargo clippy --workspace --all-targets -- -D warnings` | Clean, no warnings | PASS |
| famp-crypto doc tests (incl. `compile_fail`) | `cargo test -p famp-crypto --doc` | 3/3 passed (line 39 runtime redaction + line 48 compile_fail + lib.rs:37) | PASS |
| No OpenSSL in dep tree | `cargo tree -i openssl` | `error: package ID specification 'openssl' did not match any packages` — gate holds | PASS |
| No native-tls in dep tree | `cargo tree -i native-tls` | Same "did not match" — gate holds | PASS |
| Live `famp init` smoke (fresh `FAMP_HOME`) | `FAMP_HOME=$TMPD/famp cargo run -q -p famp -- init` | exit 0; stdout = 43-char base64url pubkey (`YHvFniQsKppOz3o4zDZb0HjQKEQJ_AcQh24HI6XLMTo`); stderr = `initialized FAMP home at …`; six files created with exact modes `600/600/644/644/644/644`; `config.toml` = `listen_addr = "127.0.0.1:8443"` (31 bytes); `peers.toml` = 0 bytes | PASS |

## Anti-Patterns Scanned

None found in Phase 1 files. Specific checks:
- No `TODO`/`FIXME`/`PLACEHOLDER` in the Phase-1 surface (all deviations documented in SUMMARY files as intentional lint concessions).
- No empty `return Ok(())` stubs — `init::run_at` has real materialization logic; `load_identity` walks real entries.
- The one documented stub from Plan 01 (`cli::run` returning `HomeNotSet`) was replaced in Plan 02 per the interface contract.
- `bin/famp.rs` is 40 lines but 33 of those are `use X as _;` silencer stanzas forced by the workspace `unused_crate_dependencies` lint — the functional body is 7 lines of `Cli::parse` → `cli::run` → exit. Documented in `01-02-SUMMARY.md` Deviation 2.
- `unsafe_code` forbidden at bin entry; no `unwrap`/`expect` in production paths (tests legitimately use them behind `#![allow]`).

## Deviations Between SUMMARY Claims and Codebase

None. Every file claimed in the three SUMMARY frontmatters exists on disk; every asserted test passes; live smoke matches claimed stdout/stderr byte-for-byte (pubkey value differs as expected since each run generates a fresh seed).

## Deferred to Later Phases (Not Gaps)

The following are explicitly narrowed/deferred per CONTEXT D-12, D-14, and the plan text — they are NOT gaps:

- `config.toml` `principal` and `inbox_path` fields — land when Phase 2/3 first read them.
- `peers.toml` populated `PeerEntry` fields — land in Phase 3 via `famp peer add` (CLI-06).
- Wrong-permission detection on key files — deferred to Phase 2+ readers that will `O_NOFOLLOW` + stat on open.
- Symlink-attack hardening in `init_refuses` — deferred per Plan 03 epistemic limits section.

## Human Verification Required

None. Phase 1 produces no user-visible UI, no real-time behavior, and no external-service integration. Every success criterion is programmatically testable and is covered by automated tests + one live smoke run executed during this verification.

## Gaps

None.

## Final Verdict

**PASS.** All 8 requirements (CLI-01, CLI-07, IDENT-01..06) satisfied with in-repo evidence. All 5 ROADMAP success criteria verified. 284/284 workspace tests green (31 net added), clippy clean under `-D warnings`, `cargo tree -i openssl` empty, live `famp init` smoke test produces byte-exact expected output with correct modes. Phase goal achieved.

---
*Verified 2026-04-14 by gsd-verifier (Claude Opus 4.6, 1M context).*

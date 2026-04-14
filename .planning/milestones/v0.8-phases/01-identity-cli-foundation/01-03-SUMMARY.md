---
phase: 01-identity-cli-foundation
plan: 03
subsystem: famp-cli
tags: [cli, init, identity, integration-tests, tls, leak-scan, compile-fail, ident-05]
requires:
  - famp::cli::init::run_at / run / InitOutcome (01-02)
  - famp::cli::init::{tls, atomic} helpers (01-02)
  - famp::cli::{CliError, InitArgs, IdentityLayout} (01-01)
  - famp_transport_http::tls::{load_pem_cert, load_pem_key, build_server_config} (v0.7)
provides:
  - famp::cli::init::load_identity(&Path) -> Result<IdentityLayout, CliError>
  - Six integration test binaries under crates/famp/tests/
  - compile_fail doc-test on famp_crypto::FampSigningKey (D-17 mechanism #1)
affects:
  - crates/famp/src/cli/init/mod.rs (added load_identity + 3 unit tests)
  - crates/famp-crypto/src/keys.rs (added module-level doc-test; no impl changes)
tech-stack:
  added: []
  patterns:
    - Rust-API test route (CD-05) — &mut Vec<u8> stdout/stderr, zero subprocess
    - Serial env-var test isolated in its own integration-test binary
    - 8-byte sliding-window substring scan for key-material leakage
    - compile_fail rustdoc block as a structural forcing function
key-files:
  created:
    - crates/famp/tests/init_happy_path.rs
    - crates/famp/tests/init_force.rs
    - crates/famp/tests/init_refuses.rs
    - crates/famp/tests/init_identity_incomplete.rs
    - crates/famp/tests/init_no_leak.rs
    - crates/famp/tests/init_home_env.rs
  modified:
    - crates/famp/src/cli/init/mod.rs
    - crates/famp-crypto/src/keys.rs
decisions:
  - "load_identity reports the FIRST missing file (deterministic order from IdentityLayout::entries) so Phase 2 subcommands get a single actionable path"
  - "All integration tests route through famp::cli::init::run_at with captured Vec<u8> handles — no process env mutation except init_home_env (CD-05, Pitfall 1)"
  - "init_home_env keeps scope minimal (env-routing reaches run_at and populates FAMP_HOME); D-15 byte assertions live in init_happy_path"
  - "compile_fail doc-test lives on the FampSigningKey struct itself in famp-crypto, so cargo test -p famp-crypto --doc picks it up without new harness code"
metrics:
  duration: "~10min"
  tasks_completed: 2
  files_touched: 8
  tests_added: 11   # 3 unit (load_identity_tests) + 2 doc (famp-crypto) + 8 integration test fns across 6 bins (init_happy_path has 2)
  commits: 2
completed: 2026-04-14
---

# Phase 01 Plan 03: Phase 1 Verification Tests Summary

**One-liner:** Locks every Phase 1 ROADMAP success criterion with a named integration test (six new test binaries), adds the Phase 2-facing `load_identity` read path, and installs a `compile_fail` doc-test that structurally forbids `Display` on `FampSigningKey`.

## What Was Built

### `load_identity` — Phase 2 read path (IDENT-05 Phase 1 slice)

Added `pub fn load_identity(home: &Path) -> Result<IdentityLayout, CliError>` in `crates/famp/src/cli/init/mod.rs`. Walks `IdentityLayout::at(home).entries()` and returns `Err(CliError::IdentityIncomplete { missing })` on the first missing file. Rejects relative paths with `HomeNotAbsolute`. Three unit tests under `cli::init::load_identity_tests` cover the happy path, first-missing, and relative-path branches.

### `compile_fail` doc-test — D-17 mechanism #1

Added a module-level rustdoc block on `famp_crypto::FampSigningKey` with two doc-tests:

1. A runtime doc-test asserting `format!("{:?}", sk)` contains `"redacted"` and does NOT contain the digit `'7'` when the seed is `[7u8; 32]` (Debug redaction is honored).
2. A ```` ```compile_fail ```` block that tries `format!("{}", sk)` — this must fail to compile, which is what the doc-test harness treats as pass. If anyone ever adds a `Display` impl, the doc-test breaks the build.

No impl changes to `famp-crypto` — the doc block is additive and sits above the existing struct definition.

### Six integration tests under `crates/famp/tests/`

Each file is its own integration-test binary (separate compile unit, separate process under nextest). Every test except `init_home_env` routes through `famp::cli::init::run_at(&home, force, &mut out, &mut err)` — the CD-05 Rust-API route — so stdout/stderr are captured into `Vec<u8>` without touching `std::io::stdout` or process env.

| File | Covers | Asserts |
|---|---|---|
| `init_happy_path.rs` | CLI-01 happy path, IDENT-01, IDENT-02, narrowed IDENT-03/04 | 6 files exist, byte lengths for `key.ed25519`/`pub.ed25519` = 32, unix modes 0600/0644, exact `listen_addr = "127.0.0.1:8443"\n` body, zero-byte `peers.toml`, D-15 stdout (pubkey, unpadded b64url, one line) and stderr (`initialized FAMP home at <abs>\n`); second test round-trips the generated PEMs through `famp_transport_http::tls::{load_pem_cert, load_pem_key, build_server_config}` as the cross-phase conformance gate. |
| `init_force.rs` | CLI-01 `--force` | Second `run_at(..., force=true)` produces a DIFFERENT pubkey and key file than the first run; all six files present after force |
| `init_refuses.rs` | CLI-01 refuse | Non-empty `FAMP_HOME` + `force=false` returns `CliError::AlreadyInitialized` with `existing_files` listing `key.ed25519`; stale file bytes untouched |
| `init_identity_incomplete.rs` | IDENT-05 Phase 1 | After init + removing `tls.key.pem`, `load_identity` returns `IdentityIncomplete { missing }` ending in `tls.key.pem`; `load_identity("relative/path")` returns `HomeNotAbsolute` |
| `init_no_leak.rs` | IDENT-06 / D-17 #3 | Reads back the 32-byte seed from disk, scans concatenated stdout+stderr for any 8-byte sliding window, asserts zero matches. Epistemic limit documented inline (RESEARCH Pitfall 8). |
| `init_home_env.rs` | CLI-07 | Sets `FAMP_HOME` via `std::env::set_var` (edition 2021: safe fn, no `unsafe` block), calls `famp::cli::init::run(args)`, asserts outcome home equals env path and `key.ed25519` exists. Single test in its own binary = serial by construction (Pitfall 1). |

### Test-to-requirement map

| Requirement | Test locking it |
|---|---|
| CLI-01 (happy path, --force, refusal) | `init_happy_path::init_creates_all_six_files_with_correct_modes`, `init_force::force_atomically_replaces_existing_home`, `init_refuses::refuses_non_empty_without_force` |
| CLI-07 (FAMP_HOME env override) | `init_home_env::famp_home_env_var_overrides_default` |
| IDENT-01 (keys 32 bytes + modes) | `init_happy_path::init_creates_all_six_files_with_correct_modes` |
| IDENT-02 (TLS cross-phase conformance) | `init_happy_path::init_tls_output_loads_via_transport_http` |
| IDENT-05 (IdentityIncomplete loader) | `init_identity_incomplete::load_identity_reports_missing_file`, `cli::init::load_identity_tests::*` |
| IDENT-06 (no key bytes in output) | `init_no_leak::init_output_contains_no_8byte_window_of_secret_seed` + `famp_crypto::FampSigningKey` compile_fail doc-test + structural `CliError` audit (inherited from 01-01) |

## Commits

| Task | Commit | Files | Description |
|---|---|---|---|
| 1 | `91766cd` | 2 | `feat(01-03): add load_identity read path and compile_fail doc-test` |
| 2 | `7beb5f3` | 6 | `test(01-03): ship six integration tests locking Phase 1 success criteria` |

## Verification

- `cargo nextest run -p famp cli::init::load_identity_tests` → **3/3 passed**
- `cargo test -p famp-crypto --doc` → **3/3 passed** (includes the `compile_fail` block, which the doc-test harness treats as pass when the body fails to compile)
- `cargo nextest run -p famp --test init_happy_path --test init_force --test init_refuses --test init_identity_incomplete --test init_no_leak --test init_home_env` → **8/8 passed** (init_happy_path contains 2 test fns, the others 1 each)
- `cargo nextest run -p famp` → **39/39 passed** (famp crate total, up from 31 in 01-02)
- `cargo nextest run --workspace` → **284/284 passed** (v0.7's 253 + Phase 1 deltas from 01-01/01-02/01-03), no regressions
- `cargo clippy -p famp --all-targets -- -D warnings` → 0 warnings
- `cargo tree -i openssl` → empty (E2E-03 regression guard holds)
- `grep -q 'unsafe' crates/famp/tests/init_home_env.rs` → FALSE (edition 2021, no `unsafe { }` block)
- `grep -q 'D-15 output bytes' crates/famp/tests/init_home_env.rs` → TRUE (scope comment present)
- `grep -q 'pub fn load_identity' crates/famp/src/cli/init/mod.rs` → TRUE
- `grep -q 'compile_fail' crates/famp-crypto/src/keys.rs` → TRUE
- `grep -q 'D-17 mechanism #1' crates/famp-crypto/src/keys.rs` → TRUE

## Deviations from Plan

### 1. [Rule 1 - Lint] `clippy::doc_markdown` on `FAMP_HOME` / `--force` in test-file module docs

- **Found during:** Task 2 clippy pass
- **Issue:** The workspace `clippy::pedantic` profile (deny-warnings) flags unbacked identifiers like `FAMP_HOME` and `--force` in module-level doc comments. The plan text used those identifiers unbacked in `init_refuses.rs` and `init_home_env.rs`.
- **Fix:** Backticked `FAMP_HOME`, `--force`, `Cargo.toml`, and `run_at` in the two affected module doc comments. No semantic change; the test bodies are unchanged.
- **Files modified:** `crates/famp/tests/init_refuses.rs`, `crates/famp/tests/init_home_env.rs`
- **Commit:** `7beb5f3` (fix was applied before commit)

### 2. [Rule 3 - Blocking] `unused_crate_dependencies` on new integration-test compile units

- **Found during:** Task 2 first clippy run (anticipated from 01-01 / 01-02 precedent)
- **Issue:** Every integration test file under `crates/famp/tests/` is its own compile unit. The `famp` crate has ~17 runtime deps visible to each test binary, and the workspace `unused_crate_dependencies = warn` (escalated to deny by `-D warnings`) fires on every dep the test binary does not explicitly `use`. The existing tests (`runtime_unit.rs` etc.) solve this with `#![allow(unused_crate_dependencies)]` at the crate root of each test binary.
- **Fix:** Added `#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]` to all six new test files — matching the precedent in `runtime_unit.rs`. This is a lint concession, not a relaxation of production code; integration tests legitimately use `.unwrap()` and `.expect()` for assertion ergonomics.
- **Files modified:** all six new test files
- **Commit:** `7beb5f3`

### 3. [Rule 2 - Test hygiene] Added a third `load_identity` unit test

- **Found during:** Task 1 write
- **Issue:** The plan specified two unit tests for `load_identity` (happy + first-missing). Adding a third test for the relative-path rejection branch cost ~5 lines and locks the `HomeNotAbsolute` early-return — the same guard the plan emphasizes for `init`'s home-resolution path.
- **Fix:** Added `load_identity_rejects_relative_home` inside `cli::init::load_identity_tests`. This is additive only; no interface change. (Note: `init_identity_incomplete.rs` integration test covers the same branch from the public-API side as a defense-in-depth duplicate.)
- **Files modified:** `crates/famp/src/cli/init/mod.rs`
- **Commit:** `91766cd`

## Epistemic Limits & Deferred Hardening

- **`init_no_leak.rs` false-negative window.** An 8-byte sliding-window scan cannot distinguish a legitimately leaked run of bytes from a coincidental collision over a 32-byte high-entropy seed. We accept this as defense-in-depth alongside D-17 mechanisms #1 (compile_fail doc-test, this plan) and #2 (structural `CliError` variant audit, 01-01). A stronger proof would require byte-level taint tracking, which is out of scope for Phase 1.
- **Symlink attack on pre-existing state.** `init_refuses.rs` seeds a plain file, not a symlink. `perms::write_secret` uses `O_EXCL` which already rejects symlinks at the target path, but the behavior is not tested here. Deferred to a future hardening pass (see `<threat_model>` T-1-02).
- **Permission-check in `load_identity`.** This slice only checks file existence. Wrong-permissions detection (e.g. someone `chmod 0644 key.ed25519`) is deferred to the Phase 2+ code that reads the key material — at which point opening with `O_NOFOLLOW` and stat'ing the result is a single `match metadata.permissions().mode() & 0o777`.
- **Phase 1 edition is 2021.** `std::env::set_var` / `remove_var` are safe fns in `init_home_env.rs`. If the workspace is ever bumped to edition 2024, wrap the two calls in `unsafe { }` — the test scope comment already flags this.
- **IDENT-03 / IDENT-04 narrowing.** `config.toml` ships one field (`listen_addr`); `peers.toml` ships empty. `principal` / `inbox_path` / peer fields land in Phase 2 / 3 per CONTEXT.md D-12/D-14. `init_happy_path` asserts the narrowed byte layout.

## Known Stubs

None. `load_identity` is a complete, test-covered function on day one — Phase 2 subcommands can call it verbatim.

## Threat Flags

None. No new network endpoints, auth paths, or schema changes at trust boundaries. The `init_tls_output_loads_via_transport_http` cross-phase gate exercises `famp-transport-http` loaders against local files only — there is no socket bind and no client/server handshake in the test.

## `cargo tree -i openssl` — E2E-03 regression guard

```
$ cargo tree -i openssl
error: package ID specification `openssl` did not match any packages
```

Empty (the `error` line is cargo's standard "no match" message). Guard holds: Phase 1 added no dep that pulls OpenSSL transitively.

## Self-Check: PASSED

Files verified to exist:
- FOUND: crates/famp/tests/init_happy_path.rs
- FOUND: crates/famp/tests/init_force.rs
- FOUND: crates/famp/tests/init_refuses.rs
- FOUND: crates/famp/tests/init_identity_incomplete.rs
- FOUND: crates/famp/tests/init_no_leak.rs
- FOUND: crates/famp/tests/init_home_env.rs
- FOUND: crates/famp/src/cli/init/mod.rs (load_identity added)
- FOUND: crates/famp-crypto/src/keys.rs (compile_fail doc-test added)

Commits verified via `git log --oneline`:
- FOUND: 91766cd feat(01-03): add load_identity read path and compile_fail doc-test
- FOUND: 7beb5f3 test(01-03): ship six integration tests locking Phase 1 success criteria

Test results verified:
- `cargo nextest run --workspace` → 284/284 passed, 1 skipped (no regressions vs v0.7's 253)
- `cargo test -p famp-crypto --doc` → 3/3 doc tests passed (includes compile_fail block)
- `cargo clippy -p famp --all-targets -- -D warnings` → 0 warnings
- `cargo tree -i openssl` → empty

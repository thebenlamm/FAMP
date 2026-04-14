---
phase: 01-identity-cli-foundation
plan: 01
subsystem: famp-cli
tags: [cli, scaffolding, identity, toml, clap]
requires: []
provides:
  - famp::cli module tree
  - CliError enum (11 variants, zero key material)
  - resolve_famp_home()
  - IdentityLayout + 6 canonical filename constants
  - Config / Peers / PeerEntry serde types
  - write_secret / write_public (unix, 0600/0644 atomic)
affects:
  - crates/famp/Cargo.toml (clap, toml, rcgen, tempfile, serde promoted to [dependencies])
  - crates/famp/src/lib.rs (pub mod cli)
tech-stack:
  added:
    - clap 4.6 (derive)
    - toml 1.1
  patterns:
    - thiserror typed-error enum with #[source] wrapping
    - serde deny_unknown_fields on all on-disk formats
    - O_EXCL + mode(0o600) + fsync for secret files
key-files:
  created:
    - crates/famp/src/cli/mod.rs
    - crates/famp/src/cli/error.rs
    - crates/famp/src/cli/paths.rs
    - crates/famp/src/cli/perms.rs
    - crates/famp/src/cli/home.rs
    - crates/famp/src/cli/config.rs
  modified:
    - crates/famp/Cargo.toml
    - crates/famp/src/lib.rs
    - crates/famp/examples/cross_machine_two_agents.rs
    - crates/famp/examples/personal_two_agents.rs
    - crates/famp/examples/_gen_fixture_certs.rs
decisions:
  - "D-04: CliError ships all 11 variants upfront; Plan 02 consumes them verbatim"
  - "D-05: Structural exclusion of key material — error variants carry only PathBuf + #[source]"
  - "D-07/D-08: resolve_famp_home reads env once; no canonicalize, no tilde expansion, absolute-only"
  - "D-12: Config is a single listen_addr field; principal/inbox_path deferred to later phases"
  - "D-14: PeerEntry is an empty struct in Phase 1; fields land in Phase 3 via `famp peer add`"
metrics:
  duration: "702s"
  tasks_completed: 2
  files_touched: 11
  tests_added: 9
  commits: 2
completed: 2026-04-14
---

# Phase 01 Plan 01: famp CLI Module Tree Scaffolding Summary

**One-liner:** Scaffolds the `famp::cli` module tree — typed errors, `FAMP_HOME` resolver, TOML config/peers types, and an atomic 0600 secret-file writer — providing the exact interface Plan 02's init implementation will consume.

## What Was Built

This is an **interface-first** plan. No subcommand logic; no binary rewrite. Every file compiles, every public type/function that Plan 02 depends on is frozen at the signature level, and every unit-level invariant that can be tested without filesystem orchestration has a test.

### Module tree under `crates/famp/src/cli/`

```
cli/
├── mod.rs      Cli, Commands, InitArgs, pub use CliError, stub run()
├── error.rs    CliError thiserror enum (11 variants)
├── home.rs     resolve_famp_home() -> Result<PathBuf, CliError>
├── paths.rs    6 filename consts + IdentityLayout helper
├── config.rs   Config { listen_addr }, Peers { peers }, PeerEntry {}
└── perms.rs    #[cfg(unix)] write_secret (0600) / write_public (0644)
```

### Exact `CliError` variant list (11)

1. `HomeNotSet`
2. `HomeNotAbsolute { path: PathBuf }`
3. `HomeHasNoParent { path: PathBuf }`
4. `HomeCreateFailed { path: PathBuf, #[source] source: io::Error }`
5. `AlreadyInitialized { existing_files: Vec<PathBuf> }`
6. `IdentityIncomplete { missing: PathBuf }`
7. `KeygenFailed(#[source] Box<dyn Error + Send + Sync>)`
8. `CertgenFailed(#[source] rcgen::Error)`
9. `Io { path: PathBuf, #[source] source: io::Error }`
10. `TomlSerialize(#[source] toml::ser::Error)`
11. `TomlParse { path: PathBuf, #[source] source: toml::de::Error }`

D-05 structural check: `grep -E '\[u8|FampSigningKey' crates/famp/src/cli/error.rs` only matches the **doc comment** explaining the exclusion; no variant embeds key material.

### Tests (9 passing)

| Test | Asserts |
|---|---|
| `cli::perms::tests::write_secret_is_0600` | mode 0600 + content round-trip |
| `cli::perms::tests::write_public_is_0644` | mode 0644 + content round-trip |
| `cli::perms::tests::write_secret_refuses_existing` | `O_EXCL` semantics — `ErrorKind::AlreadyExists` |
| `cli::home::tests::resolves_and_rejects` | 4 cases: FAMP_HOME abs / FAMP_HOME relative / HOME fallback / both unset |
| `cli::config::tests::config_default_serializes_one_field` | byte-exact `listen_addr = "127.0.0.1:8443"\n` |
| `cli::config::tests::config_roundtrip` | serialize → parse → SocketAddr equality |
| `cli::config::tests::config_rejects_unknown_fields` | D-13 enforcement |
| `cli::config::tests::peers_empty_file_loads_empty` | D-14 — zero-byte `peers.toml` → `Peers { peers: [] }` |
| `cli::config::tests::peers_rejects_unknown_fields` | D-13 enforcement |

## Commits

| Task | Commit | Files | Description |
|---|---|---|---|
| 1 | `1486914` | 11 | Scaffold cli module tree; add clap/toml deps; secure write helpers |
| 2 | `b2236e6` | 6 | Implement resolve_famp_home + Config/Peers serde types with round-trip tests |

## Verification

- `cargo clippy -p famp --all-targets -- -D warnings` — clean (0 warnings)
- `cargo nextest run -p famp --lib` — **9/9 passed**
- `cargo tree -i openssl` — still empty (E2E-03 regression guard holds)
- D-05 grep: zero `[u8` or `FampSigningKey` tokens in code (only a doc comment)

## Deviations from Plan

Two auto-fixes required under Rules 2 and 3 (missing critical functionality / blocking issues), both forced by the workspace lint profile (`unused_crate_dependencies = warn` escalated to deny via `-D warnings` plus `clippy::pedantic` deny).

### 1. [Rule 3 - Blocking] serde was not in famp's `[dependencies]`

- **Found during:** Task 2
- **Issue:** The plan's config.rs requires `#[derive(serde::Deserialize, serde::Serialize)]`, but `crates/famp/Cargo.toml` previously carried only `serde_json` transitively. `toml::from_str::<Config>` failed with E0277 "the trait `Deserialize` is not implemented".
- **Fix:** Added `serde = { workspace = true }` to `[dependencies]`.
- **Files modified:** `crates/famp/Cargo.toml`
- **Commit:** `b2236e6`

### 2. [Rule 3 - Blocking] Promoting rcgen/tempfile broke example `unused_crate_dependencies`

- **Found during:** Task 1 clippy pass
- **Issue:** Moving `rcgen`, `tempfile`, `clap`, `toml`, and (later) `serde` from `[dev-dependencies]` to `[dependencies]` made them visible to every example binary. The three existing examples (`cross_machine_two_agents`, `personal_two_agents`, `_gen_fixture_certs`) do not reference them, so the workspace `unused_crate_dependencies` lint fired on each example compile unit.
- **Fix:** Added `use clap as _; use toml as _; use serde as _;` silencer stanzas to each of the three example files, matching the existing project pattern (which already silences `axum`, `reqwest`, etc. the same way).
- **Files modified:** `crates/famp/examples/cross_machine_two_agents.rs`, `crates/famp/examples/personal_two_agents.rs`, `crates/famp/examples/_gen_fixture_certs.rs`
- **Commit:** `1486914` (clap/toml) and `b2236e6` (serde)

### 3. [Rule 2 - Missing correctness] Pedantic-lint annotations on stub functions

- **Found during:** Task 1 clippy pass
- **Issue:** The plan-specified stub `pub fn run(_cli: Cli) -> Result<(), CliError>` trips `clippy::missing_const_for_fn` and `clippy::needless_pass_by_value`. Because the body is a deliberate placeholder that Plan 02 will replace, adjusting the signature to placate clippy would change the public API Plan 02 depends on.
- **Fix:** Added a targeted `#[allow(clippy::missing_const_for_fn, clippy::needless_pass_by_value)]` on `run` and `#[allow(clippy::missing_const_for_fn)]` on the Task 2 resolver's temporary form. (The resolver ended up non-const once the real body landed, so only `run`'s allow persists.)
- **Files modified:** `crates/famp/src/cli/mod.rs`
- **Commit:** `1486914`

### 4. [Rule 1 - Pattern cleanup] `match` → `if let`, IP literal → `Ipv4Addr::LOCALHOST`, doc backticks

- **Found during:** Task 2 clippy pass
- **Issue:** `clippy::single_match_else` on `resolve_famp_home`'s env match; `clippy::lossy_float_literal`/`clippy::hand_coded_well_known_ip` on `Ipv4Addr::new(127, 0, 0, 1)`; several `doc_markdown` misses on uppercase identifiers in doc comments (`FAMP_HOME`, `CONTEXT.md`, field names).
- **Fix:** Converted the match to `if let ... else`; switched to `Ipv4Addr::LOCALHOST`; backticked every bare identifier in doc comments.
- **Files modified:** `crates/famp/src/cli/home.rs`, `crates/famp/src/cli/config.rs`, `crates/famp/src/cli/paths.rs`, `crates/famp/src/cli/mod.rs`
- **Commits:** `1486914`, `b2236e6`

None of these deviations touched the `<interfaces>` block Plan 02 depends on — `resolve_famp_home`'s signature, every `CliError` variant, `IdentityLayout::{at, entries}`, `Config`/`Peers`/`PeerEntry`, and `write_secret`/`write_public` match the plan verbatim.

## Known Stubs

| Stub | File | Reason / Resolution |
|---|---|---|
| `cli::run` returns `Err(CliError::HomeNotSet)` unconditionally | `crates/famp/src/cli/mod.rs` | Intentional. Plan 02 replaces the body with `init::run_at(...)` dispatch. The stub is `#[allow(clippy::missing_const_for_fn, clippy::needless_pass_by_value)]`-annotated because its signature is fixed by the plan interface contract. |

## Threat Flags

None. No new network endpoints, auth paths, or schema changes at trust boundaries — this plan is pure types + pure functions + one local-filesystem helper, all within trust boundaries already enumerated in the plan's `<threat_model>`.

## Self-Check: PASSED

Files verified to exist:
- FOUND: crates/famp/src/cli/mod.rs
- FOUND: crates/famp/src/cli/error.rs
- FOUND: crates/famp/src/cli/paths.rs
- FOUND: crates/famp/src/cli/perms.rs
- FOUND: crates/famp/src/cli/home.rs
- FOUND: crates/famp/src/cli/config.rs

Commits verified:
- FOUND: 1486914 (Task 1)
- FOUND: b2236e6 (Task 2)

Tests verified: 9/9 passed under `cargo nextest run -p famp --lib`.
Lints verified: `cargo clippy -p famp --all-targets -- -D warnings` exits 0.

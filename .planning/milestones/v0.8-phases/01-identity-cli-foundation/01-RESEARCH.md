# Phase 1: Identity & CLI Foundation — Research

**Researched:** 2026-04-14
**Domain:** Rust CLI scaffolding, on-disk identity materialization, rcgen self-signed TLS
**Confidence:** HIGH on stack, HIGH on reuse paths, MEDIUM only on the TLS-key-algorithm gray-area (deferred decision §TLS Cert Parameters)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**CLI Framework & Code Layout**
- **D-01:** CLI parsing uses **`clap` derive**. No hand-rolled parser, no `argh`. Derive macros on a `Cli` struct + `Subcommand` enum in `crates/famp/src/cli/mod.rs`.
- **D-02:** Subcommand implementations live in the lib crate under a new `pub mod cli` in `crates/famp/src/lib.rs` (e.g. `famp::cli::init::run(home: &Path) -> Result<InitOutcome, CliError>`). The binary at `crates/famp/src/bin/famp.rs` stays ~20 lines: parse clap args, call `famp::cli::run(args)`, map the returned `Result` to a process exit code.
- **D-03:** `main()` → `run()` → subcommand. `main()` is a trivial wrapper that calls `famp::cli::run(Cli::parse())` and converts `Result<(), CliError>` to an exit code via `eprintln!("{e}")` + `std::process::exit(1)`.
- **D-04:** Typed `CliError` enum in `crates/famp/src/cli/error.rs` using `thiserror::Error`. No `anyhow` in the binary path. Variants (non-exhaustive): `HomeNotAbsolute`, `HomeCreateFailed`, `AlreadyInitialized { existing_files }`, `IdentityIncomplete { missing }`, `KeygenFailed`, `CertgenFailed`, `Io`, `TomlSerialize`.
- **D-05:** `CliError` carries **no private key material** in any variant. `#[source]` chain is the only error plumbing; no `format!` of key bytes into `Display`.

**Identity Directory Layout & FAMP_HOME Resolution**
- **D-06:** Flat directory layout. Six entries directly under FAMP_HOME:
  - `key.ed25519` — raw 32-byte private key, mode 0600
  - `pub.ed25519` — raw 32-byte public key, mode 0644
  - `tls.cert.pem` — self-signed cert, mode 0644
  - `tls.key.pem` — cert private key in PEM, mode 0600
  - `config.toml` — mode 0644
  - `peers.toml` — empty file on init, mode 0644
- **D-07:** FAMP_HOME resolution: `$FAMP_HOME` verbatim → else `$HOME/.famp`. No XDG. No tilde expansion.
- **D-08:** Absolute paths only. Relative FAMP_HOME → `CliError::HomeNotAbsolute`.
- **D-09:** `init` creates the directory if missing, mode 0700. Parent must exist (no `mkdir -p`).
- **D-10:** `init` refuses any non-empty FAMP_HOME without `--force`. `--force` wipes and rewrites **atomically**: write all six files into `tempfile::TempDir::new_in(parent)`, rename-swap the tempdir over the target, delete the old directory.
- **D-11:** Non-init subcommands (Phase 2+) return `CliError::IdentityIncomplete { missing }` on partial state.

**Config & Peers File Contents**
- **D-12:** `config.toml` contains **strictly one field** in Phase 1: `listen_addr = "127.0.0.1:8443"`.
- **D-13:** Config struct uses `#[serde(deny_unknown_fields)]`.
- **D-14:** `peers.toml` on init is a **zero-byte file**. Struct also `#[serde(deny_unknown_fields)]`.

**First-Run UX & Output Discipline**
- **D-15:** On success, `famp init` writes two lines total:
  - **stdout (one line):** newly generated public key, base64url-unpadded, same format `famp-keyring` uses, `\n`.
  - **stderr (one line):** `initialized FAMP home at <absolute path>\n`. No banners.
- **D-16:** On failure, `thiserror::Display` of `CliError` to stderr, exit non-zero. No colors/unicode.
- **D-17:** Private-key leakage defense — three stacked mechanisms:
  1. Verify (don't add) that `FampSigningKey` has no `Display` and no byte-printing `Debug` — Phase 1 adds a compile-time check / doc-test.
  2. No `CliError` variant embeds key bytes.
  3. Integration test `tests/init_no_leak.rs` runs `famp::cli::init::run` against a tempdir, reads `key.ed25519` back, asserts no 8+ byte substring appears in captured stdout/stderr.
- **D-18:** **No `zeroize-on-drop` work in Phase 1.** Orthogonal threat model.

### Claude's Discretion
- **CD-01:** Exact layout of `cli` module tree — keep each file <200 lines.
- **CD-02:** Whether `InitOutcome` is struct / enum / `()`.
- **CD-03:** Whether `--force` is a top-level or subcommand-scoped arg.
- **CD-04:** Exact `toml` crate choice (`toml` vs `toml_edit` vs `basic-toml`) — must honor `deny_unknown_fields`.
- **CD-05:** Whether the integration test uses `std::env::set_var` or threads FAMP_HOME through the Rust API.

### Deferred Ideas (OUT OF SCOPE)
- **TLS cert parameters (Gray Area 3)** — key algorithm (Ed25519 vs ECDSA P-256 vs RSA-2048), SANs, CN, validity window, serial. Researched below; planner picks in PLAN.md.
- Richer error UX (colors, hints, "Did you mean...").
- `XDG_CONFIG_HOME` compliance.
- Windows path/mode handling — Phase 1 is Unix-only in practice.
- `zeroize-on-drop` for in-memory key material.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CLI-01 | `famp init` creates `~/.famp/` with fresh Ed25519 keypair, self-signed TLS cert+key, default `config.toml`, empty `peers.toml`; refuses overwrite without `--force` | §Reuse: `FampSigningKey::from_bytes` + CSPRNG seed; rcgen 0.14 `generate_simple_self_signed` already used in v0.7 examples; `tempfile::TempDir::new_in` for atomic `--force` rewrite |
| CLI-07 | `FAMP_HOME` env var overrides `~/.famp/` for every subcommand | §FAMP_HOME Resolution — simple `std::env::var("FAMP_HOME")` + `home::home_dir()` or `dirs::home_dir()` fallback |
| IDENT-01 | `key.ed25519` (32-byte secret, 0600) and `pub.ed25519` (32-byte public) in raw bytes | §File Permissions — `OpenOptionsExt::mode(0o600)` + tempfile-then-rename pattern |
| IDENT-02 | `tls.cert.pem` + `tls.key.pem` self-signed via `rcgen`, CN = configured principal name | §TLS Cert Parameters — rcgen 0.14 `CertificateParams` for fine CN control; `generate_simple_self_signed` for quick start. **Note tension:** D-12 narrows `config.toml` to `listen_addr` only, so Phase 1 has no "configured principal name" yet — CN falls back to a placeholder per deferred decisions |
| IDENT-03 | `config.toml` with `principal`, `listen_addr`, `inbox_path` | **TENSION** — D-12 narrows this to **only** `listen_addr = "127.0.0.1:8443"` in Phase 1; `principal` and `inbox_path` are not emitted by `init` and will be added by the phase that first consumes them. IDENT-03's full schema is a v0.8 milestone-level commitment, not a Phase 1 commitment. Planner should document this narrowing in PLAN.md. |
| IDENT-04 | `peers.toml` holds an array of peer entries; readable by `famp-keyring` via adapter | D-14 narrows Phase 1 to a **zero-byte file**. The adapter / array shape lands in Phase 3 when `famp peer add` ships. Phase 1 only verifies the empty-file deserializes to the empty-peers representation. |
| IDENT-05 | On startup, every subcommand loads identity + config from `$FAMP_HOME` and fails closed on missing/malformed/wrong-perms | Phase 1 ships the loader but only `init` exercises it (write path). Phase 2+ subcommands reuse it on read path. Scope for Phase 1: `CliError::IdentityIncomplete { missing: PathBuf }` variant + a loader function that checks all six files exist. Permissions-check ("wrong perms") is deferred to the phase that reads identity — noted below. |
| IDENT-06 | No private key material ever logged, printed to stdout, returned from MCP tool | §Leakage Defense — three stacked mechanisms per D-17, including `tests/init_no_leak.rs` substring scan |
</phase_requirements>

---

## Summary

Phase 1 is a pure composition phase. Every load-bearing primitive it needs already exists in the v0.7 substrate:

- **Keygen:** `famp_crypto::FampSigningKey::from_bytes([u8; 32])` with a CSPRNG seed. The newtype already has a **redacted `Debug`** (`FampSigningKey(<redacted>)`) and **no `Display`** — D-17 mechanism #1 is already satisfied in `crates/famp-crypto/src/keys.rs`; Phase 1's job is to *lock it in* with a test, not to add it.
- **Public-key base64url encoding:** `TrustedVerifyingKey::to_b64url()` emits `URL_SAFE_NO_PAD` encoding of the 32-byte key — this is the exact format `famp-keyring`'s file format stores pubkeys in (`crates/famp-keyring/src/file_format.rs`), which is what D-15's stdout line is referring to.
- **Self-signed cert generation:** `rcgen 0.14.7` is already a workspace dev-dep; `crates/famp/examples/cross_machine_two_agents.rs` at line 186 already uses `rcgen::generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()])` returning a `CertifiedKey { cert, signing_key }` with `cert.pem()` + `signing_key.serialize_pem()`. Phase 1 lifts this pattern into `famp::cli::init`.
- **TLS PEM loading:** `famp_transport_http::tls::{load_pem_cert, load_pem_key, build_server_config}` already know how to consume a PEM cert+key pair into a `rustls::ServerConfig`. Phase 1's init output *must* satisfy these loaders without modification — this is the cross-phase conformance gate.

The only genuine research tasks are: (a) `clap` 4.x derive shape for a subcommand-dispatching binary, (b) the `toml` crate choice under `deny_unknown_fields`, (c) `FAMP_HOME` resolution + absolute-path check idioms, (d) atomic directory rewrite via `tempfile`, and (e) the deferred TLS-cert-parameters gray area (key algorithm, SANs, validity window).

**Primary recommendation:** `clap 4.6.0` derive + `toml 1.1.2` with `deny_unknown_fields` + `dirs 6.0.0` for `$HOME` fallback + keep `rcgen::generate_simple_self_signed(vec!["localhost", "127.0.0.1", "::1"])` for consistency with v0.7. Self-signed cert uses rcgen's **default key algorithm (ECDSA P-256)** — see §TLS Cert Parameters for the deferred-decision rationale.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| clap parsing + subcommand dispatch | `famp` binary (`src/bin/famp.rs`) | — | Thin wrapper (~20 lines) — parses `Cli`, calls `famp::cli::run(args)`, maps `Result` to exit code (D-02/D-03). |
| Subcommand logic (`init`, later `listen`/`send`/...) | `famp` lib crate (`src/lib.rs::cli::*`) | — | D-02: lib-hosted so integration tests call `famp::cli::init::run` directly without `assert_cmd` subprocess overhead. Every Phase 2–4 subcommand lands as another module here. |
| Ed25519 keygen | `famp-crypto` | — | Already exists. `FampSigningKey::from_bytes` + `rand::rngs::OsRng` fill-32-bytes is the entry point. No new crypto. |
| Principal / pubkey base64url encoding | `famp-crypto` (`TrustedVerifyingKey::to_b64url`) | `famp-keyring` file format reuses it | `URL_SAFE_NO_PAD` over 32 raw bytes. Phase 1 stdout line calls this function directly. |
| Self-signed TLS cert generation | `famp` lib crate (`cli::init::tls`) or a tiny helper | `rcgen` 0.14.7 | rcgen is a one-shot utility; no new home crate needed. The generated PEMs are consumed by `famp-transport-http::tls::load_pem_cert/key` in Phase 2. |
| `FAMP_HOME` resolution | `famp` lib crate (`cli::home::resolve`) | `dirs` crate or `std::env::var("HOME")` | Single small module — env lookup, absolute-path check, join with `.famp`. |
| File permissions (0600/0644/0700) | Unix-only module guarded by `#[cfg(unix)]` | `std::os::unix::fs::{OpenOptionsExt, PermissionsExt}` | Phase 1 is Unix-only in practice; the planner documents Windows as "not supported in v0.8". |
| Atomic directory replace (`--force`) | `famp` lib crate (`cli::init::atomic`) | `tempfile::TempDir::new_in` | Write all six files into a sibling tempdir on the same filesystem, `rename` the old target aside, `rename` the tempdir into place, delete the old target. Same-filesystem rename is POSIX-atomic. |
| `config.toml` / `peers.toml` schema | `famp` lib crate (`cli::config`) | `serde` + `toml` | Tiny structs with `#[serde(deny_unknown_fields)]`. No new crate. |
| Leakage defense test | `famp` lib crate `tests/init_no_leak.rs` | — | Integration test runs `famp::cli::init::run` against a tempdir, reads `key.ed25519` from disk, asserts no 8+ byte substring in captured stdout/stderr. Lives in the lib crate so no subprocess machinery is needed (D-02). |

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `clap` | `4.6.0` | CLI argument parsing (derive macros) | `[VERIFIED: cargo search 2026-04-14]` — de facto standard for Rust CLIs. Derive API is the idiomatic choice for structured subcommand trees. |
| `rcgen` | `0.14.7` | Self-signed X.509 cert + key generation | `[VERIFIED: Cargo.lock already at 0.14.7]` — already used in `crates/famp/examples/cross_machine_two_agents.rs:186`. API surface Phase 1 needs: `generate_simple_self_signed(Vec<String>) -> Result<CertifiedKey, rcgen::Error>`; `CertifiedKey { cert, signing_key }`; `cert.pem() -> String`; `signing_key.serialize_pem() -> String`. |
| `toml` | `1.1.2+spec-1.1.0` | TOML serialize/deserialize for `config.toml` / `peers.toml` | `[VERIFIED: cargo info toml 2026-04-14]` — native Rust, clean serde integration, `deny_unknown_fields` compatible out of the box. `toml_edit` is for comment-preserving edits (wrong tool); `basic-toml` is a stripped-down fork (no reason to use). |
| `tempfile` | `3.27.0` | `TempDir::new_in(parent)` for atomic `--force` rewrite | `[VERIFIED: Cargo.lock]` — already a workspace dev-dep at 3.27.0. Phase 1 promotes it to a regular dep (or keeps it dev-only if `--force` can be test-exercised without production use — unlikely since `--force` is user-facing). |
| `dirs` | `6.0.0` | `dirs::home_dir()` for `$HOME` fallback in FAMP_HOME resolution | `[VERIFIED: cargo search 2026-04-14]` — tiny, cross-platform. Alternative: read `HOME` env var directly with `std::env::var("HOME")` and skip the crate entirely. The planner picks. **Recommendation:** use `std::env::var("HOME")` directly since Phase 1 is explicitly Unix-only (D-07 rules out XDG, tilde expansion is shell's job). Adds zero deps. |
| `rand` | `0.8` (workspace) | CSPRNG seed for `FampSigningKey::from_bytes` | `[VERIFIED: workspace Cargo.toml]` — already present. Use `rand::rngs::OsRng` to fill `[u8; 32]`. |

### Supporting (Already in the Workspace)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `thiserror` | `2.0.18` | `CliError` enum derivation | Every new error variant in `cli/error.rs` |
| `famp-crypto` | path dep | `FampSigningKey` keygen, `TrustedVerifyingKey::to_b64url` | D-15 stdout line, Phase 1 keygen |
| `famp-keyring` | path dep | Round-trip fixture reference (not directly called by Phase 1) | Planner reads its file format to confirm D-15 stdout matches |
| `famp-transport-http` | path dep | Conformance target for the generated TLS cert+key (loaded by `tls::load_pem_cert/key` in Phase 2) | Not called by Phase 1, but Phase 1's output must satisfy this loader byte-for-byte |
| `base64` | `0.22.1` | Only indirectly — `famp-crypto` already uses it for `to_b64url` | Phase 1 should NOT depend on `base64` directly; go through `TrustedVerifyingKey::to_b64url` |

### Alternatives Considered

| Instead of | Could Use | Tradeoff / Why Rejected |
|------------|-----------|--------------------------|
| `clap` derive | `clap` builder | Builder is more flexible for runtime-driven CLIs; derive is more idiomatic for static command trees. Phase 1's subcommand tree is static → derive wins. D-01 locks this. |
| `clap` | `argh` / `pico-args` / `lexopt` | `argh` is Google's minimal fork — fine but non-standard; `pico-args` / `lexopt` require hand-rolling subcommand dispatch. D-01 explicitly rejects. |
| `toml` | `toml_edit` | `toml_edit` preserves comments and formatting on round-trip — useful if we ever need `famp config set X Y` to edit in place. We don't in Phase 1; `toml` is simpler and smaller. If Phase 3/4 adds in-place edit, that phase can swap. |
| `toml` | `basic-toml` | Stripped-down fork by dtolnay. No advantage for Phase 1; fewer features means potentially missing edge cases. |
| `dirs` | `std::env::var("HOME")` | **Recommend this.** `dirs` pulls in `dirs-sys` and platform shims that Phase 1 doesn't need. Unix-only Phase 1 + explicit "no XDG, no tilde" rule (D-07) means the whole `dirs` crate collapses to two `std::env::var` calls. |
| `rcgen::generate_simple_self_signed` | `rcgen::CertificateParams::new(...)` + manual key algorithm selection | Simple form uses rcgen defaults (ECDSA P-256 as of 0.14.x). CertificateParams gives fine control over key algorithm, validity window, serial, CN. **Planner likely wants CertificateParams** to set a finite validity window (D-deferred: 10 years) and an explicit CN, instead of rcgen defaults. See §TLS Cert Parameters. |
| `OsRng` from `rand 0.8` | `rand 0.9` / `getrandom` directly | `ed25519-dalek 2.2.0` pins `rand_core 0.6` API — stay on `rand 0.8` to avoid version mismatch (per CLAUDE.md Version-compatibility notes). |

**Installation commands:**

```bash
# In crates/famp/Cargo.toml [dependencies]:
clap = { version = "4.6", features = ["derive"] }
toml = "1.1"
rcgen = "0.14"       # promote from dev-dependencies to dependencies
tempfile = "3.27"    # promote from dev-dependencies to dependencies
# 'rand' already present via workspace
```

**Version verification (ran 2026-04-14):**
- `clap 4.6.0` — `cargo search clap` → latest stable, derive feature gated under `derive` flag
- `toml 1.1.2+spec-1.1.0` — `cargo info toml` → Rust MSRV 1.85, serde-based
- `rcgen 0.14.7` — already in `Cargo.lock`
- `tempfile 3.27.0` — already in `Cargo.lock`
- `dirs 6.0.0` — `cargo search dirs` → not recommended (see alternatives row)

---

## Architecture Patterns

### System Architecture Diagram

```
  ┌────────────────────────────────────┐
  │  user shell                        │
  │  $ famp init [--force]             │
  └──────────────┬─────────────────────┘
                 │ argv
                 ▼
  ┌────────────────────────────────────┐
  │  crates/famp/src/bin/famp.rs       │     ~20 lines
  │  main() → Cli::parse() →           │
  │   famp::cli::run(args) → exit code │
  └──────────────┬─────────────────────┘
                 │ Cli struct
                 ▼
  ┌────────────────────────────────────┐
  │  famp::cli::run(args)              │     src/lib.rs :: cli::mod.rs
  │   match args.command {             │
  │     Init(a) => cli::init::run(a)   │
  │   }                                │
  └──────────────┬─────────────────────┘
                 │
                 ▼
  ┌─────────────────────────────────────────────────────┐
  │  famp::cli::init::run(args) -> Result<_,CliError>   │
  │                                                      │
  │  1. resolve FAMP_HOME (env or $HOME/.famp)          │
  │  2. assert absolute (else HomeNotAbsolute)          │
  │  3. probe existing state:                            │
  │      - empty / missing  → proceed                    │
  │      - non-empty + !force → AlreadyInitialized       │
  │      - non-empty +  force → atomic replace path      │
  │  4. mkdir target (0700) OR TempDir::new_in(parent)   │
  │  5. keygen:  FampSigningKey::from_bytes(OsRng.gen())│
  │  6. cert:    rcgen::CertificateParams → CertifiedKey │
  │  7. write six files with correct modes:              │
  │      key.ed25519   (0600)  raw 32 bytes              │
  │      pub.ed25519   (0644)  raw 32 bytes              │
  │      tls.cert.pem  (0644)  cert.pem()                │
  │      tls.key.pem   (0600)  signing_key.serialize_pem │
  │      config.toml   (0644)  toml::to_string(&Config)  │
  │      peers.toml    (0644)  "" (zero bytes)           │
  │  8. [--force path] rename tempdir over target        │
  │  9. return InitOutcome { pubkey_b64 }                │
  └──────────────┬──────────────────────────────────────┘
                 │
                 ▼
  ┌────────────────────────────────────┐
  │  back in main()                    │
  │  stdout: "{pubkey_b64}\n"          │
  │  stderr: "initialized FAMP home    │
  │          at {absolute_path}\n"     │
  │  exit 0                            │
  └────────────────────────────────────┘

  Side channel — leakage test:
  ┌────────────────────────────────────┐
  │  tests/init_no_leak.rs             │
  │  - TempDir FAMP_HOME                │
  │  - call famp::cli::init::run        │
  │  - read key.ed25519 from disk       │
  │  - scan captured stdout+stderr for  │
  │    any 8-byte substring of secret   │
  │  - fail if found                    │
  └────────────────────────────────────┘
```

### Recommended Project Structure

```
crates/famp/
├── Cargo.toml               # +clap, +toml, promote rcgen/tempfile
├── src/
│   ├── bin/
│   │   └── famp.rs          # REWRITTEN — ~20-line main()
│   ├── lib.rs               # +pub mod cli;
│   ├── runtime/             # existing (untouched)
│   └── cli/                 # NEW
│       ├── mod.rs           # Cli struct, Subcommand enum, pub fn run(args)
│       ├── error.rs         # CliError enum (thiserror)
│       ├── home.rs          # FAMP_HOME resolution + absolute-path check
│       ├── paths.rs         # the six canonical file names (consts)
│       ├── config.rs        # Config struct (listen_addr), Peers struct
│       ├── perms.rs         # #[cfg(unix)] set_mode helpers
│       └── init/
│           ├── mod.rs       # pub fn run(args) -> Result<InitOutcome, CliError>
│           ├── atomic.rs    # TempDir-then-rename for --force
│           └── tls.rs       # rcgen wrapper
└── tests/
    ├── init_happy_path.rs   # default + FAMP_HOME override
    ├── init_force.rs        # --force atomic replace
    ├── init_refuses.rs      # non-empty without --force
    ├── init_identity_incomplete.rs  # partial state → IdentityIncomplete
    └── init_no_leak.rs      # D-17 mechanism #3 — substring scan
```

**Size discipline (CD-01):** each file < 200 lines. The split above keeps `init/mod.rs` around 120 lines and delegates cert, atomic rewrite, and permission setting to siblings.

### Pattern 1: clap 4 derive subcommand dispatch

```rust
// Source: clap 4.x derive docs (docs.rs/clap/latest/clap/_derive/index.html)
// [CITED: clap 4.x derive tutorial]
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "famp", version, about = "FAMP v0.5.1 reference CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a FAMP home directory.
    Init(InitArgs),
    // Phase 2: Listen(ListenArgs),
    // Phase 3: Send, Await, Inbox, Peer
    // Phase 4: Mcp
}

#[derive(clap::Args)]
pub struct InitArgs {
    /// Overwrite an existing FAMP home (atomic replace).
    #[arg(long)]
    pub force: bool,
}

pub fn run(cli: Cli) -> Result<(), crate::cli::error::CliError> {
    match cli.command {
        Commands::Init(a) => crate::cli::init::run(a).map(|_| ()),
    }
}
```

**Binary (bin/famp.rs):**
```rust
// Source: standard Rust bin pattern + D-03
fn main() {
    let cli = <famp::cli::Cli as clap::Parser>::parse();
    if let Err(e) = famp::cli::run(cli) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
```

### Pattern 2: FAMP_HOME resolution

```rust
// Source: D-07/D-08 + std::env docs. [VERIFIED: std::env::var, std::path::Path::is_absolute]
use std::path::PathBuf;
use crate::cli::error::CliError;

pub fn resolve_famp_home() -> Result<PathBuf, CliError> {
    let path: PathBuf = match std::env::var_os("FAMP_HOME") {
        Some(v) => PathBuf::from(v),
        None => {
            let home = std::env::var_os("HOME").ok_or(CliError::HomeNotSet)?;
            PathBuf::from(home).join(".famp")
        }
    };
    if !path.is_absolute() {
        return Err(CliError::HomeNotAbsolute { path });
    }
    Ok(path)
}
```

**Pitfall:** Do **not** call `path.canonicalize()` — D-08 is explicit that a relative `FAMP_HOME` should error rather than silently resolve. Also, `canonicalize` requires the path to already exist, which breaks the "init creates the directory" flow.

### Pattern 3: Secure file write with mode 0600

```rust
// Source: std::os::unix::fs::OpenOptionsExt [CITED: doc.rust-lang.org/std/os/unix/fs/trait.OpenOptionsExt.html]
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

pub fn write_secret(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    // Create-new so we never clobber; exclusive mode + 0600 on unix.
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)   // O_EXCL — errors if the file already exists
        .mode(0o600)        // unix-only
        .open(path)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    Ok(())
}
```

**Why `create_new` + `mode`:** combining `O_CREAT|O_EXCL` with the mode argument of `open(2)` means the file is created with mode 0600 atomically — there is no moment where it exists on disk with a wider mode. Using `create(true).open(...)` then `set_permissions` has a brief TOCTOU window where the file is mode 0644 by default umask.

**Pitfall:** `.mode()` only affects file *creation*. If the file already exists with different permissions, `.mode()` does not change them. Combine with `create_new` to be safe.

### Pattern 4: Atomic directory replacement (`--force`)

```rust
// Source: tempfile::TempDir::new_in + std::fs::rename [CITED: docs.rs/tempfile/latest/]
use tempfile::TempDir;
use std::path::Path;

pub fn atomic_replace(target: &Path, writer: impl FnOnce(&Path) -> Result<(), CliError>)
    -> Result<(), CliError>
{
    let parent = target.parent().ok_or(CliError::HomeHasNoParent)?;
    // TempDir sibling of target — same filesystem guarantees rename(2) is atomic
    let staging = TempDir::new_in(parent).map_err(CliError::io_at(parent))?;
    writer(staging.path())?;

    // Two-step: move old target aside, rename staging into place.
    let backup = parent.join(format!(".famp-old-{}", std::process::id()));
    if target.exists() {
        std::fs::rename(target, &backup).map_err(CliError::io_at(target))?;
    }
    // into_path() disables TempDir's drop-delete so the rename target persists
    let staging_path = staging.keep();
    std::fs::rename(&staging_path, target).map_err(|e| {
        // Best-effort rollback: put the old directory back
        let _ = std::fs::rename(&backup, target);
        CliError::io_at_static(target, e)
    })?;
    if backup.exists() {
        std::fs::remove_dir_all(&backup).ok();
    }
    Ok(())
}
```

**Pitfall — `TempDir::into_path` vs `keep`:** in `tempfile 3.27` the cleanup-disabling method is `TempDir::keep()` (was `into_path()` in older versions). **[VERIFIED: cargo info tempfile shows 3.27.0 in Cargo.lock]** — the planner should grep the installed version's docs to confirm the exact method name.

**Pitfall — rename across filesystems:** `std::fs::rename` is only atomic *within* one filesystem. If a user sets `FAMP_HOME=/tmp/foo` and `/tmp` is a tmpfs while `$HOME` is ext4, a TempDir created in `/tmp`'s parent (`/`) and renamed into `/tmp/foo` crosses a boundary. **Mitigation:** `TempDir::new_in(parent_of_target)` puts the tempdir on the same filesystem as the target. Correct.

### Pattern 5: rcgen 0.14 self-signed cert

```rust
// Source: rcgen 0.14 docs + crates/famp/examples/cross_machine_two_agents.rs:186
// [VERIFIED: already in use in v0.7 example]
use rcgen::{generate_simple_self_signed, CertifiedKey};

pub fn generate_tls() -> Result<(String, String), rcgen::Error> {
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec![
            "localhost".into(),
            "127.0.0.1".into(),
            "::1".into(),
        ])?;
    Ok((cert.pem(), signing_key.serialize_pem()))
}
```

**For finer control (validity window, CN, key algorithm) — rcgen `CertificateParams`:**

```rust
// Source: rcgen 0.14 docs — CertificateParams::new + self_signed
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, PKCS_ECDSA_P256_SHA256};
use time::{Duration, OffsetDateTime};

pub fn generate_tls_precise() -> Result<(String, String), rcgen::Error> {
    let mut params = CertificateParams::new(vec![
        "localhost".into(),
        "127.0.0.1".into(),
        "::1".into(),
    ])?;

    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "famp-local");
    params.distinguished_name = dn;

    params.not_before = OffsetDateTime::now_utc();
    params.not_after  = OffsetDateTime::now_utc() + Duration::days(3650); // 10 years

    let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)?;
    let cert = params.self_signed(&key_pair)?;
    Ok((cert.pem(), key_pair.serialize_pem()))
}
```

**[CONFIDENCE: MEDIUM]** — the exact rcgen 0.14 API names (`CertificateParams::new`, `KeyPair::generate_for`, `self_signed`) are inferred from the v0.7 example and from published 0.14 docs. The planner should docs.rs-verify the exact method names for `rcgen = "0.14.7"` before committing to PLAN.md.

### Pattern 6: TOML serialize with deny_unknown_fields

```rust
// Source: toml crate serde docs [CITED: docs.rs/toml/latest]
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub listen_addr: SocketAddr,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: "127.0.0.1:8443".parse().unwrap_or_else(|_| unreachable!()),
        }
    }
}

// Write:
let s = toml::to_string(&Config::default())?;
// Read:
let cfg: Config = toml::from_str(&s)?;
```

**Pitfall — `unwrap_used = "deny"`:** CLAUDE.md lints forbid `.unwrap()` and `.expect()` in lib code. The `Default` impl above uses `unwrap_or_else(|_| unreachable!())` which clippy treats more kindly. Alternative: use a `const` parse via `SocketAddr::new(Ipv4Addr::new(127,0,0,1), 8443)` which is infallible.

```rust
impl Default for Config {
    fn default() -> Self {
        use std::net::{IpAddr, Ipv4Addr, SocketAddr};
        Self { listen_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8443) }
    }
}
```

### Pattern 7: Verify FampSigningKey cannot leak (doc-test / compile-time check)

```rust
// Source: D-17 mechanism #1 — LOCK existing Debug redaction
// This is a doc-test that lives on the FampSigningKey type or in
// crates/famp/src/cli/init/mod.rs. It fails to compile / runs red
// if someone later adds a non-redacted Debug or any Display.

/// ```
/// use famp_crypto::FampSigningKey;
/// let sk = FampSigningKey::from_bytes([7u8; 32]);
/// let dbg = format!("{:?}", sk);
/// // Must NOT contain any byte of the seed.
/// assert!(!dbg.contains('7'));
/// assert!(dbg.contains("redacted"));
/// ```
/// ```compile_fail
/// // Display impl must not exist.
/// use famp_crypto::FampSigningKey;
/// let sk = FampSigningKey::from_bytes([0u8; 32]);
/// let _ = format!("{}", sk); // should fail to compile
/// ```
```

The `compile_fail` block is the locking mechanism: if anyone later adds `impl Display for FampSigningKey`, this doc-test starts compiling, and `cargo test --doc` fails because a `compile_fail` block compiled successfully.

### Anti-Patterns to Avoid

- **`anyhow::Error` returned from `famp::cli::run`.** D-04 explicitly rejects this. Typed `CliError` only.
- **`tilde` expansion in `FAMP_HOME`.** D-07 says a literal `~` is a filesystem name, not `$HOME`. This is counter-intuitive for shell users but is the right call because the shell does the expansion before the process sees the env var.
- **`mkdir_p` on the parent of FAMP_HOME.** D-09 forbids creating arbitrary ancestor chains. Only the final directory is created. If parent doesn't exist, fail with a typed error.
- **Writing secrets with default umask and then `set_permissions`.** TOCTOU window. Use `OpenOptions::create_new().mode(0o600)`.
- **Emitting any success message on stdout other than the pubkey.** D-15 mandates strict one-line output for pipeability.
- **Adding `log_level`, `principal`, or `inbox_path` to `config.toml` in Phase 1.** D-12 narrows it to one field; do not speculatively add more. IDENT-03's full schema is a milestone commitment, not a Phase 1 commitment.
- **Using `base64` crate directly in Phase 1 code.** Go through `TrustedVerifyingKey::to_b64url()` — that is the single source of truth for pubkey encoding in the workspace.
- **`set_var` on `FAMP_HOME` in parallel tests.** `std::env::set_var` is process-global. Parallel nextest tests setting/unsetting `FAMP_HOME` will race. Prefer CD-05's "Rust API route" — pass `home: &Path` into `famp::cli::init::run` directly and let only the binary `main()` (and one serial test) exercise the env-var resolution path.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Argument parsing + subcommand dispatch | Manual `std::env::args` loop | `clap 4.6` derive | Error messages, `--help`, version strings, type-checked positional/optional/flag discrimination. Hand-rolled parsers routinely miss edge cases like `--force=true` vs `--force true` vs `--force`. |
| TOML serialize/deserialize | `serde_json::to_string` with renamed keys or hand-written TOML | `toml` + `serde` | TOML has multiple lexical forms for the same value; matching canonical form by hand is a footgun. |
| Self-signed X.509 | OpenSSL shell-out, or DER byte construction | `rcgen 0.14` | rcgen handles PKIX extensions, subject alt name encoding, ECDSA/Ed25519 key format, PKCS#8 wrapping. An in-house implementation would ship with bugs the test suite wouldn't catch for months. |
| Atomic directory replace | `fs::remove_dir_all` then `fs::rename` | `tempfile::TempDir::new_in` + `rename` | The naïve delete-then-rename sequence leaves a window where FAMP_HOME doesn't exist. A crash there wedges every subsequent `famp` invocation. Stage-then-rename is the only crash-safe path on POSIX. |
| Unix file mode setting | `chmod` subprocess | `std::os::unix::fs::OpenOptionsExt::mode` | OpenOptionsExt applies the mode at `open(2)` time — no TOCTOU. A `chmod` subprocess also drags in fork overhead and a shell dependency. |
| CSPRNG for key seed | `/dev/urandom` direct read | `rand::rngs::OsRng` | `OsRng` is a thin wrapper over the OS entropy source on each platform and is exactly what `ed25519-dalek` expects via its `rand_core` compat layer. |
| Pubkey base64url encoding | `base64::engine::general_purpose::URL_SAFE_NO_PAD::encode` directly | `TrustedVerifyingKey::to_b64url()` | This is the canonical encoder used by `famp-keyring` and `famp-crypto`. Wrapping it again introduces a second source of truth that could drift. |
| "Secret redaction" for errors/logs | Scrub key bytes out of `CliError::Display` | Don't put key bytes in errors at all (D-05) | Scrubbing is a decorative mitigation; structural exclusion is a verified mitigation. D-17 mechanism #2 is the right level. |

**Key insight:** Every single one of these has a one-crate answer already in the ecosystem, and in several cases (`rcgen`, `tempfile`, `rand`) the crate is already in `Cargo.lock`. Phase 1's novelty budget is spent on **composition**, not any one subproblem.

---

## Runtime State Inventory

Phase 1 is a greenfield feature (no rename, no refactor, no migration). Section omitted.

---

## Common Pitfalls

### Pitfall 1: `std::env::set_var` races across parallel test threads

**What goes wrong:** Two integration tests both run `famp::cli::init::run`. Test A sets `FAMP_HOME=/tmp/a`, starts work. Test B sets `FAMP_HOME=/tmp/b`. Test A reads `std::env::var("FAMP_HOME")` and sees `/tmp/b`. Data lands in the wrong tempdir.

**Why it happens:** `std::env::set_var` mutates process-global state. `cargo nextest` runs integration test binaries in parallel by default (and even within a single binary, `#[test]` functions can run in parallel threads).

**How to avoid:** Prefer CD-05's "Rust API route": make `famp::cli::init::run` take an explicit `home: &Path` parameter. Only `main()` and the `FAMP_HOME` resolution helper read the env var. The env-var path is then covered by exactly one serial test (`#[test]` in its own single-threaded test binary, or guarded with a `serial_test` mutex, or run as a subprocess via `assert_cmd`).

**Warning signs:** Flaky tests that pass in isolation but fail under `cargo nextest`; tests that write to / read from the wrong tempdir path.

### Pitfall 2: `rcgen` default key algorithm silently changes between versions

**What goes wrong:** `generate_simple_self_signed` uses rcgen's default key algorithm. In 0.12 it was ECDSA P-256; future versions could change. An Ed25519-signed cert may or may not round-trip through every rustls verifier across `rustls-platform-verifier` versions.

**Why it happens:** "Simple" APIs hide policy decisions. rcgen's default is not a public-API commitment.

**How to avoid:** Use `CertificateParams` + explicit `KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)`. Pin the algorithm at the call site. Document the choice in PLAN.md. This is one of the Gray Area 3 decisions the planner must make.

**Warning signs:** `famp listen` in Phase 2 fails with obscure rustls errors; `famp-transport-http::tls::load_pem_key` returns `NoPrivateKey` or a parse error.

### Pitfall 3: `config.toml` with `SocketAddr` may surprise TOML users

**What goes wrong:** `toml::to_string(&Config { listen_addr })` serializes `SocketAddr` via its `Display` impl → the TOML value is a string `"127.0.0.1:8443"`, not a structured table. Users who hand-edit to `listen_addr = { ip = "...", port = 8443 }` get a typed error. Good — but the error message should explain why.

**Why it happens:** `SocketAddr`'s serde impl serializes as a string.

**How to avoid:** Document the string form in PLAN.md + include a round-trip fixture test. Optional: write a custom `Display`-like error wrapper in `CliError::ConfigLoadFailed` that hints "expected `listen_addr = \"IP:PORT\"` string form".

### Pitfall 4: `deny_unknown_fields` on a zero-byte `peers.toml`

**What goes wrong:** D-14 says `peers.toml` on init is zero bytes. If the `Peers` struct is defined as `struct Peers { peers: Vec<PeerEntry> }` with `deny_unknown_fields`, deserializing `""` gives `Ok(Peers { peers: vec![] })` *only* if `peers` is `#[serde(default)]`. Without `default`, the empty doc is a parse error for the missing required field.

**Why it happens:** TOML's empty-document semantics: an empty document = an empty root table = all fields default-or-missing. `serde(default)` is load-bearing.

**How to avoid:**
```rust
#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Peers {
    #[serde(default)]
    pub peers: Vec<PeerEntry>,
}
```

**Warning signs:** A round-trip test `Peers::default() → toml::to_string → toml::from_str` fails; a "missing field `peers`" error at first-run load.

### Pitfall 5: `TempDir::keep` / `into_path` method-name drift

**What goes wrong:** The method that disables TempDir drop-cleanup was `into_path` in `tempfile 3.x` for a long time, then `keep` was added (and may be the only name in 3.27). Code copied from old tutorials breaks.

**Why it happens:** Tempfile 3.x evolved.

**How to avoid:** Look up the exact method name on docs.rs for `tempfile = "3.27"` during planning. If `keep()` is available, use it. Otherwise fall back to `into_path()`. **[CONFIDENCE: MEDIUM]** — verify before coding.

### Pitfall 6: `is_absolute` and Windows UNC paths

**What goes wrong:** On Windows, `\\?\C:\...` is absolute but not "canonical". Phase 1 is Unix-only so this doesn't bite, but the planner must explicitly document Windows as unsupported in PLAN.md's limitations section or the code will confuse future Windows porters.

**How to avoid:** Add `#[cfg(unix)]` to permission-setting code. Gate integration tests behind `#[cfg(unix)]` too. PLAN.md §Limitations explicitly says "Phase 1 is Unix-only".

### Pitfall 7: Umask interference with 0600

**What goes wrong:** `OpenOptions::mode(0o600)` interacts with the process umask: the actual mode written is `mode & !umask`. If the user has `umask 0077`, you get `0600 & !0077 = 0600` — fine. If they have `umask 0000`, also fine. If they have some exotic umask that clears 0o400, the file is created with no owner-read permission, which immediately breaks every subsequent `famp` call.

**How to avoid:** After `open`, explicitly `set_permissions(Permissions::from_mode(0o600))` as a belt-and-braces step, OR accept that `0o600 & !umask` is what the user asked for by setting their umask. Recommend the explicit `set_permissions` call — it's one extra line and eliminates the umask interaction.

### Pitfall 8: The leakage scan test is weaker than it looks

**What goes wrong:** `tests/init_no_leak.rs` scans captured stdout+stderr for "any 8+ byte substring of the private key". But if the key is all zeros (`[0u8; 32]`), or has a long run of a single byte, an 8-byte substring collision against *any* text happens by chance. A test that passes on `OsRng`-generated keys but would fail on `[0u8; 32]` is epistemically weaker than claimed.

**How to avoid:** Generate the test key from a *known* high-entropy seed (e.g. `OsRng` or `ChaCha20Rng::from_seed([42u8; 32])`) and additionally check that the key bytes themselves don't contain suspicious runs. Alternatively, scan for *all* 8-byte windows of the key material (not just one) and flag if any appears.

**How to avoid (simpler):** D-17 mechanism #3 is the bar — don't try to strengthen it beyond that, but document the epistemic limit in a comment in the test file so a future reviewer doesn't over-trust it.

---

## Code Examples

(All code in Patterns above is the canonical set. Nothing additional here.)

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `clap 2.x` / `structopt` | `clap 4.x` derive (unified into `clap` itself) | 2022 (clap 4.0) | Derive macros live in `clap` directly; `structopt` is deprecated. |
| `base64::encode_config(URL_SAFE_NO_PAD, ...)` | `URL_SAFE_NO_PAD.encode(...)` (Engine API) | `base64 0.21` (2023) | Free functions removed. Tutorials using `base64::encode` will not compile against `0.22`. FAMP uses `TrustedVerifyingKey::to_b64url` to avoid this entirely. |
| `rcgen::generate_simple_self_signed` returning `(Certificate, KeyPair)` tuple | `CertifiedKey { cert, signing_key }` struct | `rcgen 0.13`/`0.14` | The v0.7 example already uses the `CertifiedKey` form. |
| `rustls-native-certs` OS trust store integration | `rustls-platform-verifier 0.5` | `rustls 0.23` era | FAMP's `famp-transport-http::tls::build_client_config` uses the newer path; Phase 1 inherits. |
| `tempfile::TempDir::into_path` (disable drop-delete) | `TempDir::keep()` added in recent 3.x | uncertain, monitor | Verify for `tempfile 3.27` on docs.rs before coding. |
| `anyhow` anywhere in library crates | `thiserror` in libs, `anyhow` only in bins (CLAUDE.md §11) | FAMP project-wide | **Phase 1 narrows further:** typed `CliError` even in the binary path (D-04). |

**Deprecated/outdated:**
- `structopt`: replaced by clap derive.
- `native-tls` / `openssl` crate: explicitly forbidden by FAMP workspace policy (`cargo tree -i openssl` stays empty).
- `rand_core 0.9` API for keys that share a dalek key — stay on `rand_core 0.6` because `ed25519-dalek 2.2` still uses 0.6.

---

## TLS Cert Parameters — Deferred Decision Research (Gray Area 3)

The user explicitly deferred this to RESEARCH. Here is the short comparison with a recommendation.

### Key Algorithm

| Option | Pros | Cons | Verdict |
|--------|------|------|---------|
| **ECDSA P-256** (`PKCS_ECDSA_P256_SHA256`) | Broadest rustls + browser compatibility; default in rcgen historically; well-supported by `rustls-platform-verifier`; the v0.7 cross-machine example already round-trips it | Not as elegant as "protocol uses Ed25519, TLS uses Ed25519 too" | **RECOMMEND** — conservative-compat wins per user's provisional stance; v0.7 has already proven this algorithm works end-to-end through `famp-transport-http::tls` |
| **Ed25519** (`PKCS_ED25519`) | Matches the protocol signing algorithm; smaller certs; smaller signatures | rustls 0.23 supports Ed25519 certs in principle via `ring`, but `rustls-platform-verifier` behavior on OS trust stores is uneven; browsers do not accept Ed25519 TLS certs (TLS 1.3 requires them but major browsers haven't shipped); extra risk for a deferred gray-area decision | Reject for v0.8 — revisit in a future "Ed25519 everywhere" phase once we have a reason. |
| **RSA-2048** | Maximum backward compat | Deprecated hygiene, larger keys, slower | Reject. |

**[CONFIDENCE: HIGH]** on ECDSA P-256 being the safe choice; `[CONFIDENCE: MEDIUM, ASSUMED]` on the exact Ed25519-in-rustls compatibility story — we haven't tested it, so recommending against it is the conservative call.

### Subject Alternative Names

Recommend: `["localhost", "127.0.0.1", "::1"]`. Matches v0.7 example. Phase 2's `famp listen` binds to `127.0.0.1:8443` by default per D-12, so `127.0.0.1` + `localhost` cover the same-laptop case. `::1` is included for IPv6 loopback parity (no cost).

**Cross-machine deployment (v0.9+)** will need a wider SAN list or a real hostname — that's a deliberate deferral.

### Common Name (CN)

Recommend: `"famp-local"` — a boring placeholder. Modern TLS verifiers ignore CN and only use SAN anyway. IDENT-02 says "CN = configured principal name" but D-12 narrows `config.toml` to not contain `principal` in Phase 1 → there is no configured principal name at init time. The placeholder `"famp-local"` keeps the cert structurally valid.

**Alternative:** defer CN to Phase 3 when `famp peer add` / principal wiring appears. Phase 1 could use `"famp-uninitialized"` to make the deferred nature visible. The planner picks in PLAN.md.

### Validity Window

Recommend: **10 years (3650 days)**. Finite (hygiene), but long enough that `famp init --force` is not a frequent chore. Plausible alternatives: 5 years (stricter), 100 years (effectively forever). The user's provisional stance was "long but finite"; 10 years operationalizes that.

### Serial Number

Recommend: rcgen default (random 128-bit). No reason to override.

### Summary TLS Decision Block for PLAN.md

```toml
# Phase 1 TLS cert parameters
key_algorithm   = "PKCS_ECDSA_P256_SHA256"  # conservative-compat, matches v0.7 example
sans            = ["localhost", "127.0.0.1", "::1"]
cn              = "famp-local"              # placeholder — no principal in config.toml yet
validity_days   = 3650                      # 10 years
serial          = "rcgen default (random 128-bit)"
```

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `rcgen 0.14.7`'s exact API is `CertificateParams::new(sans) -> Result<Params>` + `KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)` + `params.self_signed(&key_pair) -> Result<Certificate>` | Pattern 5 | Minor — the `generate_simple_self_signed` path is already in use in v0.7 example. Worst case the planner uses the simple form with rcgen defaults. Either works. |
| A2 | `tempfile 3.27` exposes `TempDir::keep()` to disable drop-delete (not just `into_path`) | Pitfall 5 / Pattern 4 | Minor — both methods likely work; planner greps installed version before coding. |
| A3 | rustls 0.23 + `rustls-platform-verifier 0.5` accepts an ECDSA P-256 self-signed cert from `rcgen 0.14` without warnings | §TLS Cert Parameters | Medium — if wrong, Phase 2 fails to load the cert. Mitigation: Phase 1 adds a smoke test that builds a `rustls::ServerConfig` from the generated PEMs via `famp_transport_http::tls::build_server_config` and asserts `Ok`. That directly validates the compatibility claim. |
| A4 | `toml = "1.1.2"` round-trips `SocketAddr` via its `Display`/`FromStr` serde path and honors `deny_unknown_fields` on struct root | Pattern 6 | Low — standard serde behavior. Add a fixture test `Config::default() → toml::to_string → toml::from_str` as a gate. |
| A5 | `std::os::unix::fs::OpenOptionsExt::mode(0o600)` combined with `create_new(true)` on Linux creates the file atomically at mode 0600 (no TOCTOU window) | Pattern 3 | Low — this is documented Rust stdlib behavior; confirmed in `std::os::unix::fs` docs. |
| A6 | The D-15 stdout line "base64url-unpadded, as raw bytes (same format `famp-keyring` already uses for Principal)" refers to the **pubkey encoding** (`TrustedVerifyingKey::to_b64url`), not the **Principal string** (`agent:local/alice`). | §Summary, §Phase Requirements | **MEDIUM-HIGH** — Principal is a URI-like string in this repo, not a raw-byte encoding. The phrasing in D-15 conflates two things. I am interpreting it as "the same base64url encoding `famp-keyring` uses for the pubkey half of each entry", which is `TrustedVerifyingKey::to_b64url()`. Planner should confirm with user before coding if there is any doubt. |
| A7 | Phase 1's `Config` struct can safely have only `listen_addr` even though IDENT-03 lists three fields, because CONTEXT.md D-12 explicitly narrows it. Later phases will add `principal` and `inbox_path` when those phases actually consume them. | §Phase Requirements, §Anti-Patterns | Low — CONTEXT.md is the authority by design. Document the narrowing in PLAN.md's §Requirements-coverage section so the Phase 1 → v0.8-milestone trace is explicit. |

**If this table is empty:** it isn't. Seven assumptions above, one flagged MEDIUM-HIGH (A6) for user confirmation if the planner has any doubt about D-15's intent.

---

## Open Questions

1. **A6 above: does D-15's "same format `famp-keyring` already uses for Principal" refer to the pubkey b64url encoding or to the Principal string form?**
   - What we know: `famp-keyring` file format is two columns per line — a Principal string (`agent:local/alice`) and a base64url-unpadded pubkey. `TrustedVerifyingKey::to_b64url` is the sanctioned encoder for the pubkey column. `Principal` has its own `Display`/`FromStr` that emits `agent:{authority}/{name}`.
   - What's unclear: D-15 says "the newly generated public key … same format famp-keyring already uses for Principal". The pubkey is not the Principal in this codebase; they're separate columns.
   - **Recommendation:** emit `TrustedVerifyingKey::to_b64url()` of the newly generated pubkey. This is what lets `famp init | famp peer add alice --pubkey -` work as D-15's rationale claims. The Principal string is user-supplied (`alice`) not machine-generated, so it can't be what `famp init` emits.

2. **Does `famp-transport-http::tls::load_pem_cert` / `build_server_config` accept the exact byte output of `rcgen::CertifiedKey::{cert.pem(), signing_key.serialize_pem()}` without any transformation?**
   - What we know: the v0.7 example at `crates/famp/examples/cross_machine_two_agents.rs` calls exactly this sequence and the cross-machine test passes. So empirically yes for `generate_simple_self_signed`.
   - What's unclear: whether switching to `CertificateParams::new` + `KeyPair::generate_for(P256)` + `self_signed` produces a PEM with the same private-key encoding (`PRIVATE KEY` vs `EC PRIVATE KEY` headers). `rustls_pemfile::private_key` accepts both.
   - **Recommendation:** Phase 1 adds a smoke test that runs the generator, then loads the PEMs via the Phase 2 loader functions, and asserts `Ok`. This is a 10-line test that directly validates the cross-phase conformance gate.

3. **Is `dirs::home_dir()` worth pulling in for just one call, given `std::env::var("HOME")` works on Unix?**
   - What we know: `dirs 6.0.0` is well-maintained but adds `dirs-sys` + platform shims. Phase 1 is Unix-only.
   - What's unclear: whether Windows support is a future goal.
   - **Recommendation:** use `std::env::var("HOME")` directly in Phase 1. If/when Windows support is added, the resolution function changes in one place.

4. **Should the `init` leakage test use `std::env::set_var` or pass `FAMP_HOME` through the Rust API (CD-05)?**
   - What we know: `set_var` is process-global; parallel integration tests race.
   - **Recommendation:** Rust API route. Define `famp::cli::init::run(home: &Path, force: bool) -> Result<InitOutcome, CliError>`. Only the `resolve_famp_home()` helper reads the env var, and it's covered by one serial test (or a subprocess test) that explicitly exercises the env precedence chain. Every other test passes a `TempDir` path directly. This also eliminates Pitfall 1 entirely.

5. **Rollback policy if atomic `--force` fails mid-rename.**
   - Pattern 4 sketches a two-step rename with best-effort rollback. On a hard crash between `rename(target, backup)` and `rename(staging, target)`, the filesystem is left with `<parent>/.famp-old-<pid>` but no `<parent>/.famp`. A subsequent `famp` call sees "identity missing" and fails closed (by D-11 design). A manual `mv .famp-old-<pid> .famp` recovers. **Recommendation:** document this in PLAN.md and accept it as the Phase 1 behavior. Adding crash-safe fsync-based recovery is out of scope for a personal tool.

---

## Environment Availability

Phase 1 has **no new external dependencies** beyond the Rust toolchain and workspace path-dependent crates. No network services, no databases, no external processes. The only "environment" checks are:

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| Rust toolchain | All | ✓ | `1.89+` per `rust-toolchain.toml` | — |
| `rcgen 0.14.x` | TLS cert gen | ✓ | `0.14.7` in Cargo.lock | — |
| `tempfile 3.x` | Atomic `--force` | ✓ | `3.27.0` in Cargo.lock | — |
| `clap 4.x` | CLI parsing | Needs add | target `4.6.0` | — |
| `toml 1.x` | Config files | Needs add | target `1.1.2` | — |
| Unix filesystem | 0600/0700 modes | ✓ | Linux (`std::os::unix::fs`) | Windows explicitly unsupported in v0.8 |
| `$HOME` env var | FAMP_HOME fallback | ✓ | Set in dev env + CI | Fails with `CliError::HomeNotSet` if missing |

No blocking gaps. Phase 1 can be planned and executed immediately.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | `cargo test` + `cargo nextest 0.9.132` |
| Config file | `Cargo.toml` workspace defaults; no nextest-specific config in repo |
| Quick run command | `cargo nextest run -p famp --tests` |
| Full suite command | `just ci` (alias for `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo nextest run --workspace`) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CLI-01 | `famp init` creates all six files on a fresh tempdir | integration | `cargo nextest run -p famp --test init_happy_path` | ❌ Wave 0 |
| CLI-01 | `famp init` refuses non-empty FAMP_HOME without `--force` → `AlreadyInitialized` | integration | `cargo nextest run -p famp --test init_refuses` | ❌ Wave 0 |
| CLI-01 | `famp init --force` atomically replaces an existing FAMP_HOME | integration | `cargo nextest run -p famp --test init_force` | ❌ Wave 0 |
| CLI-07 | `FAMP_HOME=<path>` resolves to that path, not `~/.famp` | integration (serial / subprocess) | `cargo nextest run -p famp --test init_home_env -- --test-threads=1` | ❌ Wave 0 |
| CLI-07 | Relative FAMP_HOME → `HomeNotAbsolute` | unit | `cargo nextest run -p famp resolve_home_rejects_relative` | ❌ Wave 0 |
| IDENT-01 | `key.ed25519` is 32 bytes, mode 0600; `pub.ed25519` is 32 bytes, mode 0644 | integration (unix) | `cargo nextest run -p famp --test init_happy_path ident_01_key_files` | ❌ Wave 0 |
| IDENT-02 | `tls.cert.pem` + `tls.key.pem` load via `famp_transport_http::tls::build_server_config` | integration | `cargo nextest run -p famp --test init_tls_roundtrip` | ❌ Wave 0 |
| IDENT-03 (narrowed) | `config.toml` contains only `listen_addr = "127.0.0.1:8443"`, loads into typed `Config`, rejects unknown fields | unit + fixture | `cargo nextest run -p famp config_roundtrip config_deny_unknown` | ❌ Wave 0 |
| IDENT-04 (narrowed) | `peers.toml` is zero bytes, loads into empty `Peers { peers: vec![] }` via `serde(default)` | unit | `cargo nextest run -p famp peers_empty_file_loads_empty` | ❌ Wave 0 |
| IDENT-05 (Phase 1 slice) | Loader detects missing `key.ed25519` and returns `CliError::IdentityIncomplete { missing }` | integration | `cargo nextest run -p famp --test init_identity_incomplete` | ❌ Wave 0 |
| IDENT-06 | Substring scan test: running `init` against a tempdir emits no 8-byte window of `key.ed25519` on stdout/stderr | integration | `cargo nextest run -p famp --test init_no_leak` | ❌ Wave 0 |
| IDENT-06 | Compile-time check: `FampSigningKey` has redacted `Debug` and no `Display` | doc-test | `cargo test -p famp-crypto --doc` | exists in famp-crypto; Phase 1 adds a `compile_fail` reinforcement |

### Sampling Rate

- **Per task commit:** `cargo nextest run -p famp --tests`
- **Per wave merge:** `cargo nextest run --workspace -E 'package(famp) + test(init)'` plus `cargo clippy -p famp --all-targets -- -D warnings`
- **Phase gate:** `just ci` fully green — including all 253 v0.7 tests unchanged — before `/gsd-verify-work`

### Wave 0 Gaps

- [ ] `crates/famp/tests/init_happy_path.rs` — covers CLI-01, IDENT-01, IDENT-02 happy path
- [ ] `crates/famp/tests/init_refuses.rs` — covers CLI-01 non-`--force` refusal
- [ ] `crates/famp/tests/init_force.rs` — covers CLI-01 `--force` atomic replace
- [ ] `crates/famp/tests/init_home_env.rs` — covers CLI-07 env-var path (serial or subprocess)
- [ ] `crates/famp/tests/init_tls_roundtrip.rs` — covers IDENT-02 `rcgen → famp-transport-http::tls` conformance gate
- [ ] `crates/famp/tests/init_identity_incomplete.rs` — covers IDENT-05 Phase 1 slice
- [ ] `crates/famp/tests/init_no_leak.rs` — covers IDENT-06 substring scan (D-17 mechanism #3)
- [ ] Unit-test module in `crates/famp/src/cli/config.rs` — config/peers round-trip + `deny_unknown_fields` fixtures
- [ ] Unit-test module in `crates/famp/src/cli/home.rs` — FAMP_HOME resolution edge cases (absolute, relative, missing env)
- [ ] Framework install: none — `cargo nextest` already a workspace dev-tool per CLAUDE.md §13

---

## Security Domain

**`security_enforcement`:** `.planning/config.json` does not explicitly disable it — treat as enabled.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | Indirectly — Phase 1 generates the identity later phases authenticate with | Ed25519 keygen via `FampSigningKey::from_bytes` + CSPRNG seed |
| V3 Session Management | No — no sessions in Phase 1 | — |
| V4 Access Control | File-system level only — 0600 on secrets | `OpenOptionsExt::mode(0o600)` + `PermissionsExt::set_permissions` belt-and-braces |
| V5 Input Validation | Yes — CLI args, TOML config, FAMP_HOME path | `clap` derive (types are the validation); `serde(deny_unknown_fields)`; `Path::is_absolute` for FAMP_HOME |
| V6 Cryptography | Yes — Ed25519 keygen, TLS cert gen | `ed25519-dalek 2.2` + `rand::rngs::OsRng`; `rcgen 0.14` for X.509; **never** hand-roll DER or PEM |
| V8 Data Protection | Yes — secret-at-rest + secret-in-memory | On-disk: mode 0600; in-memory: existing `ed25519-dalek zeroize` feature (inherited); no `zeroize-on-drop` additions per D-18 |
| V14 Configuration | Yes — `deny_unknown_fields` on config/peers | `serde(deny_unknown_fields)` + `toml` crate |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Private key bytes in logs / error messages | Information Disclosure | **Structural exclusion** (D-05/D-17): key bytes never appear in `CliError` variants; `FampSigningKey` has redacted `Debug` and no `Display`; substring scan test catches regressions |
| Private key file created with wide mode, then chmod'd narrower (TOCTOU) | Tampering / Info Disclosure | `OpenOptions::create_new().mode(0o600)` sets the mode at `open(2)` time atomically |
| `famp init` crashed mid-write leaves FAMP_HOME in a partial state | Tampering / DoS (fail-closed) | Atomic `--force` path: stage in `TempDir::new_in(parent)`, `rename` into place. Partial state on first-run init is detected by Phase 2+ via `CliError::IdentityIncomplete` (D-11) and requires user to re-init. |
| Symlink attack on `$HOME/.famp` (attacker pre-creates the dir as a symlink to `/etc`) | Elevation / Tampering | `create_new` + O_EXCL on individual files prevents following a symlink into an unexpected target; `mkdir` with 0700 on the dir is the weak point — if an attacker pre-creates `$HOME/.famp` as a symlink, `mkdir` fails with EEXIST and `init` takes the non-empty path (refused without `--force`). Document this in PLAN.md as a known behavior. |
| Race between `FAMP_HOME` env var resolution and parallel test setup | Tampering (test-time only) | CD-05 recommendation: pass `home: &Path` through the Rust API; only one serial test touches the env var. |
| Hand-rolled base64url causing divergence from `famp-keyring` | Information Disclosure (non-obvious) | Only path is `TrustedVerifyingKey::to_b64url` — single source of truth |
| Predictable or weak seed for keygen | Cryptographic Weakness | `rand::rngs::OsRng` — OS entropy source; no dev-fixture seeds in production paths |
| TLS cert with Ed25519 key fails to round-trip through rustls verifier (Pitfall 2) | DoS (downstream phase fails) | Pin ECDSA P-256 explicitly via `KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)` + add a smoke test that runs the cert through `famp_transport_http::tls::build_server_config` in the same Phase 1 test suite |
| CLI emits to stdout in a way that pollutes pipes (`famp init \| ...`) | Usability / composition | D-15: stdout is **exactly** the pubkey line; all human-readable output goes to stderr |

---

## Project Constraints (from CLAUDE.md)

Actionable directives extracted from `./CLAUDE.md` that Phase 1 must honor:

- **Rust stack — pinned workspace versions.** Use `clap`, `toml`, `rcgen`, `tempfile` only; no `openssl`/`native-tls` (enforced by `cargo tree -i openssl` empty gate — Phase 1 must not regress this).
- **`thiserror` in libs, `anyhow` only in CLI/bin.** D-04 narrows further: typed `CliError` even in the binary, no `anyhow` at all in Phase 1. Compatible — just stricter.
- **`unwrap_used = "deny"` and `expect_used = "deny"`** at workspace clippy level. No `.unwrap()` / `.expect()` in Phase 1 code. Use pattern-match + typed error return, or `unwrap_or_else(|| unreachable!())` in `Default` impls only if the value is provably const.
- **`clippy::all` + `clippy::pedantic` denied at workspace level.** Phase 1 code must pass `cargo clippy --all-targets -- -D warnings`. Common snags: `must_use_candidate` (allowed), `missing_errors_doc` (allowed), but many pedantic lints are active — the planner should expect small clippy touch-up rounds.
- **`unsafe_code = "forbid"`.** Phase 1 has no reason to use unsafe; this is fine.
- **`just ci` gate:** 253 v0.7 tests + new Phase 1 tests must all pass. No `--no-verify` shortcuts.
- **Narrow typed error enums, not one god enum.** Phase 1 adds `CliError` as a new enum, not as a variant in an existing error type.
- **`#[serde(deny_unknown_fields)]` on every on-wire/on-disk struct.** D-13/D-14 already lock this.
- **No new crate dependency that pulls `openssl` or `native-tls`.** `clap`, `toml`, `rcgen`, `tempfile`, `dirs` — verify each with `cargo tree -i openssl` after adding. (`rcgen` uses `ring` via `rustls` ecosystem; safe.)
- **Never emit private key bytes to stdout/stderr/logs.** Enforced by D-17 stack.
- **GSD workflow enforcement from project CLAUDE.md:** Do not make direct repo edits outside a GSD workflow. Phase 1 work proceeds via `/gsd:execute-phase` after planning.

---

## Sources

### Primary (HIGH confidence)
- `crates/famp/Cargo.toml` — existing dev-deps (`rcgen 0.14`, `tempfile 3`, `reqwest 0.13`, `axum 0.8`) — read 2026-04-14
- `crates/famp/examples/cross_machine_two_agents.rs` lines 50, 186-205 — canonical in-repo `rcgen::generate_simple_self_signed` usage
- `crates/famp/src/lib.rs` — current public re-exports; the place `pub mod cli` lands
- `crates/famp/src/bin/famp.rs` — 8-line placeholder being replaced
- `crates/famp-crypto/src/keys.rs` — `FampSigningKey` redacted Debug (line ~100), `TrustedVerifyingKey::to_b64url` (line 122)
- `crates/famp-keyring/src/file_format.rs` — canonical pubkey column encoding format
- `crates/famp-keyring/src/lib.rs` — `Keyring`, `Principal` → `TrustedVerifyingKey` mapping
- `crates/famp-transport-http/src/tls.rs` — `load_pem_cert`, `load_pem_key`, `build_server_config` — the conformance target for Phase 1's output
- `crates/famp-core/src/identity.rs` — `Principal` struct (authority + name strings, NOT raw bytes)
- `Cargo.lock` — `rcgen 0.14.7`, `tempfile 3.27.0` already locked
- `Cargo.toml` (workspace) — `ed25519-dalek 2.2.0`, `rand 0.8`, `thiserror 2.0.18`, `tokio 1.51.1`, `rustls 0.23.38` all pinned
- `./CLAUDE.md` §Technology Stack — workspace crate version table
- `.planning/milestones/v0.8-phases/01-identity-cli-foundation/01-CONTEXT.md` — all D-01..D-18 decisions
- `.planning/REQUIREMENTS.md` — CLI-01/07, IDENT-01..06 full text
- `.planning/ROADMAP.md` §v0.8 Phase 1 — 5 success criteria

### Secondary (MEDIUM confidence)
- `cargo search clap` (2026-04-14) → `clap 4.6.0`
- `cargo search dirs` (2026-04-14) → `dirs 6.0.0`
- `cargo info toml` (2026-04-14) → `toml 1.1.2+spec-1.1.0`, MSRV 1.85
- `cargo info tempfile` (2026-04-14) → confirms 3.27.0
- clap 4.x derive API — from training data + general Rust ecosystem knowledge; planner should double-check on docs.rs/clap/latest before writing code
- `std::os::unix::fs::OpenOptionsExt::mode` semantics — Rust stdlib docs (not re-verified in this session)

### Tertiary (LOW confidence — needs validation)
- Exact `rcgen 0.14.7` `CertificateParams` method names (A1) — inferred from published docs; planner verifies on docs.rs
- `tempfile::TempDir::keep` vs `into_path` method name (A2) — verify on docs.rs before coding
- rustls 0.23 + `rustls-platform-verifier 0.5` acceptance of ECDSA P-256 self-signed cert (A3) — likely fine based on v0.7 success, but the exact CA-unknown verifier path is not directly tested in v0.7 (v0.7 uses `--trust-cert` to add the cert as a trust anchor); gate with a Phase 1 smoke test that uses `build_server_config` (server side — no CA check) rather than client-side validation

---

## Metadata

**Confidence breakdown:**
- **Standard stack:** HIGH — every recommended crate is either already in Cargo.lock or verified via `cargo search` / `cargo info` on 2026-04-14.
- **Architecture:** HIGH — module structure is a conservative split; nothing novel.
- **Pitfalls:** HIGH — every pitfall listed is either in-repo (v0.7 patterns) or documented in standard Rust/crate docs.
- **Reuse of v0.7 substrate:** HIGH — direct file references confirmed for `FampSigningKey` Debug impl, `TrustedVerifyingKey::to_b64url`, `famp-transport-http::tls::build_server_config`, `rcgen::generate_simple_self_signed` existing usage.
- **TLS cert parameters (deferred decision):** MEDIUM — the ECDSA P-256 recommendation is conservative-safe, but A3 (rustls compatibility for the *exact* combination) should be smoke-tested rather than assumed.
- **D-15 stdout format interpretation (A6):** MEDIUM-HIGH — the interpretation that "base64url-unpadded … same format as famp-keyring" means `TrustedVerifyingKey::to_b64url` (not `Principal::to_string`) is near-certain given the codebase, but the planner or discuss-phase should confirm if there is any user-facing doubt.

**Research date:** 2026-04-14
**Valid until:** 2026-05-14 (30 days — the stack is stable; Rust ecosystem moves slowly for these crates)

---

## RESEARCH COMPLETE

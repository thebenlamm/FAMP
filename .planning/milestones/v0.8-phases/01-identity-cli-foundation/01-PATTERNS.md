# Phase 01: Identity & CLI Foundation — Pattern Map

**Mapped:** 2026-04-14
**Files analyzed:** 13 (new + modified)
**Analogs found:** 11 / 13

This phase is a pure *composition* phase: nearly every primitive already exists in the v0.7 substrate. The closest analogs live in `famp-crypto`, `famp-keyring`, `famp-transport-http`, `famp/src/runtime/`, and the v0.7 two-agent example. The planner should treat this document as "copy patterns from *these specific lines*" — do not reinvent.

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| `crates/famp/src/bin/famp.rs` *(rewrite)* | bin entry | request-response (argv→exit) | `crates/famp/examples/cross_machine_two_agents.rs` (clap main shape) + RESEARCH §Pattern 1 | role-match |
| `crates/famp/src/lib.rs` *(modify)* | lib re-exports | — | current `lib.rs` lines 38–52 (module declaration idiom) | exact |
| `crates/famp/src/cli/mod.rs` | CLI dispatch | request-response | `crates/famp/src/runtime/mod.rs` (module root + `pub use` pattern) | role-match |
| `crates/famp/src/cli/error.rs` | typed error | — | `crates/famp-keyring/src/error.rs` + `crates/famp/src/runtime/error.rs` | **exact** |
| `crates/famp/src/cli/home.rs` | path resolver | file-I/O (read env) | RESEARCH §Pattern 2 (no direct analog in workspace) | none |
| `crates/famp/src/cli/paths.rs` | constants | — | `crates/famp-keyring/src/file_format.rs` (const + small helper layout) | role-match |
| `crates/famp/src/cli/config.rs` | serde model | file-I/O (TOML) | `crates/famp-keyring/src/file_format.rs` (serialize/parse pair + `deny_unknown_fields` convention noted in CONTEXT) | role-match |
| `crates/famp/src/cli/perms.rs` | unix fs helper | file-I/O | RESEARCH §Pattern 3 (no workspace analog; new helper) | none |
| `crates/famp/src/cli/init/mod.rs` | subcommand entry | file-I/O (CRUD-on-dir) | `crates/famp/examples/cross_machine_two_agents.rs` lines ~180–225 (keygen + rcgen + file-write composition) | role-match |
| `crates/famp/src/cli/init/tls.rs` | rcgen wrapper | file-I/O | `crates/famp/examples/cross_machine_two_agents.rs` line ~186 `generate_simple_self_signed(...)` + `cert.pem()/signing_key.serialize_pem()` | **exact** |
| `crates/famp/src/cli/init/atomic.rs` | tempdir+rename helper | file-I/O | RESEARCH §Pattern 4 (no workspace analog) | none |
| `crates/famp/tests/init_*.rs` (5 files) | integration tests | file-I/O | `crates/famp-transport-http/src/tls.rs` lines 101–185 (tempfile + PEM write + typed-error assertions) | role-match |
| `crates/famp/Cargo.toml` *(modify)* | manifest | — | current file lines 14–40 | exact |

---

## Pattern Assignments

### `crates/famp/src/cli/error.rs` (typed error, thiserror)

**Analogs:** `crates/famp-keyring/src/error.rs` and `crates/famp/src/runtime/error.rs`.
Both are the canonical project shape for phase-local typed errors. **Copy this exact shape.**

**From `crates/famp-keyring/src/error.rs` lines 10–33:**
```rust
#[derive(Debug, thiserror::Error)]
pub enum KeyringError {
    #[error("duplicate principal at line {line}: {principal}")]
    DuplicatePrincipal { principal: Principal, line: usize },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("crypto error: {0}")]
    Crypto(#[from] famp_crypto::CryptoError),
}
```

**From `crates/famp/src/runtime/error.rs` lines 9–49** — use of `#[source]` for wrapping without embedding sensitive payload, distinct-variant discipline for test `matches!`:
```rust
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("envelope decode error")]
    Decode(#[source] famp_envelope::EnvelopeDecodeError),

    #[error("transport error")]
    Transport(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("keyring error")]
    Keyring(#[source] famp_keyring::KeyringError),
}
```

**Apply to `CliError`:**
- Use `#[derive(Debug, thiserror::Error)]`.
- Every variant has an `#[error("…")]` attribute — its `Display` output is exactly what `main()` prints to stderr on failure (D-16).
- **D-05 constraint:** no variant embeds `[u8; 32]`, `FampSigningKey`, `&str` of key material, or any rcgen secret. IO errors use `#[source]` or `#[from] std::io::Error`, never `format!("{:?}", key)`.
- Distinct variants for each failure class so integration tests can `matches!(err, CliError::AlreadyInitialized { .. })`. This is the same "distinct variant per adversarial case" discipline as `RuntimeError` lines 12–39 (CONF-05 / CONF-06 / CONF-07).
- Expected variants per CONTEXT D-04: `HomeNotAbsolute { path }`, `HomeNotSet`, `HomeCreateFailed { path, source: io::Error }`, `HomeHasNoParent`, `AlreadyInitialized { existing_files: Vec<PathBuf> }`, `IdentityIncomplete { missing: PathBuf }`, `KeygenFailed(#[source] …)`, `CertgenFailed(#[source] rcgen::Error)`, `Io { path: PathBuf, #[source] source: io::Error }`, `TomlSerialize(#[source] toml::ser::Error)`.

---

### `crates/famp/src/cli/init/tls.rs` (rcgen wrapper)

**Analog:** `crates/famp/examples/cross_machine_two_agents.rs` lines ~183–200. This is the only in-tree caller of `rcgen` and is the exact pattern to lift into the new init module.

**Excerpt to copy (example file, load-or-generate branch):**
```rust
let ck = generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()])?;
// …
std::fs::write(&cp, ck.cert.pem())?;
std::fs::write(&kp, ck.signing_key.serialize_pem())?;
```

**Key call signatures this phase relies on (verified against the working example):**
- `rcgen::generate_simple_self_signed(Vec<String>) -> Result<CertifiedKey, rcgen::Error>`
- `CertifiedKey { cert, signing_key }` destructure
- `cert.pem() -> String`
- `signing_key.serialize_pem() -> String`

**Phase 1 refinements on top of the example pattern:**
- Replace the example's `std::fs::write` (which uses default umask) with the secure-write helper (`OpenOptions::create_new().mode(0o600)`) for `tls.key.pem` — see Shared Pattern §Secure Secret File Write.
- Add `"::1"` to the SAN list (CONTEXT deferred §TLS cert params — `localhost, 127.0.0.1, ::1`).
- If the planner picks `CertificateParams` over `generate_simple_self_signed` for explicit CN/validity-window control (per CONTEXT deferred Gray Area 3), the conformance target is still that the output must be loadable unmodified by `famp_transport_http::tls::load_pem_cert` / `load_pem_key` (see Shared Pattern §Conformance Gate below).

---

### `crates/famp/src/cli/init/mod.rs` (init orchestration)

**Analog:** `crates/famp/examples/cross_machine_two_agents.rs` lines ~180–225. This block is the closest in-tree example of the *composition* init has to perform: keygen (implicitly), cert gen, PEM write, load-back. Phase 1 lifts this into a standalone subcommand and inserts the FAMP_HOME / atomic-replace / permissions / pubkey-stdout layers.

**Reuse, do not reinvent, these call paths:**

1. **Keygen (imports from `famp-crypto`):**
   ```rust
   use famp_crypto::FampSigningKey;
   use rand::RngCore;

   let mut seed = [0u8; 32];
   rand::rngs::OsRng.fill_bytes(&mut seed);
   let sk = FampSigningKey::from_bytes(seed);
   let vk = sk.verifying_key();
   ```
   `FampSigningKey::from_bytes` is defined at `crates/famp-crypto/src/keys.rs` lines 70–74. `verifying_key()` at lines 91–94.

2. **Pubkey → base64url for stdout (D-15):** call `vk.to_b64url()` — `crates/famp-crypto/src/keys.rs` lines 122–124. **Do not** import `base64` directly; `TrustedVerifyingKey::to_b64url` is the single source of truth used by `famp-keyring`'s save format (`crates/famp-keyring/src/file_format.rs` line 73: `format!("{}  {}\n", principal, key.to_b64url())`).

3. **Raw 32-byte private-key serialization to disk:** `FampSigningKey::from_bytes` stores the 32-byte seed; the write path needs the raw bytes. `famp-crypto` currently does not expose a `to_bytes()` method on `FampSigningKey` (it exposes `to_b64url` via `self.0.to_bytes()`, lines 84–86). The planner should either (a) add a narrow `pub fn to_seed_bytes(&self) -> [u8; 32]` accessor to `FampSigningKey` gated by a doc comment explaining the "only for on-disk identity" use case, or (b) have `cli::init` generate the seed locally and write it before handing it to `from_bytes`. Option (b) keeps `famp-crypto` untouched and is strictly smaller surface — prefer it.

4. **Self-signed TLS:** see `cli/init/tls.rs` pattern above.

5. **File-write order and fsync discipline:** each secure write calls `f.sync_all()?` before dropping (see RESEARCH §Pattern 3). This matches the project's "write then verify" stance — no half-synced directories.

---

### `crates/famp/src/cli/config.rs` (Config + Peers structs)

**Analog:** `crates/famp-keyring/src/file_format.rs`. Same role (on-disk text format for identity-adjacent data) with the same `deny_unknown_fields` stance CONTEXT D-13 / D-14 require.

**Pattern to mirror** — a `parse`/`serialize` pair as free functions, narrow struct, reject-unknown-fields on load:

```rust
// from crates/famp-keyring/src/file_format.rs:32–68 (parse_line) and :72–74 (serialize)
pub fn parse_line(raw: &str, line_no: usize) -> Result<ParsedEntry, KeyringError> { … }
pub fn serialize_entry(principal: &Principal, key: &TrustedVerifyingKey) -> String { … }
```

**Apply to `cli/config.rs`:**
```rust
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub listen_addr: SocketAddr,
}

impl Default for Config {
    fn default() -> Self {
        Self { listen_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8443) }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Peers {
    #[serde(default)]
    pub peers: Vec<PeerEntry>, // PeerEntry body deferred to Phase 3
}
```

**Key decisions enforced by analog:**
- One field per struct, added only when a phase needs it. `file_format.rs` is the workspace's "narrow on purpose" exemplar.
- `#[serde(default)]` on `Peers::peers` is load-bearing for the zero-byte `peers.toml` round-trip (RESEARCH §Pitfall 4).
- Infallible `Default` via `Ipv4Addr::new` avoids the `unwrap_used = "deny"` lint.

---

### `crates/famp/src/cli/mod.rs` (CLI dispatch root)

**Analog:** `crates/famp/src/runtime/mod.rs` — same role (module root that re-exports a small surface from children). The planner should structurally mirror it: `pub mod error; pub mod home; pub mod paths; pub mod config; pub mod init; pub use error::CliError;`.

**Shape from RESEARCH §Pattern 1 (clap derive), to live in this file:**
```rust
use clap::{Parser, Subcommand};

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
}

#[derive(clap::Args)]
pub struct InitArgs {
    #[arg(long)]
    pub force: bool,
}

pub fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Commands::Init(a) => init::run(a).map(|_| ()),
    }
}
```

---

### `crates/famp/src/bin/famp.rs` (rewrite)

**Current state** (lines 1–9) — 8-line placeholder to be replaced:
```rust
#![forbid(unsafe_code)]
#![allow(unused_crate_dependencies)]

fn main() {
    println!("famp v0.5.1 placeholder");
}
```

**Target shape** (RESEARCH §Pattern 1, lines 311–321 of RESEARCH.md):
```rust
#![forbid(unsafe_code)]

fn main() {
    let cli = <famp::cli::Cli as clap::Parser>::parse();
    match famp::cli::run(cli) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
```

The bin stays ~20 lines (D-02). `InitOutcome`'s pubkey stdout print lives inside `cli::run` or `cli::init::run` — the bin does not format output. Keep `#![forbid(unsafe_code)]` (workspace convention).

---

### `crates/famp/src/lib.rs` (modify)

**Current relevant lines (38–52):**
```rust
pub use famp_canonical::{…};
pub use famp_core::{…};
pub use famp_crypto::{…};
pub use famp_envelope::{…};

pub mod runtime;
```

**Change:** Add `pub mod cli;` directly below `pub mod runtime;` — this is the load-bearing integration point from CONTEXT "Integration Points" (§code_context line 125: `pub mod cli;` lands as a sibling of `pub mod runtime;`). **Also clean up the dead dev-only `use clap as _;`-style stanzas if clap becomes a real dep** — the current lines 22–36 silence `unused_crate_dependencies` for crates now used by `cli::*`, so `rcgen` and `tempfile` move out of the `#[cfg(test)]` block.

---

### `crates/famp/tests/init_*.rs` (5 integration test files)

**Analog:** `crates/famp-transport-http/src/tls.rs` lines 101–185 — the in-tree exemplar for "write temp files, exercise the happy path, assert typed errors via `matches!`, run under `nextest`."

**Pattern to copy (tempfile + PEM write + matches! assertion):**
```rust
// from crates/famp-transport-http/src/tls.rs:145–149
match load_pem_cert(&path) {
    Err(TlsError::NoCertificatesInPem(p)) => assert_eq!(p, path),
    other => panic!("expected NoCertificatesInPem, got {other:?}"),
}
```

**Apply to `tests/init_refuses.rs`:**
```rust
let tmp = tempfile::TempDir::new().unwrap();
// leave one stray file in the FAMP_HOME target
std::fs::write(tmp.path().join("key.ed25519"), b"stale").unwrap();

match famp::cli::init::run_at(tmp.path(), /*force=*/false) {
    Err(famp::cli::CliError::AlreadyInitialized { existing_files }) => {
        assert!(existing_files.iter().any(|p| p.ends_with("key.ed25519")));
    }
    other => panic!("expected AlreadyInitialized, got {other:?}"),
}
```

**Critical: `init_no_leak.rs` test — D-17 mechanism #3.** Read `key.ed25519` from disk after init, then scan captured stdout+stderr for any 8-byte substring of the seed. The test runs in the lib crate (not as a subprocess) per D-02, so no `assert_cmd` — capture by having `famp::cli::init::run_at` accept `impl Write` for stdout/stderr OR by running inside a helper that swaps `std::io::stdout`/`stderr`. **Prefer** threading `&mut dyn Write` parameters into `init::run_at`; that keeps the test free of env/TLS globals.

**CD-05 note:** Prefer the "Rust API route" — `init::run_at(home: &Path, force: bool, out: &mut dyn Write, err: &mut dyn Write) -> Result<InitOutcome, CliError>`. Only `main()` and one `init_env_home.rs` serial test exercise `std::env::var("FAMP_HOME")`. This avoids the `std::env::set_var` parallel-race pitfall (RESEARCH Pitfall 1).

---

### `crates/famp/Cargo.toml` (modify)

**Current state** (lines 14–40). Needs to gain `clap`, `toml`, promote `rcgen` and `tempfile` from `[dev-dependencies]` to `[dependencies]`.

**Exact additions (into `[dependencies]`):**
```toml
clap = { version = "4.6", features = ["derive"] }
toml = "1.1"
rcgen = "0.14"        # moved from dev-dependencies
tempfile = "3"        # moved from dev-dependencies
```

**Remove from `[dev-dependencies]`** the `rcgen` and `tempfile` lines (lines 36–37 today), since they become regular deps. Leave `reqwest`, `axum`, `famp-transport feature=test-util` untouched.

**Lint silencing:** with `rcgen` and `tempfile` now regular deps consumed by `cli::*`, the `#[cfg(test)] use rcgen as _;` and `#[cfg(test)] use tempfile as _;` stanzas in `crates/famp/src/lib.rs` lines 30–34 can be removed.

---

## Shared Patterns

### Secure Secret File Write (0600)
**Source:** RESEARCH §Pattern 3 (no workspace analog; this helper is new).
**Apply to:** all writes of `key.ed25519` and `tls.key.pem`.
```rust
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::fs::OpenOptions;
use std::io::Write;

pub fn write_secret(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)   // O_CREAT|O_EXCL
        .mode(0o600)
        .open(path)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    Ok(())
}
```
- `create_new` + `.mode(0o600)` together close the TOCTOU window that `set_permissions` would open.
- `.sync_all()` before drop matches project discipline (no half-fsynced state).
- Gate with `#[cfg(unix)]`; add a companion `write_public` with `.mode(0o644)` for `pub.ed25519`, `tls.cert.pem`, `config.toml`, `peers.toml`.

### Pubkey Base64url Encoding (single source of truth)
**Source:** `crates/famp-crypto/src/keys.rs` lines 122–124 (`TrustedVerifyingKey::to_b64url`).
**Apply to:** the D-15 stdout line and any future place that prints a pubkey.
```rust
let pub_line = vk.to_b64url(); // URL_SAFE_NO_PAD over 32 raw bytes
println!("{pub_line}");
```
**Do not** import `base64` directly in `cli/*` — that would create a second source of truth. The `famp-keyring` save format (`file_format.rs:73`) already uses `key.to_b64url()`; byte-for-byte match with Phase 3's `famp peer add` comes for free by routing through this one function.

### Private-Key Leakage Defense (D-17, three stacked mechanisms)
**Mechanism 1 — lock existing `FampSigningKey` `Debug`/`Display`:**
- Verified at `crates/famp-crypto/src/keys.rs` lines 97–101 (`Debug` prints literal `"FampSigningKey(<redacted>)"`).
- No `impl Display for FampSigningKey` exists anywhere in the crate — verified by grepping `impl.*Display.*FampSigningKey` returning zero hits.
- Phase 1 *locks* this with a doc-test (RESEARCH §Pattern 7) including a `compile_fail` block on `format!("{}", sk)`. Lives on the `CliError::KeygenFailed` variant site or in `cli/init/mod.rs` module docs.

**Mechanism 2 — no key bytes in `CliError`:** enforced structurally at error-definition time (D-05). Reviewer reads `cli/error.rs` once and confirms every variant carries at most a `PathBuf` or `#[source]`-wrapped error. This is the same discipline as `KeyringError` (`error.rs:10–32`) and `RuntimeError` (`error.rs:9–49`), both of which are deliberately free of key material.

**Mechanism 3 — `tests/init_no_leak.rs`:** see test pattern above. Asserts no 8+ byte substring of the on-disk seed appears in captured stdout/stderr.

### FAMP_HOME Resolution Boundary (CD-05)
**Source:** RESEARCH §Pattern 2, §Pitfall 1.
**Apply to:** every subcommand added in Phases 2–4.
- `cli::home::resolve() -> Result<PathBuf, CliError>` reads env.
- **`init::run_at(home: &Path, …)`** takes an explicit `&Path` so tests do not mutate process env.
- The bin's `main()` is the *only* caller of `cli::home::resolve()`; the bin then calls `init::run_at(&resolved, …)`. One serial test exercises the resolve path.

### Conformance Gate: init output must load unmodified by `famp-transport-http::tls`
**Source:** `crates/famp-transport-http/src/tls.rs` lines 51–64 (`load_pem_cert`, `load_pem_key`).
**Apply to:** at least one Phase 1 integration test (e.g. `init_happy_path.rs`) that, after running `init::run_at`, calls:
```rust
let cert = famp_transport_http::tls::load_pem_cert(&home.join("tls.cert.pem"))?;
let key  = famp_transport_http::tls::load_pem_key(&home.join("tls.key.pem"))?;
let _cfg = famp_transport_http::tls::build_server_config(cert, key)?;
```
This is the cross-phase gate that proves the rcgen key-algorithm / PEM-format choice from CONTEXT deferred Gray Area 3 is compatible with Phase 2's `famp listen`. If this test fails, the TLS key-algorithm decision was wrong and the planner re-picks.

### Absolute-Path Invariant
**Source:** RESEARCH §Pattern 2 + CONTEXT D-08.
- `resolve_famp_home` calls `path.is_absolute()` and returns `CliError::HomeNotAbsolute { path }` on false. **Never** call `canonicalize()` (breaks the "init creates missing dir" flow and silently promotes relative paths).

### Atomic `--force` Replacement
**Source:** RESEARCH §Pattern 4 (no in-workspace analog for directory-level atomic replace).
- `TempDir::new_in(parent_of_target)` — same-filesystem guarantee for rename atomicity.
- Two-step: rename old target → `.famp-old-{pid}`, rename staging → target, then `remove_dir_all` on the backup.
- Best-effort rollback: on the second rename failing, put the backup back.
- **Pitfall (RESEARCH §Pitfall 5):** `TempDir::keep()` vs `TempDir::into_path()` — verify exact method name for `tempfile 3.27` before coding.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `crates/famp/src/cli/home.rs` | path resolver | file-I/O (env read) | No existing env-based config-root loader in the workspace; runtime tests set paths explicitly. New pattern per RESEARCH §Pattern 2. |
| `crates/famp/src/cli/perms.rs` | unix fs mode helper | file-I/O | No workspace code currently sets Unix file modes explicitly; transport examples rely on default umask. New pattern per RESEARCH §Pattern 3. |
| `crates/famp/src/cli/init/atomic.rs` | tempdir+rename | file-I/O | No workspace code currently performs atomic directory replacement. New pattern per RESEARCH §Pattern 4. |

For all three "no analog" files, the planner should use the RESEARCH.md patterns verbatim (they are fully worked examples with pitfalls documented) rather than inventing new shapes.

---

## Metadata

**Analog search scope:** `crates/famp/`, `crates/famp-crypto/`, `crates/famp-keyring/`, `crates/famp-transport-http/`, `crates/famp/examples/`.
**Files scanned:** 14 source files read end-to-end; `.planning/milestones/v0.8-phases/01-identity-cli-foundation/01-CONTEXT.md` and `01-RESEARCH.md` read in full.
**Notable non-analogs:** no existing `clap`-using binary in the workspace; `cross_machine_two_agents.rs` uses a local `clap` struct in the example, which is the closest shape. No existing `toml`-deserializing struct in the workspace (config files are either keyring-format text or JSON envelopes) — `config.rs` / `peers.rs` will be the first TOML-backed serde types; `file_format.rs` is the closest *role* analog (line-oriented on-disk format with round-trip discipline).
**Pattern extraction date:** 2026-04-14.

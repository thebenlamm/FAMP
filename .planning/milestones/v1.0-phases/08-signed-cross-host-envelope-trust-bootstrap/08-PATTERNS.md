# Phase 8: Signed Cross-Host Envelope + Trust Bootstrap - Pattern Map

**Mapped:** 2026-07-23
**Files analyzed:** 12 (new + modified)
**Analogs found:** 12 / 12

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `crates/famp-envelope/src/wire.rs` (modify) | model | transform | itself (existing `Option`+`skip_serializing_if` fields) | exact |
| `crates/famp-envelope/src/envelope.rs` (modify) | model | transform | itself (`WireEnvelopeRef` construction sites, compile_fail doctest) | exact |
| `crates/famp-crypto/src/keys.rs` (modify: `generate()`) | utility | transform | itself (`FampSigningKey::from_bytes` pattern) | exact |
| `crates/famp-crypto/src/keys.rs` or new `fingerprint.rs` (`key_id`) | utility | transform | `TrustedVerifyingKey::to_b64url`-style helper in `keys.rs` + `sha256_digest` in `hash.rs` | exact |
| `crates/famp-gateway/src/verify.rs` (new) | service | request-response | `crates/famp-envelope/src/peek.rs` (two-phase decode) + `crates/famp-crypto/src/verify.rs` (`verify_value`) | role-match (composition of two exact analogs) |
| `crates/famp-gateway/src/error.rs` (modify: add `RejectReason`) | model | — | itself, `GatewayError` (thiserror enum) | exact |
| `crates/famp-gateway/src/identity.rs` (new) | service | file-I/O | `crates/famp/src/cli/home.rs` (`resolve_famp_home`, generate-if-absent pattern is new but path-resolution style matches) | role-match |
| `crates/famp-keyring/src/lib.rs` (no change expected; reused) | model | CRUD | itself, `Keyring::pin_tofu`/`load_from_file`/`save_to_file` | exact (reuse, no new pattern) |
| `crates/famp/src/cli/peer/mod.rs` (new) | route | request-response | `crates/famp/src/cli/daemon/mod.rs` (`Args`+`Subcommand` tree) | exact |
| `crates/famp/src/cli/peer/export.rs` (new) | controller | request-response | `crates/famp/src/cli/info.rs` (`InfoArgs`, `PeerCard`, `run`/`run_at` split) | exact |
| `crates/famp/src/cli/peer/import.rs` (new) | controller | request-response | `crates/famp/src/cli/info.rs` (`run`/`run_at` split) + `famp-keyring::file_format::parse_line` (parsing style, NOT reused verbatim) | role-match |
| Test: `famp-envelope` federation round-trip | test | transform | `crates/famp-envelope/src/envelope.rs` `sign_consumes_unsigned_and_returns_signed` test | exact |
| Test: `famp-gateway` verify_inbound unit tests | test | request-response | `crates/famp-envelope/src/peek.rs` `#[cfg(test)]` module | exact |
| Test: `famp peer export/import` round-trip (integration, subprocess) | test | event-driven | `crates/famp/tests/common/child_guard.rs` / `crates/famp-gateway/tests/common/child_guard.rs` (`ChildGuard` RAII) | exact |

## Pattern Assignments

### `crates/famp-envelope/src/wire.rs` (model, transform)

**Analog:** itself — `WireEnvelope<B>` (lines 28-53)

**Core pattern to replicate** (existing, lines 44-52):
```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub terminal_status: Option<TerminalStatus>,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub idempotency_key: Option<String>,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub extensions: Option<BTreeMap<String, serde_json::Value>>,
pub body: B,
```
Add the 7 new fields (`from_domain`, `to_domain`, `sender_key_id`, `nonce`, `expiry: Option<Timestamp>`, `capability: Option<serde_json::Value>`, `approval: Option<serde_json::Value>`) using this **exact** attribute shape, inserted before `pub body: B` (JCS sorts keys — declaration order doesn't matter, only name+presence). Preserve the file's top-of-file warning banner: **no `#[serde(flatten)]`, no `#[serde(tag=...)]`** anywhere — do not introduce a nested `FederationFields` sub-struct via flatten even if it looks cleaner (defeats `deny_unknown_fields`, see RESEARCH Pitfall 1's anti-pattern note).

---

### `crates/famp-envelope/src/envelope.rs` (model, transform)

**Analog:** itself. Three struct-literal construction sites must gain the same 7 fields in lockstep:
- `WireEnvelopeRef<'a, B>` struct definition (line 208)
- `sign()`'s `WireEnvelopeRef` literal (line 178)
- `encode()`'s `WireEnvelopeRef` literal (line 348)
- `UnsignedEnvelope<B>` public struct (line 71)
- The `# Version-drift compile_fail gate` doctest (line 44-68) — its struct literal **must be updated to list the 7 new fields** (with placeholder values matching existing style) or the doctest silently passes for the wrong reason (RESEARCH Pitfall 2).

**Action:** grep all four struct-literal sites in the same commit; add one round-trip test (`sign()` → `encode()` → `decode()`) asserting the new fields survive, per RESEARCH's recommended regression test.

---

### `crates/famp-crypto/src/keys.rs` (utility, transform) — `FampSigningKey::generate()`

**Analog:** itself — existing `FampSigningKey::from_bytes` constructor pattern (same file, `impl FampSigningKey` block).

**Pattern:**
```rust
impl FampSigningKey {
    #[must_use]
    pub fn generate() -> Self {
        use rand::rngs::OsRng;
        Self(ed25519_dalek::SigningKey::generate(&mut OsRng))
    }
}
```
Requires adding `rand = { workspace = true }` to `crates/famp-crypto/Cargo.toml` (not currently a dep there; `rand_core` feature on `ed25519-dalek` is already workspace-enabled). Follow the existing `Debug`-redaction contract already established in this file — no new leak surface.

---

### `crates/famp-crypto/src/keys.rs` — `key_id` fingerprint helper

**Analog:** existing `sha256_digest` (`hash.rs`) + `URL_SAFE_NO_PAD` b64url encoding already used for `TrustedVerifyingKey::to_b64url`-style methods in `keys.rs`.

**Pattern:**
```rust
use crate::hash::sha256_digest;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

pub fn key_id(vk: &TrustedVerifyingKey) -> String {
    let digest = sha256_digest(vk.as_bytes());
    URL_SAFE_NO_PAD.encode(digest).chars().take(16).collect()
}
```
Zero new deps — reuses two already-exported primitives.

---

### `crates/famp-gateway/src/verify.rs` (new — service, request-response)

**Analog 1 (two-phase decode):** `crates/famp-envelope/src/peek.rs` — `peek_sender(bytes: &[u8]) -> Result<Principal, EnvelopeDecodeError>` (full file read; ~34 lines). This is the exact "peek `from` before you know which key to verify with" primitive, already used by v0.8's HTTP sig-verify middleware.

**Analog 2 (verification call):** `crates/famp-crypto/src/verify.rs` — `verify_value<T>(verifying_key, value, signature) -> Result<(), CryptoError>` (lines 22-28), which internally routes through `verify_strict` only, never plain `verify`.

**Composed pattern to build:**
```rust
pub enum RejectReason {
    InvalidSignature,
    UnpinnedKey { principal: Principal },
}

pub fn verify_inbound<B: BodySchema>(
    bytes: &[u8],
    keyring: &Keyring,
) -> Result<SignedEnvelope<B>, RejectReason> {
    let peeked_from = famp_envelope::peek_sender(bytes)
        .map_err(|_| RejectReason::InvalidSignature)?;
    let Some(vk) = keyring.get(&peeked_from) else {
        return Err(RejectReason::UnpinnedKey { principal: peeked_from });
    };
    SignedEnvelope::decode(bytes, vk).map_err(|_| RejectReason::InvalidSignature)
}
```
D-08 requires this stay a pure function: no bus write, no state mutation on either error path — matches the "data-as-input over synthetic routing" project convention already logged in `learned-rules.md`.

**Cargo.toml change:** add `famp-keyring = { path = "../famp-keyring", version = "0.11.0" }` as a direct dep of `famp-gateway` (currently missing).

---

### `crates/famp-gateway/src/error.rs` (modify — add `RejectReason` or fold in)

**Analog:** itself — `GatewayError` (full file, 8-45). Same thiserror shape, same doc-comment discipline (each variant explains *why*, references the design decision by name, e.g. GW-04's `DuplicatePrincipal`):
```rust
#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("io error talking to broker")]
    Io(#[source] std::io::Error),
    ...
}
```
Follow this exactly for `RejectReason` (or add variants to `GatewayError` if the planner chooses to fold it in) — two variants only, `InvalidSignature` and `UnpinnedKey { principal }`, matching D-08's two-reason split. Do NOT resurrect the orphaned `CliError::TlsFingerprintMismatch`/`TofuBootstrapRefused`/`KeyringBuildFailed` fossils in `crates/famp/src/cli/error.rs` — unrelated shape (TLS-cert era), per RESEARCH Pitfall 5.

---

### `crates/famp-gateway/src/identity.rs` (new — service, file-I/O)

**No direct analog exists** — this is genuinely new surface (RESEARCH Pitfall 3: no live keygen/persistence path exists anywhere in the codebase today). Closest structural precedent for "resolve a path under a home dir, read-or-create":
- `crates/famp/src/cli/home.rs` — `resolve_famp_home()` (env-var-then-default path resolution style, error type `CliError::HomeNotSet`/`HomeNotAbsolute`).
- `crates/famp/src/cli/paths.rs` — `IdentityLayout` (canonical filename constants joined onto a home dir) — mirror this pattern for the NEW gateway keypair path (recommend `~/.famp/gateway/identity.ed25519`, deliberately not reusing the stale `key.ed25519` name per Pitfall 3).

**Pattern to write:** `load_or_generate(path: &Path) -> Result<FampSigningKey, ...>` — read file if present, else `FampSigningKey::generate()` + write, following the `Io { path, source }` error-wrapping style used throughout `cli/info.rs` (`std::fs::read(...).map_err(|e| CliError::Io { path, source: e })`).

---

### `crates/famp-keyring` (no code change expected — pure reuse)

**Analog:** itself. `Keyring::pin_tofu`, `load_from_file`, `save_to_file` (`lib.rs`) — TRUST-01/02 mechanism already complete. `file_format::parse_line`/`serialize_entry` (2-field format) stays as-is for the on-disk keyring file; **do not extend it to 3 fields** — the export/import blob's 3rd field (fingerprint) is a CLI-layer concern only (RESEARCH "Alternatives Considered").

---

### `crates/famp/src/cli/peer/mod.rs` (new — route, request-response)

**Analog:** `crates/famp/src/cli/daemon/mod.rs` (full file, lines 1-40+) — `Args`+`Subcommand` tree pattern:
```rust
#[derive(Args, Debug)]
pub struct DaemonArgs {
    #[command(subcommand)]
    pub command: DaemonSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum DaemonSubcommand {
    /// doc comment becomes --help text
    Install(install::DaemonInstallArgs),
    ...
}
```
Mirror exactly for `PeerArgs { command: PeerSubcommand }` with `Export(export::PeerExportArgs)` / `Import(import::PeerImportArgs)`. Wire into `crates/famp/src/cli/mod.rs`'s `Commands` enum the same way `Daemon(daemon::DaemonArgs)` is wired (enum variant line 153 + match-arm dispatch line 212 — grep both, they must move together).

---

### `crates/famp/src/cli/peer/export.rs` (new — controller, request-response)

**Analog:** `crates/famp/src/cli/info.rs` (full file read) — `InfoArgs` struct, `PeerCard` output struct, and critically the **`run()` / `run_at()` split**:
```rust
pub fn run(args: &InfoArgs) -> Result<PeerCard, CliError> {
    let home_path = home::resolve_famp_home()?;
    let mut stdout = std::io::stdout().lock();
    run_at(&home_path, args, &mut stdout)
}

pub fn run_at(home: &Path, args: &InfoArgs, out: &mut dyn std::io::Write) -> Result<PeerCard, CliError> {
    ...
}
```
This split (production entrypoint reads env/home; test-facing entrypoint takes explicit `&Path` + writer) is the established convention (`home.rs`'s own top comment: "every other call site takes `&Path` explicitly to avoid the `std::env::set_var` parallel-test race") — replicate it for `PeerExportArgs`/`run`/`run_at`. Output is the new 3-field line (see Code Examples in RESEARCH), not the `PeerCard` JSON struct — but the `--format json|text` flag idea and b64url encoding (`URL_SAFE_NO_PAD`) are directly reusable from this file's imports.

---

### `crates/famp/src/cli/peer/import.rs` (new — controller, request-response)

**Analog:** `crates/famp/src/cli/info.rs`'s `run`/`run_at` split (same as export) for the CLI shape; `crates/famp-keyring/src/file_format.rs`'s `parse_line` (lines 30-60) for **parsing style only** — same whitespace-split + explicit error-per-missing-field idiom, but do NOT call `parse_line` itself (2-field strict format, rejects a 3rd token — RESEARCH anti-pattern). Write a sibling `parse_export_line` in the CLI layer using the identical idiom:
```rust
let mut parts = line.trim().split_whitespace();
let principal_str = parts.next().ok_or(PeerError::Malformed("missing principal"))?;
let pubkey_str = parts.next().ok_or(PeerError::Malformed("missing pubkey"))?;
let fingerprint_str = parts.next(); // optional 3rd field
```
Then call `Principal::from_str` + `TrustedVerifyingKey::from_b64url` (same primitives `parse_line` uses internally) followed by `Keyring::pin_tofu` + `Keyring::save_to_file`.

---

### Tests

**Envelope round-trip test** — analog: `crates/famp-envelope/src/envelope.rs`'s existing `sign_consumes_unsigned_and_returns_signed` test (in the `#[cfg(test)] mod tests` at bottom of file). Same style: build `UnsignedEnvelope`, `.sign(&sk)`, `.encode()`, `decode()`, assert equality — extend to populate all 7 new fields.

**Gateway verify unit tests** — analog: `crates/famp-envelope/src/peek.rs`'s `#[cfg(test)] mod tests` (bottom of file) — small, focused `#[test] fn` per case (missing field, malformed, happy path). Mirror for `verify_inbound_rejects_unsigned`, `_rejects_bad_signature`, `_rejects_unpinned_key`.

**`famp peer export/import` integration round-trip** — analog: `crates/famp-gateway/tests/common/child_guard.rs` and `crates/famp/tests/common/child_guard.rs` — **MUST** use the existing `ChildGuard` RAII (kill+wait on drop) for any subprocess spawned in this test, per the project's `test_child_guard_convention` memory note, even though this is single-machine/in-process per CONTEXT.md `<specifics>` (no live broker/gateway process needed for the export→import→verify assertion — but if the test shells out to the `famp` binary via `assert_cmd`, wrap any spawned child in `ChildGuard`).

## Shared Patterns

### No-`flatten`/no-`tag` serde discipline
**Source:** `crates/famp-envelope/src/wire.rs` lines 1-8 (crate warning), `crates/famp-envelope/src/lib.rs` top-of-file
**Apply to:** `wire.rs`, `envelope.rs` — every new federation field must be a plain `Option<T>` member, never composed via `#[serde(flatten)]`/`#[serde(tag)]`.

### Loud, disambiguated errors (never a flat "rejected")
**Source:** `crates/famp-gateway/src/error.rs` (`GatewayError` — each variant documents *why*, references the decision by name)
**Apply to:** `RejectReason` in `verify.rs`; `PeerError` (new) in `cli/peer/`.

### `run`/`run_at` production-vs-test entrypoint split
**Source:** `crates/famp/src/cli/info.rs` `run()`/`run_at()`; rationale in `crates/famp/src/cli/home.rs` top comment (env race avoidance)
**Apply to:** `cli/peer/export.rs`, `cli/peer/import.rs`.

### `verify_strict`-only crypto surface
**Source:** `crates/famp-crypto/src/verify.rs` (doc comments explicitly forbid plain `verify`)
**Apply to:** `verify_inbound` in `famp-gateway` — must go through `TrustedVerifyingKey`/`SignedEnvelope::decode`, never construct a raw `ed25519_dalek::VerifyingKey`.

### `ChildGuard` RAII for any spawned test process
**Source:** `crates/famp/tests/common/child_guard.rs`, `crates/famp-gateway/tests/common/child_guard.rs`
**Apply to:** any Phase 8 integration test that shells out to the `famp` binary.

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `crates/famp-gateway/src/identity.rs` (`load_or_generate` keypair persistence) | service | file-I/O | No live keygen/persistence code path exists anywhere in the current codebase (RESEARCH Pitfall 3) — nearest precedent is the *reading* side (`cli/info.rs`, `cli/home.rs`/`paths.rs`) but there is no *writer* analog to copy; planner should treat this as new-pattern surface, following `IdentityLayout`'s path-joining style and `cli/info.rs`'s `Io { path, source }` error-wrapping only. |

## Metadata

**Analog search scope:** `crates/famp-envelope/src`, `crates/famp-crypto/src`, `crates/famp-keyring/src`, `crates/famp-gateway/src`, `crates/famp/src/cli/{daemon,info.rs,home.rs,paths.rs}`, `crates/famp{,-gateway}/tests/common`
**Files scanned:** ~20 (full or targeted reads)
**Pattern extraction date:** 2026-07-23

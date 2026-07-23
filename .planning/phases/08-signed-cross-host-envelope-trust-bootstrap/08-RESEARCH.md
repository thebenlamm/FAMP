# Phase 8: Signed Cross-Host Envelope + Trust Bootstrap - Research

**Researched:** 2026-07-23
**Domain:** Rust workspace — envelope wire-format extension, Ed25519 trust bootstrap CLI, gateway ingress verification
**Confidence:** HIGH (code-grounded; every claim below is read directly from the current FAMP source tree, not inferred from training data)

## Summary

Phase 8 is almost entirely a **code-reading exercise, not a library-research exercise** — every reused primitive (`famp-crypto` Ed25519, `famp-canonical` JCS, `famp-keyring` TOFU pinning) already exists and works. The real design work is threading three new pieces through existing seams: (1) seven new optional, omit-when-empty fields on the *one* envelope wire struct, added in **three parallel places** that must stay in lockstep; (2) a brand-new `famp peer export`/`famp peer import` CLI surface that reuses `famp-keyring`'s pin/TOFU machinery but needs a **new 3-field export blob format** (principal + pubkey + human fingerprint) since the existing 2-field `parse_line`/`serialize_entry` format has no room for a fingerprint; and (3) a pure `verify_inbound(bytes, &keyring)` function in `famp-gateway` that composes `famp_canonical::from_slice_strict` → `famp_crypto::verify_value` → keyring lookup.

The single most important finding of this research is **not** in the phase's stated scope: **there is currently no live code path anywhere in the v0.9+/v0.11 codebase that generates or persists an Ed25519 signing keypair.** The old `key.ed25519`/`pub.ed25519` file layout (`IdentityLayout`, `cli/paths.rs`, `cli/home.rs`) is read by the still-wired `famp info` command, but the commands that used to *write* those files (`famp init`/`famp setup`/`famp keygen`) were hard-deleted in the v0.9 Phase 4 CLI purge. `famp info` today has no live producer for its own input files. Phase 8's plan must include keypair generation + persistence as first-class scope, not assume it falls out of "reusing famp-crypto as-is."

**Primary recommendation:** Add the 7 federation fields as `Option<T>` + `skip_serializing_if` to `WireEnvelope<B>`, `UnsignedEnvelope<B>`, and `WireEnvelopeRef<'a, B>` in lockstep (JCS key-sorting makes field *declaration order* irrelevant — only name + presence matter); add `FampSigningKey::generate()` to `famp-crypto` (the `rand_core` feature is already workspace-enabled on `ed25519-dalek`, just needs `rand` added to `famp-crypto`'s own `Cargo.toml`); build `famp peer export`/`import` as a new `crates/famp/src/cli/peer/` module (mirroring the existing `cli/daemon/` subcommand-tree pattern) that generates+persists the gateway's own keypair on first use, writes to `~/.famp/gateway/peers.keyring`, and reuses `Keyring::pin_tofu` for the actual trust decision; build `verify_inbound` as a pure function in `famp-gateway` taking `(bytes, &Keyring)` per the locked D-07 decision.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Envelope wire-format extension (WIRE-02) | Protocol primitive (`famp-envelope`) | — | Transport-neutral; must stay identical whether the envelope later rides HTTP (Phase 9), a future relay (v1.1), or a test harness |
| Ed25519 sign/verify + domain prefix (WIRE-01) | Protocol primitive (`famp-crypto`) | — | Already exists; INV-10 enforced at the type level, no new crypto logic needed |
| key_id / fingerprint derivation | Protocol primitive (`famp-crypto`) | CLI display (`famp peer export`) | Deriving a fingerprint from a pubkey is a pure crypto-adjacent function; the CLI only formats it for humans |
| Keypair generation + persistence (NEW — not in CONTEXT.md scope explicitly, but load-bearing) | Gateway process (`famp-gateway`) or CLI (`famp peer` / a new `famp-gateway` identity file) | Protocol primitive (`famp-crypto::FampSigningKey::generate`) | The signing key is the gateway's own machine identity, not a session/MCP concept — must be a persistent, gateway-owned file, generated once |
| TOFU pin/keyring storage (TRUST-01/02) | CLI (`famp peer export`/`import`) writes; Gateway (`famp-gateway`) reads | Protocol primitive (`famp-keyring`) | `famp-keyring::Keyring` is the reused mechanism; the CLI and gateway are two different processes sharing one on-disk file |
| Ingress signature + trust verification (WIRE-01, TRUST-02) | Gateway (`famp-gateway`) | — | Locked D-07: pure, transport-agnostic `verify_inbound(bytes, &keyring)` — no HTTP/transport dependency this phase (that's Phase 9) |
| Canonical JSON (JCS) round-trip (WIRE-02) | Protocol primitive (`famp-canonical`) | — | Already exists (`serde_jcs`), sorts keys lexicographically — irrelevant to struct field order |

## User Constraints (from CONTEXT.md)

<user_constraints>

### Locked Decisions

- **D-01 — One envelope, one signature.** Extend the existing `famp-envelope` wire type with the federation fields as **optional, `skip_serializing_if`-omitted-when-empty** additions — `from_domain`, `to_domain`, `sender_key_id`, `nonce`, `expiry`, plus reserved `capability` / `approval`. All covered by the *single existing INV-10 signature*. No nested/double-signed outer wrapper.
- **D-02 — Local path stays byte-identical.** Every new field is omit-when-empty, so a local-bus envelope serializes to the exact bytes it does today. The cross-host path is the only place the gateway populates + signs the federation fields. Preserve the no-`serde(flatten)` / no-`serde(tag)` discipline.
- **D-03 — Domain = `Principal.authority`.** `from_domain`/`to_domain` derive from the sender/receiver authority. `key_id` = a stable fingerprint of the Ed25519 verifying key (recommend `b64url(sha256(pubkey))` truncated to a documented length, human-comparable). Researcher confirms exact key_id derivation + length; planner locks it.
- **D-04 — Carry + sign both nonce/expiry; actively enforce neither this phase.** `nonce` (random 128-bit) and `expiry` (absolute timestamp) are populated and covered by the signature, but Phase 8 does NOT build a replay cache and does NOT reject on expiry. Format is validated only.
- **D-05 — Reuse the peer-card + keyring line format.** Add `famp peer export --as <name>` emitting a single, copy/paste-safe line (principal + `b64url` pubkey + a human-readable key_id/fingerprint). Add `famp peer import [<file>|-]` that parses via the existing `famp-keyring` `parse_line` and pins via `Keyring::pin_tofu`. No key material ever traverses FAMP.
- **D-06 — Dedicated gateway peer keyring on disk.** The gateway reads/writes a peer keyring at a stable path under `~/.famp/` (recommend `~/.famp/gateway/peers.keyring`), separate from any per-session identity store.
- **D-07 — Verify is a pure, transport-agnostic function.** `verify_inbound(bytes, &keyring) -> Result<SignedEnvelope, RejectReason>` in `famp-gateway`: canonical-decode → `verify_strict` over `FAMP-sig-v1\0` → keyring lookup requiring the signing key to match.
- **D-08 — Two distinct, loud rejection reasons, no state, no bus write.** `invalid_signature` (bad crypto/unsigned) vs `unpinned_key` (unknown peer), logged at `warn` with sender principal + key_id.

### Claude's Discretion

- Exact `key_id` derivation function + truncation length (D-03) — researcher confirms, planner locks. **Resolved below** (see Code Examples / key_id section).
- Exact on-disk peer-keyring path (D-06) — planner confirms against `paths.rs` / `famp home`. **Resolved below**: `~/.famp/gateway/peers.keyring`, reachable via the existing `home::resolve_famp_home()` helper (already used by `famp info`).
- CLI noun/verb final spelling (`famp peer export/import` vs a `gateway` subcommand) — planner picks against existing clap tree in `cli/mod.rs`; recommend top-level `famp peer` to mirror `famp info`. **Recommended below**: follow the `cli/daemon/` subcommand-tree pattern exactly (`PeerArgs { command: PeerSubcommand }`).

### Deferred Ideas (OUT OF SCOPE)

- Active nonce/replay cache + expiry rejection — v1.1.
- Public-internet dumb relay — v1.1.
- Cross-person trust + signed peer directory — v1.1.
- FAMP-Sec capability/approval/tool-admission plane — v2.0+; only reserved wire fields here.
- Live two-process HTTP cross-host cycle — Phase 9 (GW-01/02/03).
- Deferred federation test triage (`crates/famp/tests/_deferred_v1/`) — Phase 10 (TEST-01/02).

</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| WIRE-01 | Every cross-host envelope is Ed25519-signed under `FAMP-sig-v1\0`; unsigned/invalid rejected at the gateway before touching the local bus | `verify_value`/`verify_strict` already exist and are `deny_unknown_fields`-safe (envelope.rs); `verify_inbound` composes them — see Code Examples §4 |
| WIRE-02 | Envelope carries sender/receiver domain + key_id, nonce, expiry, capability/approval omitted-when-empty, byte-exact JCS round-trip | JCS canonicalizer (`serde_jcs`) sorts object keys lexicographically — struct field *declaration* order is irrelevant to byte output, only field *name* + *presence*; `Timestamp` is directly reusable for `expiry` (RFC 3339, byte-preserving) — see Envelope Extension section |
| TRUST-01 | `famp peer export`/`import` round-trip establishes mutual TOFU trust, no key material over FAMP | `Keyring::pin_tofu` + `load_from_file`/`save_to_file` already exist and are exactly this mechanism; the missing piece is the export/import CLI + a NEW 3-field blob format (existing 2-field format has no room for a fingerprint) — see Trust Bootstrap CLI section |
| TRUST-02 | Envelope from unpinned key rejected, no state, no implicit trust | `Keyring::get(principal)` returns `None` for an unpinned principal; `verify_inbound` must treat that as a hard reject before any bus write — see Code Examples §4 |
</phase_requirements>

## Standard Stack

### Core (all already in the workspace — no new external packages)

| Library | Version (workspace-pinned) | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `ed25519-dalek` | 2.2.0 (`features = ["std","zeroize","rand_core"]`) | Ed25519 sign/verify | Already the sole crypto backend (`famp-crypto`); `rand_core` feature is **already enabled at the workspace level** — `SigningKey::generate()` is available today without a feature-flag change |
| `sha2` | 0.11.0 | SHA-256 for `key_id` fingerprint | Already a dep of `famp-crypto`; `sha256_digest(&[u8]) -> [u8;32]` is already `pub` and exported |
| `base64` | 0.22.1 | b64url encode/decode | Already used throughout (`TrustedVerifyingKey::to_b64url`, CLI `info.rs`) |
| `rand` | 0.8 (`features=["std","std_rng"]`) | CSPRNG for keypair generation | Already a workspace dep (used by `famp` crate); NOT currently a dep of `famp-crypto` — must be added there |
| `serde_jcs` | (pinned via `famp-canonical`) | RFC 8785 JCS canonicalization | Already the byte-exact substrate every signature is computed over |
| `thiserror` | 2.x (workspace) | Typed error enums | Existing project convention (`GatewayError`, `KeyringError`, `EnvelopeDecodeError`) |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `clap` (derive) | workspace-pinned | New `famp peer export`/`import` subcommands | Follow the `cli/daemon/` `Args`+`Subcommand` pattern exactly |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Extending `WireEnvelope` in place (D-01, locked) | A separate `CrossHostEnvelope` wrapper with its own signature | Rejected in CONTEXT.md — double-sign, two canonical forms to keep byte-exact. Not reconsidered here. |
| `sha2`-based key_id | A KDF (`HKDF`) or a distinct hash family | Unnecessary — key_id is a diagnostic/UX label, not itself a trust anchor (the trust anchor is the pinned 32-byte pubkey in the keyring); plain SHA-256 truncation is standard practice (SSH/GPG fingerprint precedent) and avoids a new dependency |
| CLI-layer 3-field export blob | Extending `famp-keyring::file_format` to support an optional 3rd field | Either works; recommend CLI-layer (see Trust Bootstrap CLI section) to avoid growing the keyring file format's parse surface for a CLI-only concern (fingerprint is advisory, not authoritative — the keyring file itself stays the clean 2-field format `Keyring::save_to_file` already produces) |

**Installation:**
```bash
# famp-crypto/Cargo.toml — add:
rand = { workspace = true }

# famp-gateway/Cargo.toml — add (currently missing; needed for verify_inbound + Keyring):
famp-keyring = { path = "../famp-keyring", version = "0.11.0" }
# famp-crypto / famp-envelope / famp-core types are already reachable via the
# `famp` path-dep's public re-exports (see famp/src/lib.rs `pub use famp_crypto::{...}`,
# `pub use famp_envelope::{...}`, `pub use famp_core::{...}`) — no new direct deps needed
# for those three.
```

**Version verification:** All versions above were read directly from `Cargo.toml` / the workspace root `Cargo.toml` in this session — `[VERIFIED: workspace Cargo.toml]`. No registry lookup needed since nothing new is being pulled from the public registry (only an existing already-vendored dependency, `rand`, is being added to one more workspace member, and an already-workspace-member crate, `famp-keyring`, is being added as a direct dep of another workspace member).

## Package Legitimacy Audit

**No new external packages are introduced by this phase.** Every dependency change is either (a) an already-workspace-pinned external crate (`rand`) being added to one more internal crate's `Cargo.toml`, or (b) an already-existing internal workspace member (`famp-keyring`) being added as a direct dependency of another internal workspace member (`famp-gateway`). Neither is a new supply-chain surface — both are already vetted, already in `Cargo.lock`, already used elsewhere in this exact codebase.

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `rand` 0.8 | crates.io | already vendored workspace-wide | already vendored | github.com/rust-random/rand | N/A — already in use | No audit needed; adding as a dep of one more workspace crate |
| `famp-keyring` | internal workspace member | pre-existing crate in this repo | N/A | N/A (internal) | N/A | No audit needed; internal crate |

**Packages removed due to [SLOP] verdict:** none.
**Packages flagged as suspicious [SUS]:** none.

## Architecture Patterns

### System Architecture Diagram

```
                         MACHINE A                                    MACHINE B
                    ┌─────────────────┐                          ┌─────────────────┐
  operator (Ben) ──▶│ famp peer export│                          │                 │
                    │  --as <name>    │──(1) one-line blob──┐     │                 │
                    └─────────────────┘   (Signal/clipboard) │     │                 │
                                                              ▼     │                 │
                                                    ┌───────────────────┐             │
                                                    │ famp peer import  │             │
                                                    │ (parses blob,      │             │
                                                    │  Keyring::pin_tofu)│             │
                                                    └────────┬───────────┘             │
                                                             │ writes                  │
                                                             ▼                         │
                                              ~/.famp/gateway/peers.keyring (machine B) │
                                                             │ read at verify-time      │
                                                             ▼                         │
  [Phase 9: not built yet] inbound bytes ──▶ verify_inbound(bytes, &keyring) ──▶ SignedEnvelope | RejectReason
                                                     │                    │
                                       canonical-decode              (invalid_signature |
                                       (from_slice_strict)             unpinned_key)
                                                     │                    │
                                       verify_value / verify_strict       ▼
                                       (FAMP-sig-v1\0 domain prefix)   log at warn,
                                                     │                 NO bus write,
                                       keyring.get(from_principal)      NO state created
                                       must match signing key
                                                     │
                                                     ▼
                                         only a verified SignedEnvelope
                                         reaches GatewayRegistry::back()
                                         and the local bus (Phase 9 wires
                                         the actual delivery hop)
```

The reverse direction (B → A) is symmetric: `famp peer export --as <name>` on B, out-of-band move, `famp peer import` on A.

### Recommended Project Structure

```
crates/famp-envelope/src/
├── wire.rs           # WireEnvelope<B> gains 7 new Option fields (D-01/D-02)
├── envelope.rs        # UnsignedEnvelope<B> + WireEnvelopeRef<'a,B> gain the SAME 7 fields;
│                      # new accessor methods on SignedEnvelope; new builder methods on
│                      # UnsignedEnvelope (`.with_from_domain(...)`, etc.)
├── federation.rs      # NEW (optional) — if the 7-field group grows unwieldy inline in
│                      # wire.rs/envelope.rs, consider a small FederationFields sub-struct
│                      # embedded via plain (non-flatten) field — see Pitfall 1 below for
│                      # why this must NOT use #[serde(flatten)]

crates/famp-crypto/src/
├── keys.rs            # FampSigningKey::generate() — NEW
├── keys.rs            # TrustedVerifyingKey fingerprint/key_id helper — NEW (or leave at
│                      # call site using existing sha256_digest — see Code Examples §2)

crates/famp-gateway/src/
├── verify.rs           # NEW — verify_inbound(bytes, &Keyring) -> Result<SignedEnvelope, RejectReason>
├── error.rs            # RejectReason enum (or fold into GatewayError) — NEW
├── identity.rs         # NEW — gateway's own keypair generation + persistence
                         # (~/.famp/gateway/identity.ed25519 or similar — see Pitfall 3)

crates/famp/src/cli/peer/
├── mod.rs              # PeerArgs { command: PeerSubcommand }, mirrors cli/daemon/mod.rs
├── export.rs           # famp peer export --as <name>
├── import.rs           # famp peer import [<file>|-]
```

### Pattern 1: Omit-when-empty federation fields (D-01/D-02)

**What:** Add each new field as `#[serde(default, skip_serializing_if = "Option::is_none")] pub field: Option<T>` to `WireEnvelope<B>`, exactly matching the existing pattern already used for `causality`, `terminal_status`, `idempotency_key`, `extensions`.

**When to use:** Always for this phase — this is the locked pattern (D-01/D-02), not a choice.

**Example:**
```rust
// Source: crates/famp-envelope/src/wire.rs (existing pattern, lines 45-54),
// extended per D-01/D-02.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound(/* ... unchanged ... */))]
pub(crate) struct WireEnvelope<B: BodySchema> {
    // ... existing fields unchanged ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<BTreeMap<String, serde_json::Value>>,

    // --- NEW (Phase 8, D-01) — order here does NOT affect JCS output,
    // only field NAME + PRESENCE matter (serde_jcs sorts object keys
    // lexicographically at canonicalize time) ---
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_domain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_domain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender_key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expiry: Option<crate::Timestamp>,   // reuse Timestamp — see below
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<serde_json::Value>,  // reserved, never interpreted (v2.0+)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval: Option<serde_json::Value>,    // reserved, never interpreted (v2.0+)

    pub body: B,
}
```

**`Timestamp` reuse confirmed:** `crates/famp-envelope/src/timestamp.rs` is an opaque, byte-preserving RFC 3339 string with only a *shallow* format check (no `OffsetDateTime` round-trip). This is exactly the byte-exactness contract `expiry` needs and it is already `pub use`d from the crate root — `Option<Timestamp>` is a direct, zero-new-code reuse.

**`nonce` needs no new type.** D-04 specifies "random 128-bit," but the nonce is not actively enforced this phase (no replay cache) — a plain `Option<String>` (b64url-encoded 16 random bytes, generated by whichever process signs the envelope — the gateway, in Phase 9) is sufficient. No new `Nonce` newtype is needed in `famp-envelope` since there is no round-trip-normalization risk (unlike `Timestamp`) — a random opaque string has no canonical form to preserve.

### Pattern 2: `Keyring`-backed TOFU trust (already exists, D-07)

**What:** `famp-keyring::Keyring` is a `HashMap<Principal, TrustedVerifyingKey>` with `pin_tofu` (first-sight pin, fails closed on conflict), `get`, `load_from_file`/`save_to_file`.

**When to use:** `famp peer import` calls `pin_tofu` then `save_to_file`; `verify_inbound` calls `load_from_file` (or receives an already-loaded `&Keyring`) then `get`.

**Example:**
```rust
// Source: crates/famp-keyring/src/lib.rs (existing, unmodified)
pub fn pin_tofu(&mut self, principal: Principal, key: TrustedVerifyingKey)
    -> Result<(), KeyringError>;   // Ok on first-sight or idempotent re-pin,
                                   // Err(KeyConflict) on a DIFFERENT key for
                                   // an already-pinned principal
```

### Anti-Patterns to Avoid

- **`#[serde(flatten)]` or `#[serde(tag = ...)]` anywhere in the envelope** — the crate-level warning in `lib.rs`/`wire.rs` is explicit: this is the only serde composition in serde 1.0.228 that actually enforces `deny_unknown_fields` on both envelope and body. A "cleaner" nested `FederationFields` struct added via `#[serde(flatten)]` would silently defeat `deny_unknown_fields` and reopen the exact vulnerability class this codebase has already hardened against.
- **Reusing the old `famp::cli::peer` / `famp::cli::init` / `famp::cli::setup` CLI surface from `crates/famp/tests/_deferred_v1/`** — that surface (TOML `peers.toml`, `alias`/`endpoint`/`pubkey_b64`/`tls_fingerprint_sha256` shape, `CliError::PeerDuplicate`/`PeerPubkeyInvalid`/etc.) was **hard-deleted** in the v0.9 Phase 4 CLI purge; the tests survive only as dormant "intent documents," and several of the `CliError` variants they reference were subsequently deleted too (commit `260708-g01`). Do not resurrect this design — Phase 8's `famp peer` is a fresh build against `famp-keyring`, not a reactivation.
- **Reusing `famp-keyring::file_format::parse_line`/`serialize_entry` verbatim for the export/import blob** — that format is strictly 2 whitespace-separated fields (principal, pubkey) with `deny inline '#'` and rejects any trailing content past the 2nd token. D-05's export blob needs a 3rd field (human fingerprint). Feeding a 3-field line to `parse_line` will hit `"unexpected trailing content"`. Build a small CLI-layer parser instead (see Code Examples §3).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Ed25519 sign/verify | A raw `ed25519_dalek::VerifyingKey::verify` call | `famp_crypto::verify_value` / `verify_strict` via `TrustedVerifyingKey` | Plain `verify` accepts malleable signatures and small-order points; `verify_strict` (the only path `TrustedVerifyingKey` allows) closes both holes. Already enforced at the type level — do not add a second verification code path. |
| Canonical JSON | A hand-written key-sorter | `famp_canonical::canonicalize` (`serde_jcs`) | RFC 8785 JCS compliance is load-bearing for interop; already the substrate every existing signature is computed over |
| Duplicate-key JSON parsing | `serde_json::from_slice` directly on inbound bytes | `famp_canonical::from_slice_strict` | Plain `serde_json` silently merges duplicate object keys; strict parse rejects at any depth — this is a documented FAMP protocol guarantee, not a hygiene nicety |
| TOFU pin/conflict detection | A new `HashMap` + manual conflict check in the gateway or CLI | `famp_keyring::Keyring::pin_tofu` | Already implements exactly this semantics (idempotent re-pin, fails closed on key conflict) |
| CSPRNG keypair generation | Hand-rolled `[u8;32]` from `std::time`/PID-based entropy | `ed25519_dalek::SigningKey::generate(&mut OsRng)` (already feature-enabled workspace-wide) | Never construct a signing key from anything but a CSPRNG; the crate's own doc comments explicitly call out that `from_bytes([0u8;32])`-style fixed seeds are test-only |

**Key insight:** every crypto/canonicalization primitive this phase needs already exists, is already tested against KATs/RFC vectors, and is already the sole sanctioned path in this codebase. The only genuinely new logic is (a) wiring — adding fields, adding a keyring dependency, adding a CLI subcommand tree — and (b) the previously-unaccounted-for keypair generation/persistence step.

## Runtime State Inventory

> Not a rename/refactor/migration phase — omitted per the trigger condition in the verification protocol. This phase adds new fields/files; it does not rename or move existing runtime state. (One related note: `~/.famp/key.ed25519`/`pub.ed25519`/`config.toml`/`peers.toml` under `IdentityLayout` are dead-in-practice files with no live writer — see Common Pitfall 3. This is a pre-existing condition Phase 8 should not "fix" by resurrecting the old writer, but the planner should be aware the files are effectively vestigial so as not to accidentally build on top of them without a fresh keygen step.)

## Common Pitfalls

### Pitfall 1: Three parallel struct definitions must add identical fields, in lockstep

**What goes wrong:** `WireEnvelope<B>` (wire.rs, serde ser+de target), `UnsignedEnvelope<B>` (envelope.rs, public builder type), and `WireEnvelopeRef<'a, B>` (envelope.rs, private borrowing serialize-only view used by both `sign()` and `encode()`) are three separate struct declarations that must carry the *same* field set with the *same* serde attributes, or signing/encoding will silently omit a field that decoding expects (or vice versa).

**Why it happens:** `sign()` builds a `WireEnvelopeRef` view to avoid cloning `B`; `encode()` builds another `WireEnvelopeRef`; `decode_value()` deserializes into `WireEnvelope<B>` directly. Three call sites, three struct literals — easy to update two and forget the third.

**How to avoid:** When adding the 7 new fields, grep all three struct definitions AND the two struct-literal *construction* sites (`sign()`, `encode()`) in `envelope.rs` in the same commit. Add a single test that round-trips an envelope with every new field populated through `sign()` → `encode()` → `decode()` and asserts equality — this test will fail loudly if any of the three struct shapes drifts.

**Warning signs:** A new field serializes on `encode()` but decoding drops it silently, or `sign()`'s computed signature doesn't match what `encode()` later emits (because the *signed* view and the *encoded* view came from two different, now-diverged, struct shapes).

### Pitfall 2: The `UnsignedEnvelope` compile-fail doctest enumerates every field by name

**What goes wrong:** `envelope.rs`'s `# Version-drift compile_fail gate` doctest (lines ~46-68) is a `compile_fail` block that constructs `UnsignedEnvelope` as a **struct literal listing every field**, deliberately trying (and failing) to set the private `famp` field directly. Adding new `pub` fields to `UnsignedEnvelope` without adding them to this struct literal means the doctest would fail to compile for the WRONG reason (missing required fields) rather than the intended reason (private field). Since it's `compile_fail`, the test would still "pass" (any compile error satisfies `compile_fail`) — silently defeating the intended narrow assertion without failing CI.

**Why it happens:** `compile_fail` doctests only assert *that* a compile error occurs, not *which* one. A missing-field error and a private-field error both satisfy it equally.

**How to avoid:** When adding the 7 new fields to `UnsignedEnvelope`, update this doctest's struct literal to include them (with `unimplemented!()` or `None`, matching the existing style), preserving the property that the ONLY error the doctest should trip is "field `famp` is private."

**Warning signs:** None visible from `cargo test` output alone (a passing `compile_fail` doctest gives no signal about *why* it failed) — must be checked by inspection when touching this file.

### Pitfall 3: There is no live Ed25519 keypair generation/persistence path anywhere in this codebase

**What goes wrong:** Assuming `famp-crypto`'s "already reused as-is" framing (per CONTEXT.md's `<domain>` section) means keypair *generation* is a solved problem. It is not. `IdentityLayout`/`cli/paths.rs`/`cli/home.rs` describe a `key.ed25519`/`pub.ed25519` file layout, but grepping the entire `crates/famp/src/` tree shows only `famp info` *reads* those files — nothing writes them. The `famp init`/`famp setup`/`famp keygen` commands that used to write them were hard-deleted in the v0.9 Phase 4 CLI purge (only recoverable via `git checkout v0.8.1-federation-preserved`, per `learned-rules.md`'s federation-preservation rule).

**Why it happens:** Phase 8's CONTEXT.md correctly identifies `famp-crypto` (the *sign/verify* logic) as fully reusable, but doesn't separately call out that *key generation and storage* — a distinct concern from sign/verify — has no current owner.

**How to avoid:** Plan an explicit task for gateway keypair generation + persistence: add `FampSigningKey::generate()` to `famp-crypto` (using the already-workspace-enabled `rand_core` feature on `ed25519-dalek` + `rand::rngs::OsRng`, which requires adding `rand` as a new dependency of the `famp-crypto` crate specifically — it is not currently a dep there even though it's a workspace-level dependency elsewhere), then have `famp-gateway` (or `famp peer export`, whichever process first needs a key) generate-once-and-persist a keypair at a NEW path (recommend a fresh file under `~/.famp/gateway/`, e.g. `~/.famp/gateway/identity.ed25519`, deliberately NOT reusing the stale `~/.famp/key.ed25519` name to avoid conflating this with the dead `IdentityLayout` concept).

**Warning signs:** A plan that says "load the signing key from `~/.famp/key.ed25519`" without an accompanying "generate it if absent" task will fail on literally every machine, since nothing has ever written that file in the current codebase.

### Pitfall 4: A single remote signing key backing multiple proxied principal *names* breaks keyring reload

**What goes wrong:** `Keyring::load_from_file` (used whenever a keyring is re-read from disk, e.g. gateway restart) explicitly rejects **duplicate pubkeys across different principals** (`KeyringError::DuplicatePubkey`) — but `Keyring::pin_tofu` (used by `famp peer import`) does **not** perform this check when adding entries in-memory. If Ben's machine B runs a single gateway backing two named principals (e.g. `bob` and `carol`) both signing with the SAME machine-level Ed25519 key, importing both on machine A will succeed at import time (two successive `pin_tofu` calls, no dup-check) but the NEXT time the keyring is loaded from disk via `load_from_file` (e.g. gateway process restart), it will hard-fail with `DuplicatePubkey`.

**Why it happens:** `pin_tofu`'s conflict check is per-principal (same principal, different key = reject); `load_from_file`'s conflict check is per-pubkey (different principal, same key = reject). These are two different invariants from two different design eras (`load_from_file`'s dup-pubkey check was designed for a one-principal-per-human-operator model), and nothing currently reconciles them.

**How to avoid:** For Phase 8's own-two-machines scope, recommend constraining the trust model to **one signing key per remote principal name** (i.e., if Ben wants two agent names trusted on the same remote machine, each needs its own keypair) — this matches the phase's `<specifics>` testing scenarios (single round-trip export/import) and avoids the landmine entirely. Flag this as an explicit scope note in the plan rather than silently hitting it later. Alternatively, this could be resolved by making `verify_inbound` load the keyring via a non-dup-checking path if one is added — but that requires a `famp-keyring` change, which is a bigger surface than this phase's stated scope; recommend deferring the multi-principal-per-domain generalization to v1.1 unless a plan explicitly re-scopes it.

**Warning signs:** A test scenario or manual setup where the SAME `famp-gateway` process backs two remote-facing principal names sharing one keypair will pass `famp peer import` on both, then fail mysteriously on the next gateway restart with a `DuplicatePubkey` error that traces to `load_from_file`, not to anything the operator just did.

### Pitfall 5: `KeyringBuildFailed` / `TofuBootstrapRefused` / `TlsFingerprintMismatch` are orphaned `CliError` fossils

**What goes wrong:** These three `CliError` variants already exist in `crates/famp/src/cli/error.rs` and are referenced by the exhaustive MCP `error_kind.rs` mapper, but have **zero construction sites** anywhere in the current codebase — they predate the v0.9 CLI purge and were not pruned in the `260708-g01` fossil-cleanup pass (unlike `PeerDuplicate`/`PeerPubkeyInvalid`/etc., which WERE pruned). Their exact shape was designed for the old `peers.toml`/TLS-fingerprint federation model, not the `famp-keyring::Principal`-keyed model this phase uses.

**Why it happens:** Incomplete fossil pruning — these three variants happened to survive the cleanup commit that removed 8 siblings.

**How to avoid:** Don't resurrect/repurpose these three variants for Phase 8's new errors (`KeyConflict` mapping, `RejectReason`, blob-parse failures, etc.) — their names are suggestive but their shape doesn't match this phase's needs (`TlsFingerprintMismatch` in particular is TLS-cert-specific, unrelated to Ed25519 key fingerprints). Add fresh, narrowly-scoped variants instead, following the existing narrow-enum convention (`GatewayError`, `KeyringError`).

## Code Examples

### 1. `key_id` derivation (D-03, resolving Claude's Discretion)

```rust
// Recommended: add to crates/famp-crypto/src/keys.rs (or a new small
// fingerprint.rs) as a method/free function on TrustedVerifyingKey.
// Reuses the ALREADY-EXPORTED `sha256_digest` from hash.rs — zero new deps.
use crate::hash::sha256_digest;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

/// Stable, human-comparable fingerprint of an Ed25519 verifying key.
/// NOT itself a trust anchor — the trust anchor is the full 32-byte pubkey
/// pinned in the keyring. This is diagnostic/UX metadata carried on the
/// wire (`sender_key_id`) and shown to humans during `famp peer export`.
///
/// Derivation: SHA-256(raw 32-byte pubkey) -> base64url(unpadded) -> first
/// 16 characters (~96 bits of the 256-bit digest — ample for a 2-endpoint
/// own-machines setting where collision is a UX concern, not a security
/// boundary).
pub fn key_id(vk: &TrustedVerifyingKey) -> String {
    let digest = sha256_digest(vk.as_bytes());
    let full = URL_SAFE_NO_PAD.encode(digest);
    full.chars().take(16).collect()
}
```

**Confidence:** This exact truncation length (16 chars) is a reasoned engineering recommendation, not verified against any spec or external precedent this session — `[ASSUMED]`, tag accordingly in the plan. The derivation *algorithm* (SHA-256 + b64url) is `[VERIFIED: existing famp-crypto hash.rs + keys.rs primitives]` — both `sha256_digest` and `to_b64url`-style encoding already exist and are exercised by KAT tests elsewhere in this crate.

### 2. Keypair generation (new — resolves Pitfall 3)

```rust
// crates/famp-crypto/Cargo.toml — ADD:
// rand = { workspace = true }

// crates/famp-crypto/src/keys.rs — ADD to impl FampSigningKey:
impl FampSigningKey {
    /// Generate a fresh signing key from the OS CSPRNG. The `rand_core`
    /// feature on `ed25519-dalek` is already enabled at the workspace level
    /// (Cargo.toml: `features = ["std", "zeroize", "rand_core"]`), so this
    /// requires no ed25519-dalek feature change — only adding `rand` as a
    /// direct dependency of famp-crypto (currently NOT a dep there).
    #[must_use]
    pub fn generate() -> Self {
        use rand::rngs::OsRng;
        Self(ed25519_dalek::SigningKey::generate(&mut OsRng))
    }
}
```

**Confidence:** `[CITED: ed25519-dalek README/docs.rs — verified via web search this session]` for the `rand_core` feature requirement and `SigningKey::generate(&mut csprng)` signature; `[VERIFIED: workspace Cargo.toml]` that the feature is already enabled (`ed25519-dalek = { version = "2.2.0", ..., features = ["std", "zeroize", "rand_core"] }`).

### 3. Export blob format (new — resolves Pitfall "reusing parse_line verbatim")

```rust
// Recommended: crates/famp/src/cli/peer/export.rs and import.rs.
// A NEW format, NOT famp-keyring::file_format::parse_line (which is
// strict 2-field, no room for a fingerprint).
// One line, whitespace-separated, 3 fields:
//   <principal> <pubkey-b64url> <key_id-fingerprint>
// e.g.: agent:ben-mbp.local/gateway  Rap...qso  a1b2c3d4e5f6a7b8

pub fn format_export_line(principal: &Principal, vk: &TrustedVerifyingKey) -> String {
    format!("{} {} {}\n", principal, vk.to_b64url(), famp_crypto::key_id(vk))
}

pub fn parse_export_line(line: &str) -> Result<(Principal, TrustedVerifyingKey), PeerError> {
    let mut parts = line.trim().split_whitespace();
    let principal_str = parts.next().ok_or(PeerError::Malformed("missing principal"))?;
    let pubkey_str = parts.next().ok_or(PeerError::Malformed("missing pubkey"))?;
    let fingerprint_str = parts.next(); // optional 3rd field, informational only
    // ... parse principal + pubkey via the SAME primitives parse_line uses
    // (Principal::from_str, TrustedVerifyingKey::from_b64url) ...
    // Optionally: re-derive key_id from the parsed pubkey and compare
    // against fingerprint_str as a paste-corruption integrity check
    // (warn, don't hard-fail, if it's absent — keeps the format forgiving
    // for a hand-typed/partial paste).
    todo!()
}
```

### 4. `verify_inbound` (D-07, WIRE-01, TRUST-02)

```rust
// crates/famp-gateway/src/verify.rs — NEW
use famp::{Principal, SignedEnvelope, TrustedVerifyingKey, from_slice_strict, verify_value};
use famp_keyring::Keyring;

#[derive(Debug)]
pub enum RejectReason {
    InvalidSignature,
    UnpinnedKey { principal: Principal },
}

/// Pure, transport-agnostic. Phase 9's HTTP handler is the only future
/// caller — this phase unit-tests it directly with byte slices.
pub fn verify_inbound<B: famp::body::BodySchema>(
    bytes: &[u8],
    keyring: &Keyring,
) -> Result<SignedEnvelope<B>, RejectReason> {
    // SignedEnvelope::decode already does: strict-parse -> strip signature
    // -> verify_value over the raw Value -> typed decode. But it needs a
    // SINGLE TrustedVerifyingKey up front, and we don't know which key to
    // verify against until we've peeked the `from` principal — so decode
    // in two passes: peek `from`, look up its pinned key, THEN decode+verify.
    let peeked_from: Principal = famp_envelope::peek_sender(bytes)
        .map_err(|_| RejectReason::InvalidSignature)?;
    let Some(vk) = keyring.get(&peeked_from) else {
        return Err(RejectReason::UnpinnedKey { principal: peeked_from });
    };
    SignedEnvelope::decode(bytes, vk).map_err(|_| RejectReason::InvalidSignature)
}
```

**Confirmed:** `famp_envelope::peek_sender(bytes: &[u8]) -> Result<Principal, EnvelopeDecodeError>` (`crates/famp-envelope/src/peek.rs`, re-exported from the crate root) is EXACTLY this primitive — it was built in the v0.8 era specifically so `famp-transport-http`'s sig-verify middleware could "read `from` before you know which key to verify with," using the same strict duplicate-key-rejecting parse as everything else. It performs NO signature verification itself (by design) and lives in `famp-envelope` (not `crates/famp`) precisely so `famp-gateway` can reach it without a heavier dependency. `[VERIFIED: crates/famp-envelope/src/peek.rs, read in full this session]` — the two-pass flow in the `verify_inbound` sketch above (`peek_sender` → keyring lookup → `SignedEnvelope::decode`) is confirmed viable as written, no adjustment needed.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|---------------|--------|
| v0.8 `famp listen` HTTPS daemon per identity, `peers.toml` (TOML), TLS-fingerprint trust | v0.9+ crypto-free local UDS bus + v1.0 gateway proxying with `famp-keyring` line-format TOFU | v0.9 Phase 4 (local bus), v1.0 Phase 7-8 (gateway) | The entire `famp::cli::peer`/`init`/`setup` surface referenced by `_deferred_v1/` tests is gone; Phase 8 builds a fresh, smaller CLI against a different underlying trust primitive (`famp-keyring::Keyring` vs TOML `peers.toml`) |
| `FAMP_HOME`-per-identity model (`~/.famp/key.ed25519`) | Session-bound MCP identity (crypto-free, name-only) for the local bus; a NEW, not-yet-built gateway-owned keypair for cross-host signing | v0.8.x → v0.9 | Two entirely separate "identity" concepts now coexist: (1) local-bus name resolution (`cli/identity.rs`, no crypto), and (2) cross-host signing identity (Phase 8, new) — do not conflate them |

**Deprecated/outdated:**
- `famp::cli::init`/`setup`/`peer` (TOML `peers.toml`): deleted in v0.9 Phase 4; recoverable only via `git checkout v0.8.1-federation-preserved` for historical reference, not for direct reuse.
- `IdentityLayout`'s implicit assumption that `~/.famp/key.ed25519` exists: no longer true on any fresh machine — `famp info` is effectively dead-in-practice until Phase 8 (or a prerequisite task within it) adds a keygen step.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `key_id` truncation length of 16 base64url characters (~96 bits) is an adequate human-comparable fingerprint length for this phase's own-two-machines scope | Code Examples §1 | Low — key_id is diagnostic metadata, not a trust anchor (full pubkey match still gates trust); wrong length is a cosmetic/UX fix, not a security regression |
| A2 | The recommended new keypair file path `~/.famp/gateway/identity.ed25519` (vs. resurrecting `~/.famp/key.ed25519`) is the right naming choice | Pitfall 3 / Recommended Project Structure | Low — purely a naming/path choice; planner should confirm against `paths.rs` conventions before locking |
| A3 | CLI-layer 3-field export blob format (vs. extending `famp-keyring::file_format`) is the right layering choice | Standard Stack "Alternatives Considered", Code Examples §3 | Low-medium — if the planner prefers keeping ALL keyring-adjacent parsing inside `famp-keyring`, this is a straightforward refactor, not a design failure |
| A4 | One signing key per remote principal name (not one key shared across multiple principal names on one remote machine) is the right scope constraint for this phase | Pitfall 4 | Medium — if Ben's actual v1.0 usage needs multiple trusted agent names per remote machine sharing one key, this constraint would need lifting via a `famp-keyring` change (bigger than this phase's stated scope) |
| A5 | ~~`famp_envelope::peek_sender` signature compatible with the two-pass verify flow~~ — **RESOLVED**: read in full this session, confirmed compatible (see Code Examples §4) | Code Examples §4 | Resolved — no residual risk |

## Open Questions

1. **Is a single gateway-wide keypair (one per machine) or a per-proxied-principal keypair the intended v1.0 trust model?**
   - What we know: CONTEXT.md's D-05 language ("export --as `<name>`") is ambiguous between "export my machine's one key, labeled as principal `<name>`" and "export a key specific to principal `<name>`." `Keyring::load_from_file`'s dup-pubkey rejection (Pitfall 4) actively breaks the shared-key-multiple-principals case on reload.
   - What's unclear: whether Ben's actual v1.0 usage (per STATE.md, "own-two-machines... Ben controls both") ever needs more than one principal name per remote machine trusted simultaneously.
   - Recommendation: default to one keypair per remote principal name (simplest, avoids the Pitfall 4 landmine entirely) unless the planner has explicit signal that multiple simultaneously-trusted principal names per remote machine is needed for this phase's own test scenarios.

3. **Where exactly should the gateway's own signing keypair be generated — inside `famp-gateway` itself (at process start, lazily), or as a `famp peer export` prerequisite (generate-if-absent before first export)?**
   - What we know: `famp peer export` needs a key to display; `verify_inbound`/signing (Phase 9, not this phase) needs a key to sign with. Both need to agree on the SAME persisted key.
   - What's unclear: whether Phase 8 should build the actual cross-host *signing* call site (likely not — Phase 8's CONTEXT.md explicitly scopes signing/verification as "pure functions" this phase, deferring live wiring to Phase 9) or only the export/import/verify-function pieces, with the "generate the actual signing key at process start" wiring deferred to Phase 9 alongside the live transport.
   - Recommendation: Phase 8 should build the keygen-if-absent + persistence function as a small, independently testable unit (e.g. `famp_gateway::identity::load_or_generate(path) -> FampSigningKey`), used by BOTH `famp peer export` (to have something to export) and left ready for Phase 9 to call when it actually signs outbound envelopes. This keeps Phase 8's scope honest (build the piece, don't fake it) without requiring Phase 8 to build the live signing call path Phase 9 owns.

## Environment Availability

Not applicable — this phase has no external tool/service dependencies beyond the Rust toolchain already required project-wide (confirmed present per CLAUDE.md's Phase 0 onboarding assumption and this session's ability to read/build the existing workspace). No new CLI tools, databases, or network services are introduced.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo test` / `cargo nextest` (per-crate `#[test]`, plus `crates/famp-gateway/tests/*.rs` integration binaries) |
| Config file | none (standard Cargo test harness); `just lint` runs `cargo clippy --workspace --all-targets -- -D warnings` |
| Quick run command | `cargo test -p famp-envelope --lib` / `cargo test -p famp-crypto --lib` / `cargo test -p famp-gateway --lib` (unit-level, fast) |
| Full suite command | `cargo test --workspace --lib` then targeted integration binaries (`cargo test -p famp-gateway --test <name>`); **do not use `cargo nextest --list`** — it hangs (documented project pitfall); use plain `cargo test --lib`/`--test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| WIRE-02 | Extended envelope round-trips byte-exact through sign→encode→decode with every new field populated | unit (proptest-style or plain `#[test]`) | `cargo test -p famp-envelope --lib federation_fields_roundtrip -x` | ❌ Wave 0 — new test |
| WIRE-02 | Local-bus envelope (no federation fields set) serializes to byte-identical output vs. pre-Phase-8 baseline | unit | `cargo test -p famp-envelope --lib local_bus_byte_identical -x` | ❌ Wave 0 — new test (can reuse existing `vector_0`/`smoke.rs` fixtures as the baseline) |
| WIRE-01 | `verify_inbound` rejects an unsigned envelope with `RejectReason::InvalidSignature`, no state created | unit | `cargo test -p famp-gateway --lib verify_inbound_rejects_unsigned -x` | ❌ Wave 0 — new test |
| WIRE-01 | `verify_inbound` rejects a tampered/wrong-signature envelope | unit | `cargo test -p famp-gateway --lib verify_inbound_rejects_bad_signature -x` | ❌ Wave 0 — new test |
| TRUST-01 | Single-machine round-trip: `export` → blob → `import` → pinned; matching-key envelope verifies | integration (in-process, per CONTEXT.md `<specifics>`) | `cargo test -p famp --test peer_export_import_roundtrip -x` | ❌ Wave 0 — new test |
| TRUST-02 | Envelope signed by a never-imported key is rejected with `RejectReason::UnpinnedKey`, no bus write, no state created | unit | `cargo test -p famp-gateway --lib verify_inbound_rejects_unpinned_key -x` | ❌ Wave 0 — new test |

### Sampling Rate

- **Per task commit:** the relevant crate's `--lib` quick run (`cargo test -p famp-envelope --lib`, `-p famp-crypto --lib`, `-p famp-gateway --lib`, or `-p famp --lib` depending on which task).
- **Per wave merge:** `cargo test --workspace --lib` + the new `famp-gateway`/`famp` integration test binaries this phase adds.
- **Phase gate:** full suite green, `just lint` clean (clippy pedantic — this project's `just lint` promotes nursery lints beyond plain clippy per `learned-rules.md`), before `/gsd-verify-work`.

### Wave 0 Gaps

- [ ] `crates/famp-envelope/src/wire.rs` / `envelope.rs` — federation field round-trip test (covers WIRE-02)
- [ ] `crates/famp-envelope` — local-bus byte-identity regression test against the existing `vector_0`/`smoke.rs` fixtures (covers WIRE-02/D-02)
- [ ] `crates/famp-gateway/src/verify.rs` + unit tests — covers WIRE-01, TRUST-02
- [ ] `crates/famp/src/cli/peer/` + integration test (single-machine export→import round-trip, per CONTEXT.md `<specifics>`) — covers TRUST-01
- [ ] Framework install: none — `cargo test`/`cargo nextest` already fully configured project-wide

## Security Domain

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | Partial | Not human/session auth — this is inter-agent message-origin authentication via Ed25519 signature + TOFU key pinning, not covered by a standard V2 library; the project's own `famp-crypto`/`famp-keyring` ARE the control |
| V3 Session Management | No | No session concept at this layer (gateway is a long-lived process, not per-request session state) |
| V4 Access Control | No | Out of scope this phase — TRUST-02 is identity verification, not authorization/capability enforcement (that's the FAMP-Sec v2.0+ plane, explicitly deferred) |
| V5 Input Validation | Yes | `famp_canonical::from_slice_strict` (duplicate-key rejection) + `deny_unknown_fields` on every envelope struct + `Timestamp`'s shallow-but-present format validation |
| V6 Cryptography | Yes | `ed25519-dalek` via `famp-crypto`'s `TrustedVerifyingKey`/`verify_strict` — never hand-roll; already the sole sanctioned path in this codebase |

### Known Threat Patterns for this stack

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Envelope signature stripping/forgery | Spoofing/Tampering | `verify_strict` (not plain `verify`) rejects malleable signatures and small-order points — already enforced at the `TrustedVerifyingKey` type level |
| Unpinned-key implicit trust ("TOFU bypass") | Spoofing | TRUST-02: `verify_inbound` MUST hard-reject any `from` principal absent from the keyring — no fallback, no "trust on first signed message" auto-pin at the gateway (TOFU pinning happens ONLY via the explicit out-of-band `famp peer import`, never implicitly at message-receive time) |
| Duplicate object-key JSON smuggling (verify one key's value, act on another's) | Tampering | `from_slice_strict` rejects duplicate keys at any depth before the signature is even checked — already handled by the existing canonical-parse substrate |
| Weak/small-order Ed25519 public keys | Tampering/Repudiation | `TrustedVerifyingKey::from_bytes`/`from_b64url` already reject weak keys (`is_weak()` check) at construction — any key pinned via `Keyring::pin_tofu` necessarily passed through this gate already, since `pin_tofu` takes a `TrustedVerifyingKey`, not raw bytes |
| Reserved `capability`/`approval` fields accidentally interpreted | Elevation of Privilege | Explicit scope fence (D-01, `<domain>`): these fields are typed as opaque `serde_json::Value`, carried and signed, but **nothing in this phase reads or acts on their contents** — a code reviewer should verify no new code path pattern-matches on `capability`/`approval` beyond serializing/deserializing them as opaque values |

## Sources

### Primary (HIGH confidence — direct source read this session)
- `crates/famp-envelope/src/{wire,envelope,timestamp,error,causality,bus,version,lib}.rs` — envelope wire-format, type-state, signing/decoding path, existing pitfall documentation
- `crates/famp-crypto/src/{keys,verify,hash,prefix}.rs` + `Cargo.toml` — Ed25519 primitives, domain-separation prefix, SHA-256 hashing, existing dependency set
- `crates/famp-keyring/src/{lib,file_format,peer_flag,error}.rs` + `Cargo.toml` — TOFU keyring mechanism, existing file/flag formats
- `crates/famp-core/src/identity.rs` — `Principal` parsing/validation, `authority()`/`name()` accessors
- `crates/famp-gateway/src/{lib,principal,registry,error,main}.rs` + `Cargo.toml` — Phase 7 gateway skeleton, confirmed no HTTP transport yet
- `crates/famp/src/cli/{info,home,paths,identity,mod,error}.rs` — existing `famp info` peer-card command, `FAMP_HOME` resolution, dead-in-practice `IdentityLayout`, CLI subcommand-tree pattern (`cli/daemon/`)
- `crates/famp/src/lib.rs` — confirmed which crypto/envelope/core types `famp` re-exports (usable by `famp-gateway` without new direct deps) and that `famp-keyring` is currently `#[cfg(test)]`-only there
- `crates/famp/tests/_deferred_v1/{peer_import,peer_add,README}.rs` — confirmed the old federation CLI surface is fully deleted, dormant-as-intent-only
- `crates/famp-canonical/src/{lib,canonical,strict_parse}.rs` — confirmed `serde_jcs`-backed key-sorting canonicalization and strict duplicate-key rejection
- `.planning/phases/07-broker-liveness-fork-gateway-skeleton/07-VERIFICATION.md` — ground-truth of exactly what Phase 7 delivered
- `.planning/{REQUIREMENTS,STATE}.md`, `08-CONTEXT.md` — locked decisions, requirement text, project history
- `learned-rules.md` — federation-preservation, keyring-cache-ordering, and CI-gate project-specific rules

### Secondary (MEDIUM confidence)
- `ed25519-dalek` `rand_core` feature requirement for `SigningKey::generate` — confirmed via WebSearch this session against README/docs.rs, cross-checked against the workspace `Cargo.toml`'s already-enabled feature flag

### Tertiary (LOW confidence / flagged as ASSUMED)
- `key_id` truncation length (16 chars) — engineering recommendation, not externally verified
- Recommendation to scope this phase to one-keypair-per-remote-principal (Pitfall 4 / Open Question 2) — a scope interpretation, not an explicit CONTEXT.md decision

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — every crate/version claim was read directly from `Cargo.toml`, zero external registry lookups needed since no new external packages are introduced
- Architecture: HIGH for the envelope-extension and verify-function design (directly grounded in existing, working code); MEDIUM for the keypair-generation/persistence design (genuinely new surface, reasoned from first principles + one external API confirmation)
- Pitfalls: HIGH — all five pitfalls are grounded in direct code reads (grep + read of the actual struct/function definitions involved), not speculation

**Research date:** 2026-07-23
**Valid until:** 30 days (stable, code-grounded; re-verify if Phase 7's gateway skeleton or `famp-envelope`/`famp-crypto`/`famp-keyring` change materially before Phase 8 planning executes)

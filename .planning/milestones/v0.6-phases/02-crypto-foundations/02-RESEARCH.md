# Phase 2: Crypto Foundations - Research

**Researched:** 2026-04-13
**Domain:** Ed25519 sign/verify with domain separation, weak-key rejection, base64url encoding, cross-language interop
**Confidence:** HIGH (stack locked; research is about implementation mechanics, not library selection)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Stack (all locked from CLAUDE.md and 02-CONTEXT.md):**
- `ed25519-dalek 2.2.0` with `features = ["std", "zeroize"]` — already in workspace
- `base64 0.22.1` `URL_SAFE_NO_PAD` engine
- `sha2 0.11.0`
- `serde_jcs 0.2.0` via `famp-canonical` (Phase 1 complete, SEED-001 resolved)
- `thiserror 2.0.18` for `CryptoError`; never `anyhow` from public API
- `proptest 1.11.0` + `insta 1.47.2` for tests

**API shape (all locked from D-01 through D-29 in 02-CONTEXT.md):**
- Free functions are primary; traits are thin sugar (`sign_value`, `verify_value`, `sign_canonical_bytes`, `verify_canonical_bytes`)
- FAMP-owned newtypes: `FampSigningKey`, `TrustedVerifyingKey`, `FampSignature`
- `TrustedVerifyingKey` is the only verifying-key type on the public API; its constructor performs all ingress checks
- `DOMAIN_PREFIX: &[u8; 12] = b"FAMP-sig-v1\x00"` as a public constant
- `canonicalize_for_signature(unsigned_value: &serde_json::Value) -> Result<Vec<u8>, CryptoError>` returns `prefix || canonical_json_bytes`
- `zeroize` on `FampSigningKey` via derive macro
- Constant-time approach: document + wrapper audit (no statistical dudect tests in CI)
- `CryptoError` variants: `InvalidKeyEncoding`, `InvalidSignatureEncoding`, `WeakKey`, `Canonicalization(CanonicalError)`, `VerificationFailed`, `InvalidSigningInput`, optionally `Base64`
- SHA-256: satisfied via `famp-canonical` phase-1 helpers (no new code in Phase 2)

**Worked-example fixture (locked from D-13 through D-18):**
- Location: `crates/famp-crypto/tests/vectors/famp-sig-v1/worked-example.json`
- Schema from D-15 (including `domain_prefix_hex`, `signing_input_hex`, etc.)
- Bytes sourced externally from Python `jcs 0.2.1` + `cryptography 46.0.7` (NOT self-generated)
- RFC 8032 vectors: `crates/famp-crypto/tests/vectors/rfc8032/`

### Claude's Discretion

- Exact `zeroize` integration mechanics (derive macro vs manual `Drop`)
- `Debug` impl for `FampSigningKey` — MUST redact
- Whether `FampSignature` implements `PartialEq` constant-time via `subtle::ConstantTimeEq`
- Internal module layout (`keys.rs`, `sign.rs`, `verify.rs`, `encoding.rs`, etc.)
- Whether to expose `Display` on key/signature types

### Deferred Ideas (OUT OF SCOPE)

- `sign_envelope` / `verify_envelope` — deferred to `famp-envelope`
- FIPS profile via `aws-lc-rs`
- `no_std` support
- Statistical timing tests (dudect-style)
- Agent Card / trust-store policy beyond raw weak-key rejection
- Protocol error taxonomy §15.1 — that is `famp-core` (Phase 3)
- Key generation / KMS / hardware-backed signing
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CRYPTO-01 | `famp-crypto` exposes `Signer` and `Verifier` traits over Ed25519 | Trait sealing pattern; free-function primary with thin trait sugar |
| CRYPTO-02 | Only `verify_strict` exposed; raw `verify` hidden | Newtype wrapper hides `VerifyingKey` entirely; `TrustedVerifyingKey` delegates only to `verify_strict` |
| CRYPTO-03 | Weak public key rejection at trust store / Agent Card ingress | `VerifyingKey::is_weak()` at `TrustedVerifyingKey::from_bytes` time; "must-reject" fixture set |
| CRYPTO-04 | Domain-separation prefix applied before signing per SPEC-03 | Prefix bytes fully extracted from spec: `b"FAMP-sig-v1\x00"` (12 bytes, hex `46414d502d7369672d763100`) |
| CRYPTO-05 | RFC 8032 Ed25519 test vectors pass as hard CI gate | All 5 RFC 8032 §7.1 vectors extracted verbatim; CI command identified |
| CRYPTO-06 | Base64url unpadded encoding for keys (32 bytes) and signatures (64 bytes) per SPEC-19 | `base64 0.22.1` `URL_SAFE_NO_PAD` with strict decoder; gotchas documented |
| CRYPTO-07 | SHA-256 content-addressing via `sha2` crate | Satisfied by `famp-canonical` Phase 1 helpers; no new Phase 2 code needed |
| CRYPTO-08 | Constant-time signature verification path | `ed25519-dalek verify_strict` is the CT boundary; wrapper audit approach documented |
| SPEC-03 | Signature domain-separation byte format specified with hex-dump worked example | Full prefix bytes extracted from spec §7.1a; worked-example fixture schema locked |
| SPEC-19 | Ed25519 key encoding locked (raw 32-byte pub, 64-byte sig, unpadded base64url) | Strict decoder rejection list from spec §7.1b fully documented |
</phase_requirements>

---

## Summary

Phase 2 builds `famp-crypto` on top of the Phase 1 canonical-JSON substrate. The research reveals three hard problems worth detailed treatment: (1) ensuring `VerifyingKey::from_bytes` rejects small-order points at ingress (not just at verify time) using `is_weak()`, (2) avoiding any public surface that allows a caller to reach `dalek::VerifyingKey::verify` instead of `verify_strict`, and (3) generating the §7.1c worked-example bytes externally from Python to satisfy PITFALLS P10. Everything else is well-understood plumbing.

The domain-separation prefix is fully specified in the spec: exactly `b"FAMP-sig-v1\x00"` (12 bytes, hex `46414d502d7369672d763100`), prepended to canonical JSON bytes before signing. There is no ambiguity. The worked-example bytes are embedded in the spec verbatim (§7.1c.3 through §7.1c.6), sourced from Python `jcs 0.2.1` + `cryptography 46.0.7` already — the Phase 2 fixture file contains them literally, not recomputed.

The constant-time guarantee is achieved by delegation: `ed25519-dalek 2.2`'s `verify_strict` uses the cofactor-checked group equation with a points table and does not branch on message content. Our wrapper introduces no avoidable pre-verification branching on secret material. The only required actions are: (a) use `verify_strict` exclusively, (b) ensure the weak-key ingress check fires on public material (not secret), (c) use `zeroize` derive on `FampSigningKey`, and (d) use `subtle::ConstantTimeEq` for `FampSignature::eq` if implementing `PartialEq`.

**Primary recommendation:** Keep the API surface minimal, delegate CT guarantees entirely to `ed25519-dalek`, gate CI on RFC 8032 vectors and the §7.1c fixture, and let `TrustedVerifyingKey`'s constructor enforce weak-key rejection as a compile-time-enforced invariant.

---

## Standard Stack

### Core

| Library | Version | Purpose | Feature flags |
|---------|---------|---------|---------------|
| `ed25519-dalek` | `2.2.0` | Ed25519 sign/verify | `std`, `zeroize` (already in workspace) |
| `base64` | `0.22.1` | Base64url unpadded encode/decode | default features |
| `sha2` | `0.11.0` | SHA-256 (via `famp-canonical` — no new dep) | workspace |
| `zeroize` | (transitive via dalek) | Wipe signing key on drop | `derive` feature |
| `subtle` | (transitive via dalek) | Constant-time equality for `FampSignature::eq` | — |
| `thiserror` | `2.0.18` | Typed `CryptoError` | workspace |
| `serde` / `serde_json` | `1.0.228` / `1.0.149` | `&serde_json::Value` for `canonicalize_for_signature` | workspace |
| `famp-canonical` | (workspace path) | `canonicalize()` + `CanonicalError` | path dep |

**Notes on `zeroize`:** `ed25519-dalek 2.2` ships its own `zeroize` feature that calls `zeroize` on the inner `SigningKey` bytes. When we wrap `SigningKey` in `FampSigningKey`, we derive `Zeroize` + `ZeroizeOnDrop` on the wrapper (requires adding `zeroize = { version = "1", features = ["derive"] }` as a direct dep or using the re-exported derive). The workspace `ed25519-dalek = { features = ["std", "zeroize"] }` already enables the internal zeroize path. For `FampSigningKey`'s derive, add `zeroize` as a direct dep with `features = ["derive"]`.

**Notes on `subtle`:** `ed25519-dalek` already transitively depends on `subtle` from the RustCrypto stack. No additional entry in `Cargo.toml` is required unless `FampSignature::ct_eq` is explicitly used in our code (it is — for `PartialEq`). Add as a direct dep if calling `ConstantTimeEq::ct_eq` directly.

### Dev Dependencies

| Library | Version | Purpose |
|---------|---------|---------|
| `hex` | `0.4.3` | Hex encode/decode in tests and fixture parsing |
| `serde_json` | (workspace) | Parse fixture JSON in test harnesses |
| `proptest` | `1.11.0` | Base64url round-trip property tests |
| `insta` | `1.47.2` | Snapshot of fixture verification output |

### Cargo.toml additions for `famp-crypto`

```toml
[dependencies]
ed25519-dalek    = { workspace = true }     # already has ["std", "zeroize"]
base64           = { workspace = true }
thiserror        = { workspace = true }
serde            = { workspace = true }
serde_json       = { workspace = true }
zeroize          = { version = "1", features = ["derive"] }
subtle           = "2"                      # for FampSignature::ct_eq
famp-canonical   = { path = "../famp-canonical" }

[dev-dependencies]
hex              = "0.4.3"
proptest         = { workspace = true }
insta            = { workspace = true }
```

**Version verification (live as of 2026-04-12):** All workspace deps confirmed in root `Cargo.toml`. `hex 0.4.3` is current stable (HIGH confidence). `subtle 2.6.1` is current stable — specify `"2"` and let SemVer resolve.

---

## Architecture Patterns

### Recommended Project Structure

```
crates/famp-crypto/
├── src/
│   ├── lib.rs           # pub re-exports; crate-level doc with DOMAIN_PREFIX example
│   ├── keys.rs          # FampSigningKey, TrustedVerifyingKey, FampSignature newtypes
│   ├── sign.rs          # sign_value, sign_canonical_bytes (free functions)
│   ├── verify.rs        # verify_value, verify_canonical_bytes (free functions)
│   ├── encoding.rs      # base64url helpers used by newtype methods
│   ├── prefix.rs        # DOMAIN_PREFIX constant + canonicalize_for_signature
│   └── traits.rs        # Signer, Verifier trait definitions
└── tests/
    ├── vectors/
    │   ├── rfc8032/
    │   │   └── test-vectors.json        # RFC 8032 §7.1 Test 1-5 verbatim
    │   ├── famp-sig-v1/
    │   │   └── worked-example.json      # §7.1c fixture (externally sourced bytes)
    │   └── must-reject/
    │       ├── weak-keys.json           # Small-order public keys
    │       └── malformed-b64.json       # Base64 decode failure cases
    ├── rfc8032_vectors.rs               # hard CI gate: all 5 vectors
    ├── worked_example.rs                # §7.1c byte-exact gate
    ├── weak_key_rejection.rs            # must-reject fixture runner
    └── base64_roundtrip.rs              # proptest round-trip
```

### Pattern 1: Newtype Wrapping as API Surface Control

The fundamental pattern for hiding `verify` and exposing only `verify_strict`: own the types entirely so no method from `dalek` is reachable.

**What:** `TrustedVerifyingKey` wraps `ed25519_dalek::VerifyingKey` with `pub(crate)` visibility on the inner field. No `Deref`. No re-export of the inner type.

**When to use:** Any time a library rule ("use strict only") must be compiler-enforced, not just documented.

```rust
// Source: Rust API Guidelines + 02-CONTEXT.md D-06/D-10
use ed25519_dalek::VerifyingKey;

/// The only verifying-key type reachable from public API.
/// Construction enforces weak-key rejection (SPEC §7.1b, CRYPTO-02/03).
pub struct TrustedVerifyingKey(VerifyingKey);   // field is NOT pub

impl TrustedVerifyingKey {
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, CryptoError> {
        let vk = VerifyingKey::from_bytes(bytes)
            .map_err(|_| CryptoError::InvalidKeyEncoding)?;
        if vk.is_weak() {
            return Err(CryptoError::WeakKey);
        }
        Ok(Self(vk))
    }

    pub fn from_b64url(input: &str) -> Result<Self, CryptoError> {
        let bytes: Vec<u8> = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(input)
            .map_err(|_| CryptoError::InvalidKeyEncoding)?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| CryptoError::InvalidKeyEncoding)?;
        Self::from_bytes(&arr)
    }

    pub fn to_b64url(&self) -> String {
        use base64::Engine as _;
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(self.0.as_bytes())
    }

    // ONLY internal method that touches the inner VerifyingKey
    pub(crate) fn verify_strict_raw(
        &self,
        msg: &[u8],
        sig: &ed25519_dalek::Signature,
    ) -> Result<(), CryptoError> {
        self.0.verify_strict(msg, sig)
            .map_err(|_| CryptoError::VerificationFailed)
    }
}
```

Because `TrustedVerifyingKey.0` is `pub(crate)` at most, and because we never `impl Deref<Target=VerifyingKey>`, callers cannot reach `dalek`'s `verify` method at all. `verify_strict_raw` is also `pub(crate)`, used only by our `verify_canonical_bytes` free function.

### Pattern 2: Domain Prefix Prepend

**What:** A single function owns the concatenation `DOMAIN_PREFIX || canonical_bytes`. No caller assembles this themselves.

```rust
// Source: FAMP-v0.5.1-spec.md §7.1a, §7.1c.4
pub const DOMAIN_PREFIX: &[u8; 12] = b"FAMP-sig-v1\x00";

/// Returns `DOMAIN_PREFIX || canonical_json_bytes`.
/// This is the exact byte sequence passed to Ed25519 sign/verify.
/// Callers provide the envelope with `signature` field already removed.
pub fn canonicalize_for_signature(
    unsigned_value: &serde_json::Value,
) -> Result<Vec<u8>, CryptoError> {
    let canonical = famp_canonical::canonicalize(unsigned_value)
        .map_err(CryptoError::Canonicalization)?;
    let mut signing_input = Vec::with_capacity(DOMAIN_PREFIX.len() + canonical.len());
    signing_input.extend_from_slice(DOMAIN_PREFIX);
    signing_input.extend_from_slice(&canonical);
    Ok(signing_input)
}
```

**Why:** Separates the "what bytes go to Ed25519" question from the "how do I strip the signature field" question. The latter belongs in `famp-envelope`. This function takes an already-stripped `serde_json::Value`.

### Pattern 3: Sign + Verify Free Functions

```rust
// Source: 02-CONTEXT.md D-03
pub fn sign_value<T: serde::Serialize + ?Sized>(
    signing_key: &FampSigningKey,
    value: &T,
) -> Result<FampSignature, CryptoError> {
    let canonical = famp_canonical::canonicalize(value)
        .map_err(CryptoError::Canonicalization)?;
    Ok(sign_canonical_bytes(signing_key, &canonical))
}

pub fn sign_canonical_bytes(
    signing_key: &FampSigningKey,
    canonical_bytes: &[u8],
) -> FampSignature {
    use ed25519_dalek::Signer as _;
    let mut input = Vec::with_capacity(DOMAIN_PREFIX.len() + canonical_bytes.len());
    input.extend_from_slice(DOMAIN_PREFIX);
    input.extend_from_slice(canonical_bytes);
    let sig = signing_key.0.sign(&input);
    FampSignature(sig)
}

pub fn verify_canonical_bytes(
    verifying_key: &TrustedVerifyingKey,
    canonical_bytes: &[u8],
    signature: &FampSignature,
) -> Result<(), CryptoError> {
    let mut input = Vec::with_capacity(DOMAIN_PREFIX.len() + canonical_bytes.len());
    input.extend_from_slice(DOMAIN_PREFIX);
    input.extend_from_slice(canonical_bytes);
    verifying_key.verify_strict_raw(&input, &signature.0)
}
```

### Pattern 4: Zeroize on FampSigningKey

```rust
// Source: zeroize crate docs 1.x + ed25519-dalek 2.2 zeroize feature
use zeroize::{Zeroize, ZeroizeOnDrop};

// ed25519_dalek::SigningKey implements Zeroize when the "zeroize" feature is enabled.
// We derive ZeroizeOnDrop on the wrapper so drop wipes the inner key.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct FampSigningKey(ed25519_dalek::SigningKey);
```

Note: `#[derive(Zeroize)]` alone no longer adds a `Drop` impl as of `zeroize 1.x`. Must use `#[derive(Zeroize, ZeroizeOnDrop)]` to get the drop-zeroize behavior.

### Pattern 5: Constant-Time FampSignature Equality

```rust
// Source: subtle crate docs; RustCrypto conventions
use subtle::ConstantTimeEq;

impl PartialEq for FampSignature {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bytes().ct_eq(&other.0.to_bytes()).into()
    }
}
impl Eq for FampSignature {}
```

`subtle::ConstantTimeEq::ct_eq` returns a `subtle::Choice`, which is converted to `bool` via `.into()`. This prevents timing leaks when comparing signatures in test/fixture code.

### Pattern 6: Redacted Debug for Signing Key

```rust
// Source: Rust API Guidelines (no secret leakage in Debug)
impl std::fmt::Debug for FampSigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FampSigningKey(<redacted>)")
    }
}
```

### Anti-Patterns to Avoid

- **`impl Deref<Target=VerifyingKey> for TrustedVerifyingKey`:** Exposes all of `VerifyingKey`'s methods including `verify`. Never do this.
- **`pub use ed25519_dalek::VerifyingKey`:** Re-exports the raw type, defeating newtype wrapping.
- **Assembling signing input in caller code:** `DOMAIN_PREFIX` is public for testing; `canonicalize_for_signature` is the only sanctioned path that assembles the full signing input.
- **Using `dalek::Signer` trait directly on `FampSigningKey`:** The `ed25519_dalek::Signer` trait signs raw bytes without the domain prefix. Our FAMP `Signer` trait must apply the prefix internally.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Ed25519 arithmetic | Any custom scalar/point code | `ed25519-dalek 2.2` | Curve arithmetic has subtle security properties; dalek is audited |
| Small-order point check | `is_small_order()` inline | `VerifyingKey::is_weak()` | dalek already checks the 8 torsion points; hand-rolling will miss cases |
| Base64url encode/decode | Custom encoder | `base64 0.22.1` `URL_SAFE_NO_PAD` | Strict decoder rejects padding and wrong alphabet — hard to replicate correctly |
| Constant-time comparison | `==` on `[u8; 64]` | `subtle::ConstantTimeEq::ct_eq` | Compiler optimizes `==` into a branch; `subtle` uses volatile/fence to prevent |
| SHA-256 | Any hash impl | `sha2` via `famp-canonical` | Same crate used by dalek internally; one hash library, one source of truth |
| Signing input concat | Inline `Vec::new(); push prefix` | `canonicalize_for_signature()` | The sanctioned path; ad-hoc assembly is a footgun (wrong order, wrong prefix) |
| Secret key zeroing | Manual `Drop` | `zeroize::ZeroizeOnDrop` derive | `zeroize` handles move/copy edge cases (see pitfall below); manual is error-prone |

**Key insight:** The entire security model rests on two library guarantees: dalek's `verify_strict` does the cofactor math, and dalek's `is_weak()` catches ingress torsion points. Any deviation from these primitives requires a security audit.

---

## Exact Spec Bytes (Extracted Verbatim from `FAMP-v0.5.1-spec.md`)

### SPEC-03 Domain-Separation Prefix (§7.1a)

```
DOMAIN_PREFIX (12 bytes):
  Hex: 46 41 4d 50 2d 73 69 67 2d 76 31 00
  Rust: b"FAMP-sig-v1\x00"
  Byte breakdown: F=0x46 A=0x41 M=0x4d P=0x50 -=0x2d s=0x73 i=0x69 g=0x67 -=0x2d v=0x76 1=0x31 NUL=0x00
```

Source: FAMP-v0.5.1-spec.md §7.1a (HIGH confidence — primary spec document).

### §7.1c.1 Test Keypair (RFC 8032 §7.1 Test 1 — also used in RFC 8032 CI gate)

```
secret_key (32 bytes, hex):
  9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60

public_key (32 bytes, raw, hex):
  d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a
```

WARNING: Published worldwide. Test use only.

### §7.1c.3 Canonical JSON (324 bytes)

```
{"authority":"advisory","body":{"disposition":"accepted"},"causality":{"ref":"01890a3b-1111-7222-8333-444444444444","rel":"acknowledges"},"class":"ack","famp":"0.5.1","from":"agent:example.test/alice","id":"01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a6b","scope":"standalone","to":"agent:example.test/bob","ts":"2026-04-13T00:00:00Z"}
```

### §7.1c.5 Signing Input (336 bytes)

```
signing_input_bytes = b"FAMP-sig-v1\x00" || canonical_json_bytes
Hex prefix segment: 46414d502d7369672d763100
Total length: 12 + 324 = 336 bytes
```

Full hex is embedded in spec §7.1c.5 and must be copied verbatim into the fixture file.

### §7.1c.6 Signature (64 bytes)

```
signature_hex: 9366aaced854c7898735908d2e2d973208905fd80e2f93fe505710f58f0ed1fc92e3b9d7a19b30b2cf184f703552dafcf91ca81321f57fa689d1a96865d0b608
  R: 9366aaced854c7898735908d2e2d973208905fd80e2f93fe505710f58f0ed1fc
  S: 92e3b9d7a19b30b2cf184f703552dafcf91ca81321f57fa689d1a96865d0b608

signature_b64url: k2aqzthUx4mHNZCNLi2XMgiQX9gOL5P-UFcQ9Y8O0fyS47nXoZswss8YT3A1Utr8-RyoEyH1f6aJ0aloZdC2CA
```

Source: FAMP-v0.5.1-spec.md §7.1c.6 (HIGH confidence — primary spec document, externally sourced via Python).

---

## RFC 8032 Ed25519 Test Vectors (CI Gate)

All 5 vectors extracted from RFC 8032 §7.1 (HIGH confidence — IETF RFC, official):

| Test | SECRET KEY (hex) | PUBLIC KEY (hex) | MESSAGE | SIGNATURE (hex, first 16 bytes...) |
|------|-----------------|-----------------|---------|-------------------------------------|
| 1 | `9d61b19d...cae7f60` | `d75a9801...07511a` | (empty) | `e5564300...a100b` |
| 2 | `4ccd089b...a6fb` | `3d4017c3...4660c` | `72` | `92a009a9...0c00` |
| 3 | `c5aa8df4...458f7` | `fc51cd8e...8025` | `af82` | `6291d657...c40a` |
| SHA(abc) | `833fe624...a3d42` | `ec172b93...2ebf` | (64-byte SHA-512) | `dc2a4459...e2bf` |
| 1024 | `f5e5767c...c0ee5` | `278117fc...426e` | (1023 bytes) | `0aab4c90...a03` |

Full hex bytes are in RFC 8032 §7.1; they must be copied verbatim into `tests/vectors/rfc8032/test-vectors.json`. The fixture runner parses the JSON and iterates all entries.

**Important:** RFC 8032 Test 1 uses an **empty message** (0 bytes). The test fixture must represent this as an empty hex string `""` or empty bytes, not null. A null message produces a different signing input.

---

## Common Pitfalls

### Pitfall 1: `VerifyingKey::from_bytes` Accepts Some Weak Keys

**What goes wrong:** `ed25519_dalek::VerifyingKey::from_bytes` performs ZIP-215 point validation, not RFC 8032 strict validation. A point on the 8-torsion subgroup (a "weak key") may pass `from_bytes` without error. Code that constructs a `VerifyingKey` and immediately uses it has accepted a weak key without detection.

**Why it happens:** The dalek docs explicitly say ZIP-215 rules are used for construction. `is_weak()` is a separate call needed after construction.

**How to avoid:** In `TrustedVerifyingKey::from_bytes`, call both:
1. `VerifyingKey::from_bytes(bytes)` — rejects non-canonical point encoding
2. `.is_weak()` — rejects the 8 torsion points that pass the encoding check

**Warning signs:** "must-reject" fixture test for the identity point (all-zero public key) passes when it should fail.

**Source:** `ed25519-dalek` docs.rs `is_weak()` method + PITFALLS P4 (HIGH confidence).

### Pitfall 2: Using `dalek::Signer` Trait Directly on the Wrapped Key

**What goes wrong:** `impl std::ops::Deref<Target=SigningKey> for FampSigningKey` or `impl ed25519_dalek::Signer for FampSigningKey` that delegates directly to the inner key's `sign()` bypasses the domain prefix. A FAMP signature produced this way will be a valid Ed25519 signature but not a valid FAMP signature — it will fail cross-implementation verification.

**Why it happens:** `ed25519_dalek::Signer` is a re-exported RustCrypto trait. It looks like the "right" way to add signing behavior.

**How to avoid:** Never `impl Deref` or `impl ed25519_dalek::Signer` on `FampSigningKey`. Our `sign_canonical_bytes` free function is the only path, and it prepends `DOMAIN_PREFIX` internally.

**Warning signs:** Cross-language verify (Python `cryptography`) fails on all messages. RFC 8032 test vectors (which don't use a prefix) pass because they sign the raw message, not the prefixed one.

### Pitfall 3: Self-Generated Worked-Example Bytes (PITFALLS P10)

**What goes wrong:** Writing a Rust program to produce the §7.1c fixture, committing those bytes, and then testing Rust against them. This is circular: the test validates internal consistency, not interop.

**Why it happens:** It is the path of least resistance. The spec already has the bytes.

**How to avoid:** The §7.1c bytes are committed verbatim from the spec (which documents their Python provenance). The test fixture file `worked-example.json` is authored by copying hex strings from the spec, not by running Rust code.

**Warning signs:** The `worked-example.json` creation commit timestamp is the same as the test implementation commit. No non-Rust tool appears in the git history for that file.

### Pitfall 4: `zeroize` Move/Copy Pitfall

**What goes wrong:** A `FampSigningKey` is moved (not dropped) before the program ends. If `ZeroizeOnDrop` is not used, the memory may not be zeroed on drop because moves don't trigger `Drop`. Additionally, stack copies during function calls may leave zeroed "shadows" elsewhere in memory.

**Why it happens:** Rust's drop guarantee only fires at the binding's end of life. A move transfers ownership but the old stack frame's bytes may persist until overwritten.

**How to avoid:** Use `#[derive(Zeroize, ZeroizeOnDrop)]` on `FampSigningKey`. The `ZeroizeOnDrop` derive generates a `Drop` impl. Do NOT use `FampSigningKey` in async contexts where futures are large and memcpy-heavy — for v0.6 this is not a concern.

**Source:** `zeroize` crate docs, benma.github.io Rust zeroize blog post (MEDIUM confidence — correct but single blog source; principle confirmed by `zeroize` docs).

### Pitfall 5: Base64url Strict Decoding Not Configured

**What goes wrong:** Using `base64::decode()` (removed in 0.21+) or using `STANDARD` engine instead of `URL_SAFE_NO_PAD`. The `STANDARD` engine uses `+`/`/` alphabet and accepts `=` padding. A FAMP key encoded by `STANDARD` will look like it decoded correctly, but the alphabet is wrong per SPEC-19.

**Why it happens:** Old tutorial code. The `base64 0.22` Engine API is not the same as pre-0.21 free functions.

**How to avoid:** Always use:
```rust
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
let decoded = URL_SAFE_NO_PAD.decode(input)?;
```
The `URL_SAFE_NO_PAD` constant rejects `+`/`/` characters, requires no `=` padding, and errors on any trailing garbage. This is the required behavior per SPEC-19's decoder rejection list.

**Warning signs:** Tests pass with `STANDARD` engine but cross-language verification fails because Python `base64.urlsafe_b64decode` expects the URL-safe alphabet.

### Pitfall 6: Python `cryptography` API — `from_private_bytes` Takes the 32-byte SEED

**What goes wrong:** Confusion between the 32-byte Ed25519 seed (what RFC 8032 calls the "private key") and the 64-byte "expanded secret key" used internally by some implementations.

**Why it happens:** Some crypto libraries (notably `libsodium`) use a 64-byte representation. Python's `cryptography` library uses 32-byte seeds exclusively.

**How to avoid:** Python `Ed25519PrivateKey.from_private_bytes(seed)` takes exactly 32 bytes — the same bytes that `ed25519_dalek::SigningKey::from_bytes(&[u8; 32])` takes. For the RFC 8032 §7.1 Test 1 keypair: `from_private_bytes(bytes.fromhex("9d61b19d..."))` produces the same signing key as `SigningKey::from_bytes(&[0x9d,0x61,...])`. There is no endianness issue; both use the seed bytes directly.

**Source:** Python `cryptography` docs `Ed25519PrivateKey.from_private_bytes` + RFC 8032 §5.1.5 (HIGH confidence — official docs).

### Pitfall 7: Non-Canonical Signature S Component

**What goes wrong:** An attacker replaces signature component `S` with `S + L` (where `L` is the Ed25519 group order), producing a mathematically valid signature that fails `verify_strict`'s scalar malleability check. Code that uses `verify` (cofactor-tolerant) may accept this. Code that uses `verify_strict` will correctly reject it.

**Why it happens:** `verify_strict` enforces `S < L`. `verify` does not. Most tutorial code uses `verify`.

**How to avoid:** By design, `TrustedVerifyingKey` only calls `verify_strict_raw` which calls `dalek::VerifyingKey::verify_strict`. There is no code path to `verify`.

**Source:** ed25519-dalek docs + "Taming the Many EdDSAs" (IACR ePrint 2020/1244) (HIGH confidence).

---

## Code Examples

### Full Working Sign/Verify (Prescriptive Pattern)

```rust
// Source: 02-CONTEXT.md D-03 + spec §7.1a
use famp_crypto::{
    keys::{FampSigningKey, TrustedVerifyingKey, FampSignature},
    sign::sign_canonical_bytes,
    verify::verify_canonical_bytes,
    prefix::DOMAIN_PREFIX,
};

// Sign
let sk = FampSigningKey::from_bytes([...32 bytes...]);
let canonical: Vec<u8> = famp_canonical::canonicalize(&envelope)?;
let sig: FampSignature = sign_canonical_bytes(&sk, &canonical);

// Verify
let vk = TrustedVerifyingKey::from_bytes(&[...32 bytes...])?; // rejects weak keys
verify_canonical_bytes(&vk, &canonical, &sig)?;
```

### RFC 8032 Vector Test Structure

```rust
// Source: FAMP-v0.5.1-spec.md §7.1c.1 + RFC 8032 §7.1
// Note: RFC 8032 vectors sign the raw message WITHOUT the FAMP domain prefix.
// These test dalek's Ed25519 correctness, not FAMP's prefix application.
#[test]
fn rfc8032_test1() {
    let sk_bytes = hex::decode(
        "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60"
    ).unwrap();
    let pk_bytes = hex::decode(
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
    ).unwrap();
    let message = b""; // empty
    let expected_sig = hex::decode(
        "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e06522490155\
         5fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"
    ).unwrap();

    let sk = ed25519_dalek::SigningKey::from_bytes(
        &sk_bytes.try_into().unwrap()
    );
    use ed25519_dalek::Signer as _;
    let sig = sk.sign(message);
    assert_eq!(sig.to_bytes().as_slice(), expected_sig.as_slice());
}
```

### §7.1c Worked-Example Test Structure

```rust
// Source: FAMP-v0.5.1-spec.md §7.1c.3/c.5/c.6
#[test]
fn worked_example_byte_exact() {
    // Bytes from external Python tool — do NOT recompute in Rust
    let fixture: serde_json::Value = serde_json::from_str(
        include_str!("vectors/famp-sig-v1/worked-example.json")
    ).unwrap();

    let prefix_hex = fixture["domain_prefix_hex"].as_str().unwrap();
    assert_eq!(prefix_hex, "46414d502d7369672d763100");

    let signing_input_hex = fixture["signing_input_hex"].as_str().unwrap();
    let expected_signing_input = hex::decode(signing_input_hex).unwrap();

    // Parse unsigned envelope from fixture, canonicalize, prepend prefix
    let unsigned: serde_json::Value =
        serde_json::from_str(fixture["unsigned_envelope_json"].as_str().unwrap()).unwrap();
    let actual = famp_crypto::prefix::canonicalize_for_signature(&unsigned).unwrap();
    assert_eq!(actual, expected_signing_input, "signing input must be byte-exact");

    // Verify signature
    let pk_hex = fixture["public_key_hex"].as_str().unwrap();
    let pk_bytes: [u8; 32] = hex::decode(pk_hex).unwrap().try_into().unwrap();
    let vk = famp_crypto::keys::TrustedVerifyingKey::from_bytes(&pk_bytes).unwrap();

    let sig_hex = fixture["signature_hex"].as_str().unwrap();
    let sig_bytes: [u8; 64] = hex::decode(sig_hex).unwrap().try_into().unwrap();
    let sig = famp_crypto::keys::FampSignature::from_bytes(sig_bytes);

    let canonical_hex = fixture["canonical_json_hex"].as_str().unwrap();
    let canonical = hex::decode(canonical_hex).unwrap();
    famp_crypto::verify::verify_canonical_bytes(&vk, &canonical, &sig)
        .expect("§7.1c worked example must verify byte-exact");
}
```

### Weak Key Rejection Test Structure

```rust
// Source: ed25519-dalek docs is_weak() + PITFALLS P4
// The identity point (all zeros) is the simplest small-order key.
// Additional torsion points from the ed25519-speccheck project.
#[test]
fn rejects_identity_point() {
    let identity = [0u8; 32];
    let result = famp_crypto::keys::TrustedVerifyingKey::from_bytes(&identity);
    assert!(
        matches!(result, Err(famp_crypto::error::CryptoError::WeakKey)),
        "identity point must be rejected at ingress"
    );
}

#[test]
fn rejects_all_must_reject_fixtures() {
    let fixtures: serde_json::Value = serde_json::from_str(
        include_str!("vectors/must-reject/weak-keys.json")
    ).unwrap();
    for entry in fixtures.as_array().unwrap() {
        let hex_key = entry["public_key_hex"].as_str().unwrap();
        let name = entry["name"].as_str().unwrap();
        let key_bytes: [u8; 32] = hex::decode(hex_key).unwrap().try_into().unwrap();
        let result = famp_crypto::keys::TrustedVerifyingKey::from_bytes(&key_bytes);
        assert!(result.is_err(), "key '{name}' must be rejected at ingress");
    }
}
```

### Python Fixture Generator (For Provenance Documentation)

The following Python script is the provenance of `worked-example.json`. It MUST be saved to `crates/famp-crypto/tests/vectors/famp-sig-v1/PROVENANCE.md` as documentation. Do not run it to regenerate bytes; the bytes are already in the spec.

```python
# Source: FAMP-v0.5.1-spec.md §7.1c.0 — Python reference used to generate bytes
# Tool versions: jcs==0.2.1, cryptography==46.0.7
import jcs, base64
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

envelope = {
    "famp": "0.5.1",
    "id": "01890a3b-2c4d-7e5f-8a1b-0c2d3e4f5a6b",
    "from": "agent:example.test/alice",
    "to": "agent:example.test/bob",
    "scope": "standalone",
    "class": "ack",
    "causality": { "rel": "acknowledges", "ref": "01890a3b-1111-7222-8333-444444444444" },
    "authority": "advisory",
    "ts": "2026-04-13T00:00:00Z",
    "body": { "disposition": "accepted" },
}
canonical = jcs.canonicalize(envelope)
sk = Ed25519PrivateKey.from_private_bytes(bytes.fromhex(
    "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60"))
sig = sk.sign(b"FAMP-sig-v1\x00" + canonical)
print("canonical_hex:", canonical.hex())
print("signing_input_hex:", (b"FAMP-sig-v1\x00" + canonical).hex())
print("signature_hex:", sig.hex())
print("signature_b64url:", base64.urlsafe_b64encode(sig).rstrip(b"=").decode())
```

**Python API notes (no interop surprises):**
- `Ed25519PrivateKey.from_private_bytes(seed)` takes the 32-byte seed — same byte layout as `ed25519_dalek::SigningKey::from_bytes(&[u8; 32])`. No conversion needed.
- `.sign(msg)` returns raw 64-byte `R || S`. No DER encoding. No ASN.1.
- `sk.public_key().public_bytes(Raw, Raw)` returns the raw 32-byte public key — same as `dalek::VerifyingKey::to_bytes()`.
- There is NO endianness difference between Python `cryptography` and `ed25519-dalek` for the key seed or signature bytes.

---

## Must-Reject Fixtures (CRYPTO-03)

The `tests/vectors/must-reject/weak-keys.json` fixture must include at minimum:

| Name | Public Key Hex | Why Rejected |
|------|----------------|--------------|
| `identity` | `0000000000000000000000000000000000000000000000000000000000000000` | Identity point, order 1, weakest small-order key |
| `low-order-1` | `0100000000000000000000000000000000000000000000000000000000000000` | Edwards (1,0) order-1 point (neutral element in twisted Edwards) |
| `torsion-point-8-order` | From ed25519-speccheck | 8-torsion subgroup point |

Note: For the full 8-torsion point set, use the `novifinancial/ed25519-speccheck` repository's generated `cases.json` file. The identity and (1,0) points above are deterministic and can be hardcoded. Author the fixture file manually rather than running speccheck as a build dep.

The key property: `VerifyingKey::is_weak()` returns `true` for all these points. The fixture runner must verify `TrustedVerifyingKey::from_bytes` returns `Err(CryptoError::WeakKey)` for each entry.

Also add `tests/vectors/must-reject/malformed-b64.json` with these rejection cases (from spec §7.1b decoder rejection list):
- padded input: `"d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a="`
- wrong alphabet (`+`/`/`): replace any URL-safe char with standard-base64 char
- embedded whitespace: `"d75a9801 82b10ab7..."`
- wrong length: `"d75a9801"` (too short)
- trailing garbage: `"d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511aXX"`

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `ed25519-dalek::Keypair` | `ed25519-dalek::SigningKey` | dalek 2.0 (2023) | Old `Keypair` API is gone; all tutorials pre-2023 show wrong API |
| `base64::encode_config()` free functions | `base64::Engine::encode()` method | base64 0.21 (2023) | Free functions removed; must use Engine API |
| `#[async_trait]` on Transport trait | Native async fn in traits | Rust 1.75 (Dec 2023) | No boxing overhead; `famp-crypto` is sync so not relevant here |
| `zeroize 1.x #[zeroize(drop)]` attribute | `#[derive(Zeroize, ZeroizeOnDrop)]` | zeroize 1.5+ | Must explicitly derive `ZeroizeOnDrop`; derive alone no longer adds Drop |
| `dalek::Signer::verify` (cofactor-tolerant) | `VerifyingKey::verify_strict` | dalek 1.x→2.x | Must explicitly call `verify_strict`; `verify` still exists as a footgun |

**Deprecated/outdated:**
- `ed25519-dalek 1.x` docs and tutorials (Keypair API, `PublicKey` type) — all renamed in 2.x
- `base64::encode()` / `base64::decode()` free functions — removed in 0.21
- `ed25519-dalek::PublicKey` type — renamed to `VerifyingKey` in 2.x

---

## Open Questions

1. **Small-order point hex values beyond identity**
   - What we know: `VerifyingKey::is_weak()` catches all 8 torsion points; the identity (all-zero) is the simplest to hardcode
   - What's unclear: The exact hex representation of all 8 torsion points of edwards25519 is not in dalek's public docs; `ed25519-speccheck` generates them but as a build step
   - Recommendation: Use identity + `(1,0)` as named fixtures; note in `weak-keys.json` that `is_weak()` covers all 8 torsion points; add a comment pointing to `ed25519-speccheck` for completeness. This is sufficient for the "must reject" gate.

2. **`FampSignature::PartialEq` via `subtle`**
   - What we know: `subtle::ConstantTimeEq` is the right primitive (transitive dep, already available)
   - What's unclear: Whether `FampSignature` actually needs `PartialEq` in any Phase 2 test code
   - Recommendation: Implement it with `subtle` at Phase 2 to set the right pattern; cost is near zero

3. **`subtle` as a direct dependency**
   - What we know: It is a transitive dep via `ed25519-dalek`; pinning it directly ensures the version we test against
   - Recommendation: Add as a direct dep with `version = "2"` in `famp-crypto/Cargo.toml`

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | `cargo-nextest 0.9.132` |
| Config file | `.cargo/nextest.toml` (or none — workspace default applies) |
| Quick run command | `cargo nextest run -p famp-crypto` |
| Full suite command | `cargo nextest run -p famp-crypto --no-fail-fast` |
| CI gate | `just test-canonical-strict` (Phase 1 pattern); Phase 2 adds `just test-crypto-strict` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CRYPTO-01 | `Signer`/`Verifier` traits compile and delegate correctly | unit | `cargo nextest run -p famp-crypto tests::traits` | ❌ Wave 0 |
| CRYPTO-02 | `raw verify` unreachable from public API | compile-time / unit | `cargo nextest run -p famp-crypto tests::api_surface` (negative compile test via `trybuild` or visibility check) | ❌ Wave 0 |
| CRYPTO-03 | Weak keys rejected at ingress with must-reject fixtures | unit | `cargo nextest run -p famp-crypto tests::weak_key_rejection` | ❌ Wave 0 |
| CRYPTO-04 | Domain prefix applied; signing input byte-exact | unit | `cargo nextest run -p famp-crypto tests::worked_example` | ❌ Wave 0 |
| CRYPTO-05 | RFC 8032 Ed25519 vectors (all 5) pass | unit (CI gate) | `cargo nextest run -p famp-crypto tests::rfc8032_vectors` | ❌ Wave 0 |
| CRYPTO-06 | Base64url round-trip; strict decoder rejects malformed input | unit + proptest | `cargo nextest run -p famp-crypto tests::base64_roundtrip tests::base64_must_reject` | ❌ Wave 0 |
| CRYPTO-07 | SHA-256 artifact ID available via `famp-canonical` | integration (smoke) | `cargo nextest run -p famp-crypto tests::sha256_available` | ❌ Wave 0 |
| CRYPTO-08 | No early-return on secret-dependent data (wrapper audit) | doc comment + manual (no automated timing test) | N/A — documented in `lib.rs` + wrapper audit task | N/A |
| SPEC-03 | Domain prefix hex documented + conformance vector #1 committed | unit | Covered by CRYPTO-04 test | ❌ Wave 0 |
| SPEC-19 | Key encoding strictness (rejection list) | unit | Covered by CRYPTO-06 malformed test | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo nextest run -p famp-crypto`
- **Per wave merge:** `cargo nextest run -p famp-crypto --no-fail-fast`
- **Phase gate:** Full suite green (`just ci`) before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `crates/famp-crypto/tests/rfc8032_vectors.rs` — covers CRYPTO-05
- [ ] `crates/famp-crypto/tests/worked_example.rs` — covers CRYPTO-04, SPEC-03
- [ ] `crates/famp-crypto/tests/weak_key_rejection.rs` — covers CRYPTO-03
- [ ] `crates/famp-crypto/tests/base64_roundtrip.rs` — covers CRYPTO-06, SPEC-19
- [ ] `crates/famp-crypto/tests/vectors/rfc8032/test-vectors.json` — RFC 8032 §7.1 fixture data
- [ ] `crates/famp-crypto/tests/vectors/famp-sig-v1/worked-example.json` — §7.1c fixture (copy bytes from spec)
- [ ] `crates/famp-crypto/tests/vectors/must-reject/weak-keys.json` — small-order public keys
- [ ] `crates/famp-crypto/tests/vectors/must-reject/malformed-b64.json` — base64 rejection cases
- [ ] Justfile: add `test-crypto` and `test-crypto-strict` recipes mirroring Phase 1's `test-canonical` pattern
- [ ] `Cargo.toml` for `famp-crypto`: add `ed25519-dalek`, `base64`, `zeroize`, `subtle`, `thiserror`, `serde`, `serde_json`, `famp-canonical` deps; dev deps `hex`, `proptest`, `insta`

---

## Sources

### Primary (HIGH confidence)

- `FAMP-v0.5.1-spec.md` §7.1a, §7.1b, §7.1c — domain prefix bytes, encoding rules, full worked example with all hex values
- `FAMP-v0.5.1-spec.md` §7.1c.0 — Python script provenance of worked-example bytes
- `.planning/phases/02-crypto-foundations/02-CONTEXT.md` — all implementation decisions D-01 through D-29
- `.planning/research/PITFALLS.md` — P4 (weak keys), P5 (domain sep), P10 (self-generated vectors)
- RFC 8032 §7.1 (via rfc-editor.org) — all 5 Ed25519 test vectors extracted verbatim
- `docs.rs/ed25519-dalek/latest/ed25519_dalek/struct.VerifyingKey.html` — `verify_strict`, `is_weak`, `from_bytes` API signatures
- Python `cryptography` official docs `Ed25519PrivateKey.from_private_bytes` — 32-byte raw seed API confirmed

### Secondary (MEDIUM confidence)

- `deepwiki.com/dalek-cryptography/ed25519-dalek/4.1-test-suites` — confirmed `is_weak()` and weak-key tests exist in dalek test suite
- `github.com/novifinancial/ed25519-speccheck` — 12-vector edge case suite; behavioral outcome matrix (not raw hex values for torsion points)
- `zeroize` crate docs (docs.rs) — `ZeroizeOnDrop` derive pattern confirmed
- `subtle` crate docs (docs.rs) — `ConstantTimeEq::ct_eq` returns `Choice`, converts to `bool` via `.into()`
- `predr.ag/blog/definitive-guide-to-sealed-traits-in-rust` — sealed trait pattern with private module
- IACR ePrint 2020/1244 "Taming the Many EdDSAs" — confirms `verify_strict` vs `verify` semantics

### Tertiary (LOW confidence — flagged for validation)

- Torsion point hex values for edwards25519 beyond identity and `(1,0)` — not independently verified from official source; use `is_weak()` as the authoritative check rather than hardcoding all 8

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all versions in workspace Cargo.toml, confirmed current
- Domain prefix bytes: HIGH — extracted verbatim from spec §7.1a and §7.1c.4
- Worked example bytes: HIGH — in spec §7.1c.3/c.6, provenance Python script in §7.1c.0
- Architecture patterns: HIGH — derived from CONTEXT.md locked decisions D-01 through D-29
- Weak key rejection: HIGH — `is_weak()` API confirmed from docs.rs; specific torsion point hex is MEDIUM
- Python interop quirks: HIGH — `from_private_bytes` takes 32-byte seed, sign returns raw bytes, no endianness issue
- Constant-time discipline: HIGH — delegation to `verify_strict` is the authoritative CT claim; wrapper audit approach is sound

**Research date:** 2026-04-13
**Valid until:** 2026-05-13 (stable cryptographic library ecosystem; unlikely to change in 30 days)

# famp-crypto

FAMP v0.5.1 Ed25519 sign/verify with domain separation (`FAMP-sig-v1\0`),
weak-key ingress rejection, and `verify_strict`-only exposure. Every byte
that enters FAMP's signature path passes through functions in this crate
which prepend the domain prefix internally — callers never assemble
signing input manually.

## Domain separation

All FAMP signatures are computed over the exact byte sequence:

```
DOMAIN_PREFIX || canonical_json_bytes
```

Where `DOMAIN_PREFIX = b"FAMP-sig-v1\x00"` (12 bytes, hex
`46414d502d7369672d763100`) as fixed by FAMP-v0.5.1 spec §7.1a.

The sanctioned path to produce the full signing input is
`canonicalize_for_signature(unsigned_value)`, which returns
`DOMAIN_PREFIX || famp_canonical::canonicalize(unsigned_value)`. The
signing and verification free functions (`sign_canonical_bytes`,
`verify_canonical_bytes`) prepend the prefix themselves internally — no
public surface expects a caller to concatenate the prefix manually.

`DOMAIN_PREFIX` is exposed as a public constant for fixture/test use
only. Using it outside of `canonicalize_for_signature` is an anti-pattern.

## Worked example (§7.1c)

The FAMP v0.5.1 spec §7.1c ships a full worked example over the RFC 8032
§7.1 Test 1 keypair. This crate commits those bytes verbatim as a
cross-language conformance fixture:

- `tests/vectors/famp-sig-v1/worked-example.json` — the fixture itself
  (secret/public key, unsigned envelope, canonical hex, signing-input hex,
  signature hex + base64url, re-embedded envelope).
- `tests/vectors/famp-sig-v1/PROVENANCE.md` — documents the Python
  reference tooling (`jcs 0.2.1` + `cryptography 46.0.7`) used to generate
  the bytes in the first place. The Rust implementation's job is to MATCH
  those bytes, not reproduce them. See PITFALLS P10 for why self-generated
  bytes defeat interop.
- `tests/worked_example.rs` — asserts `canonicalize_for_signature` output
  is byte-identical to `signing_input_hex` and that
  `verify_canonical_bytes` accepts the spec signature. This test runs as a
  blocking CI gate via `just test-crypto`.

Abbreviated test body:

```rust
let unsigned: serde_json::Value = serde_json::from_str(&f.unsigned_envelope_json)?;
let actual = canonicalize_for_signature(&unsigned)?;
assert_eq!(actual, hex::decode(&f.signing_input_hex)?);

let vk = TrustedVerifyingKey::from_bytes(&pk_bytes)?;
let sig = FampSignature::from_bytes(sig_bytes);
verify_canonical_bytes(&vk, &canonical, &sig)?;
```

## Public API

Free functions (primary surface):

- `sign_value<T: Serialize>(&FampSigningKey, &T) -> Result<FampSignature, CryptoError>`
- `sign_canonical_bytes(&FampSigningKey, &[u8]) -> FampSignature`
- `verify_value<T: Serialize>(&TrustedVerifyingKey, &T, &FampSignature) -> Result<(), CryptoError>`
- `verify_canonical_bytes(&TrustedVerifyingKey, &[u8], &FampSignature) -> Result<(), CryptoError>`
- `canonicalize_for_signature(&serde_json::Value) -> Result<Vec<u8>, CryptoError>`

Types:

- `FampSigningKey` — wraps `ed25519_dalek::SigningKey` with `zeroize` on drop
- `TrustedVerifyingKey` — wraps `ed25519_dalek::VerifyingKey`; the only
  verifying-key type reachable from the public API. Construction performs
  weak-key ingress rejection (see below).
- `FampSignature` — wraps `ed25519_dalek::Signature`; uses
  `subtle::ConstantTimeEq` for equality.
- `DOMAIN_PREFIX: &[u8; 12]` — public read-only constant.
- `CryptoError` — narrow error enum (`thiserror`), Phase 2 scope only.

**Explicitly NOT re-exported:** `ed25519_dalek::VerifyingKey` is not
re-exported. There is NO public path from this crate to
`ed25519_dalek::VerifyingKey::verify` (non-strict); only `verify_strict`
is reachable, and only via `TrustedVerifyingKey`, which cannot be
constructed without passing ingress checks.

## Weak-key rejection at ingress

Per CRYPTO-02/03 and spec §7.1b, every public key MUST be validated
before it can be trusted by the verification path. `TrustedVerifyingKey`
is the compiler-enforced expression of this rule: there is no public
untrusted-verifying-key type, so no verify call site can be reached
with an unchecked key.

`TrustedVerifyingKey::from_bytes` performs:

1. Length check (exactly 32 bytes — compiler-enforced by `[u8; 32]`).
2. `ed25519_dalek::VerifyingKey::from_bytes` — rejects non-canonical
   Edwards-point encodings (→ `CryptoError::InvalidKeyEncoding`).
3. `is_weak()` — rejects small-order / 8-torsion public keys
   (→ `CryptoError::WeakKey`).

Named "must-reject" fixtures live under
`tests/vectors/must-reject/weak-keys.json` and are exercised by
`tests/weak_key_rejection.rs`.

## Content addressing (CRYPTO-07)

FAMP content-addresses artifacts using the form `sha256:<lowercase-hex>`,
matching the spec's `artifact-id` scheme. This crate exposes the single
sanctioned path to produce that string — callers MUST NOT re-implement the
hash or the hex encoding locally.

Public API:

- `famp_crypto::sha256_artifact_id(bytes: &[u8]) -> String` — returns a
  71-character `String` of the form `sha256:<64-lowercase-hex>`.
- `famp_crypto::sha256_digest(bytes: &[u8]) -> [u8; 32]` — the raw
  SHA-256 digest, exposed for callers that need the unformatted bytes.

Example:

```rust
use famp_crypto::sha256_artifact_id;
let id = sha256_artifact_id(b"abc");
assert_eq!(
    id,
    "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
);
```

The helper is backed by the RustCrypto `sha2` crate, pinned at the
workspace root (`sha2 = "0.11.0"`). The conformance gate is
`crates/famp-crypto/tests/sha256_vectors.rs`, which asserts three NIST
FIPS 180-2 Known Answer Tests byte-exactly (empty string, `"abc"`, and
the 56-byte Appendix B.2 vector) plus shape invariants and
digest/artifact-id agreement. It runs as a blocking check via
`just test-crypto` and the CI `test-crypto` job — if any KAT regresses,
the whole crate is unshippable.

## Constant-time verification (CRYPTO-08)

The Phase 2 constant-time claim, per decisions D-22 and D-23:

1. Cryptographic constant-time properties are delegated to
   `ed25519-dalek`'s `verify_strict`.
2. This wrapper introduces no avoidable pre-verification branching on
   secret-dependent data.
3. Weak-key ingress rejection and decode validation happen BEFORE the key
   is trusted — this is policy validation on public material, not
   secret-dependent runtime branching.
4. No statistical timing tests (dudect-style) run in CI — they are
   environment-sensitive and deferred.

## Wrapper audit

Each sign/verify error path has been audited for secret-dependent
short-circuiting:

- `sign_value`: canonicalization error from `famp_canonical::canonicalize`
  is a serialization error over PUBLIC value data, not secret. Short-
  circuit on canonical error happens before any secret-key access. OK
- `sign_canonical_bytes`: no branching — always prepends prefix and calls
  `dalek::SigningKey::sign`. OK
- `verify_value`: canonicalization error branches on PUBLIC value data.
  No secret involved. OK
- `verify_canonical_bytes`: unconditionally builds signing input,
  unconditionally calls `verify_strict`. Error mapping
  (`VerificationFailed`) discards upstream detail and does not leak
  timing. OK
- `TrustedVerifyingKey::from_bytes`: branches on PUBLIC key bytes
  (encoding check + `is_weak`). The key is not yet trusted at this
  point — no secret material. OK
- `FampSigningKey::from_bytes`: infallible, no branching. OK
- `FampSignature` equality uses `subtle::ConstantTimeEq::ct_eq`. OK

## What's NOT in this crate

Deferred by Phase 2 scope (see `.planning/phases/02-crypto-foundations/02-CONTEXT.md`):

- `sign_envelope` / `verify_envelope` — envelope schema and signature
  field-strip policy belong in `famp-envelope` (later milestone).
- FIPS profile via `aws-lc-rs` — deferred; v1 ships pure-Rust
  `ed25519-dalek`.
- `no_std` support — deferred.
- Statistical timing tests (dudect-style) — deferred.
- Agent Card / trust-store policy beyond raw weak-key rejection —
  belongs in `famp-identity` (later phase).
- Protocol error taxonomy mapping (spec §15.1 15 categories) — belongs
  in `famp-core` (Phase 3); `CryptoError` stays narrow.

# Phase 2: Crypto Foundations - Context

**Gathered:** 2026-04-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Ship `famp-crypto`: a FAMP-specific Ed25519 sign/verify surface with the `FAMP-sig-v1\x00` domain-separation prefix (spec §7.1a) applied internally, `verify_strict`-only exposure, weak-key rejection at ingress (spec §7.1b), unpadded base64url codec for keys and signatures (SPEC-19), and the §7.1c worked example committed as a byte-exact conformance gate alongside RFC 8032 test vectors. SHA-256 artifact helpers continue to live in `famp-canonical` (Phase 1 D-19) and are re-exported or referenced as needed.

**Out of scope:**
- Envelope schema, required-field rules, signature field strip/re-embed policy → `famp-envelope` (later milestone)
- Agent Card / trust-store policy beyond raw weak-key rejection → `famp-identity` (later phase)
- Protocol error taxonomy (spec §15.1 15 categories) → `famp-core` (Phase 3)
- FIPS profile / `aws-lc-rs` swap → deferred
- `no_std` support → deferred

</domain>

<decisions>
## Implementation Decisions

### Public API shape
- **D-01:** Mirror Phase 1's pattern: free functions are primary; traits are thin sugar that delegate to them.
- **D-02:** Typed-value APIs are the primary path (canonicalize internally, then apply prefix, then sign). Byte-level APIs exist for fixtures, the worked example, and future envelope-layer code that already holds canonical bytes.
- **D-03:** Signatures:
  ```rust
  pub fn sign_value<T: Serialize + ?Sized>(
      signing_key: &FampSigningKey,
      value: &T,
  ) -> Result<FampSignature, CryptoError>;

  pub fn verify_value<T: Serialize + ?Sized>(
      verifying_key: &TrustedVerifyingKey,
      value: &T,
      signature: &FampSignature,
  ) -> Result<(), CryptoError>;

  pub fn sign_canonical_bytes(
      signing_key: &FampSigningKey,
      canonical_bytes: &[u8],
  ) -> FampSignature;

  pub fn verify_canonical_bytes(
      verifying_key: &TrustedVerifyingKey,
      canonical_bytes: &[u8],
      signature: &FampSignature,
  ) -> Result<(), CryptoError>;

  pub trait Signer {
      fn sign_value<T: Serialize + ?Sized>(&self, value: &T) -> Result<FampSignature, CryptoError>;
      fn sign_canonical_bytes(&self, canonical_bytes: &[u8]) -> FampSignature;
  }

  pub trait Verifier {
      fn verify_value<T: Serialize + ?Sized>(&self, value: &T, signature: &FampSignature) -> Result<(), CryptoError>;
      fn verify_canonical_bytes(&self, canonical_bytes: &[u8], signature: &FampSignature) -> Result<(), CryptoError>;
  }
  ```
- **D-04:** Name the byte-level path explicitly (`*_canonical_bytes`) so misuse is self-announcing. Do NOT make pre-canonical bytes the only entrypoint — invites immediate footguns.
- **D-05:** Do NOT expose `ed25519_dalek::SigningKey` methods as the main surface. FAMP signing rules (domain prefix, canonical-JSON input) are protocol-specific, not generic Ed25519.

### Key and signature types
- **D-06:** FAMP-owned newtypes wrap `ed25519_dalek` types. Raw dalek types are NOT re-exported as the public API.
  - `FampSigningKey`
  - `TrustedVerifyingKey` (see D-10 — only trusted form is public)
  - `FampSignature`
- **D-07:** Base64url helpers live as methods on the newtypes, not in a free `encoding` module. Encoding is part of the FAMP wire contract — methods on the protocol types read cleanly and keep the invariant close to the data.
  ```rust
  impl FampSigningKey {
      pub fn from_bytes(bytes: [u8; 32]) -> Self;
      pub fn from_b64url(input: &str) -> Result<Self, CryptoError>;
      pub fn to_b64url(&self) -> String;
  }

  impl TrustedVerifyingKey {
      pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, CryptoError>; // performs ingress checks
      pub fn from_b64url(input: &str) -> Result<Self, CryptoError>;
      pub fn to_b64url(&self) -> String;
  }

  impl FampSignature {
      pub fn from_bytes(bytes: [u8; 64]) -> Self;
      pub fn from_b64url(input: &str) -> Result<Self, CryptoError>;
      pub fn to_b64url(&self) -> String;
  }
  ```
- **D-08:** Enable `zeroize` on `FampSigningKey` now (Drop wipes secret bytes). Cheap, aligned with workspace deps, deferring gains nothing.
- **D-09:** Base64url decoders enforce the spec §7.1b rejection list: no `=` padding, no `+`/`/`, no embedded whitespace, no trailing garbage, exact expected length. Use the `base64` crate `URL_SAFE_NO_PAD` engine with strict config.

### Weak-key rejection
- **D-10:** `TrustedVerifyingKey` is the **only** verifying-key type reachable from public API. Its constructor (`from_bytes` / `from_b64url`) performs the mandatory ingress checks; there is no public "untrusted" variant that can exist in an unchecked state. Every `verify_*` API takes `&TrustedVerifyingKey`, so the invariant is compiler-enforced: an untrusted key cannot reach a verify call site at all.
- **D-11:** Ingress checks performed by `TrustedVerifyingKey::from_bytes`:
  1. Length validation (exactly 32 bytes)
  2. Decode via `ed25519_dalek::VerifyingKey::from_bytes` (canonical Edwards point)
  3. Small-order / 8-torsion rejection (weak key)
  - Failures map to `CryptoError::WeakKey` or `CryptoError::InvalidKeyEncoding` with a clear split.
- **D-12:** "Must reject" fixture set committed under `famp-crypto/tests/vectors/must-reject/`:
  - Malformed 32-byte decode cases (non-canonical point encoding)
  - Identity point and small-order public-key fixtures
  - Known weak-point fixtures (source from `curve25519-dalek` / `ed25519-dalek` test material if available; author explicitly otherwise)
  - Base64url decode failures: padded input, wrong alphabet (`+`/`/`), embedded whitespace, wrong length
  - Named fixtures over corpus size — each has a human-readable name proving "these bytes never become a trusted key."

### Worked-example fixture format
- **D-13:** Single JSON vector file, pedagogical, cross-language reusable. NOT insta snapshots (snapshots are dev ergonomics, not a protocol artifact).
- **D-14:** Location: `crates/famp-crypto/tests/vectors/famp-sig-v1/worked-example.json`
- **D-15:** Schema:
  ```json
  {
    "name": "famp-v0.5.1-section-7.1c-worked-example",
    "spec_version": "0.5.1",
    "domain_prefix_hex": "46414d502d7369672d763100",
    "secret_key_hex": "...",
    "public_key_hex": "...",
    "unsigned_envelope_json": "{...}",
    "canonical_json_hex": "...",
    "signing_input_hex": "...",
    "signature_hex": "...",
    "signature_b64url": "...",
    "signed_envelope_json": "{...}"
  }
  ```
- **D-16:** Bytes sourced externally (Python `jcs 0.2.1` + `cryptography 46.0.7` per PITFALLS P10), NOT self-produced by `famp-crypto` during authoring. Provenance documented in vector file header comment or a sidecar `PROVENANCE.md`.
- **D-17:** Author this vector with the expectation that `famp-conformance` (later milestone) will consume it unchanged. Schema stability matters.
- **D-18:** RFC 8032 Ed25519 vectors live alongside in `tests/vectors/rfc8032/`. Separate file set; different purpose (algorithm-level, not protocol-level).

### Envelope signature-field handling
- **D-19:** Phase 2 stays **envelope-agnostic**. `famp-crypto` does NOT own an `Envelope` type, does NOT know which field is called `signature`, and does NOT ship `sign_envelope` / `verify_envelope` helpers. That belongs in `famp-envelope`.
- **D-20:** Expose a narrowly-scoped signing-input helper over generic JSON values:
  ```rust
  pub fn canonicalize_for_signature(
      unsigned_value: &serde_json::Value,
  ) -> Result<Vec<u8>, CryptoError>;
  ```
  Contract:
  - Caller provides the envelope with `signature` field already omitted
  - Function returns `prefix || canonical_json_bytes` (the full signing input)
- **D-21:** The §7.1c worked-example test proves byte-exact gate by: (1) parse unsigned envelope from fixture, (2) canonicalize, (3) prepend prefix, (4) assert equality with `signing_input_hex`, (5) verify signature, (6) separately assert re-embedding signature into unsigned envelope reproduces `signed_envelope_json`. Step 6 is a fixture sanity check, not a `famp-crypto` feature.

### Constant-time verification
- **D-22:** Approach: document + wrapper audit. NOT statistical timing tests in CI (dudect-style tests are noisy, environment-sensitive, and produce false confidence or flaky builds — wrong ambition for v0.6).
- **D-23:** The Phase 2 claim, documented in README and `lib.rs` doc comments:
  1. Cryptographic constant-time properties are delegated to `ed25519-dalek`'s `verify_strict`.
  2. Our wrapper introduces no avoidable pre-verification branching on secret-dependent data.
  3. Weak-key ingress rejection and decode validation happen **before** the key is trusted — this is policy validation on public material, not secret-dependent runtime branching.
- **D-24:** Wrapper audit as a planning task: enumerate every error path in `sign_*` / `verify_*` and confirm none short-circuits on secret material. Document findings inline.

### Domain prefix exposure
- **D-25:** Expose the prefix as a public read-only constant for test/fixture use:
  ```rust
  pub const DOMAIN_PREFIX: &[u8; 12] = b"FAMP-sig-v1\0";
  ```
  But DO NOT encourage callers to assemble signing input manually. `canonicalize_for_signature` is the sanctioned path.

### Error surface (CryptoError)
- **D-26:** Keep `CryptoError` narrow and phase-appropriate, matching Phase 1's discipline (Phase 1 D-16). Do NOT overfit to the spec §15.1 15-category protocol enum — that lives in `famp-core` (Phase 3).
- **D-27:** Phase 2 variants (minimum viable set):
  - `InvalidKeyEncoding` — bad bytes, wrong length, non-canonical point
  - `InvalidSignatureEncoding` — bad bytes, wrong length on signature
  - `WeakKey` — small-order / 8-torsion public key at ingress
  - `Canonicalization(famp_canonical::CanonicalError)` — transparent upstream wrap
  - `VerificationFailed` — Ed25519 `verify_strict` returned error (no detail leak)
  - `InvalidSigningInput` — signing-input helper misuse (e.g., non-object JSON where object expected)
  - Optional: `Base64` — distinct decode bucket if cleaner than folding into `InvalidKeyEncoding` / `InvalidSignatureEncoding`
- **D-28:** Use `thiserror`. Never return `anyhow::Result` from `famp-crypto` public API.

### SHA-256 / artifact ID
- **D-29:** `sha2` dependency already used by `famp-canonical` (Phase 1 D-19/D-20). Phase 2 adds no new artifact-ID helpers. CRYPTO-07 is satisfied by verifying the `famp-canonical` helpers remain callable and by listing `sha2` as a transitive availability in `famp-crypto`'s README. No re-export unless a concrete consumer need emerges during planning.

### Claude's Discretion
- Exact `zeroize` integration mechanics (derive macro vs manual `Drop`)
- `Debug` impl for `FampSigningKey` — MUST redact; default implementation choice (e.g., `"FampSigningKey(<redacted>)"`)
- Whether `FampSignature` implements `PartialEq` constant-time (probably yes, via `subtle::ConstantTimeEq`) — document either way
- Internal module layout (`keys.rs`, `sign.rs`, `verify.rs`, `encoding.rs`, etc.)
- Whether to expose `Display` on key/signature types (probably yes, defers to `to_b64url`)
- Exact wording of README worked-example section

</decisions>

<specifics>
## Specific Ideas

- Mirror Phase 1's API pattern deliberately — `famp-canonical` established "free function primary, trait as sugar, narrow error enum, external vectors as hard CI gate." Phase 2 should feel like the obvious continuation.
- The §7.1c worked example is the single most important artifact in this phase. It is the cross-language interop contract made concrete. Everything else is scaffolding around it.
- "Trusted-only verifying key" is the type-system expression of the spec's "MUST reject at ingress, not only at verify time" rule. Prefer compiler enforcement over runtime discipline wherever the spec says MUST.

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Spec (v0.5.1 fork — the authority)
- `FAMP-v0.5.1-spec.md` §7.1 — Signature (recipient binding)
- `FAMP-v0.5.1-spec.md` §7.1a — Domain separation, prefix bytes `b"FAMP-sig-v1\x00"`, signing formula
- `FAMP-v0.5.1-spec.md` §7.1b — Ed25519 encoding (32/64 byte raw, unpadded base64url, decoder rejection list, strict verification, weak-key ingress rejection)
- `FAMP-v0.5.1-spec.md` §7.1c — Worked signature example (test keypair, canonical bytes, signing input, signature, re-embedded envelope, normative verification procedure)

### Requirements
- `.planning/REQUIREMENTS.md` — CRYPTO-01..08, SPEC-03, SPEC-19 (rows and acceptance criteria)

### Research / pitfalls (context for decisions)
- `.planning/research/PITFALLS.md` P10 — Domain separation interop hazards, "worked example with hex dumps" requirement, externally-sourced bytes from Python `jcs 0.2.1` + `cryptography 46.0.7`
- `.planning/research/ARCHITECTURE.md` — Crate-level boundaries for `famp-crypto` vs `famp-canonical` vs `famp-core`
- `.planning/PROJECT.md` — Tech-stack Table row 1 (`ed25519-dalek 2.2.0`, `verify_strict`), row 5 (`base64 0.22.1` URL_SAFE_NO_PAD), row 6 (`sha2 0.11.0`)

### Prior phase context
- `.planning/phases/01-canonical-json-foundations/01-CONTEXT.md` — Phase 1 API pattern (free fn + trait), error-surface discipline (narrow, phase-appropriate), artifact-ID helpers (D-19/D-20), strict-parse ingress (D-04..D-07)

### Upstream dependency docs
- `ed25519-dalek 2.2` docs.rs — `SigningKey`, `VerifyingKey`, `verify_strict`, `Signature`
- `base64 0.22` docs.rs — `URL_SAFE_NO_PAD` engine, strict config
- RFC 8032 — Ed25519 test vectors (§7.1 Test 1 is the keypair source for §7.1c)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `famp-canonical::canonicalize` — `sign_value` delegates here to produce canonical bytes before prefix prepend
- `famp-canonical::from_slice_strict` / `from_str_strict` — duplicate-key-rejecting ingress for signed JSON bytes (inbound verify paths)
- `famp-canonical::CanonicalError` — wrapped transparently inside `CryptoError::Canonicalization`
- `famp-canonical` `sha256:<hex>` artifact-ID helpers (Phase 1 D-19/D-20) — satisfy CRYPTO-07 without new code in Phase 2
- Phase 1's RFC 8785 Appendix B vectors test harness pattern — Phase 2's RFC 8032 + §7.1c vectors mirror that harness structure

### Established Patterns
- **Free function primary, trait as sugar** (Phase 1 D-01, D-02) — `famp-crypto` follows exactly
- **Narrow, phase-appropriate error enum via `thiserror`** (Phase 1 D-16, D-18) — `CryptoError` follows exactly
- **External vectors as hard CI gate** (Phase 1 D-12, roadmap Phase 1 SC-#2) — `famp-crypto` gates on RFC 8032 vectors + §7.1c worked example
- **`#![forbid(unsafe_code)]`** already in the Phase 0 stub
- **Workspace inheritance** (`version.workspace = true` etc.) for Cargo.toml

### Integration Points
- `famp-crypto/Cargo.toml` adds deps: `ed25519-dalek = { workspace = true, features = ["zeroize"] }`, `base64 = { workspace = true }`, `zeroize = { workspace = true }`, `serde = { workspace = true }`, `serde_json = { workspace = true }`, `famp-canonical = { path = "../famp-canonical" }`, `thiserror = { workspace = true }`, `sha2 = { workspace = true }` (transitive availability for CRYPTO-07), plus dev-deps for vectors (`hex`, etc.)
- Public API is consumed by Phase 3 (`famp-core`'s error enum references `CryptoError` category) and future `famp-envelope` (which builds `sign_envelope` on top of `canonicalize_for_signature` + `sign_canonical_bytes`)
- CI `just ci` recipe adds vector-gate step; nightly workflow unchanged

</code_context>

<deferred>
## Deferred Ideas

- **`sign_envelope` / `verify_envelope` helpers** — deferred to `famp-envelope` where envelope schema and field-strip policy actually live
- **FIPS profile via `aws-lc-rs`** — deferred; v1 ships pure-Rust `ring`-free `ed25519-dalek`
- **`no_std` support** — deferred
- **Statistical timing tests (dudect-style)** — deferred (D-22); not appropriate for v0.6 ambition
- **Agent Card / trust-store policy beyond raw weak-key rejection** — belongs in `famp-identity` (later phase)
- **Protocol error taxonomy mapping** — spec §15.1 15 categories belong in `famp-core` (Phase 3); `CryptoError` stays narrow
- **Key generation / KMS integration / hardware-backed signing** — out of scope; `FampSigningKey::from_bytes` is sufficient for Phase 2

</deferred>

---

*Phase: 02-crypto-foundations*
*Context gathered: 2026-04-13*

#![forbid(unsafe_code)]
//! FAMP v0.5.1 — `famp-crypto`: Ed25519 sign/verify for FAMP envelopes with
//! domain separation and canonical-JSON preconditioning.
//!
//! # Three load-bearing invariants
//!
//! 1. **Domain separation.** Every signed payload is prefixed with
//!    [`DOMAIN_PREFIX`] (`b"FAMP-sig-v1\0"`, 12 bytes) before Ed25519 touches
//!    it. Prevents a FAMP signature from being replayed in any other context
//!    that also signs canonical JSON. Spec §7.1a, §Δ08. The prefix is the
//!    wire version: rotating to `FAMP-sig-v2\0` is the *only* sanctioned way
//!    to change signing semantics — do not add fields, do not rename, do not
//!    skip.
//!
//! 2. **`verify_strict`-only verification.** All verification routes through
//!    `ed25519_dalek::VerifyingKey::verify_strict`, never plain `verify`.
//!    `verify_strict` rejects non-canonical signatures and small-order
//!    points; plain `verify` accepts malleable signatures that silently
//!    break non-repudiation. No public path in this crate reaches the
//!    non-strict form.
//!
//! 3. **Canonicalization is a precondition, not a step.** The
//!    `_canonical_bytes` entry points ([`sign_canonical_bytes`],
//!    [`verify_canonical_bytes`]) require input that has *already* been
//!    produced by `famp_canonical::canonicalize` (RFC 8785 JCS). Passing raw
//!    `serde_json::to_vec` output produces valid-looking signatures that
//!    will not round-trip across implementations. Use [`sign_value`] /
//!    [`verify_value`] if you want the canonicalize step done for you.
//!
//! See `FAMP-v0.5.1-spec.md` §7.1 and INV-10 ("every envelope signed;
//! unsigned rejected") for the protocol framing. See `README.md` for the
//! worked §7.1c example, constant-time rationale, and wrapper audit
//! (CRYPTO-08).
//!
//! # Quick start
//!
//! ```
//! use famp_crypto::{sign_value, verify_value, FampSigningKey};
//! use serde_json::json;
//!
//! // NOTE: [0u8; 32] is a test seed only — never use in production.
//! let sk = FampSigningKey::from_bytes([0u8; 32]);
//! let vk = sk.verifying_key();
//! let v = json!({"hello": "world"});
//! let sig = sign_value(&sk, &v).unwrap();
//! verify_value(&vk, &v, &sig).unwrap();
//! ```

// `zeroize` is pulled in transitively via the `ed25519-dalek` `zeroize`
// feature (drop-time secret wipe), but the workspace dep is listed directly
// for clarity. `serde`/`serde_json` are now directly used by sign/verify.
use zeroize as _;

// Dev-deps referenced only by integration tests in `tests/`. Silence
// `unused_crate_dependencies` for the lib-test compile unit.
#[cfg(test)]
use hex as _;
#[cfg(test)]
use insta as _;
#[cfg(test)]
use proptest as _;

pub mod error;
pub mod hash;
pub mod keys;
pub mod prefix;
pub mod sign;
pub mod traits;
pub mod verify;

pub use error::CryptoError;
pub use hash::{sha256_artifact_id, sha256_digest};
pub use keys::{FampSignature, FampSigningKey, TrustedVerifyingKey};
pub use prefix::{canonicalize_for_signature, DOMAIN_PREFIX};
pub use sign::{sign_canonical_bytes, sign_value};
pub use traits::{Signer, Verifier};
pub use verify::{verify_canonical_bytes, verify_value};

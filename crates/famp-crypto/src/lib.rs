#![forbid(unsafe_code)]
//! FAMP v0.5.1 — `famp-crypto`: Ed25519 sign/verify with domain separation.
//!
//! See `README.md` for the worked §7.1c example, constant-time rationale,
//! and wrapper audit (CRYPTO-08).
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
pub mod keys;
pub mod prefix;
pub mod sign;
pub mod traits;
pub mod verify;

pub use error::CryptoError;
pub use keys::{FampSignature, FampSigningKey, TrustedVerifyingKey};
pub use prefix::{canonicalize_for_signature, DOMAIN_PREFIX};
pub use sign::{sign_canonical_bytes, sign_value};
pub use traits::{Signer, Verifier};
pub use verify::{verify_canonical_bytes, verify_value};

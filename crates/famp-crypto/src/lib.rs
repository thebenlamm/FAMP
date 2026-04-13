#![forbid(unsafe_code)]
#![doc = "FAMP v0.5.1 famp-crypto: Ed25519 sign/verify with domain separation."]

// `zeroize` is pulled in transitively via the `ed25519-dalek` `zeroize`
// feature (drop-time secret wipe), but the workspace dep is listed directly
// for clarity. `serde`/`serde_json` are now directly used by sign/verify.
use zeroize as _;

// Dev-deps referenced only by integration tests in `tests/`. Silence
// `unused_crate_dependencies` for the lib-test compile unit.
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

#![forbid(unsafe_code)]
#![doc = "FAMP v0.5.1 famp-crypto: Ed25519 sign/verify with domain separation."]

// Deps declared for Plan 02 (sign/verify + canonicalize_for_signature) that
// are not yet referenced in Plan 01 Task 1/2 code. Keeping them wired now
// avoids churning Cargo.toml between plans; silence the unused-crate lint
// until the sign/verify modules land.
use serde as _;
use serde_json as _;
// `zeroize` is pulled in transitively via the `ed25519-dalek` `zeroize`
// feature (drop-time secret wipe), but the workspace dep is listed directly
// for clarity and forward-compat with Plan 02 helpers.
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

pub use error::CryptoError;
pub use keys::{FampSignature, FampSigningKey, TrustedVerifyingKey};
pub use prefix::DOMAIN_PREFIX;

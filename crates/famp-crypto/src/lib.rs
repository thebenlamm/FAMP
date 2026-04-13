#![forbid(unsafe_code)]
#![doc = "FAMP v0.5.1 famp-crypto: Ed25519 sign/verify with domain separation."]

pub mod error;
pub mod keys;
pub mod prefix;

pub use error::CryptoError;
pub use keys::{FampSignature, FampSigningKey, TrustedVerifyingKey};
pub use prefix::DOMAIN_PREFIX;

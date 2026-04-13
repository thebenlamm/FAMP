//! Task 1 placeholder — real implementation lands in Task 2.

use ed25519_dalek::{Signature, SigningKey, VerifyingKey};

pub struct FampSigningKey(#[allow(dead_code)] pub(crate) SigningKey);
pub struct TrustedVerifyingKey(#[allow(dead_code)] pub(crate) VerifyingKey);
pub struct FampSignature(#[allow(dead_code)] pub(crate) Signature);

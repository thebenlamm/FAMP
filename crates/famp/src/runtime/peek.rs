//! Runtime wrapper around `famp_envelope::peek_sender`.
//!
//! Phase 4 Plan 04-01 lifted the canonical implementation to `famp-envelope`
//! so the HTTP sig-verify middleware can call it without depending on
//! `crates/famp`. This wrapper preserves the legacy `RuntimeError`-typed
//! signature so existing runtime call sites compile unchanged.

use crate::runtime::error::RuntimeError;
use famp_core::Principal;

pub fn peek_sender(bytes: &[u8]) -> Result<Principal, RuntimeError> {
    famp_envelope::peek_sender(bytes).map_err(RuntimeError::Decode)
}

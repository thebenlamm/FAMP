//! `--peer agent:<authority>/<name>=<base64url-unpadded-pubkey>` parser.
//!
//! D-B4: Uses `=` as the separator (NOT `:`) because the principal string
//! itself contains `:`. Returns a narrow `KeyringError` on any malformation.

use crate::error::KeyringError;
use famp_core::Principal;
use famp_crypto::TrustedVerifyingKey;
use std::str::FromStr;

pub fn parse_peer_flag(raw: &str) -> Result<(Principal, TrustedVerifyingKey), KeyringError> {
    let (principal_str, pubkey_str) =
        raw.split_once('=')
            .ok_or_else(|| KeyringError::InvalidPeerFlag {
                reason: format!("expected 'agent:auth/name=<pubkey>', got: {raw}"),
            })?;
    let principal =
        Principal::from_str(principal_str).map_err(|e| KeyringError::InvalidPeerFlag {
            reason: format!("invalid principal '{principal_str}': {e}"),
        })?;
    let key = TrustedVerifyingKey::from_b64url(pubkey_str)?;
    Ok((principal, key))
}

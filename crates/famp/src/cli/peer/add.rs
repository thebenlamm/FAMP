//! `famp peer add` — register a peer alias in `peers.toml`.
//!
//! Validation gates (all surface typed `CliError` variants, no panics):
//! - `endpoint` must parse as a URL with `https` scheme
//! - `pubkey` must base64url-unpadded-decode to exactly 32 bytes
//! - `alias` must not already exist in `peers.toml`
//!
//! Writes are atomic via `config::write_peers_atomic` (same-dir tempfile +
//! fsync + rename). Phase 1 shipped an empty `peers.toml` placeholder;
//! Phase 3 opens the first write path.

use std::path::Path;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;

use crate::cli::config::{read_peers, write_peers_atomic, PeerEntry};
use crate::cli::error::CliError;
use crate::cli::{home, paths};

/// Production entry point. Resolves `FAMP_HOME` and delegates.
pub fn run_add(
    alias: String,
    endpoint: String,
    pubkey_b64: String,
    principal: Option<String>,
) -> Result<(), CliError> {
    let home = home::resolve_famp_home()?;
    run_add_at(&home, alias, endpoint, pubkey_b64, principal)
}

/// Test-facing entry point: takes an explicit home so integration tests can
/// point at a `TempDir` without mutating process env.
pub fn run_add_at(
    home: &Path,
    alias: String,
    endpoint: String,
    pubkey_b64: String,
    principal: Option<String>,
) -> Result<(), CliError> {
    // 1. Validate endpoint — must be a well-formed HTTPS URL.
    let parsed = url::Url::parse(&endpoint).map_err(|_| CliError::PeerEndpointInvalid {
        value: endpoint.clone(),
    })?;
    if parsed.scheme() != "https" {
        return Err(CliError::PeerEndpointInvalid { value: endpoint });
    }

    // 2. Validate pubkey — base64url-unpadded → 32 bytes.
    let raw =
        URL_SAFE_NO_PAD
            .decode(pubkey_b64.as_bytes())
            .map_err(|_| CliError::PeerPubkeyInvalid {
                value: pubkey_b64.clone(),
            })?;
    if raw.len() != 32 {
        return Err(CliError::PeerPubkeyInvalid { value: pubkey_b64 });
    }

    // 3. Load peers.toml, append (rejecting duplicates), write back atomically.
    let path = paths::peers_toml_path(home);
    let mut peers = read_peers(&path)?;
    let entry = PeerEntry {
        alias: alias.clone(),
        endpoint,
        pubkey_b64,
        principal,
        tls_fingerprint_sha256: None,
    };
    if peers.try_add(entry).is_err() {
        return Err(CliError::PeerDuplicate { alias });
    }
    write_peers_atomic(&path, &peers)?;
    Ok(())
}

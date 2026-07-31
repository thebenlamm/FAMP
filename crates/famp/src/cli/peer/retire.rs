//! `famp peer retire` — the only path that removes key material from the
//! peer trust store (KEYR-02).
//!
//! Refuses to retire an `Active` entry (would silently sever the channel,
//! T-15-14) or a `Revoked` entry (a revocation tombstone is permanent —
//! deleting it would let a re-pin resurrect the key, T-15-04).

use std::path::Path;

use famp_core::Principal;
use famp_keyring::{Keyring, KeyringError};

use crate::cli::error::CliError;
use crate::cli::home;
use crate::cli::peer::identity::gateway_peers_keyring_path;

/// CLI args for `famp peer retire`.
#[derive(clap::Args, Debug)]
pub struct PeerRetireArgs {
    /// The principal whose retired key entry should be removed.
    pub principal: String,
    /// The `key_id` fingerprint of the retired entry to remove.
    #[arg(long)]
    pub key_id: String,
}

/// Production entry point.
pub fn run(args: &PeerRetireArgs) -> Result<(), CliError> {
    let home_path = home::resolve_famp_home()?;
    let principal: Principal =
        args.principal
            .parse()
            .map_err(
                |e: famp_core::ParsePrincipalError| CliError::PeerBlobMalformed {
                    reason: format!("invalid principal '{}': {e}", args.principal),
                },
            )?;
    run_at(&home_path, &principal, &args.key_id)
}

/// Test-facing entry point.
pub fn run_at(home: &Path, principal: &Principal, key_id: &str) -> Result<(), CliError> {
    let keyring_path = gateway_peers_keyring_path(home);
    let mut keyring = Keyring::load_from_file(&keyring_path).map_err(|e| {
        CliError::Generic(format!(
            "failed to load peer keyring at {}: {e}",
            keyring_path.display()
        ))
    })?;

    keyring.retire(principal, key_id).map_err(|e| match e {
        KeyringError::NoSuchKeyEntry { principal, key_id } => CliError::PeerNoSuchKey {
            principal: principal.to_string(),
            key_id,
        },
        KeyringError::CannotRetire {
            principal,
            key_id,
            state,
        } => CliError::PeerCannotRetire {
            principal: principal.to_string(),
            key_id,
            state: state.to_string(),
        },
        other => CliError::Generic(other.to_string()),
    })?;

    keyring.save_to_file(&keyring_path).map_err(|e| {
        CliError::Generic(format!(
            "failed to save peer keyring at {}: {e}",
            keyring_path.display()
        ))
    })?;

    // Gateway-restart notice (D15-B/famp-lead-730, 2026-07-31) — see
    // `peer::rotate::run_at`'s identical notice for the full rationale.
    eprintln!(
        "NOTE: famp-gateway loads its keyring once at startup and will keep honoring the \
         retired key until it restarts. Run `famp daemon restart` (or manually restart the \
         gateway if it is not daemon-managed) to pick up this change."
    );

    Ok(())
}

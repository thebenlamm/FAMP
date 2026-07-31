//! `famp peer revoke` — the LOCAL unilateral revocation path (REVK-02).
//!
//! Transitions a key entry to `revoked` in THIS machine's own trust store,
//! requiring no signature. An operator may always stop trusting a key they
//! hold — no statement from anyone is needed to sever that trust locally.
//!
//! Distinct from `famp peer import-revocation`, which consumes a statement
//! SIGNED by the peer and is fail-closed (verified before it changes any
//! state). `--emit` optionally produces that signed statement so this
//! machine can hand the remediation to the peer's own trust store too —
//! but `--emit` is only meaningful when this machine owns the principal
//! being revoked, since it signs with this machine's own gateway identity.

use std::path::{Path, PathBuf};

use famp_core::Principal;
use famp_keyring::revocation::RevocationStatement;
use famp_keyring::{Keyring, KeyringError};

use crate::cli::error::CliError;
use crate::cli::home;
use crate::cli::peer::identity::{
    gateway_identity_path, gateway_peers_keyring_path, load_or_generate,
};

/// CLI args for `famp peer revoke`.
#[derive(clap::Args, Debug)]
pub struct PeerRevokeArgs {
    /// The principal whose key entry should be revoked.
    pub principal: String,
    /// The `key_id` fingerprint of the entry to revoke.
    #[arg(long)]
    pub key_id: String,
    /// Optional human-readable reason, carried into an emitted statement.
    #[arg(long)]
    pub reason: Option<String>,
    /// Write a signed `RevocationStatement` blob to this path — only
    /// meaningful when this machine owns the principal being revoked, since
    /// it signs with this machine's own gateway identity.
    #[arg(long)]
    pub emit: Option<PathBuf>,
}

/// Production entry point.
pub fn run(args: &PeerRevokeArgs) -> Result<(), CliError> {
    let home_path = home::resolve_famp_home()?;
    let principal: Principal =
        args.principal
            .parse()
            .map_err(
                |e: famp_core::ParsePrincipalError| CliError::PeerBlobMalformed {
                    reason: format!("invalid principal '{}': {e}", args.principal),
                },
            )?;
    let now = now_canonical_utc();
    run_at(
        &home_path,
        &principal,
        &args.key_id,
        args.reason.as_deref(),
        &now,
        args.emit.as_deref(),
    )
}

/// Test-facing entry point.
pub fn run_at(
    home: &Path,
    principal: &Principal,
    key_id: &str,
    reason: Option<&str>,
    now: &str,
    emit: Option<&Path>,
) -> Result<(), CliError> {
    let keyring_path = gateway_peers_keyring_path(home);
    let mut keyring = Keyring::load_from_file(&keyring_path).map_err(|e| {
        CliError::Generic(format!(
            "failed to load peer keyring at {}: {e}",
            keyring_path.display()
        ))
    })?;

    keyring
        .revoke(principal, key_id, now)
        .map_err(|e| match e {
            KeyringError::NoSuchKeyEntry { principal, key_id } => CliError::PeerNoSuchKey {
                principal: principal.to_string(),
                key_id,
            },
            KeyringError::NonCanonicalTimestamp { value } => {
                CliError::Generic(format!("non-canonical timestamp: {value:?}"))
            }
            other => CliError::Generic(other.to_string()),
        })?;

    keyring.save_to_file(&keyring_path).map_err(|e| {
        CliError::Generic(format!(
            "failed to save peer keyring at {}: {e}",
            keyring_path.display()
        ))
    })?;

    println!("revoked {key_id} for {principal}");

    if let Some(emit_path) = emit {
        let statement = RevocationStatement {
            principal: principal.to_string(),
            revoked_key_id: key_id.to_string(),
            revoked_at: now.to_string(),
            reason: reason.map(ToString::to_string),
        };
        let sk = load_or_generate(&gateway_identity_path(home))?;
        let signed = statement
            .sign(&sk)
            .map_err(|e| CliError::Generic(format!("failed to sign revocation statement: {e}")))?;
        let blob = signed.to_blob().map_err(|e| {
            CliError::Generic(format!("failed to serialize revocation statement: {e}"))
        })?;
        std::fs::write(emit_path, blob).map_err(|e| CliError::Io {
            path: emit_path.to_path_buf(),
            source: e,
        })?;
    }

    // Gateway-restart notice (D15-B/famp-lead-730, 2026-07-31): see
    // `peer::rotate::run_at`'s identical notice for the full rationale.
    eprintln!(
        "NOTE: famp-gateway loads its keyring once at startup and will keep honoring the \
         revoked key until it restarts. Run `famp daemon restart` (or manually restart the \
         gateway if it is not daemon-managed) to pick up this change."
    );

    Ok(())
}

/// RFC 3339 (second-precision, `Z`-suffixed) timestamp built positionally
/// so the function is infallible — mirrors `peer::rotate::now_canonical_utc`.
fn now_canonical_utc() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    #[test]
    fn now_canonical_utc_is_exactly_20_bytes_ending_in_z() {
        let s = super::now_canonical_utc();
        assert_eq!(s.len(), 20, "expected exactly 20 bytes, got: {s}");
        assert!(s.ends_with('Z'), "expected trailing 'Z', got: {s}");
        assert!(
            famp_keyring::entry::is_canonical_utc(&s),
            "must satisfy the canonical-UTC gate: {s}"
        );
    }
}

//! `famp peer import-revocation` — consume a peer-signed
//! [`famp_keyring::revocation::SignedRevocation`] blob and apply it,
//! fail-closed (REVK-02).
//!
//! Distinct from `famp peer revoke` (a LOCAL unilateral decision needing no
//! signature — an operator may always stop trusting a key in their own
//! trust store): this command verifies the statement's signature against
//! the D15-B authorized-signer set before it changes anything.

use std::io::Read;
use std::path::Path;

use famp_keyring::revocation::SignedRevocation;
use famp_keyring::{Keyring, KeyringError};

use crate::cli::error::CliError;
use crate::cli::home;
use crate::cli::peer::identity::gateway_peers_keyring_path;

/// CLI args for `famp peer import-revocation`.
#[derive(clap::Args, Debug)]
pub struct PeerImportRevocationArgs {
    /// Source file, or `-` (default) for stdin.
    #[arg(default_value = "-")]
    pub source: String,
}

/// Production entry point.
pub fn run(args: &PeerImportRevocationArgs) -> Result<(), CliError> {
    let home_path = home::resolve_famp_home()?;
    let now = now_canonical_utc();
    if args.source == "-" {
        let mut stdin = std::io::stdin().lock();
        run_at(&home_path, &mut stdin, &now)
    } else {
        let path = std::path::PathBuf::from(&args.source);
        let mut f = std::fs::File::open(&path).map_err(|e| CliError::Io { path, source: e })?;
        run_at(&home_path, &mut f, &now)
    }
}

/// Test-facing entry point: takes an explicit `&Path` + reader + `now`,
/// mirroring the `run`/`run_at` split convention (`peer::import::run_at`).
pub fn run_at(home: &Path, source: &mut dyn Read, now: &str) -> Result<(), CliError> {
    let mut blob = String::new();
    source.read_to_string(&mut blob).map_err(|e| CliError::Io {
        path: home.to_path_buf(),
        source: e,
    })?;

    let signed =
        SignedRevocation::from_blob(&blob).map_err(|e| CliError::PeerRevocationRejected {
            principal: signed_blob_principal_hint(&blob),
            reason: e.to_string(),
        })?;
    let principal_str = signed.statement.principal.clone();

    let keyring_path = gateway_peers_keyring_path(home);
    let mut keyring = Keyring::load_from_file(&keyring_path).map_err(|e| {
        CliError::Generic(format!(
            "failed to load peer keyring at {}: {e}",
            keyring_path.display()
        ))
    })?;

    keyring.apply_signed_revocation(&signed, now).map_err(|e| {
        let reason = e.to_string();
        match e {
            KeyringError::RevocationSignerNotAuthorized { principal }
            | KeyringError::RevocationSignatureInvalid { principal } => {
                CliError::PeerRevocationRejected {
                    principal: principal.to_string(),
                    reason,
                }
            }
            KeyringError::RevocationBlobMalformed { reason } => CliError::PeerRevocationRejected {
                principal: principal_str.clone(),
                reason,
            },
            other => CliError::Generic(other.to_string()),
        }
    })?;

    // Save ONLY on Ok — a rejected statement must leave the keyring file
    // untouched.
    keyring.save_to_file(&keyring_path).map_err(|e| {
        CliError::Generic(format!(
            "failed to save peer keyring at {}: {e}",
            keyring_path.display()
        ))
    })?;

    println!(
        "revoked {} for {principal_str}",
        signed.statement.revoked_key_id
    );

    // Gateway-restart notice (D15-B/famp-lead-730, 2026-07-31): see
    // `peer::rotate::run_at`'s identical notice for the full rationale.
    eprintln!(
        "NOTE: famp-gateway loads its keyring once at startup and will keep honoring the \
         revoked key until it restarts. Run `famp daemon restart` (or manually restart the \
         gateway if it is not daemon-managed) to pick up this change."
    );

    Ok(())
}

/// Best-effort principal label for a blob that failed to parse at all
/// (`from_blob` itself errored, before any typed `RevocationStatement`
/// exists) — diagnostic only, never a trust decision.
fn signed_blob_principal_hint(_blob: &str) -> String {
    "<unparseable blob>".to_string()
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

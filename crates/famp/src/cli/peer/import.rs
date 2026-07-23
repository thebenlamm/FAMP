//! `famp peer import` — parse a `famp peer export` blob and TOFU-pin the
//! peer's key into the gateway peer keyring (TRUST-01, D-05, D-06).

use std::io::Read;
use std::path::Path;
use std::str::FromStr;

use famp_core::Principal;
use famp_crypto::{key_id, TrustedVerifyingKey};
use famp_keyring::{Keyring, KeyringError};

use crate::cli::error::CliError;
use crate::cli::home;
use crate::cli::peer::identity::gateway_peers_keyring_path;

/// CLI args for `famp peer import`.
#[derive(clap::Args, Debug)]
pub struct PeerImportArgs {
    /// Source file, or `-` (default) for stdin.
    #[arg(default_value = "-")]
    pub source: String,
}

/// Production entry point.
pub fn run(args: &PeerImportArgs) -> Result<(), CliError> {
    let home_path = home::resolve_famp_home()?;
    if args.source == "-" {
        let mut stdin = std::io::stdin().lock();
        run_at(&home_path, &mut stdin)
    } else {
        let path = std::path::PathBuf::from(&args.source);
        let mut f = std::fs::File::open(&path).map_err(|e| CliError::Io { path, source: e })?;
        run_at(&home_path, &mut f)
    }
}

/// Test-facing entry point: takes an explicit `&Path` + reader.
///
/// Tests can feed an export blob directly (no stdin/file plumbing),
/// mirroring the `run`/`run_at` split convention (`cli/info.rs`,
/// `cli/peer/export.rs`). No key material ever crosses FAMP itself —
/// `source` is always a local file or stdin, read once, never forwarded
/// anywhere.
pub fn run_at(home: &Path, source: &mut dyn Read) -> Result<(), CliError> {
    let mut blob = String::new();
    source.read_to_string(&mut blob).map_err(|e| CliError::Io {
        path: home.to_path_buf(),
        source: e,
    })?;

    let (principal, vk) = parse_export_line(&blob)?;

    let keyring_path = gateway_peers_keyring_path(home);
    let mut keyring = if keyring_path.exists() {
        Keyring::load_from_file(&keyring_path).map_err(|e| {
            CliError::Generic(format!(
                "failed to load peer keyring at {}: {e}",
                keyring_path.display()
            ))
        })?
    } else {
        Keyring::new()
    };

    keyring.pin_tofu(principal, vk).map_err(|e| match e {
        KeyringError::KeyConflict { principal } => CliError::PeerKeyConflict {
            principal: principal.to_string(),
        },
        other => CliError::Generic(other.to_string()),
    })?;

    if let Some(parent) = keyring_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CliError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }
    keyring.save_to_file(&keyring_path).map_err(|e| {
        CliError::Generic(format!(
            "failed to save peer keyring at {}: {e}",
            keyring_path.display()
        ))
    })?;

    Ok(())
}

/// Parse a `famp peer export` blob: `<principal> <pubkey-b64url>
/// [<key_id>]`.
///
/// The 3rd field (human fingerprint) is optional and advisory-only. This
/// is a CLI-layer parser — NOT `famp_keyring::file_format::parse_line`
/// (strict 2-field, rejects a 3rd token; RESEARCH anti-pattern) — mirrors
/// its whitespace-split + per-field-error idiom without reusing it
/// verbatim. A mismatching (or absent) fingerprint is a paste-corruption
/// WARNING, not a hard failure — the blob still parses.
pub fn parse_export_line(line: &str) -> Result<(Principal, TrustedVerifyingKey), CliError> {
    let mut parts = line.split_whitespace();
    let principal_str = parts.next().ok_or_else(|| CliError::PeerBlobMalformed {
        reason: "missing principal".to_string(),
    })?;
    let pubkey_str = parts.next().ok_or_else(|| CliError::PeerBlobMalformed {
        reason: "missing pubkey".to_string(),
    })?;
    let fingerprint_str = parts.next();

    let principal =
        Principal::from_str(principal_str).map_err(|e| CliError::PeerBlobMalformed {
            reason: format!("invalid principal '{principal_str}': {e}"),
        })?;
    let vk =
        TrustedVerifyingKey::from_b64url(pubkey_str).map_err(|e| CliError::PeerBlobMalformed {
            reason: format!("invalid pubkey encoding: {e}"),
        })?;

    if let Some(fp) = fingerprint_str {
        let derived = key_id(&vk);
        if derived != fp {
            eprintln!(
                "warning: fingerprint mismatch on import for {principal} — derived \
                 {derived}, blob carried {fp}. Verify the paste survived intact \
                 before trusting this key."
            );
        }
    }

    Ok((principal, vk))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::parse_export_line;
    use crate::cli::peer::export::format_export_line;
    use famp_core::Principal;
    use famp_crypto::FampSigningKey;
    use std::str::FromStr;

    #[test]
    fn parse_export_line_round_trips_principal_and_pubkey() {
        let principal = Principal::from_str("agent:example.com/gateway").unwrap();
        let sk = FampSigningKey::generate();
        let vk = sk.verifying_key();
        let blob = format_export_line(&principal, &vk);

        let (parsed_principal, parsed_vk) = parse_export_line(&blob).unwrap();

        assert_eq!(parsed_principal, principal);
        assert_eq!(parsed_vk.to_b64url(), vk.to_b64url());
    }

    #[test]
    fn parse_export_line_tolerates_missing_fingerprint() {
        let principal = Principal::from_str("agent:example.com/gateway").unwrap();
        let sk = FampSigningKey::generate();
        let vk = sk.verifying_key();
        // Only 2 fields — no fingerprint.
        let blob = format!("{principal} {}\n", vk.to_b64url());

        let (parsed_principal, parsed_vk) = parse_export_line(&blob).unwrap();

        assert_eq!(parsed_principal, principal);
        assert_eq!(parsed_vk.to_b64url(), vk.to_b64url());
    }

    #[test]
    fn parse_export_line_warns_but_still_parses_on_corrupted_fingerprint() {
        let principal = Principal::from_str("agent:example.com/gateway").unwrap();
        let sk = FampSigningKey::generate();
        let vk = sk.verifying_key();
        // Corrupted 3rd field — parse should still succeed (warn, not fail).
        let blob = format!("{principal} {} deadbeefdeadbeef\n", vk.to_b64url());

        let (parsed_principal, parsed_vk) = parse_export_line(&blob).unwrap();

        assert_eq!(parsed_principal, principal);
        assert_eq!(parsed_vk.to_b64url(), vk.to_b64url());
    }

    #[test]
    fn parse_export_line_rejects_missing_principal() {
        let err = parse_export_line("").unwrap_err();
        assert!(matches!(
            err,
            crate::cli::error::CliError::PeerBlobMalformed { .. }
        ));
    }
}

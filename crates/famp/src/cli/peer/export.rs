//! `famp peer export` — print this gateway's principal + pubkey + a human
//! fingerprint as a single, copy/paste-safe line (TRUST-01, D-05).

use std::path::Path;

use famp_core::Principal;
use famp_crypto::{key_id, TrustedVerifyingKey};

use crate::cli::error::CliError;
use crate::cli::home;
use crate::cli::peer::identity::{gateway_identity_path, load_or_generate};

/// CLI args for `famp peer export`.
#[derive(clap::Args, Debug)]
pub struct PeerExportArgs {
    /// Principal name to export this key under, e.g.
    /// `agent:my-mbp.local/gateway`.
    #[arg(long = "as")]
    pub as_principal: String,
}

/// Production entry point.
pub fn run(args: &PeerExportArgs) -> Result<(), CliError> {
    let home_path = home::resolve_famp_home()?;
    let mut stdout = std::io::stdout().lock();
    run_at(&home_path, args, &mut stdout)
}

/// Test-facing entry point: takes an explicit `&Path` + writer so tests
/// avoid the `std::env::set_var` parallel-test race (`home.rs` convention).
pub fn run_at(
    home: &Path,
    args: &PeerExportArgs,
    out: &mut dyn std::io::Write,
) -> Result<(), CliError> {
    let principal: Principal =
        args.as_principal
            .parse()
            .map_err(|e| CliError::PeerBlobMalformed {
                reason: format!("invalid --as principal '{}': {e}", args.as_principal),
            })?;

    let key_path = gateway_identity_path(home);
    let sk = load_or_generate(&key_path)?;
    let vk = sk.verifying_key();

    let line = format_export_line(&principal, &vk);
    out.write_all(line.as_bytes()).map_err(|e| CliError::Io {
        path: home.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

/// Format the single Signal-paste-safe export line:
/// `<principal> <pubkey-b64url> <key_id>\n` (D-05). Three
/// whitespace-separated fields, trailing newline, no multi-line PEM.
#[must_use]
pub fn format_export_line(principal: &Principal, vk: &TrustedVerifyingKey) -> String {
    format!("{principal} {} {}\n", vk.to_b64url(), key_id(vk))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::{format_export_line, Principal};
    use famp_crypto::FampSigningKey;
    use std::str::FromStr;

    #[test]
    fn format_export_line_has_three_whitespace_fields_and_trailing_newline() {
        let principal = Principal::from_str("agent:example.com/gateway").unwrap();
        let sk = FampSigningKey::generate();
        let vk = sk.verifying_key();

        let line = format_export_line(&principal, &vk);

        assert!(line.ends_with('\n'), "line must end with newline: {line:?}");
        let fields: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(
            fields.len(),
            3,
            "expected exactly 3 fields, got: {fields:?}"
        );
        assert_eq!(fields[0], principal.to_string());
        assert_eq!(fields[1], vk.to_b64url());
    }
}

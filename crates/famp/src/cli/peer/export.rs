//! `famp peer export` — print this gateway's principal + pubkey + a human
//! fingerprint as a single, copy/paste-safe line (TRUST-01, D-05).

use std::path::Path;

use famp_core::{ParsePrincipalError, Principal};
use famp_crypto::{key_id, TrustedVerifyingKey};

use crate::cli::error::CliError;
use crate::cli::home;
use crate::cli::own_domain;
use crate::cli::peer::identity::{gateway_identity_path, load_or_generate};

/// CLI args for `famp peer export`.
#[derive(clap::Args, Debug)]
pub struct PeerExportArgs {
    /// The sender agent name to export this key under — a bare name
    /// (e.g. `alice`, combined with the resolved own-domain) or a full
    /// principal (e.g. `agent:my-mbp.local/alice`). This MUST be the
    /// sender *agent* principal, not a `/gateway`-suffixed label —
    /// ingress verifies inbound envelopes on the agent `from` (RESEARCH
    /// D-05 finding #2).
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
    let principal = resolve_export_principal(home, &args.as_principal)?;

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

/// D-05 / ADDR-03: resolve the `Principal` this export line is emitted
/// under, coupling the pinned-label authority to the SAME own-domain
/// source `famp send` will stamp into a remote envelope's `from` (plan
/// 03). `famp peer export` has no `--domain` flag of its own, so
/// `own_domain::resolve_own_domain(None, home)` is called — `--domain`
/// tiering only ever comes from `famp send`.
///
/// Three shapes (RESEARCH D-05 Resolution + the `--reviews` unset-domain
/// finding):
/// - own-domain configured + bare `--as name` -> `agent:{own_domain}/{name}`.
/// - own-domain configured + full `--as agent:auth/name` -> `auth` MUST
///   equal `own_domain`; a mismatch is a loud typed reject, never a
///   silent divergent pin label.
/// - own-domain UNSET + full `--as agent:auth/name` -> accepted VERBATIM
///   with a one-line stderr warning (keeps `peer_roundtrip.rs` and the
///   e2e own-domain-unset export path green).
/// - own-domain UNSET + bare `--as name` -> `CliError::OwnDomainNotSet`
///   (nothing to synthesize an authority from).
///
/// Never constructs a `/gateway`-suffixed label (RESEARCH finding #2) —
/// the caller's `--as` name/principal is used verbatim, only the
/// authority is ever substituted or checked.
fn resolve_export_principal(home: &Path, as_principal: &str) -> Result<Principal, CliError> {
    match as_principal.parse::<Principal>() {
        Ok(principal) => match own_domain::resolve_own_domain(None, home) {
            Ok(configured) => {
                if principal.authority() == configured {
                    Ok(principal)
                } else {
                    Err(CliError::PeerBlobMalformed {
                        reason: format!(
                            "--as principal authority '{}' does not match the configured \
                             own-domain '{configured}' — pin-label authority and the future \
                             envelope `from` authority must be the same value",
                            principal.authority()
                        ),
                    })
                }
            }
            Err(CliError::OwnDomainNotSet) => {
                eprintln!(
                    "warning: own-domain unset; exporting label authority '{}' verbatim",
                    principal.authority()
                );
                Ok(principal)
            }
            Err(e) => Err(e),
        },
        Err(ParsePrincipalError::MissingScheme) => {
            // Bare name (no `agent:` prefix): synthesize the authority
            // from own-domain. `OwnDomainNotSet`'s actionable hint (naming
            // all three sources) propagates verbatim on failure.
            let own_domain = own_domain::resolve_own_domain(None, home)?;
            let full = format!("agent:{own_domain}/{as_principal}");
            full.parse::<Principal>()
                .map_err(|e| CliError::PeerBlobMalformed {
                    reason: format!("invalid --as name '{as_principal}': {e}"),
                })
        }
        Err(e) => Err(CliError::PeerBlobMalformed {
            reason: format!("invalid --as principal '{as_principal}': {e}"),
        }),
    }
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
    use super::{format_export_line, resolve_export_principal, Principal};
    use crate::cli::error::CliError;
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

    /// D-05 / ADDR-03: the three own-domain coupling shapes plus the
    /// unset-verbatim regression, all inside one `#[test] fn` and routed
    /// through `temp_env` (process-global mutex) so `FAMP_OWN_DOMAIN`
    /// mutations here cannot race `own_domain.rs`'s own env-touching test.
    #[test]
    fn resolve_export_principal_couples_to_own_domain() {
        // Shape 1: own-domain configured + bare name -> synthesized
        // authority.
        temp_env::with_var("FAMP_OWN_DOMAIN", Some("hosta.test"), || {
            let tmp = tempfile::tempdir().unwrap();
            let principal = resolve_export_principal(tmp.path(), "alice")
                .expect("bare name + configured own-domain must resolve");
            assert_eq!(principal.authority(), "hosta.test");
            assert_eq!(principal.name(), "alice");

            // Shape 2a: own-domain configured + matching full principal ->
            // accepted.
            let principal = resolve_export_principal(tmp.path(), "agent:hosta.test/alice")
                .expect("matching full principal must resolve");
            assert_eq!(principal.to_string(), "agent:hosta.test/alice");

            // Shape 2b: own-domain configured + MISMATCHED full principal
            // -> typed reject, not a silent divergent pin label.
            match resolve_export_principal(tmp.path(), "agent:other.test/alice") {
                Err(CliError::PeerBlobMalformed { reason }) => {
                    assert!(reason.contains("other.test"), "{reason}");
                    assert!(reason.contains("hosta.test"), "{reason}");
                }
                other => panic!("expected PeerBlobMalformed mismatch, got {other:?}"),
            }
        });

        // Shape 3: own-domain UNSET + full principal -> accepted verbatim
        // (the peer_roundtrip.rs / e2e regression path).
        temp_env::with_var_unset("FAMP_OWN_DOMAIN", || {
            let tmp = tempfile::tempdir().unwrap();
            let principal = resolve_export_principal(tmp.path(), "agent:machine-a.local/gateway")
                .expect("unset own-domain + full principal must accept verbatim");
            assert_eq!(principal.to_string(), "agent:machine-a.local/gateway");

            // Shape 4: own-domain UNSET + bare name -> actionable
            // OwnDomainNotSet, nothing to synthesize from.
            match resolve_export_principal(tmp.path(), "alice") {
                Err(CliError::OwnDomainNotSet) => {}
                other => panic!("expected OwnDomainNotSet, got {other:?}"),
            }
        });
    }
}

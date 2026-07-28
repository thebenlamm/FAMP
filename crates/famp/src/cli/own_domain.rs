//! Own-domain resolution (ADDR-03 / D-05). This host's federation
//! authority — the value that `famp send` stamps into a remote envelope's
//! `from` and that `famp peer export` derives its pinned label from.
//!
//! Ingress verifies on the envelope `from` (`verify.rs` -> `peek_sender` ->
//! `keyring.get(from)`), so the domain in `from` MUST byte-equal the label
//! the peer pinned via `famp peer export`. Three independent OS processes
//! (`famp send`, `famp peer export`, `famp-gateway`) can only agree without
//! drift by reading ONE shared value — this module is that single source.
//!
//! Per CD-05 / RESEARCH Pitfall 1 (mirroring `home.rs`), this is the ONLY
//! place that reads `FAMP_OWN_DOMAIN` from process env. Every other call
//! site takes the resolved `String` explicitly to avoid the
//! `std::env::set_var` parallel-test race.
//!
//! Precedence: `--domain` CLI flag (highest) > `FAMP_OWN_DOMAIN` env >
//! `$FAMP_HOME/own-domain` file (single trimmed line) > `Err(OwnDomainNotSet)`.

use std::path::Path;

use famp_core::Principal;

use crate::cli::error::CliError;

/// Resolve this host's own-domain authority.
///
/// `cli_domain` is the caller-parsed `--domain` flag value (if the calling
/// subcommand has one — `famp peer export` does not, and passes `None`).
/// `home` is the already-resolved `$FAMP_HOME` (per the `home.rs`
/// convention: callees take the resolved path, they do not re-resolve it).
///
/// The resolved value is validated as a legal `Principal` authority via a
/// probe parse of `agent:{value}/x` — `validate_authority` itself is
/// private (`famp-core::identity`), so a probe `Principal::from_str` is the
/// compliant route to the same validation.
pub fn resolve_own_domain(cli_domain: Option<&str>, home: &Path) -> Result<String, CliError> {
    let raw = if let Some(v) = cli_domain {
        v.to_string()
    } else if let Some(v) = std::env::var_os("FAMP_OWN_DOMAIN") {
        v.to_string_lossy().into_owned()
    } else if let Some(v) = read_own_domain_file(home)? {
        v
    } else {
        return Err(CliError::OwnDomainNotSet);
    };

    validate_own_domain(&raw)?;
    Ok(raw)
}

/// Validate a candidate own-domain string as a legal `Principal` authority
/// via a probe parse. Returns the typed [`CliError::OwnDomainInvalid`] on
/// failure rather than panicking.
fn validate_own_domain(value: &str) -> Result<(), CliError> {
    let probe = format!("agent:{value}/x");
    probe
        .parse::<Principal>()
        .map(|_| ())
        .map_err(|e| CliError::OwnDomainInvalid {
            value: value.to_string(),
            reason: e.to_string(),
        })
}

/// Read `$FAMP_HOME/own-domain`, returning the trimmed first line if the
/// file exists and is non-empty after trimming. A missing file or an
/// empty/whitespace-only file is treated as "not set" (returns `Ok(None)`),
/// not an error — the caller falls through to `OwnDomainNotSet`.
fn read_own_domain_file(home: &Path) -> Result<Option<String>, CliError> {
    let path = home.join("own-domain");
    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            let first_line = contents.lines().next().unwrap_or("").trim();
            if first_line.is_empty() {
                Ok(None)
            } else {
                Ok(Some(first_line.to_string()))
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(CliError::Io { path, source: e }),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// All env/file precedence cases run in one `#[test] fn`, and every
    /// `FAMP_OWN_DOMAIN` mutation goes through `temp_env` (which holds a
    /// process-global mutex across the whole test binary — see
    /// `identity.rs`'s WR-06 note) so this test cannot race against
    /// `peer::export`'s own `FAMP_OWN_DOMAIN`-touching tests even under
    /// cargo's default multi-threaded test runner.
    #[test]
    fn resolves_precedence_trims_and_rejects() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();

        // Case A: cli_domain wins over everything, even when env/file set.
        std::fs::write(home.join("own-domain"), "file.example.com\n").unwrap();
        temp_env::with_var("FAMP_OWN_DOMAIN", Some("env.example.com"), || {
            match resolve_own_domain(Some("cli.example.com"), home) {
                Ok(v) => assert_eq!(v, "cli.example.com"),
                other => panic!("Case A: expected Ok(cli.example.com), got {other:?}"),
            }

            // Case B: no cli_domain -> env wins over file.
            match resolve_own_domain(None, home) {
                Ok(v) => assert_eq!(v, "env.example.com"),
                other => panic!("Case B: expected Ok(env.example.com), got {other:?}"),
            }
        });

        // Case C: no cli_domain, no env -> file wins, trimmed.
        temp_env::with_var_unset("FAMP_OWN_DOMAIN", || match resolve_own_domain(None, home) {
            Ok(v) => assert_eq!(v, "file.example.com"),
            other => panic!("Case C: expected Ok(file.example.com), got {other:?}"),
        });

        // Case D: whitespace/trailing-newline trimmed; only the first line
        // is read even if the file has more content.
        std::fs::write(
            home.join("own-domain"),
            "  trimmed.example.com  \nignored\n",
        )
        .unwrap();
        temp_env::with_var_unset("FAMP_OWN_DOMAIN", || match resolve_own_domain(None, home) {
            Ok(v) => assert_eq!(v, "trimmed.example.com"),
            other => panic!("Case D: expected Ok(trimmed.example.com), got {other:?}"),
        });

        // Case E: empty file treated as "not set" -> NotSet error.
        std::fs::write(home.join("own-domain"), "\n\n").unwrap();
        temp_env::with_var_unset("FAMP_OWN_DOMAIN", || match resolve_own_domain(None, home) {
            Err(CliError::OwnDomainNotSet) => {}
            other => panic!("Case E: expected OwnDomainNotSet, got {other:?}"),
        });

        // Case F: no file at all -> NotSet error, message names all three
        // sources.
        std::fs::remove_file(home.join("own-domain")).unwrap();
        temp_env::with_var_unset("FAMP_OWN_DOMAIN", || match resolve_own_domain(None, home) {
            Err(CliError::OwnDomainNotSet) => {
                let msg = CliError::OwnDomainNotSet.to_string();
                assert!(msg.contains("--domain"), "msg must name --domain: {msg}");
                assert!(
                    msg.contains("FAMP_OWN_DOMAIN"),
                    "msg must name FAMP_OWN_DOMAIN: {msg}"
                );
                assert!(
                    msg.contains("own-domain"),
                    "msg must name own-domain file: {msg}"
                );
            }
            other => panic!("Case F: expected OwnDomainNotSet, got {other:?}"),
        });

        // Case G: an invalid authority (fails the probe Principal parse)
        // returns a typed OwnDomainInvalid error, not a panic.
        match resolve_own_domain(Some("not a valid authority!"), home) {
            Err(CliError::OwnDomainInvalid { value, .. }) => {
                assert_eq!(value, "not a valid authority!");
            }
            other => panic!("Case G: expected OwnDomainInvalid, got {other:?}"),
        }
    }
}

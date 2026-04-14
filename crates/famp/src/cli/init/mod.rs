//! `famp init` subcommand.

pub mod atomic;
pub mod tls;

use std::io::Write;
use std::path::{Path, PathBuf};

use rand::RngCore;

use crate::cli::config::Config;
use crate::cli::error::CliError;
use crate::cli::paths::IdentityLayout;
use crate::cli::{home, perms, InitArgs};

/// Result of a successful `famp init` run.
#[derive(Debug, Clone)]
pub struct InitOutcome {
    /// base64url-unpadded pubkey (output of `TrustedVerifyingKey::to_b64url`).
    pub pubkey_b64url: String,
    /// Absolute path to the initialized FAMP home.
    pub home: PathBuf,
}

/// Top-level entry point. Resolves `FAMP_HOME` and forwards to [`run_at`].
///
/// Writes the D-15 one-line stdout (pubkey) + one-line stderr
/// (`initialized FAMP home at <path>`) via the locked stdio handles.
///
/// Signature is fixed by the Plan 02 `<interfaces>` block (owned `InitArgs`).
#[allow(clippy::needless_pass_by_value)]
pub fn run(args: InitArgs) -> Result<InitOutcome, CliError> {
    let home_path = home::resolve_famp_home()?;
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    run_at(&home_path, args.force, &mut stdout, &mut stderr)
}

/// Test-facing entry point (CD-05 "Rust API route").
///
/// Takes an explicit `FAMP_HOME` path and writable handles so integration
/// tests can capture output without mutating process env or swapping
/// `std::io::stdout`.
pub fn run_at(
    home: &Path,
    force: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<InitOutcome, CliError> {
    if !home.is_absolute() {
        return Err(CliError::HomeNotAbsolute {
            path: home.to_path_buf(),
        });
    }

    let layout = IdentityLayout::at(home.to_path_buf());
    let existing = probe_existing(&layout);

    match (home.exists(), existing.is_empty(), force) {
        (true, false, false) => Err(CliError::AlreadyInitialized {
            existing_files: existing,
        }),
        (true, false, true) => {
            // --force: atomic replace of the entire home directory.
            let parent = home.parent().ok_or_else(|| CliError::HomeHasNoParent {
                path: home.to_path_buf(),
            })?;
            if !parent.exists() {
                return Err(CliError::HomeHasNoParent {
                    path: home.to_path_buf(),
                });
            }
            let outcome_cell = std::cell::RefCell::new(None::<InitOutcome>);
            atomic::atomic_replace(home, |staging| {
                // `tempfile::TempDir::new_in` creates the staging dir at 0o700
                // on Unix by default; we populate all six files inside it.
                let staged_layout = IdentityLayout::at(staging.to_path_buf());
                let o = materialize_identity(&staged_layout)?;
                *outcome_cell.borrow_mut() = Some(o);
                Ok(())
            })?;
            // `atomic_replace` returned Ok, which means the writer closure
            // ran to completion and set the cell. The `ok_or_else` turns the
            // theoretical None into an io error rather than a panic, honoring
            // the workspace `clippy::expect_used` lint.
            let mut outcome =
                outcome_cell
                    .into_inner()
                    .ok_or_else(|| CliError::Io {
                        path: home.to_path_buf(),
                        source: std::io::Error::other(
                            "internal: materialize_identity did not set outcome",
                        ),
                    })?;
            outcome.home = home.to_path_buf();
            emit_output(out, err, &outcome)?;
            Ok(outcome)
        }
        (true, true, _) | (false, _, _) => {
            // Empty dir or missing dir: create in place.
            ensure_home_dir(home)?;
            let mut outcome = materialize_identity(&layout)?;
            outcome.home = home.to_path_buf();
            emit_output(out, err, &outcome)?;
            Ok(outcome)
        }
    }
}

fn probe_existing(layout: &IdentityLayout) -> Vec<PathBuf> {
    layout
        .entries()
        .iter()
        .filter(|(_, p)| p.exists())
        .map(|(_, p)| p.to_path_buf())
        .collect()
}

fn ensure_home_dir(home: &Path) -> Result<(), CliError> {
    if home.exists() {
        return Ok(());
    }
    let parent = home.parent().ok_or_else(|| CliError::HomeHasNoParent {
        path: home.to_path_buf(),
    })?;
    if !parent.exists() {
        return Err(CliError::HomeHasNoParent {
            path: home.to_path_buf(),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(home)
            .map_err(|e| CliError::HomeCreateFailed {
                path: home.to_path_buf(),
                source: e,
            })?;
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir(home).map_err(|e| CliError::HomeCreateFailed {
            path: home.to_path_buf(),
            source: e,
        })?;
    }
    Ok(())
}

/// Writes all six identity files into `layout.home` (which must exist).
/// Returns the `InitOutcome` carrying the base64url pubkey.
///
/// D-17 mechanism #2: the 32-byte Ed25519 seed never leaves this function
/// except as raw bytes written to `key.ed25519` at mode 0600. No `CliError`
/// variant embeds `seed`, the `FampSigningKey`, or `vk.as_bytes()`.
fn materialize_identity(layout: &IdentityLayout) -> Result<InitOutcome, CliError> {
    // 1. Keygen via CSPRNG seed + famp_crypto::FampSigningKey::from_bytes.
    //    `famp-crypto` intentionally does NOT depend on `rand` and does NOT
    //    expose a `generate()` constructor — the seed is produced by the
    //    caller (PATTERNS §cli/init/mod.rs Option (b)). `rand` is a direct
    //    dep of the `famp` crate.
    let mut seed = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let sk = famp_crypto::FampSigningKey::from_bytes(seed);
    let vk = sk.verifying_key();
    let pubkey_b64url = vk.to_b64url();
    let pub_bytes: [u8; 32] = *vk.as_bytes();

    // 2. Ed25519 key files (raw 32 bytes). Secret at 0600, public at 0644.
    perms::write_secret(&layout.key_ed25519, &seed).map_err(|e| CliError::Io {
        path: layout.key_ed25519.clone(),
        source: e,
    })?;
    perms::write_public(&layout.pub_ed25519, &pub_bytes).map_err(|e| CliError::Io {
        path: layout.pub_ed25519.clone(),
        source: e,
    })?;

    // 3. TLS cert + key.
    let (cert_pem, key_pem) = tls::generate_tls().map_err(CliError::CertgenFailed)?;
    perms::write_public(&layout.tls_cert_pem, cert_pem.as_bytes()).map_err(|e| {
        CliError::Io {
            path: layout.tls_cert_pem.clone(),
            source: e,
        }
    })?;
    perms::write_secret(&layout.tls_key_pem, key_pem.as_bytes()).map_err(|e| CliError::Io {
        path: layout.tls_key_pem.clone(),
        source: e,
    })?;

    // 4. config.toml (single field, D-12).
    let cfg = Config::default();
    let cfg_str = toml::to_string(&cfg).map_err(CliError::TomlSerialize)?;
    perms::write_public(&layout.config_toml, cfg_str.as_bytes()).map_err(|e| CliError::Io {
        path: layout.config_toml.clone(),
        source: e,
    })?;

    // 5. peers.toml (zero bytes, D-14).
    perms::write_public(&layout.peers_toml, b"").map_err(|e| CliError::Io {
        path: layout.peers_toml.clone(),
        source: e,
    })?;

    // Seed is `[u8; 32]` (Copy), so `drop(seed)` is a no-op. D-18 is scope
    // exit, not zeroization — the value goes out of scope at function return.
    let _ = seed;

    Ok(InitOutcome {
        pubkey_b64url,
        home: layout.home.clone(),
    })
}

/// Phase 1 slice of IDENT-05: verify all six identity files exist.
///
/// Phase 2+ subcommands call this before attempting any work. On the first
/// missing file, returns [`CliError::IdentityIncomplete`] carrying that path.
/// Permission-checking ("wrong perms") is deferred to the phase that reads
/// the key material — Phase 1 only checks existence.
pub fn load_identity(home: &Path) -> Result<IdentityLayout, CliError> {
    if !home.is_absolute() {
        return Err(CliError::HomeNotAbsolute {
            path: home.to_path_buf(),
        });
    }
    let layout = IdentityLayout::at(home.to_path_buf());
    for (_label, path) in layout.entries() {
        if !path.exists() {
            return Err(CliError::IdentityIncomplete {
                missing: path.to_path_buf(),
            });
        }
    }
    Ok(layout)
}

fn emit_output(
    out: &mut dyn Write,
    err: &mut dyn Write,
    outcome: &InitOutcome,
) -> Result<(), CliError> {
    // D-15: stdout is exactly `{pubkey}\n`, stderr is exactly
    // `initialized FAMP home at {absolute path}\n`.
    writeln!(out, "{}", outcome.pubkey_b64url).map_err(|e| CliError::Io {
        path: outcome.home.clone(),
        source: e,
    })?;
    writeln!(err, "initialized FAMP home at {}", outcome.home.display()).map_err(|e| {
        CliError::Io {
            path: outcome.home.clone(),
            source: e,
        }
    })?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod load_identity_tests {
    use super::{load_identity, run_at, CliError};

    #[test]
    fn load_identity_happy_after_init() {
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path().join("famphome");
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        run_at(&home, false, &mut out, &mut err).expect("init");
        let layout = load_identity(&home).expect("load_identity");
        assert_eq!(layout.home, home);
    }

    #[test]
    fn load_identity_reports_first_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path().join("famphome");
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        run_at(&home, false, &mut out, &mut err).expect("init");

        // Remove key.ed25519 (first entry) to simulate partial state.
        std::fs::remove_file(home.join("key.ed25519")).unwrap();

        match load_identity(&home) {
            Err(CliError::IdentityIncomplete { missing }) => {
                assert!(missing.ends_with("key.ed25519"));
            }
            other => panic!("expected IdentityIncomplete, got {other:?}"),
        }
    }

    #[test]
    fn load_identity_rejects_relative_home() {
        match load_identity(std::path::Path::new("relative/path")) {
            Err(CliError::HomeNotAbsolute { .. }) => {}
            other => panic!("expected HomeNotAbsolute, got {other:?}"),
        }
    }
}

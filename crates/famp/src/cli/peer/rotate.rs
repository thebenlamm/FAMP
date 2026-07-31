//! `famp peer rotate` — accept a NEW key for an already-pinned peer,
//! refusing silently otherwise (KEYR-02/KEYR-03).
//!
//! Distinct from `famp peer import` (first-sight pinning only, fails
//! closed on ANY key change for a known peer): `rotate` is the ONLY path
//! that accepts a changed key for a known peer, and only with explicit
//! `--confirm-rotation`. The confirmation gate is enforced by
//! `famp_keyring::Keyring::rotate_to` itself (T-15-03) — this CLI layer
//! turns that library refusal into an operator-facing `KEY CHANGED` block
//! on stderr plus exit code 2, and writes nothing to the keyring file on
//! that path.

use std::io::Read;
use std::path::Path;

use famp_keyring::{Keyring, KeyringError, RotationOutcome};

use crate::cli::error::CliError;
use crate::cli::home;
use crate::cli::peer::identity::gateway_peers_keyring_path;
use crate::cli::peer::import::parse_export_line;

/// CLI args for `famp peer rotate`.
#[derive(clap::Args, Debug)]
pub struct PeerRotateArgs {
    /// Source file, or `-` (default) for stdin.
    #[arg(default_value = "-")]
    pub source: String,
    /// Explicitly accept a changed key for an already-pinned peer.
    /// Without this flag, a real key change is refused (exit 2).
    #[arg(long)]
    pub confirm_rotation: bool,
}

/// Production entry point.
pub fn run(args: &PeerRotateArgs) -> Result<(), CliError> {
    let home_path = home::resolve_famp_home()?;
    let now = now_canonical_utc();
    if args.source == "-" {
        let mut stdin = std::io::stdin().lock();
        run_at(&home_path, &mut stdin, args.confirm_rotation, &now)
    } else {
        let path = std::path::PathBuf::from(&args.source);
        let mut f = std::fs::File::open(&path).map_err(|e| CliError::Io { path, source: e })?;
        run_at(&home_path, &mut f, args.confirm_rotation, &now)
    }
}

/// Test-facing entry point: takes an explicit `&Path` + reader + `now`,
/// mirroring the `run`/`run_at` split convention (`peer::import::run_at`).
pub fn run_at(
    home: &Path,
    source: &mut dyn Read,
    confirm: bool,
    now: &str,
) -> Result<(), CliError> {
    let mut blob = String::new();
    source.read_to_string(&mut blob).map_err(|e| CliError::Io {
        path: home.to_path_buf(),
        source: e,
    })?;

    let (principal, vk) = parse_export_line(&blob)?;

    let keyring_path = gateway_peers_keyring_path(home);
    if !keyring_path.exists() {
        return Err(CliError::PeerNotPinned {
            principal: principal.to_string(),
        });
    }
    let mut keyring = Keyring::load_from_file(&keyring_path).map_err(|e| {
        CliError::Generic(format!(
            "failed to load peer keyring at {}: {e}",
            keyring_path.display()
        ))
    })?;

    // Rotation presupposes an existing pin — first-sight pinning has one
    // entry point, `famp peer import`, and the two paths never merge.
    if keyring.entries(&principal).is_empty() {
        return Err(CliError::PeerNotPinned {
            principal: principal.to_string(),
        });
    }

    let outcome = keyring
        .rotate_to(principal, vk, now, None, confirm)
        .map_err(|e| match e {
            KeyringError::KeyChangeRequiresConfirmation {
                principal,
                previous_key_id,
                new_key_id,
            } => {
                eprintln!("KEY CHANGED for {principal}");
                eprintln!("  currently pinned key: {previous_key_id}");
                eprintln!("  incoming key:         {new_key_id}");
                eprintln!(
                    "An unexpected key change for a known peer is exactly what pinning \
                     exists to catch."
                );
                eprintln!("Remedy: re-run with --confirm-rotation");
                CliError::Exit(2)
            }
            KeyringError::KeyRevoked { principal, key_id } => CliError::PeerKeyRevoked {
                principal: principal.to_string(),
                key_id,
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

    // Gateway-restart notice (D15-B/famp-lead-730, 2026-07-31):
    // `famp-gateway` loads its keyring once at startup (`Arc<Keyring>`, no
    // hot reload). A confirmed rotation that succeeds locally does NOT
    // take effect at the gateway until it restarts — surfaced loudly at
    // the moment of success rather than left to a doc the operator reads
    // later. Print-only: never affects the return type or exit code.
    if matches!(outcome, RotationOutcome::Rotated { .. }) {
        eprintln!(
            "NOTE: famp-gateway loads its keyring once at startup and will keep honoring \
             the previous key until it restarts. Run `famp daemon restart` (or manually \
             restart the gateway if it is not daemon-managed) to pick up the rotated key."
        );
    }

    Ok(())
}

/// RFC 3339 (second-precision, `Z`-suffixed) timestamp built positionally
/// so the function is infallible — the same style as
/// `famp_gateway::clock::now_canonical_utc` (a sibling crate this one does
/// not depend on; kept local rather than adding a cross-crate dependency
/// for one timestamp helper).
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

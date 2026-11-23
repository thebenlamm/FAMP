//! W0 recipient allowlist, the writer side.
//!
//! The gateway reads this exact file on every delivery
//! (`famp-gateway/src/recipient_allowlist.rs` defines the strict format and the
//! fail-closed rules); a name that is not listed is never delivered to, even for
//! a verified peer.
//!
//! `{"version":1,"names":["alice","guardian-devgpu"]}`, mode 0600, in `~/.famp`.
use std::path::{Path, PathBuf};

use famp_core::Principal;
use serde::{Deserialize, Serialize};

use super::state::{self, error};
use crate::cli::error::CliError;

pub const FILE: &str = "recipients.json";

pub fn path(home: &Path) -> PathBuf {
    home.join(FILE)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: u32,
    names: Vec<String>,
}

/// Names currently listed; empty when there is no file yet.
pub fn load(home: &Path) -> Result<Vec<String>, CliError> {
    let p = path(home);
    if !p.exists() {
        return Ok(Vec::new());
    }
    // The same limits the gateway enforces, so `doctor` never says "ok" about a
    // file the gateway will treat as deny-all.
    let bytes = std::fs::read(&p).map_err(error)?;
    if bytes.len() > 64 * 1024 {
        return Err(error(format!("{} is larger than 64 KiB", p.display())));
    }
    let file: File = serde_json::from_slice(&bytes)
        .map_err(|e| error(format!("{} is not a valid allowlist: {e}", p.display())))?;
    if file.version != 1 {
        return Err(error(format!("{} has an unsupported version", p.display())));
    }
    if file.names.iter().any(|n| n.is_empty() || n.len() > 253) {
        return Err(error(format!(
            "{} lists an empty or oversized name",
            p.display()
        )));
    }
    Ok(file.names)
}

fn save(home: &Path, names: &[String]) -> Result<(), CliError> {
    let mut names = names.to_vec();
    names.sort();
    names.dedup();
    let body = serde_json::to_vec_pretty(&File { version: 1, names }).map_err(error)?;
    state::atomic(&path(home), &body)
}

/// A name must be a valid agent name, so a typo cannot silently allow nothing.
fn validate(name: &str) -> Result<(), CliError> {
    format!("agent:allowlist.invalid/{name}")
        .parse::<Principal>()
        .map(|_| ())
        .map_err(|e| error(format!("'{name}' is not a valid agent name: {e}")))
}

/// Create the list with this computer's own name if it does not exist yet, so a
/// fresh or upgraded box can receive messages addressed to it. Never overwrites.
pub fn ensure_seeded(home: &Path, own_name: &str) -> Result<(), CliError> {
    if path(home).exists() {
        return Ok(());
    }
    save(home, &[own_name.to_owned()])
}

pub fn allow(home: &Path, name: &str) -> Result<(), CliError> {
    validate(name)?;
    let mut names = load(home)?;
    names.push(name.to_owned());
    save(home, &names)
}

pub fn deny(home: &Path, name: &str) -> Result<(), CliError> {
    let mut names = load(home)?;
    let before = names.len();
    names.retain(|n| n != name);
    if names.len() == before {
        return Err(error(format!("'{name}' is not on the allowlist")));
    }
    save(home, &names)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    #[test]
    fn allow_deny_list_round_trip_and_seeding_never_overwrites() -> Result<(), CliError> {
        let dir = tempfile::tempdir().map_err(error)?;
        let home = dir.path();
        assert!(
            load(home)?.is_empty(),
            "no file yet means nothing is allowed"
        );
        ensure_seeded(home, "mac")?;
        allow(home, "guardian-mac")?;
        allow(home, "guardian-mac")?; // idempotent
        assert_eq!(load(home)?, ["guardian-mac", "mac"]);
        // Seeding again must not undo an operator's edits.
        deny(home, "mac")?;
        ensure_seeded(home, "mac")?;
        assert_eq!(load(home)?, ["guardian-mac"]);
        assert!(deny(home, "never-listed").is_err());
        assert!(allow(home, "bad name!").is_err());
        assert!(allow(home, "").is_err());
        // Mode 0600, as the gateway requires nobody else can write it.
        assert_eq!(
            std::fs::metadata(path(home))
                .map_err(error)?
                .permissions()
                .mode()
                & 0o077,
            0
        );
        Ok(())
    }
}

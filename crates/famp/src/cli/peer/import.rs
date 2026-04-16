//! `famp peer import` — import a peer card from JSON.
//!
//! Accepts peer card JSON from stdin or --card flag and registers the peer.
//! This is the counterpart to `famp info` / `famp setup` output.

use std::io::Read as _;
use std::path::Path;

use crate::cli::error::CliError;
use crate::cli::home;
use crate::cli::peer::add::run_add_at;
use crate::cli::setup::PeerCard;

/// Production entry point. Resolves `FAMP_HOME` and delegates.
pub fn run_import(card_json: Option<String>) -> Result<(), CliError> {
    let home = home::resolve_famp_home()?;
    run_import_at(&home, card_json)
}

/// Test-facing entry point.
pub fn run_import_at(home: &Path, card_json: Option<String>) -> Result<(), CliError> {
    // Read card from arg or stdin
    let json = if let Some(j) = card_json {
        j
    } else {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| CliError::Io {
                path: home.to_path_buf(),
                source: e,
            })?;
        buf
    };

    // Parse peer card
    let card: PeerCard = serde_json::from_str(&json).map_err(|e| CliError::PeerCardInvalid {
        reason: e.to_string(),
    })?;

    // Register via existing add logic
    run_add_at(
        home,
        card.alias,
        card.endpoint,
        card.pubkey,
        Some(card.principal),
    )?;

    eprintln!("Peer imported successfully");
    Ok(())
}

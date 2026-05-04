//! `famp info` — output the current agent's peer card.
//!
//! This command reads the current identity and config to produce a peer card
//! that can be shared with other agents for registration.

use std::path::Path;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::cli::config::Config;
use crate::cli::error::CliError;
use crate::cli::home;
use crate::cli::paths::IdentityLayout;

/// Peer card: shareable identity for peer registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerCard {
    /// Suggested alias (the agent's name).
    pub alias: String,
    /// HTTPS endpoint for the agent's inbox.
    pub endpoint: String,
    /// base64url-unpadded ed25519 public key.
    pub pubkey: String,
    /// FAMP principal (e.g., `agent:localhost/alice`).
    pub principal: String,
}

/// CLI args for `famp info`.
#[derive(clap::Args, Debug)]
pub struct InfoArgs {
    /// Output format: json (default) or text.
    #[arg(long, default_value = "json")]
    pub format: String,
}

/// Production entry point.
pub fn run(args: &InfoArgs) -> Result<PeerCard, CliError> {
    let home_path = home::resolve_famp_home()?;
    let mut stdout = std::io::stdout().lock();
    run_at(&home_path, args, &mut stdout)
}

/// Test-facing entry point.
pub fn run_at(
    home: &Path,
    args: &InfoArgs,
    out: &mut dyn std::io::Write,
) -> Result<PeerCard, CliError> {
    // Verify identity is complete
    let layout = load_identity(home)?;

    // Read pubkey
    let pub_bytes = std::fs::read(&layout.pub_ed25519).map_err(|e| CliError::Io {
        path: layout.pub_ed25519.clone(),
        source: e,
    })?;
    if pub_bytes.len() != 32 {
        return Err(CliError::IdentityIncomplete {
            missing: layout.pub_ed25519,
        });
    }
    let pubkey = URL_SAFE_NO_PAD.encode(&pub_bytes);

    // Read config
    let config_bytes = std::fs::read(&layout.config_toml).map_err(|e| CliError::Io {
        path: layout.config_toml.clone(),
        source: e,
    })?;
    let config_str = std::str::from_utf8(&config_bytes).map_err(|e| CliError::Io {
        path: layout.config_toml.clone(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
    })?;
    let config: Config = toml::from_str(config_str).map_err(|e| CliError::TomlParse {
        path: layout.config_toml.clone(),
        source: e,
    })?;

    // Build peer card
    let principal = config
        .principal
        .unwrap_or_else(|| "agent:localhost/self".to_string());
    // Extract alias from principal: "agent:authority/name" -> "name"
    let alias = principal.rsplit('/').next().unwrap_or("self").to_string();
    let endpoint = format!("https://{}", config.listen_addr);

    let card = PeerCard {
        alias,
        endpoint,
        pubkey,
        principal,
    };

    // Output
    if args.format == "json" {
        let json = serde_json::to_string_pretty(&card).map_err(|e| CliError::Io {
            path: home.to_path_buf(),
            source: std::io::Error::other(e.to_string()),
        })?;
        writeln!(out, "{json}").ok();
    } else {
        writeln!(out, "Alias:     {}", card.alias).ok();
        writeln!(out, "Endpoint:  {}", card.endpoint).ok();
        writeln!(out, "Pubkey:    {}", card.pubkey).ok();
        writeln!(out, "Principal: {}", card.principal).ok();
    }

    Ok(card)
}

/// Phase 1 slice of IDENT-05: verify all six identity files exist.
fn load_identity(home: &Path) -> Result<IdentityLayout, CliError> {
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

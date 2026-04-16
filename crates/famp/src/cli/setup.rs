//! `famp setup` — one-command onboarding for FAMP agents.
//!
//! This command handles the full setup flow:
//! 1. Creates FAMP_HOME if needed (or uses existing)
//! 2. Runs `famp init` if not already initialized
//! 3. Picks an available port (default range 8443-8543)
//! 4. Updates config.toml with the selected port
//! 5. Outputs a peer card JSON that can be shared with other agents
//!
//! The peer card format:
//! ```json
//! {
//!   "alias": "alice",
//!   "endpoint": "https://127.0.0.1:8443",
//!   "pubkey": "<base64url-unpadded ed25519 pubkey>",
//!   "principal": "agent:localhost/alice"
//! }
//! ```

use std::io::Write;
use std::net::{SocketAddr, TcpListener};
use std::path::Path;

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::cli::config::Config;
use crate::cli::error::CliError;
use crate::cli::paths::IdentityLayout;
use crate::cli::init;

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

/// CLI args for `famp setup`.
#[derive(clap::Args, Debug)]
pub struct SetupArgs {
    /// Agent name/alias (used for principal and suggested alias in peer card).
    #[arg(long, default_value = "self")]
    pub name: String,

    /// Port to listen on. If not specified, auto-selects an available port
    /// in the range 8443-8543.
    #[arg(long)]
    pub port: Option<u16>,

    /// Override FAMP_HOME (useful for running multiple agents).
    /// If not specified, uses $FAMP_HOME or creates a unique directory.
    #[arg(long)]
    pub home: Option<String>,

    /// Force re-initialization even if already initialized.
    #[arg(long)]
    pub force: bool,

    /// Output format: json (default) or text.
    #[arg(long, default_value = "json")]
    pub format: String,
}

/// Production entry point.
pub fn run(args: SetupArgs) -> Result<PeerCard, CliError> {
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    run_with_io(args, &mut stdout, &mut stderr)
}

/// Test-facing entry point with injectable IO.
pub fn run_with_io(
    args: SetupArgs,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<PeerCard, CliError> {
    // 0. Validate agent name
    validate_agent_name(&args.name)?;

    // 1. Resolve or create FAMP_HOME
    let home_path = resolve_home_path(args.home.as_deref(), &args.name)?;
    writeln!(err, "Using FAMP_HOME: {}", home_path.display()).ok();

    // 2. Initialize if needed
    let layout = IdentityLayout::at(home_path.clone());
    let pubkey = if layout.key_ed25519.exists() && !args.force {
        writeln!(err, "Identity already exists, reading pubkey...").ok();
        read_existing_pubkey(&layout)?
    } else {
        writeln!(err, "Initializing new identity...").ok();
        let mut init_out = Vec::new();
        let mut init_err = Vec::new();
        let outcome = init::run_at(&home_path, args.force, &mut init_out, &mut init_err)?;
        outcome.pubkey_b64url
    };

    // 3. Select an available port
    let port = match args.port {
        Some(p) => p,
        None => find_available_port(8443, 8543)?,
    };
    writeln!(err, "Selected port: {}", port).ok();

    // 4. Update config.toml with port and principal
    let principal = format!("agent:localhost/{}", args.name);
    update_config(&layout.config_toml, port, &principal)?;
    writeln!(err, "Updated config.toml").ok();

    // 5. Build and output peer card
    let endpoint = format!("https://127.0.0.1:{}", port);
    let card = PeerCard {
        alias: args.name.clone(),
        endpoint,
        pubkey,
        principal,
    };

    // Output the peer card
    if args.format == "json" {
        let json = serde_json::to_string_pretty(&card).map_err(|e| CliError::Io {
            path: home_path.clone(),
            source: std::io::Error::other(e.to_string()),
        })?;
        writeln!(out, "{}", json).ok();
    } else {
        writeln!(out, "Alias:     {}", card.alias).ok();
        writeln!(out, "Endpoint:  {}", card.endpoint).ok();
        writeln!(out, "Pubkey:    {}", card.pubkey).ok();
        writeln!(out, "Principal: {}", card.principal).ok();
    }

    writeln!(err).ok();
    writeln!(err, "Setup complete! Next steps:").ok();
    writeln!(err, "  1. Start the daemon: FAMP_HOME={} famp listen", home_path.display()).ok();
    writeln!(err, "  2. Share your peer card (above) with other agents").ok();
    writeln!(err, "  3. Import their peer cards: famp peer import < their-card.json").ok();

    Ok(card)
}

/// Validate that the agent name contains only safe characters.
/// Allowed: alphanumeric, underscore, hyphen. Must not be empty.
fn validate_agent_name(name: &str) -> Result<(), CliError> {
    if name.is_empty() {
        return Err(CliError::InvalidAgentName {
            name: name.to_string(),
            reason: "name cannot be empty".to_string(),
        });
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(CliError::InvalidAgentName {
            name: name.to_string(),
            reason: "name must contain only alphanumeric characters, underscores, or hyphens".to_string(),
        });
    }
    Ok(())
}

/// Resolve FAMP_HOME path from args, env, or derive from name.
/// Does not create the directory — that's handled by `init::run_at`.
fn resolve_home_path(
    arg_home: Option<&str>,
    name: &str,
) -> Result<std::path::PathBuf, CliError> {
    // Priority: --home flag > FAMP_HOME env > create new
    if let Some(h) = arg_home {
        let path = std::path::PathBuf::from(h);
        if !path.is_absolute() {
            return Err(CliError::HomeNotAbsolute { path });
        }
        return Ok(path);
    }

    // Try FAMP_HOME env
    if let Ok(h) = std::env::var("FAMP_HOME") {
        let path = std::path::PathBuf::from(&h);
        if path.is_absolute() {
            return Ok(path);
        }
    }

    // Create a unique home based on name
    let base = std::env::var("HOME").map_err(|_| CliError::HomeNotSet)?;
    let home_path = std::path::PathBuf::from(base).join(".famp").join(name);
    Ok(home_path)
}

/// Read the pubkey from an existing identity.
fn read_existing_pubkey(layout: &IdentityLayout) -> Result<String, CliError> {
    let pub_bytes = std::fs::read(&layout.pub_ed25519).map_err(|e| CliError::Io {
        path: layout.pub_ed25519.clone(),
        source: e,
    })?;
    if pub_bytes.len() != 32 {
        return Err(CliError::IdentityIncomplete {
            missing: layout.pub_ed25519.clone(),
        });
    }
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&pub_bytes))
}

/// Find an available port in the given range.
///
/// NOTE: This is a best-effort check. There is a TOCTOU race between when we
/// check port availability and when `famp listen` actually binds. If another
/// process claims the port in between, `famp listen` will fail with `PortInUse`.
/// For production use, prefer specifying `--port` explicitly.
fn find_available_port(start: u16, end: u16) -> Result<u16, CliError> {
    for port in start..=end {
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        if TcpListener::bind(addr).is_ok() {
            return Ok(port);
        }
    }
    Err(CliError::PortInUse {
        addr: format!("127.0.0.1:{}-{}", start, end).parse().unwrap_or_else(|_| {
            format!("127.0.0.1:{}", start).parse().unwrap()
        }),
    })
}

/// Update config.toml with the selected port and principal.
/// Preserves any other existing fields in the config.
fn update_config(config_path: &Path, port: u16, principal: &str) -> Result<(), CliError> {
    // Read existing config if present, otherwise use default
    let mut config = if config_path.exists() {
        let bytes = std::fs::read(config_path).map_err(|e| CliError::Io {
            path: config_path.to_path_buf(),
            source: e,
        })?;
        let text = std::str::from_utf8(&bytes).map_err(|e| CliError::Io {
            path: config_path.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
        })?;
        toml::from_str(text).unwrap_or_default()
    } else {
        Config::default()
    };

    // Update only the fields we care about
    config.listen_addr = format!("127.0.0.1:{}", port).parse().unwrap();
    config.principal = Some(principal.to_string());

    let toml_str = toml::to_string(&config).map_err(CliError::TomlSerialize)?;

    // Write atomically
    let parent = config_path.parent().ok_or_else(|| CliError::HomeHasNoParent {
        path: config_path.to_path_buf(),
    })?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| CliError::Io {
        path: parent.to_path_buf(),
        source: e,
    })?;
    tmp.write_all(toml_str.as_bytes())
        .map_err(|e| CliError::Io {
            path: config_path.to_path_buf(),
            source: e,
        })?;
    tmp.as_file_mut().sync_all().map_err(|e| CliError::Io {
        path: config_path.to_path_buf(),
        source: e,
    })?;
    tmp.persist(config_path).map_err(|e| CliError::Io {
        path: config_path.to_path_buf(),
        source: e.error,
    })?;

    Ok(())
}

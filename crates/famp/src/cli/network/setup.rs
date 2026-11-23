//! Assemble a compatible gateway identity, TLS certificate and host wiring.
use std::collections::BTreeMap;
use std::net::IpAddr;
use std::path::Path;
use std::process::Command;

use super::state::{self, checked, error, Card, Config};
use super::{Host, SetupArgs};
use crate::cli::{daemon, error::CliError, install, peer::identity};

pub fn run(args: &SetupArgs) -> Result<(), CliError> {
    let home = crate::cli::home::resolve_famp_home()?;
    if !args.no_services {
        super::service::default_home_only(&home)?;
    }
    let config = prepare(&home, args)?;
    super::allowlist::ensure_seeded(&home, &config.local.name)?;
    super::perms::fix(&home)?;
    wire_host(args.host)?;
    if !args.no_services {
        daemon::install::run(daemon::install::DaemonInstallArgs { home: None })?;
        // Setup never starts the gateway: it only stages the definition.
        // `famp network enable` is the separate, reviewable step.
        if !config.peers.is_empty() {
            super::service::stage(&home)?;
        }
    }
    eprintln!(
        "Configured {} at {}. Private keys stay on this computer.",
        config.local.name,
        config.local.endpoint()
    );
    eprintln!(
        "Restart your agent host, then ask it to register with FAMP as '{}'.",
        config.local.name
    );
    eprintln!("Next: famp connect <ssh-host> [more-hosts...]\nCheck: famp doctor\nRemote messages require explicit inbox reads; they do not auto-wake agents.");
    Ok(())
}

pub fn prepare(home: &Path, args: &SetupArgs) -> Result<Config, CliError> {
    if home.join("network.json").exists() {
        let config = state::load(home)?;
        if args.name.as_ref().is_some_and(|n| *n != config.local.name)
            || args.address.is_some_and(|a| a != config.local.address)
            || args.port.is_some_and(|p| p != config.local.port)
        {
            return Err(error("This computer already has a different network identity. Refusing to rotate keys or change its address silently."));
        }
        config.local.validate()?;
        verify_identity(home, &config.local)?;
        // A rerun by a newer build refreshes the stamp, so `connect` keeps working
        // after an upgrade. Trust material is untouched.
        if config.local.version != env!("CARGO_PKG_VERSION") {
            let mut config = config;
            env!("CARGO_PKG_VERSION").clone_into(&mut config.local.version);
            state::save(home, &config)?;
            return Ok(config);
        }
        return Ok(config);
    }
    let name = if let Some(name) = &args.name {
        name.clone()
    } else {
        let bytes = checked(Command::new("hostname").arg("-s"))?;
        String::from_utf8_lossy(&bytes)
            .trim()
            .to_ascii_lowercase()
            .replace(['_', '.'], "-")
    };
    state::validate_name(&name)?;
    let address = match args.address {
        Some(address) => {
            if !args.allow_non_tailnet {
                let own = discover_address()?;
                if own != address {
                    return Err(error(format!(
                        "Refusing to bind {address}: this computer's Tailscale IP is {own}. Pass --allow-non-tailnet only for a deliberate LAN/test setup."
                    )));
                }
            }
            address
        }
        None => discover_address()?,
    };
    let domain_path = home.join("own-domain");
    let domain = if domain_path.exists() {
        std::fs::read_to_string(&domain_path)
            .map_err(error)?
            .trim()
            .to_owned()
    } else {
        format!("{name}.famp")
    };
    let key = identity::load_or_generate(&identity::gateway_identity_path(home))?;
    let tls_dir = home.join("network-tls");
    std::fs::create_dir_all(home).map_err(error)?;
    let certificate = if tls_dir.exists() {
        // An earlier setup crashed between creating the TLS pair and saving
        // network.json. Reuse it only if it is intact and binds this address.
        reuse_tls(&tls_dir, address)?
    } else {
        generate_tls(home, &tls_dir, &name, address)?
    };
    let local = Card {
        schema: 1,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        name,
        domain,
        address,
        port: args.port.unwrap_or(7443),
        public_key: key.verifying_key().to_b64url(),
        certificate,
    };
    local.validate()?;
    let config = Config {
        local,
        peers: BTreeMap::new(),
    };
    state::atomic(&domain_path, config.local.domain.as_bytes())?;
    state::save(home, &config)?;
    Ok(config)
}

fn generate_tls(
    home: &Path,
    tls_dir: &Path,
    name: &str,
    address: IpAddr,
) -> Result<String, CliError> {
    let temp = tempfile::tempdir_in(home).map_err(error)?;
    let cert = temp.path().join("cert.pem");
    let secret = temp.path().join("key.pem");
    checked(
        Command::new("openssl")
            .args([
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-days",
                "365",
                "-subj",
                &format!("/CN={name}"),
                "-addext",
                &format!("subjectAltName=IP:{address}"),
                "-addext",
                "basicConstraints=critical,CA:FALSE",
                "-addext",
                "keyUsage=critical,digitalSignature,keyEncipherment",
                "-addext",
                "extendedKeyUsage=serverAuth",
                "-keyout",
            ])
            .arg(&secret)
            .arg("-out")
            .arg(&cert),
    )?;
    {
        // LibreSSL writes the key 0644; the 0700 temp dir contains it, but don't rely on that.
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o600)).map_err(error)?;
    }
    let pem = std::fs::read_to_string(&cert).map_err(error)?;
    std::fs::rename(temp.path(), tls_dir).map_err(error)?;
    Ok(pem)
}

pub fn verify_identity(home: &Path, card: &Card) -> Result<(), CliError> {
    let path = identity::gateway_identity_path(home);
    if !path.exists() {
        return Err(error("Gateway signing key is missing; restore it from backup. Setup will not rotate an enrolled identity."));
    }
    let key = identity::load_or_generate(&path)?;
    let domain = std::fs::read_to_string(home.join("own-domain")).map_err(error)?;
    let cert = std::fs::read_to_string(home.join("network-tls/cert.pem")).map_err(error)?;
    if key.verifying_key().to_b64url() != card.public_key
        || domain.trim() != card.domain
        || cert != card.certificate
    {
        return Err(error(
            "Network configuration disagrees with the existing signing key, domain or certificate",
        ));
    }
    tls_pair_matches(&home.join("network-tls"))
}

/// The TLS private key must still correspond to the advertised certificate.
fn tls_pair_matches(dir: &Path) -> Result<(), CliError> {
    let cert_key = checked(
        Command::new("openssl")
            .args(["x509", "-pubkey", "-noout", "-in"])
            .arg(dir.join("cert.pem")),
    )?;
    let private_key = checked(
        Command::new("openssl")
            .args(["pkey", "-pubout", "-in"])
            .arg(dir.join("key.pem")),
    )?;
    if cert_key != private_key {
        return Err(error(
            "TLS private key does not match this computer's certificate",
        ));
    }
    Ok(())
}

fn reuse_tls(dir: &Path, address: IpAddr) -> Result<String, CliError> {
    tls_pair_matches(dir).map_err(|e| {
        error(format!(
            "Incomplete setup: {} exists but is unusable ({e}). Nothing was replaced; inspect it.",
            dir.display()
        ))
    })?;
    let pem = std::fs::read_to_string(dir.join("cert.pem")).map_err(error)?;
    let covers = super::cert::facts(&pem)
        .map_err(error)?
        .ips
        .contains(&address);
    if !covers {
        return Err(error(format!(
            "Incomplete setup: existing network-tls does not cover {address}. Nothing was replaced; inspect it."
        )));
    }
    Ok(pem)
}

fn discover_address() -> Result<IpAddr, CliError> {
    let binary = which::which("tailscale")
        .or_else(|_| which::which("/Applications/Tailscale.app/Contents/MacOS/Tailscale"))
        .map_err(|_| {
            error(
                "Tailscale was not found. Start Tailscale, or for a deliberate LAN/test setup run `famp setup --allow-non-tailnet --address <LAN-IP>`.",
            )
        })?;
    let bytes = checked(Command::new(binary).args(["ip", "-4"]))?;
    String::from_utf8_lossy(&bytes).trim().parse().map_err(|_| {
        error("No Tailscale IPv4 address. Connect Tailscale, or for a deliberate LAN/test setup pass --allow-non-tailnet --address <LAN-IP>.")
    })
}

fn wire_host(host: Host) -> Result<(), CliError> {
    if matches!(host, Host::Claude) || matches!(host, Host::Auto) && which::which("claude").is_ok()
    {
        if claude_plugin_present()? {
            eprintln!("Claude FAMP plugin found; preserving its integration (no duplicate MCP/hooks). Restart Claude after updating the plugin.");
        } else {
            install::claude_code::run(install::claude_code::InstallClaudeCodeArgs { home: None })?;
        }
    }
    if matches!(host, Host::Codex) || matches!(host, Host::Auto) && which::which("codex").is_ok() {
        install::codex::run(install::codex::InstallCodexArgs {
            home: None,
            project: None,
        })?;
    }
    Ok(())
}

fn claude_plugin_present() -> Result<bool, CliError> {
    let home = dirs::home_dir().ok_or_else(|| error("Cannot resolve home"))?;
    for path in [
        home.join(".claude/plugins/installed_plugins.json"),
        home.join(".claude/settings.json"),
    ] {
        if !path.exists() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).map_err(error)?).map_err(error)?;
        for field in ["plugins", "enabledPlugins"] {
            if value
                .get(field)
                .and_then(serde_json::Value::as_object)
                .is_some_and(|m| m.keys().any(|k| k == "famp" || k.starts_with("famp@")))
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

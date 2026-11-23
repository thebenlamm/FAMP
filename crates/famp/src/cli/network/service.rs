//! Persistent gateway wrapper: service managers own process lifetime.
//! `stage` writes the unit; only `enable` starts it, so a guardian can review between.
use std::path::{Path, PathBuf};
use std::process::Command;

use super::state::{self, checked, error, Config};
use crate::cli::error::CliError;

pub fn default_home_only(home: &Path) -> Result<(), CliError> {
    let user = dirs::home_dir().ok_or_else(|| error("Cannot resolve user home"))?;
    if home != user.join(".famp") || std::env::var_os("FAMP_BUS_SOCKET").is_some() {
        return Err(error("Managed services require the default ~/.famp and broker socket. Use `network setup --no-services` and `network serve` for isolated/custom homes."));
    }
    Ok(())
}

pub fn gateway_binary() -> Result<PathBuf, CliError> {
    let current = std::env::current_exe().map_err(error)?;
    let sibling = current.with_file_name("famp-gateway");
    if sibling.is_file() {
        return Ok(sibling);
    }
    which::which("famp-gateway").map_err(|_| error("famp-gateway is missing. Install the matching gateway release alongside famp; see the README section on installing from a git bundle."))
}

pub fn gateway_args(home: &Path, config: &Config) -> Result<Vec<String>, CliError> {
    if config.peers.is_empty() {
        return Err(error("No peers yet. Run `famp connect <ssh-host>`."));
    }
    let path = |p: &str| home.join(p).to_string_lossy().into_owned();
    let mut args = vec![
        "--socket".into(),
        path("bus.sock"),
        "--listen".into(),
        std::net::SocketAddr::new(config.local.address, config.local.port).to_string(),
        "--tls-cert".into(),
        path("network-tls/cert.pem"),
        "--tls-key".into(),
        path("network-tls/key.pem"),
        "--trust-cert".into(),
        path("network-trust.pem"),
        // Trust ONLY the pinned peer certificates, never OS roots: without this a
        // publicly trusted certificate for a peer's name would be accepted.
        "--pinned-only".into(),
        // W0: deliver only to local names listed here (default deny, read per message).
        "--recipient-allowlist".into(),
        path(super::allowlist::FILE),
    ];
    for peer in config.peers.values() {
        args.extend([
            "--peer".into(),
            format!("{}={}", peer.domain, peer.endpoint()),
            "--backs".into(),
            peer.principal(),
        ]);
    }
    args.extend(config.peers.values().map(|p| p.name.clone()));
    Ok(args)
}

pub fn serve(home: &Path) -> Result<(), CliError> {
    use std::os::unix::process::CommandExt as _;
    let config = state::load(home)?;
    config.local.validate()?;
    super::setup::verify_identity(home, &config.local)?;
    // No seeding here: a deleted allowlist keeps denying everything until an
    // operator runs `famp network setup` (which seeds an absent file) or `famp network allow`.
    let mut trust = String::new();
    for peer in config.peers.values() {
        // Shape only: one peer's lapsed certificate must not stop the gateway for
        // the others. That peer's TLS handshake simply fails until it is replaced.
        peer.validate_shape()?;
        if peer.cert_expires_within(0).unwrap_or(true) {
            eprintln!(
                "famp-gateway: WARNING: the TLS certificate pinned for {} has expired; messages to and from it will fail",
                peer.name
            );
        }
        trust.push_str(&peer.certificate);
        trust.push('\n');
    }
    state::atomic(&home.join("network-trust.pem"), trust.as_bytes())?;
    let args = gateway_args(home, &config)?;
    let e = Command::new(gateway_binary()?)
        .args(args)
        .env("FAMP_HOME", home)
        .env("FAMP_OWN_DOMAIN", &config.local.domain)
        .exec();
    Err(error(format!("Could not start gateway: {e}")))
}

fn prepared(home: &Path) -> Result<(Config, PathBuf), CliError> {
    default_home_only(home)?;
    let config = state::load(home)?;
    config.local.validate()?;
    super::setup::verify_identity(home, &config.local)?;
    gateway_args(home, &config)?;
    gateway_binary()?;
    let binary = crate::cli::executable::resolve_for_generated_config()?;
    Ok((config, binary.path().to_path_buf()))
}

/// Write the service definition and nothing else: no enable, no start.
pub fn stage(home: &Path) -> Result<(), CliError> {
    let (config, binary) = prepared(home)?;
    let user = dirs::home_dir().ok_or_else(|| error("Cannot resolve user home"))?;
    let path = stage_platform(&user, home, &binary, &config)?;
    eprintln!(
        "Staged {} (NOT enabled). Review with `famp network print-unit`, then run `famp network enable`.",
        path.display()
    );
    Ok(())
}

/// The gateway attaches to the local broker and never starts one (it connects
/// with `connect_no_spawn`), and `stage` deliberately installs no services, so a
/// computer set up with `--no-services` may have no broker. Refuse to enable a
/// gateway that would only crash-loop; installing the broker is the operator's call.
fn require_broker() -> Result<(), CliError> {
    crate::cli::block_on_async(async {
        let sock = crate::bus_client::resolve_sock_path();
        let probe = famp_inspect_client::raw_connect_probe(&sock);
        match tokio::time::timeout(std::time::Duration::from_secs(3), probe).await {
            Ok(famp_inspect_client::ProbeOutcome::Healthy { .. }) => Ok(()),
            _ => Err(error(
                "The local broker is not running, and the gateway never starts one. Run `famp daemon install`, confirm with `famp doctor`, then rerun `famp network enable`.",
            )),
        }
    })
}

/// Stage, then enable and (re)start the gateway service.
pub fn enable(home: &Path) -> Result<(), CliError> {
    require_broker()?;
    stage(home)?;
    let (config, _) = prepared(home)?;
    let user = dirs::home_dir().ok_or_else(|| error("Cannot resolve user home"))?;
    enable_platform(&user, home)?;
    eprintln!(
        "Gateway service enabled for {}. Run `famp doctor` to verify reachability.\nFIREWALL: this gateway has no TLS client authentication. Apply the rules from `famp network firewall` by hand and verify them (`sudo ufw status numbered` on Linux, `sudo pfctl -a famp -sr` on macOS) before relying on the mesh.",
        config.local.name
    );
    Ok(())
}

/// Both service definitions exactly as `stage` would write them.
pub fn print_units(home: &Path) -> Result<String, CliError> {
    let (config, binary) = prepared(home)?;
    Ok(format!(
        "# systemd --user: ~/.config/systemd/user/famp-gateway.service\n{}\n# launchd: staged at ~/.famp/staged/com.famp.gateway.plist, copied to ~/Library/LaunchAgents by `network enable`\n{}",
        unit(&binary, home, &config),
        plist(&binary, home)
    ))
}

/// Host firewall rules to run BY HAND, never executed by FAMP (no sudo from code).
///
/// With no TLS client authentication, any device on the tailnet can reach the
/// gateway's pre-verify parser; the firewall limits that to the pinned peers.
/// Identical on every box. Neither tool filters loopback, so this does not stop
/// other local users on the same machine.
pub fn firewall(config: &Config) -> String {
    use std::fmt::Write as _;
    let port = config.local.port;
    // The interface names below assume a Tailscale bind (the only bind `setup`
    // allows without --allow-non-tailnet); adjust them for a LAN/test setup.
    let peers: Vec<String> = config
        .peers
        .values()
        .map(|p| p.address.to_string())
        .collect();
    let mut out = String::new();
    let _ = writeln!(
        out,
        "## Linux (ufw). Run by hand, in this order; ufw is first-match."
    );
    let _ = writeln!(
        out,
        "sudo ufw insert 1 deny in on tailscale0 to any port {port} proto tcp"
    );
    for ip in &peers {
        let _ = writeln!(
            out,
            "sudo ufw insert 1 allow in on tailscale0 from {ip} to any port {port} proto tcp"
        );
    }
    let _ = writeln!(
        out,
        "# If ufw has no rules yet and `insert 1` is refused, append in the same order instead: the allows first, then the deny."
    );
    let _ = writeln!(out, "# Verify: sudo ufw status numbered");
    let _ = writeln!(
        out,
        "# Expect (approximately): the peer ALLOW rules on top, the DENY last, i.e. top to bottom:"
    );
    for (n, ip) in peers.iter().rev().enumerate() {
        let _ = writeln!(
            out,
            "#   [{}] {port}/tcp on tailscale0   ALLOW IN   {ip}",
            n + 1
        );
    }
    let _ = writeln!(
        out,
        "#   [{}] {port}/tcp on tailscale0   DENY IN    Anywhere",
        peers.len() + 1
    );
    let _ = writeln!(
        out,
        "\n## macOS (pf; the application firewall cannot filter by port or source)."
    );
    let _ = writeln!(out, "# 1. /etc/pf.anchors/famp (UNVERIFIED on your macOS version; check the interface with `ifconfig | grep utun`):");
    for ip in &peers {
        let _ = writeln!(
            out,
            "#      pass in quick on utun proto tcp from {ip} to any port {port}"
        );
    }
    let _ = writeln!(
        out,
        "#      block in quick on utun proto tcp from any to any port {port}"
    );
    let _ = writeln!(
        out,
        "# 2. add to /etc/pf.conf:  anchor \"famp\"  and  load anchor \"famp\" from \"/etc/pf.anchors/famp\""
    );
    let _ = writeln!(
        out,
        "# 3. load: sudo pfctl -f /etc/pf.conf && sudo pfctl -E"
    );
    let _ = writeln!(
        out,
        "# 4. verify: sudo pfctl -a famp -sr   (expect the pass rules, then the block)"
    );
    let _ = writeln!(
        out,
        "# Persistence: pf is not enabled at boot by default and macOS updates can rewrite /etc/pf.conf; use a LaunchDaemon that runs `pfctl -f /etc/pf.conf -E` at load, and re-verify after updates."
    );
    out
}

/// macOS: launchd loads every plist in ~/Library/LaunchAgents at the next login,
/// and ours has RunAtLoad. So `stage` writes to ~/.famp/staged/ and only
/// `enable` installs it into LaunchAgents.
#[cfg(target_os = "macos")]
fn stage_platform(
    _user: &Path,
    home: &Path,
    binary: &Path,
    _config: &Config,
) -> Result<PathBuf, CliError> {
    let path = home.join("staged/com.famp.gateway.plist");
    state::atomic(&path, plist(binary, home).as_bytes())?;
    Ok(path)
}

#[cfg(target_os = "macos")]
fn enable_platform(user: &Path, home: &Path) -> Result<(), CliError> {
    let staged = home.join("staged/com.famp.gateway.plist");
    let path = user.join("Library/LaunchAgents/com.famp.gateway.plist");
    state::atomic(&path, &std::fs::read(&staged).map_err(error)?)?;
    let domain = format!("gui/{}", nix::unistd::getuid());
    let service = format!("{domain}/com.famp.gateway");
    let loaded = || {
        Command::new("launchctl")
            .args(["print", &service])
            .output()
            .map(|o| o.status.success())
            .map_err(error)
    };
    if loaded()? {
        checked(Command::new("launchctl").args(["bootout", &service]))?;
        // bootstrap fails (exit 5) while the label is still unloading.
        for _ in 0..50 {
            if !loaded()? {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    checked(
        Command::new("launchctl")
            .args(["bootstrap", &domain])
            .arg(path),
    )?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn stage_platform(
    user: &Path,
    home: &Path,
    binary: &Path,
    config: &Config,
) -> Result<PathBuf, CliError> {
    let path = user.join(".config/systemd/user/famp-gateway.service");
    state::atomic(&path, unit(binary, home, config).as_bytes())?;
    checked(Command::new("systemctl").args(["--user", "daemon-reload"]))?;
    Ok(path)
}

#[cfg(target_os = "linux")]
fn enable_platform(_user: &Path, _home: &Path) -> Result<(), CliError> {
    checked(Command::new("systemctl").args(["--user", "enable", "famp-gateway.service"]))?;
    checked(Command::new("systemctl").args(["--user", "restart", "famp-gateway.service"]))?;
    Ok(())
}

fn plist(binary: &Path, home: &Path) -> String {
    let xml = |p: &Path| {
        p.to_string_lossy()
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>Label</key><string>com.famp.gateway</string><key>ProgramArguments</key><array><string>{}</string><string>network</string><string>serve</string></array><key>EnvironmentVariables</key><dict><key>FAMP_HOME</key><string>{}</string></dict><key>RunAtLoad</key><true/><key>KeepAlive</key><true/><key>ThrottleInterval</key><integer>10</integer><key>StandardOutPath</key><string>{}</string><key>StandardErrorPath</key><string>{}</string></dict></plist>\n", xml(binary), xml(home), xml(&home.join("gateway.log")), xml(&home.join("gateway.log")))
}

/// Hardened user unit. `IPAddressDeny=any` plus an allow-list of loopback, this
/// computer and each pinned peer. systemd may ignore these directives in a
/// `--user` unit (UNVERIFIED), so treat them as defense in depth, never as the
/// only control; the host firewall (`famp network firewall`) is the real one.
fn unit(binary: &Path, home: &Path, config: &Config) -> String {
    let quote = |p: &Path| {
        format!(
            "\"{}\"",
            p.to_string_lossy()
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('%', "%%")
                .replace('$', "$$")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
        )
    };
    // Environment= is not `$`-expanded by systemd; only `%` specifiers apply there.
    let env_quote = |p: &Path| {
        format!(
            "\"{}\"",
            p.to_string_lossy()
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('%', "%%")
        )
    };
    let mut allow = vec!["localhost".to_owned(), config.local.address.to_string()];
    allow.extend(config.peers.values().map(|p| p.address.to_string()));
    allow.sort();
    allow.dedup();
    let allow_lines = allow.iter().fold(String::new(), |mut acc, a| {
        acc.push_str("IPAddressAllow=");
        acc.push_str(a);
        acc.push('\n');
        acc
    });
    format!(
        "[Unit]\nDescription=FAMP federation gateway\nWants=famp-broker.service\nAfter=famp-broker.service\nStartLimitIntervalSec=600\nStartLimitBurst=20\n\n[Service]\nExecStart={} network serve\nEnvironment=FAMP_HOME={}\nRestart=on-failure\nRestartSec=15\nMemoryMax=512M\nNoNewPrivileges=yes\nPrivateTmp=yes\nProtectSystem=full\nIPAddressDeny=any\n{allow_lines}\n[Install]\nWantedBy=default.target\n",
        quote(binary),
        env_quote(home)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(name: &str, ip: &str) -> state::Card {
        state::Card {
            schema: 1,
            version: "0".into(),
            name: name.into(),
            domain: format!("{name}.famp"),
            address: ip
                .parse()
                .unwrap_or_else(|_| std::net::IpAddr::from([100, 0, 0, 1])),
            port: 7443,
            public_key: String::new(),
            certificate: String::new(),
        }
    }

    fn config() -> Config {
        let mut peers = std::collections::BTreeMap::new();
        peers.insert("b".into(), card("b", "100.64.0.2"));
        peers.insert("c".into(), card("c", "100.64.0.3"));
        Config {
            local: card("a", "100.64.0.1"),
            peers,
        }
    }

    #[test]
    fn service_paths_are_escaped() {
        assert!(
            plist(Path::new("/a & b/famp"), Path::new("/home/test")).contains("/a &amp; b/famp")
        );
        assert!(unit(
            Path::new("/a b/%n/$x/famp"),
            Path::new("/home/test"),
            &config()
        )
        .contains("ExecStart=\"/a b/%%n/$$x/famp\""));
    }

    #[test]
    fn enable_refuses_when_no_broker_is_running() {
        let missing = "/nonexistent-famp-test/bus.sock";
        let err = temp_env::with_var("FAMP_BUS_SOCKET", Some(missing), require_broker);
        let Err(e) = err else {
            panic!("no broker, so enable must refuse");
        };
        assert!(e.to_string().contains("famp daemon install"), "{e}");
    }

    #[test]
    fn serve_always_pins_peer_certificates_only() -> Result<(), CliError> {
        let args = gateway_args(Path::new("/h"), &config())?;
        assert!(args.contains(&"--pinned-only".to_owned()), "{args:?}");
        let at = args.iter().position(|a| a == "--recipient-allowlist");
        assert_eq!(
            at.and_then(|i| args.get(i + 1)).map(String::as_str),
            Some("/h/recipients.json")
        );
        let trust = args.iter().position(|a| a == "--trust-cert");
        assert!(trust.is_some(), "--pinned-only needs --trust-cert");
        Ok(())
    }

    #[test]
    fn unit_is_hardened_and_allows_only_known_addresses() {
        let text = unit(Path::new("/bin/famp"), Path::new("/h"), &config());
        for needle in [
            "StartLimitBurst=20",
            "MemoryMax=512M",
            "NoNewPrivileges=yes",
            "PrivateTmp=yes",
            "ProtectSystem=full",
            "IPAddressDeny=any",
            "IPAddressAllow=localhost",
            "IPAddressAllow=100.64.0.1",
            "IPAddressAllow=100.64.0.2",
            "IPAddressAllow=100.64.0.3",
        ] {
            assert!(text.contains(needle), "missing {needle}");
        }
        assert_eq!(text.matches("IPAddressAllow=").count(), 4);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn staging_does_not_put_a_plist_where_launchd_loads_it_at_login() -> Result<(), CliError> {
        let user = tempfile::tempdir().map_err(error)?;
        let home = tempfile::tempdir().map_err(error)?;
        let path = stage_platform(user.path(), home.path(), Path::new("/bin/famp"), &config())?;
        assert!(path.starts_with(home.path().join("staged")));
        assert!(path.exists());
        assert!(
            !user.path().join("Library/LaunchAgents").exists(),
            "a staged plist in LaunchAgents would load at the next login"
        );
        Ok(())
    }

    #[test]
    fn firewall_puts_allows_above_the_deny() {
        let text = firewall(&config());
        let deny = text.find("insert 1 deny").unwrap_or(usize::MAX);
        let last_allow = text.rfind("insert 1 allow").unwrap_or(0);
        assert!(deny < last_allow, "deny is inserted first, allows after it");
        assert_eq!(text.matches("insert 1 allow").count(), 2);
        assert!(!text.contains("0.0.0.0"));
        // pf: quick passes precede the block, and every peer is named.
        let block = text.find("block in quick").unwrap_or(0);
        assert!(text.rfind("pass in quick").unwrap_or(usize::MAX) < block);
        assert_eq!(text.matches("pass in quick").count(), 2);
        assert!(text.contains("sudo pfctl -a famp -sr"));
    }
}

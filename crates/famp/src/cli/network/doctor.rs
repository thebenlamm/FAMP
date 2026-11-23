//! Read-only diagnostics: transport readiness is not task completion.
use std::io::Write as _;
use std::time::Duration;

use famp_inspect_client::{call, raw_connect_probe, ProbeOutcome};
use famp_inspect_proto::{InspectIdentitiesReply, InspectIdentitiesRequest, InspectKind};
use serde::Serialize;

use super::state::{self, error, Card, Config};
use super::DoctorArgs;
use crate::cli::error::CliError;

#[derive(Serialize)]
struct Check {
    component: String,
    status: &'static str,
    detail: String,
}

fn check(component: impl Into<String>, status: &'static str, detail: impl Into<String>) -> Check {
    Check {
        component: component.into(),
        status,
        detail: detail.into(),
    }
}

pub async fn run(args: DoctorArgs) -> Result<(), CliError> {
    let home = crate::cli::home::resolve_famp_home()?;
    let mut checks = Vec::new();
    let config = state::load(&home);
    let local_name = config.as_ref().ok().map(|c| c.local.name.clone());
    let broker = tokio::time::timeout(Duration::from_secs(3), async {
        let ProbeOutcome::Healthy { mut stream } = raw_connect_probe(&crate::bus_client::resolve_sock_path()).await else {
            return Err(error("Broker unavailable or incompatible. Run `famp inspect broker`, then `famp daemon install` or `famp daemon restart`."));
        };
        let value = call(&mut stream, InspectKind::Identities(InspectIdentitiesRequest::default())).await.map_err(error)?;
        let reply: InspectIdentitiesReply = serde_json::from_value(value).map_err(error)?;
        Ok(reply)
    }).await;
    match broker {
        Ok(Ok(InspectIdentitiesReply::List(list))) => {
            checks.push(check("broker", "ok", "Broker handshake succeeded"));
            let present = local_name
                .as_ref()
                .is_some_and(|name| list.rows.iter().any(|r| r.name == *name));
            checks.push(if present { check("agent", "ok", "Named federation agent is registered; attendance does not prove wake readiness") }
                else { check("agent", "warning", format!("No named federation agent. Restart your host and ask it to register as '{}'.", local_name.as_deref().unwrap_or("<setup name>"))) });
        }
        Ok(Err(e)) => checks.push(check("broker", "error", e.to_string())),
        _ => checks.push(check(
            "broker",
            "error",
            "Broker did not return identities within 3 seconds; run `famp inspect broker`",
        )),
    }
    match config {
        Ok(config) => {
            match config
                .local
                .validate()
                .and_then(|()| super::setup::verify_identity(&home, &config.local))
            {
                Ok(()) => checks.push(check(
                    "configuration",
                    "ok",
                    format!("{} ({})", config.local.name, config.local.endpoint()),
                )),
                Err(e) => checks.push(check("configuration", "error", e.to_string())),
            }
            config_checks(&home, &config, &mut checks);
            if config.peers.is_empty() {
                checks.push(check(
                    "network",
                    "warning",
                    "No peers configured. Run `famp connect <ssh-host>`.",
                ));
            } else {
                for card in std::iter::once(&config.local).chain(config.peers.values()) {
                    match probe(card).await {
                        Ok(()) => checks.push(check(format!("gateway:{}", card.name), "ok", format!("key_id={} pinned-only TLS (network serve always passes --pinned-only) and inbox route reachable; message delivery has not been tested", card.fingerprint().unwrap_or_default()))),
                        Err(e) => checks.push(check(format!("gateway:{}", card.name), "error", format!("{e}. Run `famp network enable` on that computer; check Tailscale, its firewall and gateway.log."))),
                    }
                }
            }
        }
        Err(e) => checks.push(check("configuration", "error", e.to_string())),
    }
    checks.push(check(
        "remote-wake",
        "warning",
        "Remote messages require explicit `famp_inbox` reads. Remote auto-wake is not enabled.",
    ));
    let failed = checks.iter().any(|c| c.status == "error");
    if args.json {
        println!("{}", serde_json::to_string_pretty(&checks).map_err(error)?);
    } else {
        for c in checks {
            println!("[{}] {}: {}", c.status, c.component, c.detail);
        }
    }
    if failed {
        Err(CliError::Exit(1))
    } else {
        Ok(())
    }
}

pub async fn probe(card: &Card) -> Result<(), CliError> {
    let mut cert = tempfile::NamedTempFile::new().map_err(error)?;
    cert.write_all(card.certificate.as_bytes()).map_err(error)?;
    let tls = famp_transport_http::tls::build_pinned_client_config(cert.path()).map_err(error)?;
    let client = reqwest::Client::builder()
        .use_preconfigured_tls(tls)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(error)?;
    let url = format!("{}{}", card.endpoint(), famp_transport_http::INBOX_ROUTE);
    let mut last = String::new();
    // Allow a newly bootstrapped gateway to bind. GET never delivers a message.
    for attempt in 0..3 {
        match client.get(&url).send().await {
            Ok(response) if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED => {
                return Ok(())
            }
            Ok(response) => last = format!("Unexpected inbox probe status {}", response.status()),
            Err(e) => last = format!("Cannot reach {}: {e}", card.endpoint()),
        }
        if attempt < 2 {
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    }
    Err(error(last))
}

fn config_checks(home: &std::path::Path, config: &Config, checks: &mut Vec<Check>) {
    let drift = super::perms::problems(home);
    if drift.is_empty() {
        checks.push(check(
            "permissions",
            "ok",
            "Directories 0700, keys and keyring 0600",
        ));
    } else {
        checks.push(check(
            "permissions",
            "error",
            format!("{}. Run `famp setup` to tighten them.", drift.join("; ")),
        ));
    }
    checks.push(cert_expiry_check(home));
    if !is_tailnet(config.local.address) {
        checks.push(check(
                "bind",
                "warning",
                format!("{} is not a Tailscale address; the gateway should bind only this computer's Tailscale IP", config.local.address),
            ));
    }
    match super::allowlist::load(home) {
        Ok(names) if names.is_empty() => checks.push(check(
            "recipients",
            "warning",
            "No local agent may receive peer messages (default deny). Run `famp network allow <name>`.",
        )),
        Ok(names) => checks.push(check(
            "recipients",
            "ok",
            format!("Peers may deliver to: {}", names.join(", ")),
        )),
        Err(e) => checks.push(check("recipients", "error", e.to_string())),
    }
    for peer in config.peers.values() {
        match peer.cert_expires_within(30 * 86_400) {
            Ok(false) => {}
            Ok(true) => checks.push(check(
                format!("certificate:{}", peer.name),
                "warning",
                "Pinned TLS certificate expires within 30 days or is unreadable",
            )),
            Err(e) => checks.push(check(
                format!("certificate:{}", peer.name),
                "error",
                e.to_string(),
            )),
        }
    }
    match config.local.fingerprint() {
        Ok(fp) => checks.push(check(
            "fingerprint:local",
            "ok",
            format!("{} key_id={fp}", config.local.principal()),
        )),
        Err(e) => checks.push(check("fingerprint:local", "error", e.to_string())),
    }
    let managed: std::collections::BTreeSet<String> =
        config.peers.values().map(Card::principal).collect();
    match state::keyring_principals(&home.join("gateway/peers.keyring")) {
        Ok(pins) => {
            for pin in pins.iter().filter(|p| !managed.contains(*p)) {
                checks.push(check(
                        "pin",
                        "warning",
                        format!("{pin} is trusted but is not a configured peer; `famp connect --prune` removes it"),
                    ));
            }
        }
        Err(e) => checks.push(check("pin", "error", e.to_string())),
    }
}

fn is_tailnet(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => {
            let o = v4.octets();
            o[0] == 100 && (64..=127).contains(&o[1])
        }
        std::net::IpAddr::V6(v6) => v6.segments()[..3] == [0xfd7a, 0x115c, 0xa1e0],
    }
}

/// The certificate is valid for 365 days and is not renewed automatically.
fn cert_expiry_check(home: &std::path::Path) -> Check {
    let cert = home.join("network-tls/cert.pem");
    let soon = std::process::Command::new("openssl")
        .args(["x509", "-noout", "-checkend", "2592000", "-in"])
        .arg(&cert)
        .output();
    match soon {
        Ok(o) if o.status.success() => check("certificate", "ok", "Valid for more than 30 days"),
        Ok(_) => check(
            "certificate",
            "warning",
            "TLS certificate expires within 30 days (or is unreadable). There is no renewal command yet (tracked follow-up, see docs/NETWORK-LIMITS.md); plan a manual re-setup before it lapses.",
        ),
        Err(e) => check("certificate", "error", format!("Could not run openssl: {e}")),
    }
}

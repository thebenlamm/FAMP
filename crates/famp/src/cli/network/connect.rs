//! SSH authenticates public-card exchange. It never carries private keys and
//! never touches authorized_keys, sudoers, or anything outside `~/.famp` and
//! the user service directory.
use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use super::state::{self, error, Card, ImportOpts};
use super::ConnectArgs;
use crate::cli::error::CliError;

// Static command: no user-provided shell fragments or paths are interpolated.
const REMOTE: &str = "if command -v famp >/dev/null 2>&1; then exec famp";

fn remote_command(action: &str) -> String {
    format!("{REMOTE} {action}; elif test -x \"$HOME/.cargo/bin/famp\"; then exec \"$HOME/.cargo/bin/famp\" {action}; else echo 'FAMP missing: install the same release of famp and famp-gateway on this computer first' >&2; exit 127; fi")
}

pub fn validate_host(host: &str) -> Result<(), CliError> {
    if host.is_empty()
        || host.len() > 253
        || host.starts_with('-')
        || !host
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"@._-:".contains(&c))
    {
        return Err(error(
            "SSH destination must be an alias or user@host (no options or shell syntax)",
        ));
    }
    Ok(())
}

/// Runs a FAMP action on a remote computer. Faked in tests.
pub trait Remote {
    fn run(&self, host: &str, action: &str, input: Option<&[u8]>) -> Result<Vec<u8>, CliError>;
}

pub struct Ssh;

impl Remote for Ssh {
    fn run(&self, host: &str, action: &str, input: Option<&[u8]>) -> Result<Vec<u8>, CliError> {
        validate_host(host)?;
        eprintln!("{host}: famp {action}");
        let mut child = Command::new("ssh")
            .args(ssh_args(host, action))
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(error)?;
        if let Some(bytes) = input {
            child
                .stdin
                .take()
                .ok_or_else(|| error("SSH stdin unavailable"))?
                .write_all(bytes)
                .map_err(error)?;
        }
        let output = child.wait_with_output().map_err(error)?;
        if !output.status.success() {
            let said = String::from_utf8_lossy(&output.stdout);
            let said = said.trim();
            if !said.is_empty() {
                eprintln!(
                    "{host}: output before failure:\n{}",
                    &said[..said.len().min(2000)]
                );
            }
            return Err(error(format!("{host}: `famp {action}` failed. Check `ssh {host}` authentication and matching binaries. Earlier steps may have completed; fix the error and rerun the same connect command (it is idempotent).")));
        }
        Ok(output.stdout)
    }
}

pub fn ssh_args(host: &str, action: &str) -> Vec<String> {
    [
        "-T",
        "-o",
        "BatchMode=yes",
        "-o",
        "StrictHostKeyChecking=yes",
        "-o",
        "ConnectTimeout=8",
        "-o",
        "ServerAliveInterval=10",
        "-o",
        "ServerAliveCountMax=1",
    ]
    .into_iter()
    .map(str::to_owned)
    .chain([host.to_owned(), remote_command(action)])
    .collect()
}

/// Files each side writes, for `--dry-run` and the guardians' review.
const LOCAL_FILES: &str = "~/.famp/network.json, ~/.famp/gateway/peers.keyring, ~/.famp/recipients.json (only if absent), ~/.famp/network-trust.pem (written by `network serve`); with --prune also ~/.famp/gateway/peers.keyring.bak[.N] and ~/.famp/network.json.bak[.N]; the gateway service definition (systemd: ~/.config/systemd/user/famp-gateway.service; macOS: staged at ~/.famp/staged/com.famp.gateway.plist, copied to ~/Library/LaunchAgents only by `network enable`)";
const REMOTE_FILES: &str = "the same files as this computer; on first setup also ~/.famp/network-tls/{cert,key}.pem, ~/.famp/own-domain and ~/.famp/gateway/identity.ed25519 (only if absent); without --stage also the local broker service (`daemon install`)";

const fn setup_action(args: &ConnectArgs) -> &'static str {
    if args.stage {
        "setup --host none --no-services"
    } else {
        "setup --host none"
    }
}

fn import_action(args: &ConnectArgs, check: bool) -> String {
    let mut a = String::from("network import");
    if check {
        a.push_str(" --check");
    }
    if args.prune {
        a.push_str(" --prune");
    }
    a
}

fn dry_run(args: &ConnectArgs) {
    eprintln!("DRY RUN: nothing is executed or written.");
    for host in &args.hosts {
        eprintln!("\n== {host} ==");
        let mut actions = vec![
            "network --help".to_owned(),
            setup_action(args).to_owned(),
            "network export".to_owned(),
            import_action(args, true),
            import_action(args, false),
        ];
        actions.push(
            if args.stage {
                "network stage"
            } else {
                "network enable"
            }
            .to_owned(),
        );
        if !args.stage {
            actions.push("doctor".to_owned());
        }
        for action in actions {
            eprintln!("ssh {}", ssh_args(host, &action).join(" "));
        }
        eprintln!("writes on {host}: {REMOTE_FILES}");
    }
    eprintln!("\nwrites locally: {LOCAL_FILES}");
    eprintln!(
        "topology: {}; stale pins: {}; services: {}",
        if args.pairwise {
            "pairwise (this computer <-> each host only)"
        } else {
            "full mesh of this computer plus every listed host"
        },
        if args.prune {
            "REMOVED (exact peer set)"
        } else {
            "kept (reported)"
        },
        if args.stage {
            "staged only, NOT enabled"
        } else {
            "enabled and restarted"
        }
    );
}

fn expectations(args: &ConnectArgs) -> Result<Vec<(String, String)>, CliError> {
    let mut out = Vec::new();
    for e in &args.expect {
        let (host, id) = e
            .split_once('=')
            .ok_or_else(|| error("--expect takes <ssh-host>=<key_id>"))?;
        if !args.hosts.iter().any(|h| h == host) {
            return Err(error(format!(
                "--expect names {host}, which is not a listed host"
            )));
        }
        out.push((host.to_owned(), id.to_owned()));
    }
    Ok(out)
}

pub fn run(args: &ConnectArgs) -> Result<(), CliError> {
    let home = crate::cli::home::resolve_famp_home()?;
    super::service::default_home_only(&home)?;
    orchestrate(&home, args, &Ssh, true)
}

/// `services` is false only in tests: skips local service work and local probes.
pub fn orchestrate(
    home: &Path,
    args: &ConnectArgs,
    remote: &dyn Remote,
    services: bool,
) -> Result<(), CliError> {
    let mut unique = BTreeSet::new();
    for host in &args.hosts {
        validate_host(host)?;
        if !unique.insert(host) {
            return Err(error(format!("Host {host} listed twice")));
        }
    }
    let expected = expectations(args)?;
    if args.dry_run {
        dry_run(args);
        return Ok(());
    }
    let local = state::refresh_version(home)?.local;
    // Check every participant's command support before starting setup anywhere.
    for host in &args.hosts {
        remote.run(host, "network --help", None)?;
    }
    let mut remotes: Vec<(String, Card)> = Vec::new();
    for host in &args.hosts {
        remote.run(host, setup_action(args), None)?;
        let bytes = remote.run(host, "network export", None)?;
        if bytes.len() as u64 > state::MAX_BUNDLE_BYTES {
            return Err(error("Remote card exceeds size limit"));
        }
        let card: Card = serde_json::from_slice(&bytes).map_err(error)?;
        card.validate()?;
        card.check_version()?;
        remotes.push((host.clone(), card));
    }
    // Fingerprints are printed BEFORE any trust is written, for out-of-band checks.
    eprintln!("key_id fingerprints (compare out of band):");
    eprintln!(
        "  {} (this computer) {}",
        local.principal(),
        local.fingerprint()?
    );
    for (host, card) in &remotes {
        let fp = card.fingerprint()?;
        let verdict = match expected.iter().find(|(h, _)| h == host) {
            Some((_, want)) if *want == fp => "MATCHES --expect",
            Some((_, want)) => {
                return Err(error(format!(
                    "{host}: key_id {fp} does not match --expect {want}. Nothing was pinned."
                )))
            }
            None => "not verified out of band",
        };
        eprintln!("  {host} {} {fp} ({verdict})", card.principal());
    }
    let mut everyone = vec![local.clone()];
    everyone.extend(remotes.iter().map(|(_, c)| c.clone()));
    // Pairwise: a remote only learns this computer and itself. Mesh: everyone.
    let bundle_for = |card: &Card| -> Vec<Card> {
        if args.pairwise {
            vec![local.clone(), card.clone()]
        } else {
            everyone.clone()
        }
    };
    let opts = |check| ImportOpts {
        check,
        prune: args.prune,
    };
    let mut bundles = Vec::new();
    for (host, card) in &remotes {
        let bundle = serde_json::to_vec(&bundle_for(card)).map_err(error)?;
        state::parse_cards(&bundle)?;
        bundles.push((host, bundle));
    }
    // Check everywhere before writing anywhere.
    state::import(home, &everyone, opts(true))?;
    for (host, bundle) in &bundles {
        remote.run(host, &import_action(args, true), Some(bundle))?;
    }
    state::import(home, &everyone, opts(false))?;
    for (host, bundle) in &bundles {
        remote.run(host, &import_action(args, false), Some(bundle))?;
    }
    // All trust is on disk before any service is touched.
    if args.stage {
        if services {
            super::service::stage(home)?;
        }
        for host in &args.hosts {
            remote.run(host, "network stage", None)?;
        }
        eprintln!("Staged, not enabled. Each computer also needs a running local broker (`famp daemon install`; `famp doctor` reports it); `network enable` refuses without one. Review the files and unit (`famp network print-unit` on each computer), then run `famp network enable` on each.");
        return Ok(());
    }
    if services {
        super::service::enable(home)?;
    }
    for host in &args.hosts {
        remote.run(host, "network enable", None)?;
    }
    for host in &args.hosts {
        let out = remote.run(host, "doctor", None)?;
        eprint!("{host} doctor:\n{}", String::from_utf8_lossy(&out));
    }
    if services {
        crate::cli::block_on_async(super::doctor::run(super::DoctorArgs { json: false }))?;
    }
    eprintln!("Mesh configured. Register each computer's named agent, then explicitly read remote inboxes.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_destinations_cannot_inject_options_or_shell() {
        for bad in [
            "-oProxyCommand=x",
            "ben@host;id",
            "$(id)",
            "host\nfoo",
            "a b",
            "",
        ] {
            assert!(validate_host(bad).is_err());
        }
        for good in ["air-server", "ben@100.1.2.3", "ben@host.local"] {
            assert!(validate_host(good).is_ok());
        }
    }

    #[test]
    fn remote_command_falls_back_to_cargo_bin_off_path() {
        let cmd = remote_command("network export");
        assert!(cmd.contains("command -v famp"));
        assert!(cmd.contains("$HOME/.cargo/bin/famp"));
        assert!(cmd.contains("exit 127"));
    }
}

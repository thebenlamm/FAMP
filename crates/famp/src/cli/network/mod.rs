//! Opinionated onboarding for a personally owned, SSH-authenticated network.
//! Uses the existing signed gateway; never tunnels the local broker.

use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};

use crate::cli::error::CliError;

pub mod allowlist;
pub mod cert;
pub mod connect;
pub mod doctor;
#[cfg(test)]
mod flow_tests;
pub mod perms;
pub mod service;
pub mod setup;
pub mod state;

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Host {
    #[default]
    Auto,
    Claude,
    Codex,
    None,
}

#[derive(Debug, Default, Args)]
pub struct SetupArgs {
    /// Stable federation agent name (default: hostname, normalized).
    #[arg(long)]
    pub name: Option<String>,
    /// Bind address. Must be this computer's own Tailscale IPv4 (default: `tailscale ip -4`).
    #[arg(long)]
    pub address: Option<std::net::IpAddr>,
    /// Allow an address that is not this computer's Tailscale IP (LAN, tests).
    #[arg(long)]
    pub allow_non_tailnet: bool,
    /// Gateway HTTPS port.
    #[arg(long)]
    pub port: Option<u16>,
    /// Agent host to configure. Auto detects installed Claude/Codex CLIs.
    #[arg(long, value_enum, default_value = "auto")]
    pub host: Host,
    /// Generate configuration only; do not install or start user services.
    #[arg(long)]
    pub no_services: bool,
}

#[derive(Debug, Args)]
#[allow(clippy::struct_excessive_bools)] // independent CLI switches
pub struct ConnectArgs {
    /// Existing SSH aliases or user@host destinations. Host keys must be trusted.
    #[arg(required = true)]
    pub hosts: Vec<String>,
    /// Write config and service definitions but do NOT enable or restart any service.
    #[arg(long)]
    pub stage: bool,
    /// Pair only this computer with each host, instead of meshing all listed hosts.
    #[arg(long)]
    pub pairwise: bool,
    /// Make each computer's peer set EXACTLY what this run defines; drop stale pins.
    #[arg(long)]
    pub prune: bool,
    /// Print the exact ssh commands and files that would be written; run nothing.
    #[arg(long)]
    pub dry_run: bool,
    /// Require a host's key_id to equal this value, e.g. `air-server=AbC...`. Repeatable.
    #[arg(long, value_name = "HOST=KEY_ID")]
    pub expect: Vec<String>,
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct NetworkArgs {
    #[command(subcommand)]
    pub command: NetworkCommand,
}

#[derive(Debug, Subcommand)]
pub enum NetworkCommand {
    /// Prepare this computer for a personal FAMP network (identity, certificate, config).
    Setup(SetupArgs),
    /// Print this computer's PUBLIC connection card as JSON.
    Export,
    /// Import a JSON array of trusted public cards from a file or stdin.
    /// Only import cards obtained through an authenticated channel.
    Import {
        #[arg(default_value = "-")]
        file: PathBuf,
        /// Validate conflicts and certificates and print the diff; write nothing.
        #[arg(long)]
        check: bool,
        /// Make the peer set exactly these cards; drop every other pin.
        #[arg(long)]
        prune: bool,
    },
    /// Write the gateway service definition WITHOUT enabling or starting it.
    Stage,
    /// Stage, then enable and (re)start the persistent gateway.
    Enable,
    /// Print the service definitions `stage` would write; change nothing.
    PrintUnit,
    /// Print the host firewall commands to run by hand for this computer's peers.
    Firewall,
    /// Let verified peers deliver to this local agent name (default is deny).
    Allow { name: String },
    /// Stop accepting peer messages for this local agent name (applies at once).
    Deny { name: String },
    /// List the local agent names that may receive peer messages.
    List,
    /// Run the configured gateway in the foreground (service entry point).
    Serve,
}

pub fn run(args: NetworkArgs) -> Result<(), CliError> {
    let home = crate::cli::home::resolve_famp_home()?;
    match args.command {
        NetworkCommand::Setup(args) => setup::run(&args),
        NetworkCommand::Export => {
            let config = state::load(&home)?;
            println!(
                "{}",
                serde_json::to_string(&config.local).map_err(state::error)?
            );
            Ok(())
        }
        NetworkCommand::Import { file, check, prune } => {
            use std::io::Read as _;
            let input: Box<dyn std::io::Read> = if file == std::path::Path::new("-") {
                Box::new(std::io::stdin())
            } else {
                Box::new(std::fs::File::open(file).map_err(state::error)?)
            };
            let mut bytes = Vec::new();
            input
                .take(state::MAX_BUNDLE_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(state::error)?;
            let cards = state::parse_cards(&bytes)?;
            state::import(&home, &cards, state::ImportOpts { check, prune })?;
            if !check {
                perms::fix(&home)?;
            }
            Ok(())
        }
        NetworkCommand::Stage => service::stage(&home),
        NetworkCommand::Enable => service::enable(&home),
        NetworkCommand::PrintUnit => {
            print!("{}", service::print_units(&home)?);
            Ok(())
        }
        NetworkCommand::Firewall => {
            print!("{}", service::firewall(&state::load(&home)?));
            Ok(())
        }
        NetworkCommand::Allow { name } => allowlist::allow(&home, &name),
        NetworkCommand::Deny { name } => allowlist::deny(&home, &name),
        NetworkCommand::List => {
            for name in allowlist::load(&home)? {
                println!("{name}");
            }
            Ok(())
        }
        NetworkCommand::Serve => service::serve(&home),
    }
}

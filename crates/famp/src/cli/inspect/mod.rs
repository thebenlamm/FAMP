//! `famp inspect` subcommand surface (v0.10).
//!
//! Broker-backed inspectors expose liveness, identities, tasks, messages,
//! and waiters. `wake` composes those broker facts with local Codex project
//! configuration to diagnose the cross-layer automatic-wake path.

use clap::{Args, Subcommand};

use crate::cli::error::CliError;

pub mod broker;
pub mod identities;
pub mod messages;
pub mod tasks;
pub mod waiters;
pub mod wake;

#[derive(Args, Debug)]
pub struct InspectArgs {
    #[command(subcommand)]
    pub command: InspectSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum InspectSubcommand {
    /// Inspect broker liveness and dead-broker diagnosis.
    Broker(broker::InspectBrokerArgs),
    /// List registered identities with mailbox metadata.
    Identities(identities::InspectIdentitiesArgs),
    /// List task FSM state, envelope count, and transition ages.
    Tasks(tasks::InspectTasksArgs),
    /// List envelope metadata for a mailbox.
    Messages(messages::InspectMessagesArgs),
    /// List clients currently parked in `famp_await`.
    Waiters(waiters::InspectWaitersArgs),
    /// Diagnose whether a registered identity can wake a Codex window.
    Wake(wake::InspectWakeArgs),
}

pub async fn run(args: InspectArgs) -> Result<(), CliError> {
    match args.command {
        InspectSubcommand::Broker(args) => broker::run(args).await,
        InspectSubcommand::Identities(args) => identities::run(args).await,
        InspectSubcommand::Tasks(args) => tasks::run(args).await,
        InspectSubcommand::Messages(args) => messages::run(args).await,
        InspectSubcommand::Waiters(args) => waiters::run(args).await,
        InspectSubcommand::Wake(args) => wake::run(args).await,
    }
}

//! `famp inspect` subcommand surface (v0.10).
//!
//! D-06: Phase 1 ships ONLY `broker` and `identities` sub-subcommands.
//! `tasks` and `messages` are absent from the CLI in Phase 1. Phase 2
//! adds them after the server answers with real task/message data.

use clap::{Args, Subcommand};

use crate::cli::error::CliError;

pub mod broker;
pub mod identities;

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
}

pub async fn run(args: InspectArgs) -> Result<(), CliError> {
    match args.command {
        InspectSubcommand::Broker(args) => broker::run(args).await,
        InspectSubcommand::Identities(args) => identities::run(args).await,
    }
}

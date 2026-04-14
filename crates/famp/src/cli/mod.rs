//! FAMP CLI surface. D-02: subcommand logic lives in the lib crate so
//! integration tests can call it directly without `assert_cmd`.

use clap::{Parser, Subcommand};

pub mod config;
pub mod error;
pub mod home;
pub mod init;
pub mod paths;
pub mod perms;

pub use error::CliError;
pub use init::InitOutcome;

#[derive(Parser, Debug)]
#[command(name = "famp", version, about = "FAMP v0.5.1 reference CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize a FAMP home directory.
    Init(InitArgs),
}

#[derive(clap::Args, Debug)]
pub struct InitArgs {
    /// Overwrite an existing FAMP home (atomic replace).
    #[arg(long)]
    pub force: bool,
}

/// Top-level CLI dispatcher. Called from `bin/famp.rs`.
pub fn run(cli: Cli) -> Result<(), CliError> {
    match cli.command {
        Commands::Init(args) => init::run(args).map(|_| ()),
    }
}

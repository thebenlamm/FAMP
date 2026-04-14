//! FAMP CLI surface. D-02: subcommand logic lives in the lib crate so
//! integration tests can call it directly without `assert_cmd`.

use clap::{Parser, Subcommand};

pub mod config;
pub mod error;
pub mod home;
pub mod paths;
pub mod perms;

pub use error::CliError;

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

/// Plan 01 stub. Plan 02 replaces the body to dispatch to `init::run_at`.
#[allow(clippy::missing_const_for_fn, clippy::needless_pass_by_value)]
pub fn run(_cli: Cli) -> Result<(), CliError> {
    Err(CliError::HomeNotSet)
}

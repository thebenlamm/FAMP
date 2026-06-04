//! `famp daemon status` stub — Plan 05 fills this with the three-state detection logic.

use clap::Args;

use crate::cli::error::CliError;

#[derive(Debug, Args)]
pub struct DaemonStatusArgs {
    /// Output status as JSON.
    #[arg(long)]
    pub json: bool,

    /// Override the install target home (defaults to `dirs::home_dir()`).
    /// Hidden flag - used by integration tests to redirect to a tempdir.
    #[arg(long, env = "FAMP_INSTALL_TARGET_HOME", hide = true)]
    pub home: Option<std::path::PathBuf>,
}

pub async fn run(_args: DaemonStatusArgs) -> Result<(), CliError> {
    Err(CliError::NotImplemented {
        what: "daemon status (Plan 05)".into(),
    })
}

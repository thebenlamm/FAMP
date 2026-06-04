//! `famp daemon uninstall` stub — Plan 04 fills this with launchctl/systemctl logic.

use clap::Args;

use crate::cli::error::CliError;

#[derive(Debug, Args)]
pub struct DaemonUninstallArgs {
    /// Override the install target home (defaults to `dirs::home_dir()`).
    /// Hidden flag - used by integration tests to redirect to a tempdir.
    #[arg(long, env = "FAMP_INSTALL_TARGET_HOME", hide = true)]
    pub home: Option<std::path::PathBuf>,
}

pub fn run(_args: DaemonUninstallArgs) -> Result<(), CliError> {
    Err(CliError::NotImplemented {
        what: "daemon uninstall (Plan 04)".into(),
    })
}

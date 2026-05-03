//! `famp install-codex` subcommand handler.
//!
//! Stub - implemented in plan 03-05. Holds the args struct and a
//! `run`/`run_at` skeleton so the module tree compiles.

#[cfg(test)]
use std::path::Path;

use clap::Args;

use crate::cli::error::CliError;

#[derive(Debug, Args)]
pub struct InstallCodexArgs {
    #[arg(long, hide = true)]
    pub home: Option<std::path::PathBuf>,
}

#[allow(clippy::needless_pass_by_value)]
pub fn run(_args: InstallCodexArgs) -> Result<(), CliError> {
    Err(CliError::NotImplemented {
        what: "install-codex (plan 03-05)".to_string(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
pub fn run_at(
    _home: &Path,
    _out: &mut dyn std::io::Write,
    _err: &mut dyn std::io::Write,
) -> Result<(), CliError> {
    Err(CliError::NotImplemented {
        what: "install-codex::run_at (plan 03-05)".to_string(),
    })
}

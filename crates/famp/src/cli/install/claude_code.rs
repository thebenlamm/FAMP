//! `famp install-claude-code` subcommand handler.
//!
//! Stub - implemented in plan 03-03. Holds the args struct and a
//! `run`/`run_at` skeleton so the module tree compiles.

#[cfg(test)]
use std::path::Path;

use clap::Args;

use crate::cli::error::CliError;

#[derive(Debug, Args)]
pub struct InstallClaudeCodeArgs {
    /// Override the install target home (defaults to `dirs::home_dir()`).
    /// Used by integration tests to redirect to a tempdir.
    #[arg(long, hide = true)]
    pub home: Option<std::path::PathBuf>,
}

#[allow(clippy::needless_pass_by_value)]
pub fn run(_args: InstallClaudeCodeArgs) -> Result<(), CliError> {
    // Real implementation lands in plan 03-03.
    Err(CliError::NotImplemented {
        what: "install-claude-code (plan 03-03)".to_string(),
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
        what: "install-claude-code::run_at (plan 03-03)".to_string(),
    })
}

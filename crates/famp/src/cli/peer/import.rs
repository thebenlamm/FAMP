//! `famp peer import` — parse a `famp peer export` blob and TOFU-pin the
//! peer's key into the gateway peer keyring (TRUST-01, D-05, D-06).
//!
//! Full implementation lands in Plan 04 Task 2; this stub exists so the
//! `peer` subcommand tree (Task 1) compiles and wires end-to-end.

use crate::cli::error::CliError;

/// CLI args for `famp peer import`.
#[derive(clap::Args, Debug)]
pub struct PeerImportArgs {
    /// Source file, or `-` (default) for stdin.
    #[arg(default_value = "-")]
    pub source: String,
}

/// Production entry point (stub — Task 2 fills this in).
pub fn run(_args: &PeerImportArgs) -> Result<(), CliError> {
    Err(CliError::NotImplemented {
        what: "peer import".to_string(),
    })
}

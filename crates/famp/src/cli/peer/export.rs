//! `famp peer export` — print this gateway's principal + pubkey + a human
//! fingerprint as a single, copy/paste-safe line (TRUST-01, D-05).
//!
//! Full implementation lands in Plan 04 Task 2; this stub exists so the
//! `peer` subcommand tree (Task 1) compiles and wires end-to-end.

use crate::cli::error::CliError;

/// CLI args for `famp peer export`.
#[derive(clap::Args, Debug)]
pub struct PeerExportArgs {
    /// Principal name to export this key under, e.g.
    /// `agent:my-mbp.local/gateway`.
    #[arg(long = "as")]
    pub as_principal: String,
}

/// Production entry point (stub — Task 2 fills this in).
pub fn run(_args: &PeerExportArgs) -> Result<(), CliError> {
    Err(CliError::NotImplemented {
        what: "peer export".to_string(),
    })
}

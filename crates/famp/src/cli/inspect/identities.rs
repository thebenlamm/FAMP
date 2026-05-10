//! `famp inspect identities` -- list registered session identities.
//!
//! Wave 2: stub. Wave 3 fills in table/JSON rendering and dead-broker
//! fast-fail behavior.

use clap::Args;

use crate::cli::error::CliError;

#[derive(Args, Debug)]
pub struct InspectIdentitiesArgs {
    /// Emit JSON output instead of a fixed-width table.
    #[arg(long)]
    pub json: bool,
}

pub async fn run(_args: InspectIdentitiesArgs) -> Result<(), CliError> {
    eprintln!("famp inspect identities: not yet implemented (wave 3)");
    Ok(())
}

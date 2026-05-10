//! `famp inspect broker` -- broker liveness + dead-broker diagnosis.
//!
//! Wave 2: stub that compiles, wires the dispatch path, and exits 0
//! on `--help`. Wave 3 fills in rendering and diagnosis bodies.

use clap::Args;

use crate::cli::error::CliError;

#[derive(Args, Debug)]
pub struct InspectBrokerArgs {
    /// Emit JSON output instead of a single human-readable line.
    #[arg(long)]
    pub json: bool,
}

pub async fn run(_args: InspectBrokerArgs) -> Result<(), CliError> {
    eprintln!("famp inspect broker: not yet implemented (wave 3)");
    Ok(())
}

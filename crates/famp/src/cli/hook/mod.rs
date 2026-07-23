//! Native host Stop-hook helpers.
//!
//! The shared engine (transcript replay, rollout resolution, await, block
//! emission) lives here so Codex, and later Claude, can share one path.
//! Critical path: no `jq`, no `python3`, no shell JSON templating.

use clap::{Args, Subcommand};

use crate::cli::error::CliError;

pub mod codex_rollout;
pub mod codex_stop;
pub mod emit;
pub mod lock;
pub mod log;
pub mod pid_fallback;
pub mod stdin;
pub mod transcript;

/// `famp hook …` — host Stop-hook entrypoints.
#[derive(Debug, Args)]
pub struct HookArgs {
    #[command(subcommand)]
    pub command: HookCommand,
}

#[derive(Debug, Subcommand)]
pub enum HookCommand {
    /// Codex Stop hook: listen-mode await + native block decision.
    ///
    /// Reads Stop-hook JSON from stdin. Fail-open exit 0 on any uncertainty.
    /// Emits `{"decision":"block","reason":"..."}` on successful wake.
    #[command(name = "codex-stop")]
    CodexStop(codex_stop::CodexStopArgs),
}

/// Dispatch `famp hook <subcommand>`.
pub fn run(args: HookArgs) -> Result<(), CliError> {
    match args.command {
        HookCommand::CodexStop(a) => codex_stop::run(a),
    }
}

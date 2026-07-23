//! `famp peer` subcommand surface — two-machine Ed25519 trust bootstrap
//! (TRUST-01, Phase 8).
//!
//! `famp peer export --as <name>` prints a single, copy/paste-safe line
//! (principal + `b64url` pubkey + a human-readable `key_id` fingerprint)
//! built from this machine's own gateway signing keypair. `famp peer
//! import [<file>|-]` parses that line and TOFU-pins the peer's key into
//! the gateway peer keyring at `~/.famp/gateway/peers.keyring` — the same
//! file the (Phase 9) `verify_inbound` ingress check reads (D-06). No key
//! material ever crosses FAMP itself; the blob transport is the
//! operator's own clipboard/Signal (TRUST-01, T-08-13).
//!
//! **Scope note (RESEARCH Pitfall 4):** the trust model here is **one
//! signing key per remote principal name**. Do NOT design for a single
//! machine key shared across multiple principal names —
//! `Keyring::load_from_file` rejects duplicate pubkeys across distinct
//! principals and would hard-fail on the next gateway restart. If two
//! agent names on the same remote machine both need simultaneous trust,
//! each needs its own keypair; generalizing this is deferred to v1.1.

use clap::{Args, Subcommand};

use crate::cli::error::CliError;

pub mod export;
pub mod identity;
pub mod import;

#[derive(Args, Debug)]
pub struct PeerArgs {
    #[command(subcommand)]
    pub command: PeerSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum PeerSubcommand {
    /// Print a single, copy/paste-safe line carrying this gateway's own
    /// principal, Ed25519 verifying key (b64url), and a human-readable
    /// key_id fingerprint. Move it out-of-band (clipboard/Signal) to the
    /// peer machine — no key material ever crosses FAMP itself (TRUST-01).
    Export(export::PeerExportArgs),
    /// Parse a `famp peer export` blob (from a file or stdin) and
    /// TOFU-pin the peer's key into the gateway peer keyring. Fails
    /// closed on a conflicting re-pin (T-08-11).
    Import(import::PeerImportArgs),
}

/// Sync dispatcher — pure file I/O, no broker/tokio dependency.
pub fn run(args: PeerArgs) -> Result<(), CliError> {
    match args.command {
        PeerSubcommand::Export(args) => export::run(&args),
        PeerSubcommand::Import(args) => import::run(&args),
    }
}

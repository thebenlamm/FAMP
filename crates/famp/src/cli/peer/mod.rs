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
//! **Scope note (RESEARCH Pitfall 4, updated Phase 15):** the trust model
//! here started as **one signing key per remote principal name**; Phase
//! 15 (KEYR-02/KEYR-03) extends that to an explicit
//! active/retired/revoked lifecycle PER principal — a principal may hold
//! multiple keys at once, but at most one is ever `Active`. The
//! duplicate-pubkey-across-DIFFERENT-principals rejection still stands
//! unchanged: `Keyring::load_from_file` rejects the same pubkey pinned
//! under two distinct principals and would hard-fail on the next gateway
//! restart. If two agent names on the same remote machine both need
//! simultaneous trust, each still needs its own keypair.
//!
//! **`import` vs `rotate` (KEYR-03):** `famp peer import` is first-sight
//! pinning ONLY — it fails closed (exit 1, `PeerKeyConflict`) on ANY key
//! change for an already-pinned principal, exactly as before this phase.
//! `famp peer rotate` is the ONLY path that accepts a changed key for a
//! known principal, and only with explicit `--confirm-rotation` (exit 2
//! without it, naming both key fingerprints). The two paths never merge.
//! `famp peer retire` is the only path that removes key material from the
//! trust store.
//!
//! **`revoke` vs `import-revocation` (REVK-02):** `famp peer revoke` is a
//! LOCAL unilateral decision needing no signature — an operator may always
//! stop trusting a key in their own trust store. `famp peer
//! import-revocation` consumes a statement SIGNED by the peer and is
//! fail-closed, verified against the D15-B authorized-signer set before it
//! changes anything.

use clap::{Args, Subcommand};

use crate::cli::error::CliError;

pub mod export;
pub mod identity;
pub mod import;
pub mod import_revocation;
pub mod retire;
pub mod revoke;
pub mod rotate;

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
    /// TOFU-pin the peer's key into the gateway peer keyring. First-sight
    /// pinning ONLY — fails closed on any key change for an
    /// already-pinned principal (T-08-11). Never accepts a key change;
    /// use `rotate` for that.
    Import(import::PeerImportArgs),
    /// Accept a NEW key for an already-pinned peer (KEYR-02). The ONLY
    /// path that accepts a changed key for a known principal, and only
    /// with explicit `--confirm-rotation` — without it, exits 2 with a
    /// `KEY CHANGED` block naming both key fingerprints and mutates
    /// nothing (KEYR-03).
    Rotate(rotate::PeerRotateArgs),
    /// Remove a `retired` key entry from the trust store — the ONLY path
    /// that deletes key material. Refuses an `active` or `revoked` entry.
    Retire(retire::PeerRetireArgs),
    /// LOCAL unilateral revocation (REVK-02) — transitions a key entry to
    /// `revoked` in THIS machine's own trust store, requiring no
    /// signature. Optionally `--emit`s a signed statement for the peer's
    /// own trust store.
    Revoke(revoke::PeerRevokeArgs),
    /// Consume a peer-signed revocation statement and apply it,
    /// fail-closed (REVK-02) — verified against the D15-B
    /// authorized-signer set before it changes anything.
    #[command(name = "import-revocation")]
    ImportRevocation(import_revocation::PeerImportRevocationArgs),
}

/// Sync dispatcher — pure file I/O, no broker/tokio dependency.
pub fn run(args: PeerArgs) -> Result<(), CliError> {
    match args.command {
        PeerSubcommand::Export(args) => export::run(&args),
        PeerSubcommand::Import(args) => import::run(&args),
        PeerSubcommand::Rotate(args) => rotate::run(&args),
        PeerSubcommand::Retire(args) => retire::run(&args),
        PeerSubcommand::Revoke(args) => revoke::run(&args),
        PeerSubcommand::ImportRevocation(args) => import_revocation::run(&args),
    }
}
